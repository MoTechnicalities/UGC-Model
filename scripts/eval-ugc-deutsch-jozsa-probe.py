#!/usr/bin/env python3
import argparse
import hashlib
import json
import math
import time
from copy import deepcopy
from dataclasses import dataclass
from typing import Dict, List, Tuple


EPS = 1e-12


@dataclass(frozen=True)
class OracleSpec:
    oracle_id: str
    kind: str
    n_bits: int
    a_mask: int
    b_bit: int


def wrap_pi(theta: float) -> float:
    two_pi = 2.0 * math.pi
    out = (theta + math.pi) % two_pi - math.pi
    if out == -math.pi:
        return math.pi
    return out


def phase_of(z: complex) -> float:
    if abs(z) < EPS:
        return 0.0
    return math.atan2(z.imag, z.real)


def max_phase_torsion(before: List[complex], after: List[complex]) -> float:
    max_delta = 0.0
    for a, b in zip(before, after):
        pa = phase_of(a)
        pb = phase_of(b)
        delta = abs(wrap_pi(pb - pa))
        if delta > max_delta:
            max_delta = delta
    return max_delta


def parity(x: int) -> int:
    return x.bit_count() & 1


def f_value(spec: OracleSpec, x: int) -> int:
    if spec.kind == "constant":
        return spec.b_bit
    return parity(spec.a_mask & x) ^ spec.b_bit


