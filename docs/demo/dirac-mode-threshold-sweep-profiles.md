# Dirac Threshold Sweep Report

Generated UTC: 2026-06-09T13:48:43.711112+00:00

## Invocation

```bash
python3 scripts/run-dirac-threshold-sweep.py --n-values 5,6,7 --profiles uniform-random,contiguous-band,harmonic-stride --densities 0.02,0.08,0.10,0.12,0.14,0.2 --seeds 42,777 --output-prefix docs/demo/dirac-mode-threshold-sweep-profiles
```

## Interpretation

- Computational analog label: `computational_schwinger_limit_analog`
- Analog scope: `sparse_to_dense_crystallization_threshold`

## Summary Rows

| profile | n_qubits | first_crossing_density | spread_min | spread_max | spread | crossing_seed_count | threshold | runtime_ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| uniform-random | 5 | 0.12 | 0.12 | 0.12 | 0.0 | 2 | 0.12 | 34.50736 |
| uniform-random | 6 | 0.12 | 0.12 | 0.12 | 0.0 | 2 | 0.12 | 36.68999 |
| uniform-random | 7 | 0.14 | 0.14 | 0.14 | 0.0 | 2 | 0.12 | 51.52523 |
| contiguous-band | 5 | 0.12 | 0.12 | 0.12 | 0.0 | 2 | 0.12 | 39.779853 |
| contiguous-band | 6 | 0.12 | 0.12 | 0.12 | 0.0 | 2 | 0.12 | 42.654767 |
| contiguous-band | 7 | 0.14 | 0.14 | 0.14 | 0.0 | 2 | 0.12 | 60.909036 |
| harmonic-stride | 5 | 0.12 | 0.12 | 0.12 | 0.0 | 2 | 0.12 | 46.309563 |
| harmonic-stride | 6 | 0.12 | 0.12 | 0.12 | 0.0 | 2 | 0.12 | 41.470565 |
| harmonic-stride | 7 | 0.14 | 0.14 | 0.14 | 0.0 | 2 | 0.12 | 53.406387 |

## Artifacts

- JSON: docs/demo/dirac-mode-threshold-sweep-profiles.json
- CSV: docs/demo/dirac-mode-threshold-sweep-profiles.csv
- Markdown: docs/demo/dirac-mode-threshold-sweep-profiles.md
- SVG: docs/demo/dirac-mode-threshold-sweep-profiles.svg

