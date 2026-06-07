#!/usr/bin/env python3
import argparse
import json
import math
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Dict, List


def run_json_command(cmd: List[str]) -> Dict[str, Any]:
    start = time.perf_counter()
    proc = subprocess.run(cmd, check=False, capture_output=True, text=True)
    elapsed_ms = (time.perf_counter() - start) * 1000.0

    if proc.returncode != 0:
        message = proc.stderr.strip() or proc.stdout.strip() or f"command failed: {' '.join(cmd)}"
        raise RuntimeError(message)

    try:
        payload = json.loads(proc.stdout)
    except json.JSONDecodeError as exc:
        raise RuntimeError(f"invalid JSON from {' '.join(cmd)}: {exc}") from exc

    if not isinstance(payload, dict):
        raise RuntimeError(f"unexpected non-object payload from {' '.join(cmd)}")

    payload["_wall_runtime_ms"] = round(elapsed_ms, 6)
    return payload


def md_line(items: List[str]) -> str:
    return "| " + " | ".join(items) + " |"


def calibrate_ops_per_second(sample_ops: int) -> float:
    # Intentionally simple integer loop to estimate single-core scalar operation throughput.
    # This is a coarse baseline for estimating brute-force traversal cost.
    value = 0
    start = time.perf_counter()
    for i in range(sample_ops):
        value ^= (i * 2654435761) & 0xFFFFFFFF
    elapsed = max(time.perf_counter() - start, 1e-9)

    # Keep variable alive to avoid extreme optimizer assumptions.
    if value == -1:
        print("impossible", file=sys.stderr)

    return sample_ops / elapsed


def linear_slope(xs: List[float], ys: List[float]) -> float:
    if len(xs) < 2 or len(ys) < 2 or len(xs) != len(ys):
        return 0.0
    mx = sum(xs) / len(xs)
    my = sum(ys) / len(ys)
    num = sum((x - mx) * (y - my) for x, y in zip(xs, ys))
    den = sum((x - mx) * (x - mx) for x in xs)
    if den == 0.0:
        return 0.0
    return num / den


def scaling_markdown(report: Dict[str, Any]) -> str:
    lines: List[str] = []
    lines.append("# UGC Grover Scaling Boundary Report")
    lines.append("")
    lines.append(f"Generated UTC: {report['generated_utc']}")
    lines.append("")
    lines.append("## Command")
    lines.append("")
    lines.append("```bash")
    lines.append(report["invocation"])
    lines.append("```")
    lines.append("")
    lines.append("## Summary")
    lines.append("")
    lines.append(md_line(["Field", "Value"]))
    lines.append(md_line(["---", "---"]))
    lines.append(md_line(["probe_n_limit", str(report["summary"]["probe_n_limit"])]))
    lines.append(md_line(["n_values_total", str(report["summary"]["n_values_total"])]))
    lines.append(md_line(["n_values_executed", str(report["summary"]["n_values_executed"])]))
    lines.append(md_line(["n_values_skipped", str(report["summary"]["n_values_skipped"])]))
    lines.append(md_line(["ops_per_second_estimate", str(report["summary"]["ops_per_second_estimate"])]))
    lines.append(md_line(["runtime_log2_slope_per_bit", str(report["summary"]["runtime_log2_slope_per_bit"])]))
    lines.append(md_line(["runtime_scaling_interpretation", report["summary"]["runtime_scaling_interpretation"]]))
    lines.append("")
    lines.append("## Per-n Results")
    lines.append("")
    lines.append(md_line([
        "n",
        "status",
        "search_space",
        "grover_iter",
        "success_p",
        "ugc_wall_ms",
        "bruteforce_est_ms",
        "query_ratio_vs_worst",
    ]))
    lines.append(md_line(["---:", "---", "---:", "---:", "---:", "---:", "---:", "---:"]))

    for row in report["rows"]:
        lines.append(
            md_line(
                [
                    str(row.get("n_bits", "")),
                    str(row.get("status", "")),
                    str(row.get("search_space", "")),
                    str(row.get("grover_iterations", "")),
                    str(row.get("success_probability", "")),
                    str(row.get("ugc_wall_runtime_ms", "")),
                    str(row.get("bruteforce_estimated_runtime_ms", "")),
                    str(row.get("query_ratio_vs_classical_worst", "")),
                ]
            )
        )

    lines.append("")
    lines.append("## Notes")
    lines.append("")
    lines.append("- `status=executed` rows are measured with the current Grover probe implementation.")
    lines.append("- `status=skipped_probe_limit` rows exceed the probe's current hard limit and are included for boundary bookkeeping.")
    lines.append("- Brute-force times are coarse estimates from local integer-op calibration, not full memory-bound scans.")
    lines.append("")

    return "\n".join(lines)