def build_oracles(n_bits: int) -> List[OracleSpec]:
    target_count = 1 << n_bits
    out: List[OracleSpec] = [
        OracleSpec(
            oracle_id=f"n{n_bits}_constant_0",
            kind="constant",
            n_bits=n_bits,
            a_mask=0,
            b_bit=0,
        ),
        OracleSpec(
            oracle_id=f"n{n_bits}_constant_1",
            kind="constant",
            n_bits=n_bits,
            a_mask=0,
            b_bit=1,
        ),
    ]

    # Fill the remaining slots with deterministic balanced affine functions.
    # f(x) = parity(a & x) xor b, where a != 0 guarantees balanced behavior.
    a_values = list(range(1, 1 << n_bits))
    idx = 0
    while len(out) < target_count:
        a_mask = a_values[idx % len(a_values)]
        b_bit = (idx // len(a_values)) & 1
        out.append(
            OracleSpec(
                oracle_id=f"n{n_bits}_balanced_a{a_mask:0{n_bits}b}_b{b_bit}",
                kind="balanced",
                n_bits=n_bits,
                a_mask=a_mask,
                b_bit=b_bit,
            )
        )
        idx += 1

    return out


def apply_hadamard_all(state: List[complex], n_bits: int) -> Tuple[List[complex], int]:
    out = state[:]
    phase_ops = 0

    for qubit in range(n_bits):
        step = 1 << qubit
        block = step << 1
        scale = 1.0 / math.sqrt(2.0)
        for start in range(0, len(out), block):
            for i in range(step):
                i0 = start + i
                i1 = i0 + step
                a = out[i0]
                b = out[i1]
                out[i0] = (a + b) * scale
                out[i1] = (a - b) * scale
                phase_ops += 4

    return out, phase_ops


def apply_oracle_phase_kickback(state: List[complex], spec: OracleSpec) -> Tuple[List[complex], int]:
    out = state[:]
    phase_ops = 0
    for x in range(len(out)):
        if f_value(spec, x):
            out[x] = -out[x]
            phase_ops += 1
    return out, phase_ops


def run_case(spec: OracleSpec) -> Dict[str, object]:
    n_bits = spec.n_bits
    dimension = 1 << n_bits
    state0 = [0.0j for _ in range(dimension)]
    state0[0] = 1.0 + 0.0j

    t0 = time.perf_counter()
    state1, ops_h1 = apply_hadamard_all(state0, n_bits)
    state2, ops_oracle = apply_oracle_phase_kickback(state1, spec)
    state3, ops_h2 = apply_hadamard_all(state2, n_bits)
    elapsed_ms = (time.perf_counter() - t0) * 1000.0

    p_zero = abs(state3[0]) ** 2
    predicted = "constant" if p_zero > 0.999999 else "balanced"
    expected = spec.kind

    torsion_12 = max_phase_torsion(state1, state2)
    torsion_23 = max_phase_torsion(state2, state3)

    state_bytes = len(state0) * 16
    peak_estimated_bytes = max(3 * state_bytes, 1)

    return {
        "oracle_id": spec.oracle_id,
        "expected": expected,
        "predicted": predicted,
        "match": expected == predicted,
        "measurement": {
            "p_zero_state": round(float(p_zero), 12),
            "p_nonzero_state": round(float(max(0.0, 1.0 - p_zero)), 12),
        },
        "metrics": {
            "runtime_ms": round(float(elapsed_ms), 6),
            "phase_operations": int(ops_h1 + ops_oracle + ops_h2),
            "max_torsion_radians": round(float(max(torsion_12, torsion_23)), 12),
            "state_dimension": dimension,
            "estimated_peak_memory_bytes": peak_estimated_bytes,
        },
    }


def canonical_results_for_stability(results: List[Dict[str, object]]) -> str:
    sanitized = deepcopy(results)
    for row in sanitized:
        metrics = row.get("metrics", {})
        if isinstance(metrics, dict):
            metrics.pop("runtime_ms", None)
    return json.dumps(sanitized, sort_keys=True, ensure_ascii=True, separators=(",", ":"))


def run_batch(n_bits: int, stability_runs: int) -> Dict[str, object]:
    oracles = build_oracles(n_bits)
    results = [run_case(o) for o in oracles]

    passed = sum(1 for r in results if r["match"])
    failed = len(results) - passed

    total_runtime = sum(r["metrics"]["runtime_ms"] for r in results)
    max_torsion = max(r["metrics"]["max_torsion_radians"] for r in results)
    total_phase_ops = sum(r["metrics"]["phase_operations"] for r in results)
    peak_memory = max(r["metrics"]["estimated_peak_memory_bytes"] for r in results)

    canonical = canonical_results_for_stability(results)
    digest = hashlib.sha256(canonical.encode("utf-8")).hexdigest()
    stable = True
    for _ in range(max(1, stability_runs) - 1):
        rerun = [run_case(o) for o in oracles]
        rerun_canonical = canonical_results_for_stability(rerun)
        rerun_digest = hashlib.sha256(rerun_canonical.encode("utf-8")).hexdigest()
        if rerun_digest != digest:
            stable = False
            break

    return {
        "n_bits": n_bits,
        "oracle_count": len(oracles),
        "summary": {
            "passed": passed,
            "failed": failed,
            "stability_hash_stable": stable,
            "deterministic_output_sha256": digest,
        },
        "aggregate_metrics": {
            "total_runtime_ms": round(float(total_runtime), 6),
            "avg_runtime_ms": round(float(total_runtime / max(1, len(results))), 6),
            "total_phase_operations": int(total_phase_ops),
            "max_torsion_radians": round(float(max_torsion), 12),
            "estimated_peak_memory_bytes": int(peak_memory),
        },
        "results": results,
    }


def parse_n_values(raw: str) -> List[int]:
    values = []
    for token in raw.split(","):
        token = token.strip()
        if not token:
            continue
        n = int(token)
        if n < 1 or n > 12:
            raise ValueError("n_bits must be in [1, 12]")
        values.append(n)
    if not values:
        raise ValueError("at least one n_bits value is required")
    return values


def main() -> None:
    parser = argparse.ArgumentParser(description="Deterministic Deutsch-Jozsa quantum-analog probe for UGC phase computation.")
    parser.add_argument("--n-values", default="3,4", help="Comma-separated n values, default: 3,4")
    parser.add_argument("--stability-runs", type=int, default=3, help="Repeated runs per n for deterministic hash stability")
    parser.add_argument("--pretty", action="store_true", help="Pretty-print JSON output")
    args = parser.parse_args()

    n_values = parse_n_values(args.n_values)
    batches = [run_batch(n, args.stability_runs) for n in n_values]

    payload = {
        "object": "ugc.quantum_analog.deutsch_jozsa_probe",
        "schema_version": "ugc_deutsch_jozsa_probe_v1",
        "deterministic": True,
        "n_values": n_values,
        "batches": batches,
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
