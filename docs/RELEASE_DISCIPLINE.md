# Release Discipline and Spec Sync Process

This document defines how UGC-Model changes are released and how canonical specifications are synchronized to downstream mirrors.

## Canonical Rule

- UGC-Model is the canonical source for CSIF, RWIF, and Semantic layer specs.
- Downstream repositories may mirror specs, but canonical edits start here.

## Versioning and Sync Tag

When a specification sync is propagated cross-repo, include:

- Sync tag format: `SPEC_SYNC_vYYYY.MM.DD.N`
- Example: `SPEC_SYNC_v2026.05.31.1`

## Required Artifacts per Release/Sync

Each release or sync must include:

1. Changelog entry in `CHANGELOG.md`
2. File list of synced documents
3. Source commit hash (UGC-Model)
4. Destination commit hash (mirror repository)
5. Effective sync tag

## Release Checklist

1. Validate code and tests

```bash
cargo fmt --all -- --check
cargo check --locked
cargo test --locked
```

2. Confirm docs/spec links are valid and navigable.
3. Update `CHANGELOG.md`.
4. Tag and publish release notes.
5. If cross-repo sync applies, include `SPEC_SYNC_*` metadata in release notes and mirrored commit messages.

## Suggested Release Note Template

- Summary
- Validation evidence
- Specs changed
- API/contract changes
- Sync metadata:
  - `sync_tag`
  - `source_commit`
  - `destination_commit`
  - `synced_files`
