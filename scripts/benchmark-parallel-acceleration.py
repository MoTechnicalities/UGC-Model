#!/usr/bin/env python3
import argparse
import json
import os
import statistics
import subprocess
import time
from datetime import datetime, timezone
from pathlib import Path


def run_command(command, env=None):
    start = time.perf_counter()
    subprocess.run(command, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, env=env)
    return (time.perf_counter() - start) * 1000.0


def measure_policy(command, runs, sequential):
    env = os.environ.copy()
    env["UGC_RAYON_DISABLE"] = "1" if sequential else "0"
    samples = [run_command(command, env=env) for _ in range(runs)]
    return {
        "samples_ms": [round(v, 3) for v in samples],
        "mean_ms": round(statistics.mean(samples), 3),
        "median_ms": round(statistics.median(samples), 3),
        "min_ms": round(min(samples), 3),
        "max_ms": round(max(samples), 3),
    }


def detect_parallel_workers():
    env_value = os.environ.get("RAYON_NUM_THREADS", "").strip()
    if env_value:
        try:
            parsed = int(env_value)
            if parsed >= 1:
                return parsed
        except ValueError:
            pass

    try:
        return len(os.sched_getaffinity(0))
    except Exception:
        pass

    cpu_count = os.cpu_count()
    if cpu_count and cpu_count >= 1:
        return cpu_count
    return 1


def detect_full_threads():
    try:
        return len(os.sched_getaffinity(0))
    except Exception:
        pass

    cpu_count = os.cpu_count()
    if cpu_count and cpu_count >= 1:
        return cpu_count
    return 1


def amdahl_serial_fraction(speedup_factor, workers):
    if workers <= 1 or speedup_factor <= 0.0:
        return {
            "serial_fraction_raw": None,
            "serial_fraction_clamped": None,
            "parallelizable_fraction": None,
        }

    denominator = 1.0 - (1.0 / workers)
    if denominator == 0.0:
        return {
            "serial_fraction_raw": None,
            "serial_fraction_clamped": None,
            "parallelizable_fraction": None,
        }

    serial_raw = ((1.0 / speedup_factor) - (1.0 / workers)) / denominator
    serial_clamped = min(1.0, max(0.0, serial_raw))
    return {
        "serial_fraction_raw": round(serial_raw, 6),
        "serial_fraction_clamped": round(serial_clamped, 6),
        "parallelizable_fraction": round(1.0 - serial_clamped, 6),
    }


def amdahl_speedup_for_threads(serial_fraction, workers):
    if serial_fraction is None or workers <= 0:
        return None

    denominator = serial_fraction + ((1.0 - serial_fraction) / workers)
    if denominator <= 0.0:
        return None
    return round(1.0 / denominator, 6)


def workload_commands(n_qubits):
    dirac_annihilation = [
        "cargo",
        "run",
        "--quiet",
        "--",
        "dirac-annihilation",
        "--n-qubits",
        str(n_qubits),
        "--profiles",
        "uniform-random,low-grade-bias,high-grade-bias,harmonic-stride",
        "--unwinding-steps",
        "128",
        "--flux-coupling-density",
        "0.40",
        "--sweep-export",
        "json",
    ]
    dirac_mode = [
        "cargo",
        "run",
        "--quiet",
        "--",
        "dirac-mode",
        "--summary",
        "--profile-report",
        "--n-qubits",
        str(n_qubits),
        "--state-model",
        "high-grade-bias",
        "--sweep-export",
        "json",
        "--sweep-coupling-densities",
        "0.08,0.12,0.16,0.20,0.24,0.28,0.32,0.36,0.40",
        "--sweep-seeds",
        "42,777,20260609,271828",
        "--perturbation-amplitudes",
        "0.0,0.2,0.6",
        "--perturbation-frequency",
        "24",
    ]
    return {
        "dirac_annihilation_multi_profile": dirac_annihilation,
        "dirac_mode_dense_sweep": dirac_mode,
    }


