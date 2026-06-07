#!/usr/bin/env python3
import argparse
import csv
import json
import math
import os
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Dict, List, Optional, Sequence, Tuple


def run_json_command(cmd: List[str], cwd: Optional[str] = None) -> Dict[str, Any]:
    start = time.perf_counter()
    proc = subprocess.run(cmd, check=False, capture_output=True, text=True, cwd=cwd)
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


def calibrate_ops_per_second(sample_ops: int) -> float:
    value = 0
    start = time.perf_counter()
    for i in range(sample_ops):
        value ^= (i * 2654435761) & 0xFFFFFFFF
    elapsed = max(time.perf_counter() - start, 1e-9)
    if value == -1:
        print("impossible", file=sys.stderr)
    return sample_ops / elapsed


def linear_fit(xs: Sequence[float], ys: Sequence[float]) -> Tuple[float, float]:
    if len(xs) != len(ys) or len(xs) < 2:
        return 0.0, 0.0
    mx = sum(xs) / len(xs)
    my = sum(ys) / len(ys)
    den = sum((x - mx) * (x - mx) for x in xs)
    if den == 0.0:
        return 0.0, my
    num = sum((x - mx) * (y - my) for x, y in zip(xs, ys))
    slope = num / den
    intercept = my - slope * mx
    return slope, intercept


def fit_loglog(points: Sequence[Tuple[float, float]]) -> Dict[str, float]:
    if len(points) < 2:
        return {"slope": 0.0, "intercept": 0.0, "r2": 0.0}
    lx = [math.log10(x) for x, _ in points if x > 0.0]
    ly = [math.log10(y) for _, y in points if y > 0.0]
    if len(lx) < 2 or len(ly) < 2:
        return {"slope": 0.0, "intercept": 0.0, "r2": 0.0}
    slope, intercept = linear_fit(lx, ly)
    pred = [intercept + slope * x for x in lx]
    my = sum(ly) / len(ly)
    ss_tot = sum((y - my) * (y - my) for y in ly)
    ss_res = sum((y - p) * (y - p) for y, p in zip(ly, pred))
    r2 = 0.0 if ss_tot == 0.0 else 1.0 - (ss_res / ss_tot)
    return {"slope": round(slope, 6), "intercept": round(intercept, 6), "r2": round(r2, 6)}


def svg_escape(text: str) -> str:
    return (
        text.replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace('"', "&quot;")
    )


