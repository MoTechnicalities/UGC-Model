# CSIF v2 + RWIF v2 Project Blueprint

This document defines a practical, end-to-end blueprint for building an accomplished project using CSIF v2 (engine/runtime contract) and RWIF v2 (storage/audit contract) together.

Related specs:
- [CSIF_V2_ENGINE_SPEC.md](CSIF_V2_ENGINE_SPEC.md)
- [RWIF_V2_FIELD_SPEC.md](RWIF_V2_FIELD_SPEC.md)

## 1. Target Outcome

An accomplished CSIF/RWIF project is one that is:
- deterministic in reasoning outputs
- append-only and auditable in persistence
- replayable from stored events
- measurable in latency, throughput, and contradiction quality
- robust to contradictory or adversarial inputs

## 2. Project Architecture

### 2.1 Runtime Layer (CSIF v2)

Responsibilities:
- path composition and route selection
- coherence/contradiction calculations
- stop-reason emission
- deterministic decisioning

Key contracts:
- principal phase wrapping
- signed state tuple support (amplitude + intent)
- explicit integration rule selection

### 2.2 Persistence Layer (RWIF v2)

Responsibilities:
- append-only event persistence
- edge/node/crystal/bank serialization
- versioned schema markers
- migration compatibility from v1

Key contracts:
- event schema versioning
- edge metadata for replay
- bank/crystal schema tagging

### 2.3 Boundary Contract

Engine output MUST map directly to persisted trajectory events:
- timestamp
- phase
- confidence_band
- drift_delta
- event_type
- source
- optional amplitude_signed
- optional intent_signed
- optional phase_theta/phase_omega
- optional monotonic_index

## 3. Recommended Build Plan

### Phase A: Foundation

1. Lock engine mode:
- `phase_scalar_v1` for baseline compatibility
- `signed_i8_plus_intent_v2` for upgraded signed-state behavior

2. Lock persistence markers:
- `rwif_schema_version = RWIF_V2`
- `schema_version = RWIF_EDGE_V2` and `RWIF_EVENT_V2`

3. Define deterministic settings:
- numeric range
- quantization step
- wrap mode
- integration rule

Deliverable:
- stable configuration manifest committed in repo.

### Phase B: Migration and Data Prep

1. Migrate existing RWIF v1 artifacts to v2 additive shape.
2. Preserve all existing values; add only optional v2 metadata.
3. Keep originals for rollback and audit.

Deliverable:
- migrated bank(s) plus migration report (changed vs unchanged).

### Phase C: Engine Integration

1. Read RWIF v2 metadata into runtime config at startup.
2. Enforce deterministic composition with fixed integration rule.
3. Emit route audits with explicit stop reasons.

Deliverable:
- runtime produces deterministic, auditable results under fixed input.

### Phase D: Validation and Qualification

1. Determinism tests:
- same input/config must produce byte-stable decisions and traces.

2. Contradiction tests:
- known contradictory paths must cross threshold and emit rejection trace.

3. Replay tests:
- replay persisted events; compare route outputs for equality.

4. Performance tests:
- p50/p95 query latency
- max throughput under fixed benchmark corpus

Deliverable:
- qualification summary with PASS/FAIL gates.

### Phase E: Release

1. Publish versioned specs and migration notes.
2. Tag engine + schema version in release docs.
3. Keep rollback and compatibility guidance explicit.

Deliverable:
- release artifact that is reproducible and auditable.

## 4. Accomplished Project Checklist

A project is considered accomplished when all are true:

1. Spec alignment:
- implementation matches [CSIF_V2_ENGINE_SPEC.md](CSIF_V2_ENGINE_SPEC.md)
- persistence matches [RWIF_V2_FIELD_SPEC.md](RWIF_V2_FIELD_SPEC.md)

2. Compatibility:
- v1 RWIF loads without data loss
- v2 fields are additive and optional for readers

3. Determinism:
- repeated runs produce byte-identical decisions and route traces

4. Auditability:
- every acceptance/rejection has provenance and stop reason

5. Replayability:
- event stream replay reproduces same outcomes under declared config

6. Performance:
- meets project-defined latency and throughput budgets

7. Safety:
- contradiction handling rejects unstable writes
- negative/anti evidence behavior is explicit and test-covered

## 5. Minimal Operational Workflow

1. Ingest or migrate bank to RWIF v2.
2. Start engine in declared CSIF v2 mode.
3. Run teach/query workloads.
4. Persist all updates as append-only events.
5. Execute qualification suite:
- determinism
- contradiction
- replay
- performance
6. Produce release report and tag versions.

## 6. Example Success Criteria (Template)

- Determinism drift: 0 mismatches over N replay runs.
- Contradiction precision: >= target threshold on known adversarial set.
- p95 query latency: <= target ms.
- Migration integrity: 100% records preserved, 0 dropped fields.
- Audit completeness: 100% decisions include stop reason and provenance.

## 7. Documentation Package for Completion

For a complete handoff, keep these documents together:
- [CSIF_V2_ENGINE_SPEC.md](CSIF_V2_ENGINE_SPEC.md)
- [RWIF_V2_FIELD_SPEC.md](RWIF_V2_FIELD_SPEC.md)
- this blueprint
- migration report
- qualification report
- release notes

## 8. Practical Notes

- Keep configuration immutable during benchmark runs.
- Prefer additive migrations; avoid in-place mutation without backups.
- Use one canonical benchmark corpus for parity checks.
- Separate startup warm-up from qualification runs.

## 9. Next Technical Upgrade Path

After baseline completion:
- add executable conformance tests mapped 1:1 to spec sections
- add signed-intent benchmark profile vs legacy scalar profile
- add CI gate requiring deterministic replay PASS before release
