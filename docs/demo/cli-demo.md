# UGC CLI Demo Log

Date: 2026-06-05

This log captures a compact command-line showcase of the UGC-Model runtime and its deterministic audit surfaces.

## 1. CLI Surface

Command:

```bash
cargo run -- help
```

Observed output highlights:

- `validate-bank`
- `migrate-bank`
- `index-bank`
- `layer0-check`
- `layer0-conformance`
- `math-eval`
- `serve-openai`
- `benchmark-determinism`

## 2. Exact Math Trace

Command:

```bash
cargo run --quiet -- math-eval 'exp(i*pi) + 1'
```

Observed result:

- Final value: `Real: 0.0`
- Deterministic bridge audit emitted
- Intermediate trace included:
  - `i * 3.14159265359`
  - `exp(3.14159265359i)` -> `-1.0`
  - `-1 + 1` -> `0.0`

## 3. Complex Equation Solver

Command:

```bash
cargo run --quiet -- math-eval '(2+3i)^2 + conj(4-5i) + arg(1+i) + 5!'
```

Observed result:

- Status: `QualifiedSuccess`
- Final value: `Complex(re=119.78539816339745, im=17.0)`
- Precision class: `ExactDeterministicComplex`
- Trace highlights:
  - `(2 + 3i)^2` -> `Complex(re=-5.0, im=12.0)`
  - `conj(4-5i)` -> `Complex(re=4.0, im=5.0)`
  - `arg(1+i)` -> `0.7853981633974483`
  - `factorial(5)` -> `120.0`

## 4. Language Capability Demo

Server bootstrap:

```bash
cargo run --quiet -- serve-openai --host 127.0.0.1 --port 8081 --bank-path /tmp/ugc-demo-bank-pcQq.json
```

Observed startup:

- `OpenAI-compatible server listening on http://127.0.0.1:8081`

English disambiguation:

```bash
curl -sS http://127.0.0.1:8081/v1/csif/disambiguate \
  -H 'Content-Type: application/json' \
  -d '{"language":"en","token":"light","context":"the light helped me see with my eyes","margin":0.75}'
```

Observed summary:

- Selected sense: `visible electromagnetic radiation`
- Status: `resolved`
- Lexicon coverage: `matched_token_count=1`, `coverage_ratio=0.125`
- Top candidate score: `4.733333333333333`
- Matched edges included `light` and `lightweight` from `csif_compact_lexicon_v1`

Spanish disambiguation:

```bash
curl -sS http://127.0.0.1:8081/v1/csif/disambiguate \
  -H 'Content-Type: application/json' \
  -d '{"language":"es","token":"luz","context":"la luz me ayuda a ver con mis ojos","margin":0.75}'
```

Observed summary:

- Selected sense: `visible electromagnetic radiation`
- Status: `resolved`
- Lexicon coverage: `matched_token_count=3`, `coverage_ratio=0.3333333333333333`
- Top candidate score: `5.533333333333333`
- Matched edges included `luz`, `ver`, and `ojos` from `csif_compact_lexicon_v1`

## 5. Determinism Scoreboard

Command:

```bash
cargo run --quiet -- benchmark-determinism --iterations 5
```

Observed summary:

- `math_eval`: deterministic hash stable, p50 `0.788193 ms`, p95 `1.02502 ms`
- `retrieve`: deterministic hash stable, p50 `0.024919 ms`, p95 `0.039668 ms`
- `disambiguate`: deterministic hash stable, p50 `0.392022 ms`, p95 `0.417828 ms`

## 6. What This Demonstrates

- The binary exposes a direct CLI surface for validation, math, indexing, serving, and benchmarking.
- The math engine returns audited structured output rather than a bare scalar.
- The benchmark runner reports deterministic stability across repeated runs.
- The observable outputs are reproducible and suitable for regression capture.

## 7. Quantum-Analog Probe (Deutsch Algorithm)

This section tests the exact idea raised in design discussion: can UGC-style deterministic phase evolution reproduce the canonical Deutsch classification result.

Command:

```bash
python3 scripts/eval-ugc-deutsch-probe.py --pretty
```

Observed summary:

- Object: `ugc.quantum_analog.deutsch_probe`
- Deterministic: `true`
- Cases: `4`
- Passed: `4`
- Failed: `0`

Per-oracle outcome:

- `f_zero` expected `constant`, measured first-qubit distribution `{0: 1.0, 1: 0.0}`, predicted `constant`
- `f_one` expected `constant`, measured first-qubit distribution `{0: 1.0, 1: 0.0}`, predicted `constant`
- `f_x` expected `balanced`, measured first-qubit distribution `{0: 0.0, 1: 1.0}`, predicted `balanced`
- `f_not_x` expected `balanced`, measured first-qubit distribution `{0: 0.0, 1: 1.0}`, predicted `balanced`

Notes:

- The probe emits per-basis amplitude, magnitude, and phase for `state_after_oracle` and `state_final`.
- This is a software quantum-analog test (deterministic phase-state computation), not a claim of physical qubit execution.

## 8. Next Challenge: Deutsch-Jozsa (n=3 and n=4)

This escalates the same quantum-analog idea to higher dimensionality while keeping deterministic auditability and concrete scaling metrics.

Command:

```bash
python3 scripts/eval-ugc-deutsch-jozsa-probe.py --n-values 3,4 --stability-runs 3 --pretty
```

Observed top-level result:

- Object: `ugc.quantum_analog.deutsch_jozsa_probe`
- Deterministic: `true`
- `n=3`: `oracle_count=8`, `passed=8`, `failed=0`, `stability_hash_stable=true`
- `n=4`: `oracle_count=16`, `passed=16`, `failed=0`, `stability_hash_stable=true`

