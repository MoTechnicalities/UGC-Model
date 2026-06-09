# Parallel Acceleration Report

## Run 2026-06-09T17:49:11.076168+00:00

Runs per policy: 2
n_qubits grid: 6, 7, 8

| workload | n_qubits | sequential_mean_ms | rayon_mean_ms | speedup_factor_s |
|---|---:|---:|---:|---:|
| dirac_annihilation_multi_profile | 6 | 42.260 | 45.827 | 0.922 |
| dirac_mode_dense_sweep | 6 | 80.194 | 51.753 | 1.550 |
| dirac_annihilation_multi_profile | 7 | 46.950 | 48.353 | 0.971 |
| dirac_mode_dense_sweep | 7 | 184.916 | 65.175 | 2.837 |
| dirac_annihilation_multi_profile | 8 | 43.850 | 44.490 | 0.986 |
| dirac_mode_dense_sweep | 8 | 617.046 | 111.792 | 5.520 |

Speedup formula: S = sequential_wall_clock / rayon_wall_clock

## Run 2026-06-09T17:57:47.179372+00:00

Runs per policy: 2
Parallel workers detected (P): 28
n_qubits grid: 6, 7, 8

| workload | n_qubits | sequential_mean_ms | rayon_mean_ms | speedup_factor_s |
|---|---:|---:|---:|---:|
| dirac_annihilation_multi_profile | 6 | 58.266 | 53.880 | 1.081 |
| dirac_mode_dense_sweep | 6 | 81.698 | 49.758 | 1.642 |
| dirac_annihilation_multi_profile | 7 | 58.546 | 61.341 | 0.954 |
| dirac_mode_dense_sweep | 7 | 184.120 | 67.912 | 2.711 |
| dirac_annihilation_multi_profile | 8 | 46.925 | 52.584 | 0.892 |
| dirac_mode_dense_sweep | 8 | 611.990 | 118.544 | 5.163 |

Speedup formula: S = sequential_wall_clock / rayon_wall_clock

### Amdahl Scalability Analysis

Amdahl estimate: f = ((1/S) - (1/P)) / (1 - (1/P))

| workload | n_qubits | S | P | serial_fraction_raw_f | serial_fraction_clamped | parallelizable_fraction |
|---|---:|---:|---:|---:|---:|---:|
| dirac_annihilation_multi_profile | 6 | 1.081 | 28 | 0.921937 | 0.921937 | 0.078063 |
| dirac_mode_dense_sweep | 6 | 1.642 | 28 | 0.594568 | 0.594568 | 0.405432 |
| dirac_annihilation_multi_profile | 7 | 0.954 | 28 | 1.049508 | 1.000000 | 0.000000 |
| dirac_mode_dense_sweep | 7 | 2.711 | 28 | 0.345470 | 0.345470 | 0.654530 |
| dirac_annihilation_multi_profile | 8 | 0.892 | 28 | 1.125063 | 1.000000 | 0.000000 |
| dirac_mode_dense_sweep | 8 | 5.163 | 28 | 0.163840 | 0.163840 | 0.836160 |

## Run 2026-06-09T17:59:12.797411+00:00

Runs per policy: 2
Parallel workers detected (P): 28
Full-thread projection target (P_full): 28
n_qubits grid: 6, 7, 8

| workload | n_qubits | sequential_mean_ms | rayon_mean_ms | speedup_factor_s |
|---|---:|---:|---:|---:|
| dirac_annihilation_multi_profile | 6 | 43.831 | 49.165 | 0.892 |
| dirac_mode_dense_sweep | 6 | 84.502 | 53.944 | 1.566 |
| dirac_annihilation_multi_profile | 7 | 46.385 | 42.069 | 1.103 |
| dirac_mode_dense_sweep | 7 | 181.911 | 64.506 | 2.820 |
| dirac_annihilation_multi_profile | 8 | 50.892 | 38.191 | 1.333 |
| dirac_mode_dense_sweep | 8 | 601.759 | 98.347 | 6.119 |

Speedup formula: S = sequential_wall_clock / rayon_wall_clock

### Amdahl Scalability Analysis

Amdahl estimate: f = ((1/S) - (1/P)) / (1 - (1/P))

| workload | n_qubits | S | P | P_full | serial_fraction_raw_f | serial_fraction_clamped | parallelizable_fraction | predicted_speedup_at_full_threads |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| dirac_annihilation_multi_profile | 6 | 0.892 | 28 | 28 | 1.126202 | 1.000000 | 0.000000 | 1.000000 |
| dirac_mode_dense_sweep | 6 | 1.566 | 28 | 28 | 0.624982 | 0.624982 | 0.375018 | 1.566476 |
| dirac_annihilation_multi_profile | 7 | 1.103 | 28 | 28 | 0.903506 | 0.903506 | 0.096494 | 1.102594 |
| dirac_mode_dense_sweep | 7 | 2.820 | 28 | 28 | 0.330698 | 0.330698 | 0.669302 | 2.820066 |
| dirac_annihilation_multi_profile | 8 | 1.333 | 28 | 28 | 0.741189 | 0.741189 | 0.258811 | 1.332565 |
| dirac_mode_dense_sweep | 8 | 6.119 | 28 | 28 | 0.132449 | 0.132449 | 0.867551 | 6.118717 |

