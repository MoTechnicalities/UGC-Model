# CSIF + RWIF v2 Implementation Quickstart

This is a practical one-page guide to implement and validate CSIF v2 + RWIF v2 in this repository.

Spec references:
- [CSIF_V2_ENGINE_SPEC.md](../csif/CSIF_V2_ENGINE_SPEC.md)
- [RWIF_V2_FIELD_SPEC.md](../rwif/RWIF_V2_FIELD_SPEC.md)
- [CSIF_RWIF_V2_PROJECT_BLUEPRINT.md](CSIF_RWIF_V2_PROJECT_BLUEPRINT.md)

## 1. Where Things Live

- Rust runtime and CLI entrypoint: [src/main.rs](../../../src/main.rs)
- Cargo project manifest: [Cargo.toml](../../../Cargo.toml)
- CLI conformance tests: [tests/cli_roundtrip.rs](../../../tests/cli_roundtrip.rs)
- Layer 0 CLI tests: [tests/layer0_cli.rs](../../../tests/layer0_cli.rs)
- Conformance fixtures: [tests/conformance](../../../tests/conformance)

## 2. Environment Sanity

From repo root:

```bash
cargo check --locked
cargo test --locked
```

Expected:
- project compiles
- tests pass

## 3. Migrate Existing RWIF Data

Write to new file:

```bash
cargo run -- migrate-bank /path/to/input.json /path/to/output.json
```

What gets added (additive only):
- `rwif_schema_version`
- edge metadata (`state_encoding`, `numeric_range`, `wrap_mode`, `integration_rule`)
- event metadata (`amplitude_signed`, `intent_signed`, `phase_theta`, `phase_omega`, `quantization_step`, `monotonic_index`, `schema_version`)

Validate migrated output:

```bash
cargo run -- validate-bank /path/to/output.json
```

## 4. Engine Mode Selection

Current baseline compatibility mode:
- `phase_scalar_v1` behavior remains valid.

Target v2 mode for signed-state projects:
- `signed_i8_plus_intent_v2` semantics in event payloads and edge metadata.

Implementation note:
- Keep one canonical amplitude zero.
- Preserve directional continuity using `intent_signed`.

## 5. Minimum Integration Loop

1. Load or migrate RWIF bank/crystal JSON.
2. Run query/update flow through CSIF runtime.
3. Persist trajectory events append-only.
4. Ensure every decision emits stop reason + provenance.
5. Re-run deterministic checks against same input/config.

## 6. Quick Determinism Check

Run deterministic benchmark profile:

```bash
cargo run -- benchmark-determinism --iterations 100
```

Expected:
- benchmark report indicates stable deterministic replay behavior.

## 7. Release Gate (Minimal)

Before release/tag, require all:

1. RWIF migration dry-run reviewed and applied.
2. Tests passing.
3. Determinism check passing.
4. Contradiction trace path visible and auditable.
5. Docs updated:
- [CSIF_V2_ENGINE_SPEC.md](../csif/CSIF_V2_ENGINE_SPEC.md)
- [RWIF_V2_FIELD_SPEC.md](../rwif/RWIF_V2_FIELD_SPEC.md)
- [CSIF_RWIF_V2_PROJECT_BLUEPRINT.md](CSIF_RWIF_V2_PROJECT_BLUEPRINT.md)

## 8. Suggested Next Automation

- Add CI job that runs:
  - cargo check --locked
  - unit tests
  - RWIF migration smoke test
  - deterministic replay check
- Block merge if any gate fails.
