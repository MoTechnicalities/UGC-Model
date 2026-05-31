# Contributing to UGC-Model

Thanks for contributing to UGC-Model. This repository is deterministic-by-design and specification-driven. Please follow this workflow to keep implementation and disclosure aligned.

## Ground Rules

- Keep changes deterministic and auditable.
- Keep specs and runtime behavior aligned.
- Prefer additive, backward-compatible schema changes.
- Do not rewrite history in append-only reasoning artifacts.

## Development Setup

From repository root:

```bash
cargo check --locked
cargo test --locked
```

## Pull Request Workflow

1. Create a focused branch.
2. Make the smallest coherent change set possible.
3. Update docs/specs when behavior changes.
4. Run:

```bash
cargo fmt --all -- --check
cargo check --locked
cargo test --locked
```

5. Open a PR with:
- Problem statement
- Change summary
- Validation evidence (commands + results)
- Any spec/disclosure updates

## Spec Changes

UGC-Model is the canonical source for CSIF/RWIF/Semantic layer specs.

If you modify specification files, include:

- Updated links/indexes where relevant
- Changelog entry under `CHANGELOG.md`
- Sync metadata if mirrored cross-repo (see `docs/RELEASE_DISCIPLINE.md`)

## Commit Conventions

Use clear, scoped commit messages such as:

- `docs: ...`
- `feat: ...`
- `fix: ...`
- `test: ...`
- `ci: ...`

For cross-repo specification sync operations, include a sync tag in release/changelog notes:

- `SPEC_SYNC_vYYYY.MM.DD.N`

## Reporting Issues

For bugs, include:

- Exact command or API call
- Expected vs actual behavior
- Minimal reproduction input
- Environment details

For security issues, use the process in `SECURITY.md`.
