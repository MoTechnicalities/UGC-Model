# Dirac Threshold Sweep Report

Generated UTC: 2026-06-09T14:03:01.885180+00:00

## Invocation

```bash
python3 scripts/run-dirac-threshold-sweep.py --n-values 5,6 --profiles uniform-random,low-grade-bias,high-grade-bias --densities 0.08,0.10,0.12,0.14,0.16,0.18,0.20,0.24,0.28,0.32,0.36,0.40 --seeds 42,777 --output-prefix docs/demo/dirac-mode-threshold-sweep-bias-families
```

## Interpretation

- Computational analog label: `computational_schwinger_limit_analog`
- Analog scope: `sparse_to_dense_crystallization_threshold`
- Per-profile threshold narrative:
  - `high-grade-bias`: crosses later than `uniform-random` on average (mean `delta_from_uniform=+0.28`, mean `crossing_density_ratio_to_uniform=3.33`), with sampled first-crossing range `0.40` to `0.40`.
  - `low-grade-bias`: crosses earlier than `uniform-random` on average (mean `delta_from_uniform=-0.03`, mean `crossing_density_ratio_to_uniform=0.75`), with sampled first-crossing range `0.08` to `0.10`.
  - `uniform-random`: baseline reference curve for `delta_from_uniform` comparisons.

## Summary Rows

| profile | n_qubits | first_crossing_density | delta_from_uniform | ratio_to_uniform | spread_min | spread_max | spread | crossing_seed_count | threshold | runtime_ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| uniform-random | 5 | 0.12 | None | None | 0.12 | 0.12 | 0.0 | 2 | 0.12 | 46.712063 |
| uniform-random | 6 | 0.12 | None | None | 0.12 | 0.12 | 0.0 | 2 | 0.12 | 45.930478 |
| low-grade-bias | 5 | 0.08 | -0.04 | 0.666667 | 0.08 | 0.08 | 0.0 | 2 | 0.12 | 38.338447 |
| low-grade-bias | 6 | 0.1 | -0.02 | 0.833333 | 0.1 | 0.1 | 0.0 | 2 | 0.12 | 51.700564 |
| high-grade-bias | 5 | 0.4 | 0.28 | 3.333333 | 0.4 | 0.4 | 0.0 | 2 | 0.12 | 45.785368 |
| high-grade-bias | 6 | 0.4 | 0.28 | 3.333333 | 0.4 | 0.4 | 0.0 | 2 | 0.12 | 55.829252 |

## Artifacts

- JSON: docs/demo/dirac-mode-threshold-sweep-bias-families.json
- CSV: docs/demo/dirac-mode-threshold-sweep-bias-families.csv
- Markdown: docs/demo/dirac-mode-threshold-sweep-bias-families.md
- SVG: docs/demo/dirac-mode-threshold-sweep-bias-families.svg

