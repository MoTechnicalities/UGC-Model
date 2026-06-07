# UGC Grover Scaling Boundary Report

Generated UTC: 2026-06-06T22:58:17.179022+00:00

## Command

```bash
scripts/run-ugc-grover-scaling.py --n-min 10 --n-max 50 --step 5 --trials 3 --pretty
```

## Summary

| Field | Value |
| --- | --- |
| probe_n_limit | 50 |
| n_values_total | 9 |
| n_values_executed | 9 |
| n_values_skipped | 0 |
| ops_per_second_estimate | 18094849.86 |
| runtime_log2_slope_per_bit | 0.219721 |
| runtime_scaling_interpretation | between flat and sqrt-like exponential (close to O(2^(n/2))) |

## Per-n Results

| n | status | search_space | grover_iter | success_p | ugc_wall_ms | bruteforce_est_ms | query_ratio_vs_worst |
| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 10 | executed | 1024 | 25 | 0.999461244744 | 19.487173 | 0.056591 | 0.02441406 |
| 15 | executed | 32768 | 142 | 0.999986829519 | 19.525721 | 1.810902 | 0.0043335 |
| 20 | executed | 1048576 | 804 | 0.999999756965 | 19.237114 | 57.948864 | 0.00076675 |
| 25 | executed | 33554432 | 4549 | 0.999999999983 | 24.036289 | 1854.363659 | 0.00013557 |
| 30 | executed | 1073741824 | 25735 | 0.999999999321 | 28.998219 | 59339.6371 | 2.397e-05 |
| 35 | executed | 34359738368 | 145584 | 0.999999999999 | 76.839232 | 1898868.3872 | 4.24e-06 |
| 40 | executed | 1099511627776 | 823549 | 1.0 | 343.246477 | 60763788.3904 | 7.5e-07 |
| 45 | executed | 35184372088832 | 4658700 | 1.0 | 1851.637231 | 1944441228.4928 | 1.3e-07 |
| 50 | executed | 1125899906842624 | 26353589 | 1.0 | 10370.806551 | 62222119311.76961 | 2e-08 |

## Notes

- `status=executed` rows are measured with the current Grover probe implementation.
- `status=skipped_probe_limit` rows exceed the probe's current hard limit and are included for boundary bookkeeping.
- Brute-force times are coarse estimates from local integer-op calibration, not full memory-bound scans.

