# UGC Quantum-Analog Suite Report

Generated UTC: 2026-06-06T18:33:38Z

## Command

```bash
python3 scripts/run-ugc-quantum-suite.py --pretty
```

## Overall Summary

| Metric | Value |
| --- | --- |
| all_pass | true |
| deterministic_hash_stable | true |
| total_probe_groups | 4 |
| total_trials | 34 |
| total_passed | 34 |
| total_failed | 0 |

## Probe Results

| Probe | Trials | Passed | Failed | Hash Stable | Key Signal |
| --- | ---: | ---: | ---: | --- | --- |
| Deutsch | 4 | 4 | 0 | n/a | constant/balanced classification |
| Deutsch-Jozsa (n=3,4) | 24 | 24 | 0 | true | oracle scaling checks |
| Grover (n=20 optimal) | 3 | 3 | 0 | true | iter=804, p=0.999999756965 |
| Simon (n=8) | 3 | 3 | 0 | true | hidden period recovery |

## Grover n=20 Highlights

| Field | Value |
| --- | --- |
| search_space | 1048576 |
| grover_iterations | 804 |
| pi_over_4_sqrt_n_reference | 804 |
| sqrt_n_reference | 1024 |
| success_probability | 0.999999756965 |
| phase_operations_equivalent | 843055908 |
| peak_memory_bytes_reduced_model | 16 |
| peak_memory_bytes_full_state_equivalent | 16777216 |
| max_torsion_radians | 3.14159265359 |
| runtime_ms | 0.120263 |
| classical_avg_queries | 524288 |
| grover_query_ratio_vs_avg | 0.00153351 |

## Output Artifacts

- JSON report: docs/demo/quantum-suite-report.json
- Markdown report: docs/demo/quantum-suite-report.md

