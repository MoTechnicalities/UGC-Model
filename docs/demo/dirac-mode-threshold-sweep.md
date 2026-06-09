# Dirac Threshold Sweep Report

Generated UTC: 2026-06-09T13:43:28.003312+00:00

## Invocation

```bash
python3 scripts/run-dirac-threshold-sweep.py --n-values 4,5,6,7,8 --densities 0.02,0.08,0.10,0.12,0.14,0.2,0.3 --seeds 42,777,20260609 --output-prefix docs/demo/dirac-mode-threshold-sweep
```

## Interpretation

- Computational analog label: `computational_schwinger_limit_analog`
- Analog scope: `sparse_to_dense_crystallization_threshold`

## Summary Rows

| n_qubits | first_crossing_density | spread_min | spread_max | spread | crossing_seed_count | threshold | runtime_ms |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 4 | 0.12 | 0.12 | 0.12 | 0.0 | 3 | 0.12 | 41.482897 |
| 5 | 0.12 | 0.12 | 0.12 | 0.0 | 3 | 0.12 | 38.651368 |
| 6 | 0.12 | 0.12 | 0.12 | 0.0 | 3 | 0.12 | 49.331625 |
| 7 | 0.14 | 0.14 | 0.14 | 0.0 | 3 | 0.12 | 57.105357 |
| 8 | 0.14 | 0.14 | 0.14 | 0.0 | 3 | 0.12 | 112.421711 |

## Artifacts

- JSON: docs/demo/dirac-mode-threshold-sweep.json
- CSV: docs/demo/dirac-mode-threshold-sweep.csv
- Markdown: docs/demo/dirac-mode-threshold-sweep.md
- SVG: docs/demo/dirac-mode-threshold-sweep.svg

