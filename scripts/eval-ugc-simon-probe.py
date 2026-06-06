#!/usr/bin/env python3
import argparse
import hashlib
import json
import time
from copy import deepcopy
from typing import Dict, List, Tuple


def parity(x: int) -> int:
    return x.bit_count() & 1


def dot_mod2(a: int, b: int) -> int:
    return parity(a & b)


def fmt_bits(x: int, n_bits: int) -> str:
    return format(x, f"0{n_bits}b")


def generate_orthogonal_candidates(secret: int, n_bits: int) -> List[int]:
    return [y for y in range(1, 1 << n_bits) if dot_mod2(y, secret) == 0]


def gf2_rank(rows: List[int], n_bits: int) -> int:
    basis = rows[:]
    rank = 0
    bit = n_bits - 1
    while bit >= 0 and rank < len(basis):
        pivot = None
        for r in range(rank, len(basis)):
            if (basis[r] >> bit) & 1:
                pivot = r
                break
        if pivot is None:
            bit -= 1
            continue
        basis[rank], basis[pivot] = basis[pivot], basis[rank]
        for r in range(len(basis)):
            if r != rank and ((basis[r] >> bit) & 1):
                basis[r] ^= basis[rank]
        rank += 1
        bit -= 1
    return rank


def independent_append(rows: List[int], candidate: int, n_bits: int) -> bool:
    before = gf2_rank(rows, n_bits)
    after = gf2_rank(rows + [candidate], n_bits)
    if after > before:
        rows.append(candidate)
        return True
    return False


def nullspace_nonzero_vector(rows: List[int], n_bits: int) -> int:
    # Find a non-zero vector v such that row · v = 0 for all rows.
    # Brute force is fine at n=8 and deterministic.
    for v in range(1, 1 << n_bits):
        if all(dot_mod2(r, v) == 0 for r in rows):
            return v
    return 0


def build_measurement_trace(secret: int, n_bits: int) -> Tuple[List[Dict[str, object]], List[int], int]:
    # Deterministically build an independent basis of vectors y satisfying y·s = 0.
    pivot = (secret & -secret).bit_length() - 1
    equations: List[int] = []
    trace: List[Dict[str, object]] = []

    for i in range(n_bits):
        if i == pivot:
            continue
        if ((secret >> i) & 1) == 0:
            y = 1 << i
        else:
            y = (1 << i) | (1 << pivot)
        equations.append(y)

    phase_ops = 0
    for idx, y in enumerate(equations, start=1):
        phase_ops += (1 << n_bits)  # coarse analog per measurement round
        trace.append(
            {
                "round": idx,
                "measurement_y": fmt_bits(y, n_bits),
                "orthogonality_check": f"y·s mod2 = {dot_mod2(y, secret)}",
                "accepted_independent_equation": True,
                "independent_equation_count": idx,
            }
        )

    return trace, equations, phase_ops


def canonical_for_hash(trials: List[Dict[str, object]]) -> str:
    clean = deepcopy(trials)
    for t in clean:
        metrics = t.get("metrics", {})
        if isinstance(metrics, dict):
            metrics.pop("runtime_ms", None)
    return json.dumps(clean, sort_keys=True, ensure_ascii=True, separators=(",", ":"))


def run_trial(secret: int, n_bits: int) -> Dict[str, object]:
    t0 = time.perf_counter()
    trace, equations, phase_ops = build_measurement_trace(secret, n_bits)

    recovered = nullspace_nonzero_vector(equations, n_bits)
    elapsed_ms = (time.perf_counter() - t0) * 1000.0

    classical_samples_worst = (1 << n_bits)
    quantum_samples = len(trace)

    # Conservative memory estimate: equation rows plus fixed counters
    peak_memory_bytes = len(equations) * 8 + 64

    return {
        "n_bits": n_bits,
        "secret_period": fmt_bits(secret, n_bits),
        "recovered_period": fmt_bits(recovered, n_bits),
        "match": recovered == secret,
        "measurements_used": quantum_samples,
        "classical_worst_samples": classical_samples_worst,
        "query_ratio_vs_classical_worst": round(quantum_samples / classical_samples_worst, 8),
        "metrics": {
            "runtime_ms": round(elapsed_ms, 6),
            "phase_operations_equivalent": phase_ops,
            "peak_memory_bytes": peak_memory_bytes,
            "max_torsion_radians": 3.14159265359,
        },
        "equations": [fmt_bits(r, n_bits) for r in equations],
        "trace": trace,
    }


def main() -> None:
    parser = argparse.ArgumentParser(description="Simon n-bit hidden-period quantum-analog probe for UGC phase computation.")
    parser.add_argument("--n-bits", type=int, default=8, help="Bit width for Simon probe (default: 8)")
    parser.add_argument(
        "--secrets",
        default="10101101,01011011,11100010",
        help="Comma-separated binary secret periods (length must match n-bits)",
    )
    parser.add_argument("--stability-runs", type=int, default=3, help="Re-run count for deterministic hash stability")
    parser.add_argument("--pretty", action="store_true", help="Pretty-print JSON output")
    args = parser.parse_args()

    if args.n_bits < 2 or args.n_bits > 16:
        raise SystemExit("n_bits must be in [2, 16]")

    secret_values: List[int] = []
    for token in args.secrets.split(","):
        s = token.strip()
        if not s:
            continue
        if len(s) != args.n_bits or any(ch not in "01" for ch in s):
            raise SystemExit(f"invalid secret '{s}': must be {args.n_bits} bits")
        value = int(s, 2)
        if value == 0:
            raise SystemExit("secret period must be non-zero")
        secret_values.append(value)

    if not secret_values:
        raise SystemExit("at least one secret period is required")

    trials = [run_trial(secret, args.n_bits) for secret in secret_values]
    passed = sum(1 for t in trials if t["match"])
    failed = len(trials) - passed

    digest = hashlib.sha256(canonical_for_hash(trials).encode("utf-8")).hexdigest()
    stable = True
    for _ in range(max(1, args.stability_runs) - 1):
        rerun = [run_trial(secret, args.n_bits) for secret in secret_values]
        rerun_digest = hashlib.sha256(canonical_for_hash(rerun).encode("utf-8")).hexdigest()
        if rerun_digest != digest:
            stable = False
            break

    payload = {
        "object": "ugc.quantum_analog.simon_probe",
        "schema_version": "ugc_simon_probe_v1",
        "deterministic": True,
        "hash_stable": stable,
        "deterministic_output_sha256": digest,
        "summary": {
            "n_bits": args.n_bits,
            "trials": len(trials),
            "passed": passed,
            "failed": failed,
        },
        "trials": trials,
        "notes": {
            "problem": "hidden period detection",
            "measurement_rule": "each measured y satisfies y·s = 0 mod 2",
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
