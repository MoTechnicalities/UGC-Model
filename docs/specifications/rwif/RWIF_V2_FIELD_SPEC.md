# RWIF v2 Field Specification

This document formalizes a backward-compatible RWIF v2 extension for UGC-Model.

Goals:
- Preserve RWIF v1 compatibility.
- Add balanced signed state support for dipole-style manifolds.
- Preserve deterministic replay and append-only auditability.
- Keep CPU-friendly encoding for current runtime constraints.

## 1. Compatibility Contract

RWIF v2 is additive:
- All RWIF v1 fields remain valid.
- New fields are optional for readers.
- Writers SHOULD emit `rwif_schema_version`.

Version markers:
- Bank/crystal: `rwif_schema_version = "RWIF_V2"`
- Edge: `schema_version = "RWIF_EDGE_V2"`
- Trajectory event: `schema_version = "RWIF_EVENT_V2"`

## 2. Core v2 Model

### 2.1 Event-Level State Tuple

Trajectory events MAY carry a two-component state tuple:
- `amplitude_signed` (integer or null)
- `intent_signed` (integer or null)

Semantics:
- `amplitude_signed`: signed displacement/polarity.
- `intent_signed`: signed direction/propagation memory.

This keeps one physical/equilibrium zero in amplitude space while preserving directional memory through origin.

### 2.2 Optional Phase Companion Fields

To support dual-phase/complex-plane representations without forcing one solver:
- `phase_theta` (float or null): angle-like phase coordinate.
- `phase_omega` (float or null): angular velocity / phase rate.

### 2.3 Deterministic Replay Fields

To make event replay deterministic across engines:
- `state_encoding` (string)
- `quantization_step` (number)
- `monotonic_index` (integer or null)

## 3. Edge-Level v2 Metadata

Each edge MAY define update and numeric domain metadata:
- `state_encoding` (string)
- `numeric_range` (object)
- `wrap_mode` (string)
- `integer_wrap_mode` (string)
- `integration_rule` (string)
- `schema_version` (string)

Canonical defaults for CPU-first implementations:
- `state_encoding = "phase_scalar_v1"` (legacy behavior)
- `numeric_range = {"amplitude":{"min":-127,"max":127},"intent":{"min":-127,"max":127}}`
- `wrap_mode = "principal_pi"`
- `integer_wrap_mode = "clamp"`
- `integration_rule = "legacy_scalar"`

`wrap_mode` applies to angular/phase-domain wrapping (for example principal interval behavior).

`integer_wrap_mode` applies to signed integer state domains (`amplitude_signed`, `intent_signed`) and MUST be explicit for deterministic engine behavior under collision-heavy updates.

Allowed values:
- `clamp`: saturate to domain boundary (for example 128 -> 127).
- `overflow_modulo`: wrap through signed domain modulo range (toroidal integer topology).

## 4. Bank/Crystal Versioning

Top-level containers SHOULD include:
- `rwif_schema_version = "RWIF_V2"`

This allows migration tools to be explicit and idempotent.

## 5. Migration Behavior

v1 -> v2 migration is non-destructive:
- Preserve all v1 values exactly.
- Add missing v2 metadata with defaults.
- Keep unknown fields untouched.

Recommended default migration mappings:
- Event `phase_theta <- phase`
- Event `amplitude_signed <- null`
- Event `intent_signed <- null`
- Event `state_encoding <- "signed_i8_plus_intent_v2"`
- Event `quantization_step <- 1`

## 6. Example Event (v2)

```json
{
  "timestamp": "2026-05-27T12:00:00Z",
  "phase": 0.5236,
  "confidence_band": 0.04,
  "drift_delta": 0.0036,
  "event_type": "crystallization",
  "source": {"type":"consensus_gate"},
  "schema_version": "RWIF_EVENT_V2",
  "amplitude_signed": 64,
  "intent_signed": 1,
  "phase_theta": 0.5236,
  "phase_omega": 0.01,
  "state_encoding": "signed_i8_plus_intent_v2",
  "quantization_step": 1,
  "monotonic_index": 412
}
```

## 7. Example Edge (v2)

```json
{
  "edge_id": "...",
  "source_node": "...",
  "relation": "is_a",
  "target_node": "...",
  "lobe": "English",
  "reinforcing": true,
  "base_phase": 0.5236,
  "confidence_band": 0.04,
  "phase_trajectory": [],
  "provenance": {},
  "schema_version": "RWIF_EDGE_V2",
  "state_encoding": "phase_scalar_v1",
  "numeric_range": {
    "amplitude": {"min": -127, "max": 127},
    "intent": {"min": -127, "max": 127}
  },
  "wrap_mode": "principal_pi",
  "integer_wrap_mode": "clamp",
  "integration_rule": "legacy_scalar"
}
```

## 8. Performance/Determinism Guidance

For current CPU-native implementations:
- Prefer signed i8 domains for hot-path operations.
- Keep quantization explicit and fixed.
- Declare integer overflow behavior explicitly via `integer_wrap_mode`.
- Keep migration additive to avoid re-encoding existing banks.
- Use append-only trajectory updates; never rewrite prior events.

## 9. Security and Audit Invariants

Must remain true:
- Append-only trajectory semantics.
- Reproducible replay with same input and same update rule.
- Provenance retention for every event and edge update.

## 10. Unit Crystal Export Profile (CSIF Upgrade)

RWIF exports from CSIF math-eval now include explicit unit representation
objects and unit projection edges.

### 10.1 Unit Crystal Node

Writers SHOULD emit a node with:

- `node_type = "unit_crystal"`
- stable `node_id` (for example `unit.decimal.geometric`)
- representation provenance

### 10.2 Unit Projection Edge

Writers SHOULD emit a relation edge from numeric result node to unit crystal:

- `relation = "represented_in_unit"`

This edge follows standard RWIF v2 edge metadata conventions (`schema_version`,
`state_encoding`, `wrap_mode`, and deterministic provenance).

### 10.3 Conversion Morphism Chain Fields

Unit crystal payloads MAY include deterministic conversion chain records:

- `conversion_morphisms[*].source_unit`
- `conversion_morphisms[*].target_unit`
- `conversion_morphisms[*].hops[*].phase_drift`
- `conversion_morphisms[*].loop_metrics.loop_torsion_norm`
- `conversion_morphisms[*].loop_metrics.loop_resonance`
- `conversion_morphisms[*].loop_metrics.exceeds_threshold`

### 10.4 Contradiction Signal Mapping

When loop threshold policy is exceeded, writers SHOULD expose contradiction
context in exported unit payloads:

- contradiction code: `unit_conversion_loop_torsion_exceeded`
- contradiction stop reason: `unit_conversion_loop_torsion_exceeded`

Downstream CSIF bridge audits SHOULD propagate these into
`consistency.contradictions` for unified contradiction accounting.
