# Blade-Grade Calibration Prototype Scaffold

## Objective

Prototype bivector blade-grade calibration coefficients and measure their effect on annihilation stability across the existing multi-axis leaderboard:

- unwinding efficiency rank
- impedance matching rank
- pressure compliance rank
- combined leaderboard score (unweighted baseline)

## Baseline Reference

Use `v2.0.0` as immutable control.

## Experimental Knobs (Planned)

- blade_grade_alpha
- blade_grade_beta
- blade_grade_gamma
- transition_normalization_bias

## Evaluation Protocol

1. Run fixed profile set and seed schedule against baseline command.
2. Run calibrated variant with one coefficient change at a time.
3. Compare rank deltas for each axis and combined score.
4. Record conservation compliance transitions (`true -> false`, `false -> true`).

## Command Template

```bash
cargo run -- dirac-annihilation \
  --n-qubits 6 \
  --profiles uniform-random,low-grade-bias,high-grade-bias,harmonic-stride \
  --unwinding-steps 128 \
  --flux-coupling-density 0.40 \
  --sweep-export json \
  --output-prefix docs/demo/dirac-annihilation-dynamics
```

## Deliverables

- calibration run artifact JSON per experiment
- compact comparison table against `v2.0.0`
- recommendation for default coefficient set (if any)
