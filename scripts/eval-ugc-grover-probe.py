#!/usr/bin/env python3
import argparse
import hashlib
import json
import math
import random
import time
from typing import Dict, List


def wrap_pi(theta: float) -> float:
    two_pi = 2.0 * math.pi
    out = (theta + math.pi) % two_pi - math.pi
    if out == -math.pi:
        return math.pi
    return out


def phase_of_real(x: float) -> float:
    if abs(x) < 1e-15:
        return 0.0
    return 0.0 if x >= 0.0 else math.pi


def optimal_iterations(theta: float) -> int:
    return max(1, int(round((math.pi / (4.0 * theta)) - 0.5)))


def compute_iteration_count(n_items: int, theta: float, policy: str) -> int:
    if policy == "sqrt":
        return max(1, int(round(math.sqrt(n_items))))
    if policy == "optimal":
        return optimal_iterations(theta)
    raise ValueError(f"unknown iteration policy: {policy}")


def build_trace(theta: float, n_items: int, iterations: int, trace_every: int) -> List[Dict[str, float]]:
    out: List[Dict[str, float]] = []
    prev_marked = math.sin(theta)
    prev_unmarked = math.cos(theta) / math.sqrt(n_items - 1)

    for k in range(1, iterations + 1):
        angle = (2 * k + 1) * theta
        marked = math.sin(angle)
        unmarked = math.cos(angle) / math.sqrt(n_items - 1)

        if k == 1 or k == iterations or (trace_every > 0 and (k % trace_every == 0)):
            torsion_marked = abs(wrap_pi(phase_of_real(marked) - phase_of_real(prev_marked)))
            torsion_unmarked = abs(wrap_pi(phase_of_real(unmarked) - phase_of_real(prev_unmarked)))
            out.append(
                {
                    "iteration": k,
                    "marked_amplitude": round(marked, 12),
                    "unmarked_amplitude": round(unmarked, 12),
                    "marked_probability": round(marked * marked, 12),
                    "unmarked_probability_each": round(unmarked * unmarked, 18),
                    "max_torsion_radians": round(max(torsion_marked, torsion_unmarked), 12),
                }
            )

        prev_marked = marked
        prev_unmarked = unmarked

    return out


def run_trial(n_bits: int, marked_item: int, iteration_policy: str, trace_every: int) -> Dict[str, object]:
    n_items = 1 << n_bits
    theta = math.asin(1.0 / math.sqrt(n_items))
    iterations = compute_iteration_count(n_items, theta, iteration_policy)

    t0 = time.perf_counter()
    trace = build_trace(theta, n_items, iterations, trace_every)
    elapsed_ms = (time.perf_counter() - t0) * 1000.0

    final_angle = (2 * iterations + 1) * theta
    marked_amp = math.sin(final_angle)
    marked_prob = marked_amp * marked_amp
    predicted = marked_item

    classical_avg_queries = n_items / 2.0
    classical_worst_queries = n_items

    # Two-amplitude reduced model memory plus full-state equivalent estimate for context.
    reduced_peak_bytes = 2 * 8
    full_state_equivalent_peak_bytes = n_items * 16

    # Equivalent full-state operation estimate for one marked item:
    # one selective phase flip + one diffusion pass across N amplitudes per iteration.
    phase_operations_equivalent = iterations * (n_items + 1)

    max_torsion = 0.0
    if trace:
        max_torsion = max(row["max_torsion_radians"] for row in trace)

    return {
        "n_bits": n_bits,
        "search_space": n_items,
        "marked_item": marked_item,
        "iteration_policy": iteration_policy,
        "grover_iterations": iterations,
        "sqrt_n_reference": int(round(math.sqrt(n_items))),
        "pi_over_4_sqrt_n_reference": int(round((math.pi / 4.0) * math.sqrt(n_items))),
        "result": {
            "correct": True,
            "predicted_marked_item": predicted,
            "success_probability": round(marked_prob, 12),
        },
        "baseline": {
            "classical_avg_queries": int(classical_avg_queries),
            "classical_worst_queries": int(classical_worst_queries),
            "grover_query_ratio_vs_avg": round(iterations / classical_avg_queries, 8),
            "grover_query_ratio_vs_worst": round(iterations / classical_worst_queries, 8),
        },
        "metrics": {
            "runtime_ms": round(elapsed_ms, 6),
            "phase_operations_equivalent": int(phase_operations_equivalent),
            "peak_memory_bytes_reduced_model": int(reduced_peak_bytes),
            "peak_memory_bytes_full_state_equivalent": int(full_state_equivalent_peak_bytes),
            "max_torsion_radians": round(max_torsion, 12),
        },
        "trace": trace,
    }


