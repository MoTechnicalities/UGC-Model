# Changelog

All notable changes to this project are documented in this file.

The format is inspired by Keep a Changelog and follows semantic, auditable release notes.

## [as-if-real-quantum-mode-v1] - 2026-06-08

### Added

- Bernstein-Vazirani now ships in two explicit modes: a structural mode for deterministic oracle-inspection verification, and a black-box mode for measurement-only recovery under finite-shot constraints.
- The black-box BV path includes a reality calibration layer that reports per-bit confidence, whole-string confidence, and estimated shot budgets for a target confidence level.

### Notes

- This release is intended for experimental design and replayable auditability rather than claims of physical qubit execution or quantum speedup.

## [Unreleased]

### Added

- CI workflow for formatting, compile checks, and tests via `.github/workflows/ci.yml`.
- OSS hygiene documents: `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`.
- Release discipline policy at `docs/RELEASE_DISCIPLINE.md`.
- Technical reference extraction at `docs/TECHNICAL_REFERENCE.md` with root README converted to front-door format.
- Dual Bernstein-Vazirani modes in `src/quantum/register.rs`: structural oracle-reading mode and black-box measurement-only mode.
- Shot-based black-box BV recovery with seeded probabilistic measurement summaries and replay-stable envelopes.
- Reality calibration output for black-box BV: per-bit confidence, whole-string confidence, and minimum shots for a target confidence level.
- Root `.gitignore` to stop Rust `target/` artifacts from being tracked in future commits.

### Changed

- Spec links corrected across disclosure docs for local path validity.
- Implementation quickstart updated to reflect real Rust project paths and commands.
- Runtime OpenAI model id aligned with `ugc-model`.
- Determinism canonicalization strengthened for benchmark hash stability checks.
- Root README updated with an "as-if-real quantum mode" summary covering structural BV, black-box BV, and the calibration layer.
- Bell-state command ergonomics extended to include compact sweep export mode with `--sweep-export json|csv`, `--sweep-noise-factors`, and `--sweep-seeds`.
- BV default shot handling is now centralized via `DEFAULT_BV_BLACK_BOX_SHOTS` and shared across parser, CLI dispatch, and runtime wrapper paths.
- CLI black-box BV dispatch now routes through the default-shot wrapper when default shots are requested, keeping the default runtime path exercised.

### Validation

- Full locked Rust test suite passes after warning cleanup and Bell sweep integration updates:
	- `cargo test --locked --quiet` -> `189 passed; 0 failed; 2 ignored` (unit)
	- `tests/cli_roundtrip.rs` -> `3 passed; 0 failed`
	- `tests/layer0_cli.rs` -> `3 passed; 0 failed`
	- `tests/quantum_register_cli.rs` -> `9 passed; 0 failed`
- Bell sweep export stability/contract checks pass:
	- `cargo test --locked bell_state_sweep_export_json_and_csv_are_compact_and_consistent`
	- `cargo test --locked bell_state_cli_sweep_export_csv_emits_header_and_rows`
- Compile verification after cleanup:
	- `cargo check --locked` completes with zero warnings in current workspace state.

### Fixed

- Missing RWIF fixture set added under `tests/conformance/rwif_v2/fixtures`.
- `tests/cli_roundtrip.rs` fixture root resolution corrected for current repo layout.
- Dead-code warning noise in the quantum register/BV path reduced by wiring default-path usage and marking retained experimental helpers as intentionally non-export runtime utilities.

## [2026-05-31]

### Added

- Full CSIF/RWIF/Semantic-layer specification disclosure package under `docs/specifications/`.
- Specification governance section in root `README.md`.
- Expanded Key Features coverage in root `README.md`.

### Notes

- Cross-repo specification sync tags should follow: `SPEC_SYNC_vYYYY.MM.DD.N`.