def make_loglog_svg(
    rows: List[Dict[str, Any]],
    out_path: Path,
    py_fit: Dict[str, float],
    rust_fit: Dict[str, float],
) -> None:
    points_py = [(r["search_space"], r["python_wall_runtime_ms"]) for r in rows if r["status"] == "executed"]
    points_rust = [(r["search_space"], r["rust_wall_runtime_ms"]) for r in rows if r["status"] == "executed"]

    if len(points_py) < 2 or len(points_rust) < 2:
        out_path.write_text("<svg xmlns='http://www.w3.org/2000/svg' width='960' height='540'><text x='20' y='30'>Not enough executed points for log-log plot.</text></svg>\n", encoding="utf-8")
        return

    min_x = min(p[0] for p in points_py)
    max_x = max(p[0] for p in points_py)
    min_y = min(min(p[1] for p in points_py), min(p[1] for p in points_rust))
    max_y = max(max(p[1] for p in points_py), max(p[1] for p in points_rust))

    lx0 = math.log10(min_x)
    lx1 = math.log10(max_x)
    ly0 = math.log10(min_y)
    ly1 = math.log10(max_y)

    w = 960
    h = 540
    ml = 90
    mr = 30
    mt = 30
    mb = 70
    pw = w - ml - mr
    ph = h - mt - mb

    def to_px(x: float) -> float:
        return ml + (math.log10(x) - lx0) / max(lx1 - lx0, 1e-9) * pw

    def to_py(y: float) -> float:
        return mt + (ly1 - math.log10(y)) / max(ly1 - ly0, 1e-9) * ph

    def polyline(points: Sequence[Tuple[float, float]], color: str) -> str:
        coords = " ".join(f"{to_px(x):.2f},{to_py(y):.2f}" for x, y in points)
        return f"<polyline fill='none' stroke='{color}' stroke-width='2' points='{coords}'/>"

    py_sorted = sorted(points_py, key=lambda p: p[0])
    rust_sorted = sorted(points_rust, key=lambda p: p[0])

    fit_x = [min_x, max_x]
    py_line = [(x, 10 ** (py_fit["intercept"] + py_fit["slope"] * math.log10(x))) for x in fit_x]
    rust_line = [(x, 10 ** (rust_fit["intercept"] + rust_fit["slope"] * math.log10(x))) for x in fit_x]

    ticks_x = sorted(set([10 ** p for p in range(int(math.floor(lx0)), int(math.ceil(lx1)) + 1)] + [min_x, max_x]))
    ticks_y = sorted(set([10 ** p for p in range(int(math.floor(ly0)), int(math.ceil(ly1)) + 1)] + [min_y, max_y]))

    parts: List[str] = []
    parts.append(f"<svg xmlns='http://www.w3.org/2000/svg' width='{w}' height='{h}' viewBox='0 0 {w} {h}'>")
    parts.append("<rect x='0' y='0' width='100%' height='100%' fill='white'/>")
    parts.append(f"<text x='{ml}' y='20' font-family='monospace' font-size='14'>UGC Grover Scaling: Python vs Rust (log-log)</text>")

    # Axes
    parts.append(f"<line x1='{ml}' y1='{mt + ph}' x2='{ml + pw}' y2='{mt + ph}' stroke='black' stroke-width='1'/>")
    parts.append(f"<line x1='{ml}' y1='{mt}' x2='{ml}' y2='{mt + ph}' stroke='black' stroke-width='1'/>")

    for tx in ticks_x:
        if tx <= 0:
            continue
        x = to_px(tx)
        parts.append(f"<line x1='{x:.2f}' y1='{mt + ph}' x2='{x:.2f}' y2='{mt + ph + 5}' stroke='#666' stroke-width='1'/>")
        parts.append(f"<text x='{x:.2f}' y='{mt + ph + 20}' text-anchor='middle' font-family='monospace' font-size='10'>{svg_escape(f'{tx:.0f}')}</text>")

    for ty in ticks_y:
        if ty <= 0:
            continue
        y = to_py(ty)
        parts.append(f"<line x1='{ml - 5}' y1='{y:.2f}' x2='{ml}' y2='{y:.2f}' stroke='#666' stroke-width='1'/>")
        parts.append(f"<text x='{ml - 8}' y='{y + 3:.2f}' text-anchor='end' font-family='monospace' font-size='10'>{svg_escape(f'{ty:.3f}')}</text>")

    # Data + fit lines
    parts.append(polyline(py_sorted, "#1f77b4"))
    parts.append(polyline(rust_sorted, "#d62728"))
    parts.append(polyline(py_line, "#1f77b4"))
    parts.append(polyline(rust_line, "#d62728"))

    for x, y in py_sorted:
        parts.append(f"<circle cx='{to_px(x):.2f}' cy='{to_py(y):.2f}' r='3' fill='#1f77b4'/>")
    for x, y in rust_sorted:
        parts.append(f"<circle cx='{to_px(x):.2f}' cy='{to_py(y):.2f}' r='3' fill='#d62728'/>")

    # Labels
    parts.append(f"<text x='{ml + pw / 2:.2f}' y='{h - 20}' text-anchor='middle' font-family='monospace' font-size='12'>search space N = 2^n (log scale)</text>")
    parts.append(f"<text transform='translate(20 {mt + ph / 2:.2f}) rotate(-90)' text-anchor='middle' font-family='monospace' font-size='12'>wall runtime ms (log scale)</text>")

    legend_x = ml + 10
    legend_y = mt + 10
    parts.append(f"<rect x='{legend_x}' y='{legend_y}' width='420' height='56' fill='white' stroke='#ccc'/>")
    parts.append(f"<circle cx='{legend_x + 12}' cy='{legend_y + 16}' r='4' fill='#1f77b4'/>")
    parts.append(f"<text x='{legend_x + 24}' y='{legend_y + 20}' font-family='monospace' font-size='11'>Python fit slope={py_fit['slope']}, R^2={py_fit['r2']}</text>")
    parts.append(f"<circle cx='{legend_x + 12}' cy='{legend_y + 38}' r='4' fill='#d62728'/>")
    parts.append(f"<text x='{legend_x + 24}' y='{legend_y + 42}' font-family='monospace' font-size='11'>Rust fit slope={rust_fit['slope']}, R^2={rust_fit['r2']}</text>")

    parts.append("</svg>")
    out_path.write_text("\n".join(parts) + "\n", encoding="utf-8")


def markdown_line(items: List[str]) -> str:
    return "| " + " | ".join(items) + " |"


