# UGC Grover Scaling Comparison Report (Python vs Rust)

Generated UTC: 2026-06-06T22:59:03.264570+00:00

## Command

```bash
scripts/run-ugc-grover-scaling-rust.py --n-min 10 --n-max 50 --step 5 --trials 3 --rust-runner binary --rust-binary target/debug/csif_agent_v2_rust --pretty
```

## Summary

| Metric | Value |
| --- | --- |
| n_values_total | 9 |
| n_values_executed | 9 |
| n_values_skipped | 0 |
| ops_per_second_estimate | 17922668.64 |
| python_loglog_slope | 0.216737 |
| python_loglog_r2 | 0.806072 |
| rust_loglog_slope | 0.204773 |
| rust_loglog_r2 | 0.815429 |

## Per-n Rows

| n | status | search_space | grover_iter | python_wall_ms | rust_wall_ms | python_vs_rust_ratio | bruteforce_est_ms |
| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 10 | executed | 1024 | 25 | 19.72401 | 2.281876 | 8.643769 | 0.057134 |
| 15 | executed | 32768 | 142 | 21.308783 | 3.241313 | 6.574121 | 1.828299 |
| 20 | executed | 1048576 | 804 | 23.207542 | 3.399238 | 6.827278 | 58.505573 |
| 25 | executed | 33554432 | 4549 | 25.646479 | 3.81965 | 6.714353 | 1872.178338 |
| 30 | executed | 1073741824 | 25735 | 30.490447 | 4.803052 | 6.34814 | 59909.706825 |
| 35 | executed | 34359738368 | 145584 | 78.769931 | 9.690201 | 8.128823 | 1917110.6184 |
| 40 | executed | 1099511627776 | 823549 | 356.370255 | 36.777693 | 9.689848 | 61347539.7888 |
| 45 | executed | 35184372088832 | 4658700 | 1857.096375 | 191.649942 | 9.690044 | 1963121273.2416 |
| 50 | executed | 1125899906842624 | 26353589 | 10426.428684 | 1082.65255 | 9.630448 | 62819880743.73121 |

## Artifacts

- JSON report: docs/demo/grover-scaling-rust-compare.json
- Markdown report: docs/demo/grover-scaling-rust-compare.md
- CSV export: docs/demo/grover-scaling-rust-compare.csv
- Log-log SVG plot: docs/demo/grover-scaling-rust-compare-loglog.svg

