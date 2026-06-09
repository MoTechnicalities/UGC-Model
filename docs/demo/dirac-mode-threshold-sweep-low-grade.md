# Dirac Threshold Sweep Report

Generated UTC: 2026-06-09T13:55:00.401851+00:00

## Invocation

```bash
python3 scripts/run-dirac-threshold-sweep.py --n-values 5,6 --profiles uniform-random,low-grade-bias --densities 0.08,0.10,0.12 --seeds 42,777 --output-prefix docs/demo/dirac-mode-threshold-sweep-low-grade
```

## Interpretation

- Computational analog label: `computational_schwinger_limit_analog`
- Analog scope: `sparse_to_dense_crystallization_threshold`

## Summary Rows

| profile | n_qubits | first_crossing_density | delta_from_uniform | spread_min | spread_max | spread | crossing_seed_count | threshold | runtime_ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| uniform-random | 5 | 0.12 | None | 0.12 | 0.12 | 0.0 | 2 | 0.12 | 36.231168 |
| uniform-random | 6 | 0.12 | None | 0.12 | 0.12 | 0.0 | 2 | 0.12 | 46.840914 |
| low-grade-bias | 5 | 0.08 | -0.04 | 0.08 | 0.08 | 0.0 | 2 | 0.12 | 39.547323 |
| low-grade-bias | 6 | 0.1 | -0.02 | 0.1 | 0.1 | 0.0 | 2 | 0.12 | 33.377893 |

## Artifacts

- JSON: docs/demo/dirac-mode-threshold-sweep-low-grade.json
- CSV: docs/demo/dirac-mode-threshold-sweep-low-grade.csv
- Markdown: docs/demo/dirac-mode-threshold-sweep-low-grade.md
- SVG: docs/demo/dirac-mode-threshold-sweep-low-grade.svg