def render_markdown(report: Dict[str, Any]) -> str:
    lines: List[str] = []
    lines.append("# UGC Grover Scaling Comparison Report (Python vs Rust)")
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
    lines.append(markdown_line(["Metric", "Value"]))
    lines.append(markdown_line(["---", "---"]))
    for key in [
        "n_values_total",
        "n_values_executed",
        "n_values_skipped",
        "ops_per_second_estimate",
        "python_loglog_slope",
        "python_loglog_r2",
        "rust_loglog_slope",
        "rust_loglog_r2",
    ]:
        lines.append(markdown_line([key, str(report["summary"][key])]))

    lines.append("")
    lines.append("## Per-n Rows")
    lines.append("")
    lines.append(
        markdown_line(
            [
                "n",
                "status",
                "search_space",
                "grover_iter",
                "python_wall_ms",
                "rust_wall_ms",
                "python_vs_rust_ratio",
                "bruteforce_est_ms",
            ]
        )
    )
    lines.append(markdown_line(["---:", "---", "---:", "---:", "---:", "---:", "---:", "---:"]))
    for row in report["rows"]:
        lines.append(
            markdown_line(
                [
                    str(row.get("n_bits", "")),
                    str(row.get("status", "")),
                    str(row.get("search_space", "")),
                    str(row.get("grover_iterations", "")),
                    str(row.get("python_wall_runtime_ms", "")),
                    str(row.get("rust_wall_runtime_ms", "")),
                    str(row.get("python_over_rust_runtime_ratio", "")),
                    str(row.get("bruteforce_estimated_runtime_ms", "")),
                ]
            )
        )

    lines.append("")
    lines.append("## Artifacts")
    lines.append("")
    lines.append(f"- JSON report: {report['outputs']['json_report_path']}")
    lines.append(f"- Markdown report: {report['outputs']['markdown_report_path']}")
    lines.append(f"- CSV export: {report['outputs']['csv_report_path']}")
    lines.append(f"- Log-log SVG plot: {report['outputs']['plot_svg_path']}")
    lines.append("")
    return "\n".join(lines)


