# Changelog

All notable changes to this project are documented in this file.

The format is inspired by Keep a Changelog and follows semantic, auditable release notes.

## [Unreleased]

### Added

- CI workflow for formatting, compile checks, and tests via `.github/workflows/ci.yml`.
- OSS hygiene documents: `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`.
- Release discipline policy at `docs/RELEASE_DISCIPLINE.md`.
- Technical reference extraction at `docs/TECHNICAL_REFERENCE.md` with root README converted to front-door format.

### Changed

- Spec links corrected across disclosure docs for local path validity.
- Implementation quickstart updated to reflect real Rust project paths and commands.
- Runtime OpenAI model id aligned with `ugc-model`.
- Determinism canonicalization strengthened for benchmark hash stability checks.

### Fixed

- Missing RWIF fixture set added under `tests/conformance/rwif_v2/fixtures`.
- `tests/cli_roundtrip.rs` fixture root resolution corrected for current repo layout.

## [2026-05-31]

### Added

- Full CSIF/RWIF/Semantic-layer specification disclosure package under `docs/specifications/`.
- Specification governance section in root `README.md`.
- Expanded Key Features coverage in root `README.md`.

### Notes

- Cross-repo specification sync tags should follow: `SPEC_SYNC_vYYYY.MM.DD.N`.