Observed metrics (`n=3`):

- `total_runtime_ms=0.048615`
- `avg_runtime_ms=0.006077`
- `total_phase_operations=800`
- `estimated_peak_memory_bytes=384`
- `max_torsion_radians=3.14159265359`

Observed metrics (`n=4`):

- `total_runtime_ms=0.142398`
- `avg_runtime_ms=0.0089`
- `total_phase_operations=4224`
- `estimated_peak_memory_bytes=768`
- `max_torsion_radians=3.14159265359`

What this probe verifies:

- Correct constant-vs-balanced classification across all configured oracle cases for both sizes.
- Deterministic replay stability via per-batch SHA-256 output signatures.
- Explicit scaling visibility for runtime, phase operation count, estimated memory, and torsion envelope.

## 9. Grover Push Target (n=20)

This is the higher-impact search challenge: one marked item in a space of `2^20 = 1,048,576`.

Primary command (multiple deterministic marked-item trials, optimal iteration policy):

```bash
python3 scripts/eval-ugc-grover-probe.py \
  --n-bits 20 \
  --trials 3 \
  --seed 20260606 \
  --iteration-policy optimal \
  --trace-every 128 \
  --stability-runs 3 \
  --pretty
```

Observed top-level result:

- Object: `ugc.quantum_analog.grover_probe`
- Deterministic: `true`
- Hash stable: `true`
- `search_space=1048576`
- `grover_iterations=804` (matches `pi_over_4_sqrt_n_reference=804`)
- `sqrt_n_reference=1024`

Observed trial metrics (all 3 trials showed the same scaling profile):

- `result.correct=true`
- `success_probability=0.999999756965`
- `phase_operations_equivalent=843055908`
- `peak_memory_bytes_reduced_model=16`
- `peak_memory_bytes_full_state_equivalent=16777216`
- `max_torsion_radians=3.14159265359`
- `runtime_ms` approximately `0.10-0.12`

Baseline comparison from payload:

- `classical_avg_queries=524288`
- `classical_worst_queries=1048576`
- `grover_query_ratio_vs_avg=0.00153351`
- `grover_query_ratio_vs_worst=0.00076675`

Iteration trace evidence (sampled every 128 steps):

- `iteration=1` -> `marked_probability=0.000008583047`
- `iteration=128` -> `marked_probability=0.061677763881`
- `iteration=256` -> `marked_probability=0.230671177913`
- `iteration=384` -> `marked_probability=0.465605701287`
- `iteration=512` -> `marked_probability=0.70896115115`
- `iteration=640` -> `marked_probability=0.901155607513`
- `iteration=768` -> `marked_probability=0.995133149796`
- `iteration=804` -> `marked_probability=0.999999756965`

Reference command (forced `sqrt(N)` iterations):

```bash
python3 scripts/eval-ugc-grover-probe.py \
  --n-bits 20 \
  --marked-item 263723 \
  --iteration-policy sqrt \
  --trace-every 1024 \
  --stability-runs 1 \
  --pretty
```

Observed `sqrt(N)` reference:

- `grover_iterations=1024`
- `success_probability=0.826081881498`

This demonstrates why the `pi/4 * sqrt(N)` policy is the practical optimum for single-marked-item Grover amplification.

## 10. Simon Challenge (n=8, Hidden Period Detection)

This extends the suite to hidden-period discovery, the direct conceptual precursor to Shor-style structure finding.

Command:

```bash
python3 scripts/eval-ugc-simon-probe.py \
  --n-bits 8 \
  --secrets 10101101,01011011,11100010 \
  --stability-runs 3 \
  --pretty
```

Observed top-level result:

- Object: `ugc.quantum_analog.simon_probe`
- Deterministic: `true`
- Hash stable: `true`
- `trials=3`, `passed=3`, `failed=0`

Recovered hidden periods:

- Secret `10101101` -> recovered `10101101` (match)
- Secret `01011011` -> recovered `01011011` (match)
- Secret `11100010` -> recovered `11100010` (match)

Per-trial scaling profile:

- `measurements_used=7` (n-1 independent equations)
- `classical_worst_samples=256`
- `query_ratio_vs_classical_worst=0.02734375`
- `phase_operations_equivalent=1792`
- `peak_memory_bytes=120`
- `max_torsion_radians=3.14159265359`
- `runtime_ms` approximately `0.02-0.05`

Equation-trace evidence (example trial):

- `equations=[00000010, 00000101, 00001001, 00010000, 00100001, 01000000, 10000001]`
- Each measured equation row satisfies `y·s = 0 mod 2`
- The recovered non-zero nullspace vector equals the hidden period `s`

This adds hidden-structure recovery to the demonstrated suite, beyond classification and search amplification.

## 11. One-Command Publish Runner

All four probes are now packaged behind one top-level runner command that emits a consolidated JSON and Markdown report.

Command:

```bash
python3 scripts/run-ugc-quantum-suite.py --pretty
```

Generated artifacts:

- `docs/demo/quantum-suite-report.json`
- `docs/demo/quantum-suite-report.md`

Observed suite summary:

- `all_pass=true`
- `deterministic_hash_stable=true`
- `total_probe_groups=4`
- `total_trials=34`
- `total_passed=34`
- `total_failed=0`

Included probe groups:

- Deutsch
- Deutsch-Jozsa (`n=3,4`)
- Grover (`n=20`, optimal + sqrt reference)
- Simon (`n=8`)

This provides a single publish-ready entry point for the complete quantum-analog demonstration suite.
