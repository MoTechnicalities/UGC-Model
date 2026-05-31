# CSIF + RWIF v2 Implementation Quickstart

This is a practical one-page guide to implement and validate CSIF v2 + RWIF v2 in this repository.

Spec references:
- [CSIF_V2_ENGINE_SPEC.md](CSIF_V2_ENGINE_SPEC.md)
- [RWIF_V2_FIELD_SPEC.md](RWIF_V2_FIELD_SPEC.md)
- [CSIF_RWIF_V2_PROJECT_BLUEPRINT.md](CSIF_RWIF_V2_PROJECT_BLUEPRINT.md)

## 1. Where Things Live

- RWIF runtime and migration helpers: [storage/rwif.py](storage/rwif.py)
- RWIF migration CLI: [scripts/migrate_rwif_v2.py](scripts/migrate_rwif_v2.py)
- Demo runtime entrypoint: [demo_firewall.py](demo_firewall.py)
- Core graph mechanics: [engine/phase_graph.py](engine/phase_graph.py)
- Baseline tests: [tests/test_play_loop.py](tests/test_play_loop.py)

## 2. Environment Sanity

From repo root:

```bash
/bin/python3 -m py_compile storage/rwif.py scripts/migrate_rwif_v2.py
/bin/python3 -m unittest tests/test_play_loop.py
```

Expected:
- compile step prints no errors
- tests pass

## 3. Migrate Existing RWIF Data

Dry-run (recommended first):

```bash
/bin/python3 scripts/migrate_rwif_v2.py /path/to/bank.json --dry-run
```

Write to new file:

```bash
/bin/python3 scripts/migrate_rwif_v2.py /path/to/bank.json -o /path/to/bank.v2.json
```

In-place migration:

```bash
/bin/python3 scripts/migrate_rwif_v2.py /path/to/bank.json --in-place
```

What gets added (additive only):
- `rwif_schema_version`
- edge metadata (`state_encoding`, `numeric_range`, `wrap_mode`, `integration_rule`)
- event metadata (`amplitude_signed`, `intent_signed`, `phase_theta`, `phase_omega`, `quantization_step`, `monotonic_index`, `schema_version`)

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

Run same command twice and compare outputs bytewise:

```bash
/bin/python3 demo_firewall.py > /tmp/csif_run_a.log
/bin/python3 demo_firewall.py > /tmp/csif_run_b.log
diff -u /tmp/csif_run_a.log /tmp/csif_run_b.log
```

Expected:
- no meaningful decision drift (or document any allowed nondeterministic fields).

## 7. Release Gate (Minimal)

Before release/tag, require all:

1. RWIF migration dry-run reviewed and applied.
2. Tests passing.
3. Determinism check passing.
4. Contradiction trace path visible and auditable.
5. Docs updated:
- [CSIF_V2_ENGINE_SPEC.md](CSIF_V2_ENGINE_SPEC.md)
- [RWIF_V2_FIELD_SPEC.md](RWIF_V2_FIELD_SPEC.md)
- [CSIF_RWIF_V2_PROJECT_BLUEPRINT.md](CSIF_RWIF_V2_PROJECT_BLUEPRINT.md)

## 8. Suggested Next Automation

- Add CI job that runs:
  - py_compile
  - unit tests
  - RWIF migration smoke test
  - deterministic replay check
- Block merge if any gate fails.