def write_csv(rows: List[Dict[str, Any]], path: Path) -> None:
    fieldnames = [
        "n_bits",
        "status",
        "search_space",
        "grover_iterations",
        "success_probability",
        "query_ratio_vs_classical_worst",
        "python_wall_runtime_ms",
        "python_internal_avg_runtime_ms",
        "rust_wall_runtime_ms",
        "rust_internal_avg_runtime_ms",
        "python_over_rust_runtime_ratio",
        "bruteforce_estimated_runtime_ms",
        "theoretical_lower_bound_queries_omega_n",
        "theoretical_quantum_query_bound_o_sqrt_n",
    ]
    with path.open("w", encoding="utf-8", newline="") as fh:
        writer = csv.DictWriter(fh, fieldnames=fieldnames)
        writer.writeheader()
        for row in rows:
            writer.writerow({k: row.get(k, "") for k in fieldnames})


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Run Grover scaling in both Python and Rust native implementations, then emit JSON/Markdown/CSV and a log-log fit SVG."
    )
    parser.add_argument("--python", default=sys.executable, help="Python executable for running probe scripts")
    parser.add_argument("--n-min", type=int, default=10, help="Minimum n (default: 10)")
    parser.add_argument("--n-max", type=int, default=50, help="Maximum n (default: 50)")
    parser.add_argument("--step", type=int, default=5, help="n step size (default: 5)")
    parser.add_argument("--trials", type=int, default=3, help="Trials passed to both probes")
    parser.add_argument("--seed", type=int, default=20260606, help="Seed for deterministic marked-item generation")
    parser.add_argument("--stability-runs", type=int, default=1, help="Stability reruns passed to both probes")
    parser.add_argument("--trace-every", type=int, default=1000000, help="Trace sampling period for both probes")
    parser.add_argument("--probe-n-max", type=int, default=50, help="Current Grover probe hard n limit")
    parser.add_argument("--ops-calibration", type=int, default=5_000_000, help="Integer operations for brute-force throughput calibration")
    parser.add_argument("--rust-runner", choices=["cargo", "binary"], default="cargo", help="How to run Rust probe")
    parser.add_argument("--rust-binary", default="target/debug/csif_agent_v2_rust", help="Rust binary path when --rust-runner binary")
    parser.add_argument("--output-json", default="docs/demo/grover-scaling-rust-compare.json", help="Output JSON report path")
    parser.add_argument("--output-md", default="docs/demo/grover-scaling-rust-compare.md", help="Output Markdown report path")
    parser.add_argument("--output-csv", default="docs/demo/grover-scaling-rust-compare.csv", help="Output CSV report path")
    parser.add_argument("--output-plot", default="docs/demo/grover-scaling-rust-compare-loglog.svg", help="Output SVG plot path")
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

    repo_root = os.getcwd()
    ops_per_second = calibrate_ops_per_second(max(1_000_000, args.ops_calibration))

    rows: List[Dict[str, Any]] = []
    fit_points_python: List[Tuple[float, float]] = []
    fit_points_rust: List[Tuple[float, float]] = []

    for n_bits in n_values:
        search_space = 1 << n_bits
        brute_force_estimated_runtime_ms = (search_space / ops_per_second) * 1000.0

        if n_bits > args.probe_n_max:
            rows.append(
                {
                    "n_bits": n_bits,
                    "status": "skipped_probe_limit",
                    "search_space": search_space,
                    "bruteforce_estimated_runtime_ms": round(brute_force_estimated_runtime_ms, 6),
                    "theoretical_lower_bound_queries_omega_n": search_space,
                    "theoretical_quantum_query_bound_o_sqrt_n": int(round(math.sqrt(search_space))),
                }
            )
            continue

        py_payload = run_json_command(
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
            ],
            cwd=repo_root,
        )

        if args.rust_runner == "cargo":
            rust_cmd = [
                "cargo",
                "run",
                "--quiet",
                "--",
                "grover-probe-native",
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
        else:
            rust_cmd = [
                args.rust_binary,
                "grover-probe-native",
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

        rust_payload = run_json_command(rust_cmd, cwd=repo_root)

        py_trials = py_payload.get("trials", [])
        rust_trials = rust_payload.get("trials", [])
        if not py_trials or not rust_trials:
            raise RuntimeError(f"missing trials for n={n_bits}")

        py_first = py_trials[0]
        rust_first = rust_trials[0]

        py_internal_avg = sum(float(t.get("metrics", {}).get("runtime_ms", 0.0)) for t in py_trials) / max(1, len(py_trials))
        rust_internal_avg = sum(float(t.get("metrics", {}).get("runtime_ms", 0.0)) for t in rust_trials) / max(1, len(rust_trials))

        py_wall = float(py_payload.get("_wall_runtime_ms", 0.0))
        rust_wall = float(rust_payload.get("_wall_runtime_ms", 0.0))
        ratio = py_wall / rust_wall if rust_wall > 0 else 0.0

        fit_points_python.append((float(search_space), max(py_wall, 1e-9)))
        fit_points_rust.append((float(search_space), max(rust_wall, 1e-9)))

        rows.append(
            {
                "n_bits": n_bits,
                "status": "executed",
                "search_space": int(py_first["search_space"]),
                "grover_iterations": int(py_first["grover_iterations"]),
                "success_probability": float(py_first["result"]["success_probability"]),
                "query_ratio_vs_classical_worst": float(py_first["baseline"]["grover_query_ratio_vs_worst"]),
                "python_wall_runtime_ms": round(py_wall, 6),
                "python_internal_avg_runtime_ms": round(py_internal_avg, 6),
                "rust_wall_runtime_ms": round(rust_wall, 6),
                "rust_internal_avg_runtime_ms": round(rust_internal_avg, 6),
                "python_over_rust_runtime_ratio": round(ratio, 6),
                "deterministic_hash_stable_python": bool(py_payload.get("hash_stable", False)),
                "deterministic_hash_stable_rust": bool(rust_payload.get("hash_stable", False)),
                "bruteforce_estimated_runtime_ms": round(brute_force_estimated_runtime_ms, 6),
                "theoretical_lower_bound_queries_omega_n": search_space,
                "theoretical_quantum_query_bound_o_sqrt_n": int(round(math.sqrt(search_space))),
            }
        )

    py_fit = fit_loglog(fit_points_python)
    rust_fit = fit_loglog(fit_points_rust)

    summary = {
        "n_values_total": len(n_values),
        "n_values_executed": len([r for r in rows if r.get("status") == "executed"]),
        "n_values_skipped": len([r for r in rows if r.get("status") != "executed"]),
        "ops_per_second_estimate": round(ops_per_second, 2),
        "python_loglog_slope": py_fit["slope"],
        "python_loglog_r2": py_fit["r2"],
        "rust_loglog_slope": rust_fit["slope"],
        "rust_loglog_r2": rust_fit["r2"],
    }

    output_json = Path(args.output_json)
    output_md = Path(args.output_md)
    output_csv = Path(args.output_csv)
    output_plot = Path(args.output_plot)

    for path in [output_json, output_md, output_csv, output_plot]:
        path.parent.mkdir(parents=True, exist_ok=True)

    report: Dict[str, Any] = {
        "object": "ugc.quantum_analog.grover_scaling_rust_compare",
        "schema_version": "ugc_grover_scaling_rust_compare_v1",
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
            "rust_runner": args.rust_runner,
            "rust_binary": args.rust_binary,
        },
        "summary": summary,
        "loglog_fit": {
            "python": py_fit,
            "rust": rust_fit,
        },
        "rows": rows,
        "outputs": {
            "json_report_path": str(output_json),
            "markdown_report_path": str(output_md),
            "csv_report_path": str(output_csv),
            "plot_svg_path": str(output_plot),
        },
    }

    write_csv(rows, output_csv)
    make_loglog_svg(rows, output_plot, py_fit, rust_fit)

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
    output_md.write_text(render_markdown(report) + "\n", encoding="utf-8")

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