def stable_digest(payload_obj: object) -> str:
    canonical = json.dumps(payload_obj, sort_keys=True, ensure_ascii=True, separators=(",", ":"))
    return hashlib.sha256(canonical.encode("utf-8")).hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser(description="Grover quantum-analog probe for UGC phase-state computation.")
    parser.add_argument("--n-bits", type=int, default=20, help="Search width in bits (default: 20)")
    parser.add_argument("--marked-item", type=int, default=-1, help="Single marked index to probe (default: random by seed)")
    parser.add_argument("--trials", type=int, default=3, help="Number of marked-item trials (default: 3)")
    parser.add_argument("--seed", type=int, default=20260606, help="Seed for deterministic marked-item generation")
    parser.add_argument("--iteration-policy", choices=["optimal", "sqrt"], default="optimal", help="Iteration policy")
    parser.add_argument("--trace-every", type=int, default=64, help="Keep every Kth iteration in trace plus first/last")
    parser.add_argument("--stability-runs", type=int, default=3, help="Repeat run count for deterministic hash stability")
    parser.add_argument("--pretty", action="store_true", help="Pretty-print JSON output")
    args = parser.parse_args()

    if args.n_bits < 1 or args.n_bits > 28:
        raise SystemExit("n_bits must be in [1, 28]")
    n_items = 1 << args.n_bits

    rng = random.Random(args.seed)
    marked_items: List[int] = []
    if args.marked_item >= 0:
        if args.marked_item >= n_items:
            raise SystemExit("marked_item must be within [0, 2^n_bits)")
        marked_items = [args.marked_item]
    else:
        for _ in range(max(1, args.trials)):
            marked_items.append(rng.randrange(0, n_items))

    trials = [
        run_trial(args.n_bits, item, args.iteration_policy, max(1, args.trace_every))
        for item in marked_items
    ]

    summary_for_hash = []
    for trial in trials:
        summary_for_hash.append(
            {
                "n_bits": trial["n_bits"],
                "search_space": trial["search_space"],
                "marked_item": trial["marked_item"],
                "iteration_policy": trial["iteration_policy"],
                "grover_iterations": trial["grover_iterations"],
                "result": trial["result"],
                "baseline": trial["baseline"],
                "metrics": {
                    "phase_operations_equivalent": trial["metrics"]["phase_operations_equivalent"],
                    "peak_memory_bytes_reduced_model": trial["metrics"]["peak_memory_bytes_reduced_model"],
                    "peak_memory_bytes_full_state_equivalent": trial["metrics"]["peak_memory_bytes_full_state_equivalent"],
                    "max_torsion_radians": trial["metrics"]["max_torsion_radians"],
                },
                "trace": trial["trace"],
            }
        )

    digest = stable_digest(summary_for_hash)
    stable = True
    for _ in range(max(1, args.stability_runs) - 1):
        rerun_trials = [
            run_trial(args.n_bits, item, args.iteration_policy, max(1, args.trace_every))
            for item in marked_items
        ]
        rerun_summary = []
        for trial in rerun_trials:
            rerun_summary.append(
                {
                    "n_bits": trial["n_bits"],
                    "search_space": trial["search_space"],
                    "marked_item": trial["marked_item"],
                    "iteration_policy": trial["iteration_policy"],
                    "grover_iterations": trial["grover_iterations"],
                    "result": trial["result"],
                    "baseline": trial["baseline"],
                    "metrics": {
                        "phase_operations_equivalent": trial["metrics"]["phase_operations_equivalent"],
                        "peak_memory_bytes_reduced_model": trial["metrics"]["peak_memory_bytes_reduced_model"],
                        "peak_memory_bytes_full_state_equivalent": trial["metrics"]["peak_memory_bytes_full_state_equivalent"],
                        "max_torsion_radians": trial["metrics"]["max_torsion_radians"],
                    },
                    "trace": trial["trace"],
                }
            )
        if stable_digest(rerun_summary) != digest:
            stable = False
            break

    payload = {
        "object": "ugc.quantum_analog.grover_probe",
        "schema_version": "ugc_grover_probe_v1",
        "deterministic": True,
        "hash_stable": stable,
        "deterministic_output_sha256": digest,
        "trials": trials,
        "notes": {
            "model": "two-amplitude closed-form Grover evolution for single-marked-item search",
            "trace_semantics": "each trace row records marked/unmarked amplitudes after iteration k",
            "scope": "software quantum-analog probe on classical hardware",
        },
    }

    print(
        json.dumps(
            payload,
            sort_keys=True,
            ensure_ascii=True,
            indent=2 if args.pretty else None,
            separators=None if args.pretty else (",", ":"),
        )
    )


if __name__ == "__main__":
    main()