def interpret_slope(slope: float) -> str:
    if slope <= 0.2:
        return "near-flat/polynomial-like over sampled range"
    if slope <= 0.7:
        return "between flat and sqrt-like exponential (close to O(2^(n/2)))"
    if slope <= 1.3:
        return "near full-exponential-like in sampled wall-clock"
    return "super-exponential-like over sampled range (likely overhead/artifact dominated)"


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Run Grover scaling boundary experiment across n values and compare measured UGC runtime against brute-force and theory baselines."
    )
    parser.add_argument("--python", default=sys.executable, help="Python executable used to run probe scripts")
    parser.add_argument("--n-min", type=int, default=10, help="Minimum n (default: 10)")
    parser.add_argument("--n-max", type=int, default=35, help="Maximum n (default: 35)")
    parser.add_argument("--step", type=int, default=5, help="n step size (default: 5)")
    parser.add_argument("--trials", type=int, default=3, help="Trials passed to Grover probe (default: 3)")
    parser.add_argument("--seed", type=int, default=20260606, help="Seed passed to Grover probe")
    parser.add_argument("--stability-runs", type=int, default=1, help="Deterministic stability reruns passed to Grover probe")
    parser.add_argument("--trace-every", type=int, default=1000000, help="Trace sampling period passed to Grover probe")
    parser.add_argument("--probe-n-max", type=int, default=50, help="Current hard n limit of eval-ugc-grover-probe.py")
    parser.add_argument("--ops-calibration", type=int, default=5_000_000, help="Integer operations for brute-force throughput calibration")
    parser.add_argument("--output-json", default="docs/demo/grover-scaling-report.json", help="Output JSON report path")
    parser.add_argument("--output-md", default="docs/demo/grover-scaling-report.md", help="Output Markdown report path")
    parser.add_argument("--pretty", action="store_true", help="Pretty-print JSON output")
    args = parser.parse_args()

    if args.n_min < 1:
        raise SystemExit("n-min must be >= 1")
    if args.n_max < args.n_min:
        raise SystemExit("n-max must be >= n-min")
    if args.step < 1:
        raise SystemExit("step must be >= 1")

    n_values = list(range(args.n_min, args.n_max + 1, args.step))
    if not n_values:
        raise SystemExit("no n values produced from given range")

    ops_per_second = calibrate_ops_per_second(max(1_000_000, args.ops_calibration))

    rows: List[Dict[str, Any]] = []
    executed_n: List[float] = []
    executed_log2_runtime: List[float] = []

    for n_bits in n_values:
        n_items = 1 << n_bits
        brute_force_estimated_runtime_ms = (n_items / ops_per_second) * 1000.0

        if n_bits > args.probe_n_max:
            rows.append(
                {
                    "n_bits": n_bits,
                    "status": "skipped_probe_limit",
                    "search_space": n_items,
                    "theoretical_lower_bound_queries_omega_n": n_items,
                    "theoretical_quantum_query_bound_o_sqrt_n": int(round(math.sqrt(n_items))),
                    "bruteforce_estimated_runtime_ms": round(brute_force_estimated_runtime_ms, 6),
                }
            )
            continue

        payload = run_json_command(
            [
                args.python,
                "scripts/eval-ugc-grover-probe.py",
                "--n-bits",
                str(n_bits),
                "--trials",
                str(max(1, args.trials)),
                "--seed",
                str(args.seed),
                "--iteration-policy",
                "optimal",
                "--trace-every",
                str(max(1, args.trace_every)),
                "--stability-runs",
                str(max(1, args.stability_runs)),
            ]
        )

        trials = payload.get("trials", [])
        if not trials:
            raise RuntimeError(f"no trials returned for n={n_bits}")

        first = trials[0]
        trial_runtimes_ms = [float(t.get("metrics", {}).get("runtime_ms", 0.0)) for t in trials]
        avg_internal_runtime_ms = sum(trial_runtimes_ms) / max(1, len(trial_runtimes_ms))
        wall_runtime_ms = float(payload.get("_wall_runtime_ms", 0.0))

        executed_n.append(float(n_bits))
        executed_log2_runtime.append(math.log2(max(wall_runtime_ms, 1e-9)))

        row = {
            "n_bits": n_bits,
            "status": "executed",
            "search_space": int(first["search_space"]),
            "grover_iterations": int(first["grover_iterations"]),
            "sqrt_n_reference": int(first["sqrt_n_reference"]),
            "pi_over_4_sqrt_n_reference": int(first["pi_over_4_sqrt_n_reference"]),
            "success_probability": float(first["result"]["success_probability"]),
            "query_ratio_vs_classical_worst": float(first["baseline"]["grover_query_ratio_vs_worst"]),
            "query_ratio_vs_classical_avg": float(first["baseline"]["grover_query_ratio_vs_avg"]),
            "ugc_internal_avg_runtime_ms": round(avg_internal_runtime_ms, 6),
            "ugc_wall_runtime_ms": round(wall_runtime_ms, 6),
            "deterministic_hash_stable": bool(payload.get("hash_stable", False)),
            "bruteforce_estimated_runtime_ms": round(brute_force_estimated_runtime_ms, 6),
            "theoretical_lower_bound_queries_omega_n": n_items,
            "theoretical_quantum_query_bound_o_sqrt_n": int(round(math.sqrt(n_items))),
        }
        rows.append(row)

    slope = linear_slope(executed_n, executed_log2_runtime)
    summary = {
        "probe_n_limit": args.probe_n_max,
        "n_values_total": len(n_values),
        "n_values_executed": len(executed_n),
        "n_values_skipped": len(n_values) - len(executed_n),
        "ops_per_second_estimate": round(ops_per_second, 2),
        "runtime_log2_slope_per_bit": round(slope, 6),
        "runtime_scaling_interpretation": interpret_slope(slope),
    }

    output_json = Path(args.output_json)
    output_md = Path(args.output_md)
    output_json.parent.mkdir(parents=True, exist_ok=True)
    output_md.parent.mkdir(parents=True, exist_ok=True)

    report: Dict[str, Any] = {
        "object": "ugc.quantum_analog.grover_scaling_report",
        "schema_version": "ugc_grover_scaling_report_v1",
        "generated_utc": datetime.now(timezone.utc).isoformat(),
        "deterministic": True,
        "invocation": " ".join(sys.argv),
        "config": {
            "python": args.python,
            "n_min": args.n_min,
            "n_max": args.n_max,
            "step": args.step,
            "trials": args.trials,
            "seed": args.seed,
            "stability_runs": args.stability_runs,
            "trace_every": args.trace_every,
            "probe_n_max": args.probe_n_max,
            "ops_calibration": args.ops_calibration,
        },
        "summary": summary,
        "rows": rows,
        "outputs": {
            "json_report_path": str(output_json),
            "markdown_report_path": str(output_md),
        },
    }

    output_json.write_text(
        json.dumps(
            report,
            sort_keys=True,
            ensure_ascii=True,
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    output_md.write_text(scaling_markdown(report) + "\n", encoding="utf-8")

    print(
        json.dumps(
            report,
            sort_keys=True,
            ensure_ascii=True,
            indent=2 if args.pretty else None,
            separators=None if args.pretty else (",", ":"),
        )
    )


if __name__ == "__main__":
    main()