def append_markdown_report(path, payload):
    lines = []
    lines.append("## Run {}".format(payload["generated_at_utc"]))
    lines.append("")
    lines.append("Runs per policy: {}".format(payload["runs_per_policy"]))
    lines.append("Parallel workers detected (P): {}".format(payload["parallel_workers_detected"]))
    lines.append("Full-thread projection target (P_full): {}".format(payload["full_threads_detected"]))
    lines.append("n_qubits grid: {}".format(", ".join(str(v) for v in payload["n_qubits_values"])))
    lines.append("")
    lines.append("| workload | n_qubits | sequential_mean_ms | rayon_mean_ms | speedup_factor_s |")
    lines.append("|---|---:|---:|---:|---:|")

    for record in payload["records"]:
        lines.append(
            "| {} | {} | {:.3f} | {:.3f} | {:.3f} |".format(
                record["workload"],
                record["n_qubits"],
                record["sequential"]["mean_ms"],
                record["rayon"]["mean_ms"],
                record["speedup_factor_s"],
            )
        )

    lines.append("")
    lines.append("Speedup formula: S = sequential_wall_clock / rayon_wall_clock")
    lines.append("")
    lines.append("### Amdahl Scalability Analysis")
    lines.append("")
    lines.append("Amdahl estimate: f = ((1/S) - (1/P)) / (1 - (1/P))")
    lines.append("")
    lines.append("| workload | n_qubits | S | P | P_full | serial_fraction_raw_f | serial_fraction_clamped | parallelizable_fraction | predicted_speedup_at_full_threads |")
    lines.append("|---|---:|---:|---:|---:|---:|---:|---:|---:|")

    for record in payload["records"]:
        amdahl = record.get("amdahl", {})
        raw = amdahl.get("serial_fraction_raw")
        clamped = amdahl.get("serial_fraction_clamped")
        parallelizable = amdahl.get("parallelizable_fraction")
        projected = amdahl.get("predicted_speedup_at_full_threads")
        lines.append(
            "| {} | {} | {:.3f} | {} | {} | {} | {} | {} | {} |".format(
                record["workload"],
                record["n_qubits"],
                record["speedup_factor_s"],
                payload["parallel_workers_detected"],
                payload["full_threads_detected"],
                "n/a" if raw is None else "{:.6f}".format(raw),
                "n/a" if clamped is None else "{:.6f}".format(clamped),
                "n/a" if parallelizable is None else "{:.6f}".format(parallelizable),
                "n/a" if projected is None else "{:.6f}".format(projected),
            )
        )

    lines.append("")

    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists():
        existing = path.read_text(encoding="utf-8").rstrip() + "\n\n"
    else:
        existing = "# Parallel Acceleration Report\n\n"
    path.write_text(existing + "\n".join(lines) + "\n", encoding="utf-8")


def main():
    parser = argparse.ArgumentParser(
        description="Benchmark sequential vs Rayon acceleration on sweep-heavy workloads"
    )
    parser.add_argument("--n-values", default="6,7,8,10", help="Comma-separated n_qubits values")
    parser.add_argument("--runs", type=int, default=3, help="Runs per policy")
    parser.add_argument(
        "--json-output",
        default="docs/benchmarks/parallel-acceleration-report.json",
        help="Path for structured JSON benchmark output",
    )
    parser.add_argument(
        "--markdown-output",
        default="docs/benchmarks/parallel-acceleration-report.md",
        help="Path for markdown ledger output",
    )
    args = parser.parse_args()

    n_values = [int(token.strip()) for token in args.n_values.split(",") if token.strip()]
    if not n_values:
        raise SystemExit("--n-values must include at least one n_qubits value")

    records = []
    parallel_workers = detect_parallel_workers()
    full_threads = detect_full_threads()

    for n_qubits in n_values:
        commands = workload_commands(n_qubits)
        for workload, command in commands.items():
            # Warm-up each path once per workload/n pair.
            warm_env = os.environ.copy()
            warm_env["UGC_RAYON_DISABLE"] = "0"
            subprocess.run(command, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, env=warm_env)

            sequential = measure_policy(command, args.runs, sequential=True)
            rayon = measure_policy(command, args.runs, sequential=False)
            speedup = sequential["mean_ms"] / max(rayon["mean_ms"], 1e-9)
            amdahl = amdahl_serial_fraction(speedup, parallel_workers)
            amdahl["predicted_speedup_at_full_threads"] = amdahl_speedup_for_threads(
                amdahl["serial_fraction_clamped"],
                full_threads,
            )

            records.append(
                {
                    "workload": workload,
                    "n_qubits": n_qubits,
                    "command": command,
                    "sequential": sequential,
                    "rayon": rayon,
                    "speedup_factor_s": round(speedup, 3),
                    "amdahl": amdahl,
                }
            )

    payload = {
        "object": "csif.quantum.parallel_acceleration_report",
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "runs_per_policy": args.runs,
        "parallel_workers_detected": parallel_workers,
        "full_threads_detected": full_threads,
        "n_qubits_values": n_values,
        "records": records,
    }

    json_path = Path(args.json_output)
    json_path.parent.mkdir(parents=True, exist_ok=True)
    json_path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")

    append_markdown_report(Path(args.markdown_output), payload)

    print(json_path)
    print(args.markdown_output)


if __name__ == "__main__":
    main()
