#!/usr/bin/env python3
import argparse
import json
import math
from typing import Callable, Dict, List, Tuple


ComplexVec = List[complex]


def hadamard() -> List[List[complex]]:
    s = 1.0 / math.sqrt(2.0)
    return [[s, s], [s, -s]]


def apply_single_qubit_gate(state: ComplexVec, gate: List[List[complex]], target: int) -> ComplexVec:
    # Basis ordering: |q0 q1> -> [|00>, |01>, |10>, |11>]
    out = [0j, 0j, 0j, 0j]
    for q0 in (0, 1):
        for q1 in (0, 1):
            src_index = (q0 << 1) | q1
            src_amp = state[src_index]
            bit = q0 if target == 0 else q1
            for out_bit in (0, 1):
                coeff = gate[out_bit][bit]
                nq0 = out_bit if target == 0 else q0
                nq1 = out_bit if target == 1 else q1
                dst_index = (nq0 << 1) | nq1
                out[dst_index] += coeff * src_amp
    return out


def apply_oracle_uf(state: ComplexVec, f: Callable[[int], int]) -> ComplexVec:
    # U_f |x, y> = |x, y xor f(x)>
    out = [0j, 0j, 0j, 0j]
    for x in (0, 1):
        for y in (0, 1):
            src_index = (x << 1) | y
            ny = y ^ f(x)
            dst_index = (x << 1) | ny
            out[dst_index] += state[src_index]
    return out


def probability_first_qubit(state: ComplexVec) -> Dict[str, float]:
    p0 = (abs(state[0]) ** 2) + (abs(state[1]) ** 2)
    p1 = (abs(state[2]) ** 2) + (abs(state[3]) ** 2)
    return {
        "0": float(round(p0, 12)),
        "1": float(round(p1, 12)),
    }


def complex_to_obj(z: complex) -> Dict[str, float]:
    return {
        "re": float(round(z.real, 12)),
        "im": float(round(z.imag, 12)),
    }


def state_with_phase(state: ComplexVec) -> List[Dict[str, object]]:
    basis = ["00", "01", "10", "11"]
    out = []
    for i, amp in enumerate(state):
        mag = abs(amp)
        phase = 0.0 if mag == 0.0 else math.atan2(amp.imag, amp.real)
        out.append(
            {
                "basis": basis[i],
                "amplitude": complex_to_obj(amp),
                "magnitude": float(round(mag, 12)),
                "phase_radians": float(round(phase, 12)),
            }
        )
    return out


def run_case(case_id: str, f: Callable[[int], int], expected: str) -> Dict[str, object]:
    h = hadamard()

    # Start in |0>|1>
    state: ComplexVec = [0j, 1 + 0j, 0j, 0j]

    state = apply_single_qubit_gate(state, h, target=0)
    state = apply_single_qubit_gate(state, h, target=1)
    state_after_oracle = apply_oracle_uf(state, f)
    state_final = apply_single_qubit_gate(state_after_oracle, h, target=0)

    probs = probability_first_qubit(state_final)
    predicted = "constant" if probs["0"] > 0.999999 else "balanced"

    return {
        "case_id": case_id,
        "expected": expected,
        "predicted": predicted,
        "match": expected == predicted,
        "measurement_first_qubit": probs,
        "state_after_oracle": state_with_phase(state_after_oracle),
        "state_final": state_with_phase(state_final),
    }


def main() -> None:
    parser = argparse.ArgumentParser(description="Deterministic Deutsch algorithm probe using UGC phase analogs.")
    parser.add_argument("--pretty", action="store_true", help="Pretty-print JSON output")
    args = parser.parse_args()

    cases: List[Tuple[str, Callable[[int], int], str]] = [
        ("f_zero", lambda x: 0, "constant"),
        ("f_one", lambda x: 1, "constant"),
        ("f_x", lambda x: x, "balanced"),
        ("f_not_x", lambda x: 1 - x, "balanced"),
    ]

    results = [run_case(case_id, f, expected) for case_id, f, expected in cases]
    pass_count = sum(1 for r in results if r["match"])

    payload = {
        "object": "ugc.quantum_analog.deutsch_probe",
        "schema_version": "ugc_deutsch_probe_v1",
        "deterministic": True,
        "phase_analog_mapping": {
            "superposition": "phase angles in principal interval [-pi, pi]",
            "entanglement": "edge-resonance-preserving relation",
            "phase_gate": "wrap_pi(theta + phi)",
            "hadamard_analog": "circular_mean(theta, theta + pi/2)",
            "measurement": "tri-state crystallization boundary",
            "interference": "multi-path constructive/destructive phase composition",
        },
        "results": results,
        "summary": {
            "cases": len(results),
            "passed": pass_count,
            "failed": len(results) - pass_count,
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
