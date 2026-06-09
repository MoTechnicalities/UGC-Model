# Vector 2 Parallel Benchmark vs Shor Baseline

Generated at: 2026-06-09T17:37:02.724243+00:00
Runs per command: 3

## Commands

- shor_baseline: cargo run --quiet -- shor --factoring-target 15 --base-a 2 --max-base-retries 4
- bell_sweep_parallel: cargo run --quiet -- bell-state --sweep-export json --sweep-noise-factors 0.0,0.1,0.2,0.3,0.4 --sweep-seeds 42,777,20260609,999
- dirac_mode_sweep_parallel: cargo run --quiet -- dirac-mode --summary --profile-report --n-qubits 8 --state-model high-grade-bias --sweep-export json --sweep-coupling-densities 0.08,0.12,0.16,0.20,0.24,0.28,0.32,0.36,0.40 --sweep-seeds 42,777,20260609 --perturbation-amplitudes 0.0,0.2,0.6 --perturbation-frequency 24
- dirac_annihilation_parallel: cargo run --quiet -- dirac-annihilation --n-qubits 6 --profiles uniform-random,low-grade-bias,high-grade-bias,harmonic-stride --unwinding-steps 128 --flux-coupling-density 0.40 --sweep-export json

## Results

| command | mean_ms | median_ms | min_ms | max_ms | mean_vs_shor_baseline_x |
|---|---:|---:|---:|---:|---:|
| shor_baseline | 37.826 | 36.975 | 35.133 | 41.371 | 1.000 |
| bell_sweep_parallel | 49.643 | 53.184 | 40.612 | 55.133 | 1.312 |
| dirac_mode_sweep_parallel | 99.175 | 98.957 | 92.885 | 105.684 | 2.622 |
| dirac_annihilation_parallel | 42.232 | 41.638 | 40.522 | 44.538 | 1.116 |

## Raw Samples (ms)

- shor_baseline: [35.133, 41.371, 36.975]
- bell_sweep_parallel: [40.612, 53.184, 55.133]
- dirac_mode_sweep_parallel: [98.957, 105.684, 92.885]
- dirac_annihilation_parallel: [40.522, 44.538, 41.638]
