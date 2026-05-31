# Semantic Layer 2 Specification v0.4

This document defines Layer 2 canonical meaning and equivalence behavior on top of Semantic Layer 0 and Semantic Layer 1.

Layer 2 introduces deterministic canonical meaning signatures, policy-driven equivalence checks, and translation contracts while preserving conformance-first rollout.

## 1. Purpose

Layer 2 extends Layer 1 with:
- canonical meaning units derived from Layer 0 structure plus Layer 1 overlays
- deterministic meaning signatures for replay-safe comparison
- policy-driven equivalence classes (strict, relaxed, domain-specific)
- translation contracts with proof-grade audit traces

Layer 2 MUST NOT redefine Layer 0 or Layer 1 invariants.

## 2. Dependencies

Layer 2 requires:
- Layer 0 compliant graph inputs (C-001..C-013 pass)
- Layer 1 compliant overlays and thresholds (L1-C-014..L1-C-017 pass)
- deterministic replay configuration for all upstream layers

Every Layer 2 artifact MUST include references to:
- layer0_version
- layer1_version
- layer1_config
- layer2_policy

## 3. Normative Terms

The keywords MUST, SHOULD, and MAY are normative.

## 4. Layer 2 Core Objects

### 4.1 Meaning Unit

A Meaning Unit is the canonicalized composition of:
- a normalized Layer 0 subgraph (nodes + edges)
- attached Layer 1 modality/negation/uncertainty overlays
- provenance bindings required for auditability

Required fields:
- unit_id
- layer0_subgraph
- layer1_overlays
- provenance

### 4.1.1 Meaning Unit Normalization Procedure

Implementations MUST apply the following normalization procedure before signature generation:

1. Validate required structure:
  - `layer0_subgraph.nodes` and `layer0_subgraph.edges` present
  - `layer1_overlays` present
  - `provenance` present when `require_provenance=true`
2. Sort graph elements deterministically:
  - nodes by `node_id`
  - edges by `edge_id`
  - overlays by `(relation, source, target, assertion_id)`
3. Normalize numeric values:
  - round all floating-point values to `float_precision`
  - preserve integer values as integers
4. Normalize object encoding:
  - UTF-8 JSON
  - sorted object keys
  - compact separators (`,`, `:`)
5. Provenance handling:
  - provenance MUST be validated for audit compliance
  - provenance MAY be excluded from signature payload to avoid source-format drift, but this inclusion/exclusion choice MUST be fixed by policy and replay-stable

Normalization outputs MUST be byte-identical for byte-identical semantic inputs under identical policy context.

### 4.1.2 Provenance Canonical Form

Layer 2 MUST declare provenance canonicalization behavior in policy via `provenance_canonical_mode`:
- `include`: provenance fields are included in canonical payload before hashing
- `exclude`: provenance fields are excluded from canonical payload, but still required for audit validation when `require_provenance=true`
- `separate_hash`: provenance is excluded from primary meaning signature and hashed separately as `provenance_signature`

If `provenance_canonical_mode` is `separate_hash`, implementations MUST also declare `provenance_hash_algorithm` and emit the provenance digest in audit output.

### 4.2 Meaning Signature

A Meaning Signature is the deterministic fingerprint of a Meaning Unit under a declared Layer 2 policy.

Required signature algorithm:
1. Normalize Meaning Unit to canonical JSON with:
   - sorted keys
   - deterministic list ordering
   - normalized numeric precision
2. Concatenate policy context:
   - policy_id
   - equivalence_mode
   - float_precision
3. Serialize canonical payload using UTF-8.
4. Compute SHA-256 digest over canonical payload bytes.
5. Emit lowercase hexadecimal digest string.

Normative algorithm string:

`meaning_signature = SHA256(UTF8(canonical_json(policy_context + normalized_meaning_unit)))`

Implementations MUST use the same canonicalization and hashing steps for replay compatibility.

### 4.2.1 Canonical JSON Encoding Profile (Normative)

Canonical JSON used for signatures MUST follow this profile:
- encoding: UTF-8
- object keys: lexicographically sorted by Unicode code point
- separators: comma and colon only (no extra whitespace)
- numbers:
  - integers serialized as JSON integers
  - floating-point values rounded to `float_precision` and serialized in fixed-point decimal form
  - exponent notation MUST NOT be used
- booleans and null serialized as standard JSON literals (`true`, `false`, `null`)
- arrays preserve deterministic pre-sorted order from normalization steps

Any implementation that deviates from this profile MUST be treated as a different replay context.

### 4.2.2 Canonical JSON Byte-Stability Rules (Normative)

For byte-stable signatures, canonical serialization MUST additionally satisfy:
- output contains no trailing newline requirement (newline presence/absence MUST be policy-fixed)
- no leading or trailing whitespace anywhere in canonical payload
- object member ordering is deterministic and stable across runs
- floating values MUST be emitted with fixed decimal precision derived from `float_precision`
- floating values with zero fractional component MUST still respect fixed formatting policy
- scientific notation (for example `1e-6`) MUST NOT be emitted
- non-finite numeric values (`NaN`, `Infinity`, `-Infinity`) MUST be rejected before serialization

Implementations SHOULD fail fast when canonicalization inputs violate these constraints.

### 4.2.3 Signature Generation Pseudocode

```text
function meaning_signature(meaning_unit, layer1_config, layer2_policy):
  validate_required_fields(meaning_unit, layer2_policy)

  normalized = normalize_meaning_unit(
    meaning_unit,
    float_precision=layer2_policy.float_precision,
    include_low_confidence=layer2_policy.include_low_confidence,
    provenance_mode=layer2_policy.provenance_canonical_mode
  )

  policy_context = {
    policy_id: layer2_policy.policy_id,
    equivalence_mode: layer2_policy.equivalence_mode,
    float_precision: layer2_policy.float_precision,
    include_low_confidence: layer2_policy.include_low_confidence,
    provenance_canonical_mode: layer2_policy.provenance_canonical_mode
  }

  payload = canonical_json({
    layer1_config: layer1_config,
    layer2_policy: policy_context,
    meaning_unit: normalized
  })

  bytes = utf8(payload)
  return sha256_hex_lowercase(bytes)
```

### 4.3 Equivalence Class

An Equivalence Class is a set of Meaning Units with identical Meaning Signatures under the same policy context.

Implementations MUST NOT merge units from different policy contexts into the same class.

### 4.4 Translation Contract

A Translation Contract is valid only if:
- source unit and target unit share the same Meaning Signature under declared policy
- audit trace records source -> canonical -> target mapping
- no unresolved contradiction class is active

Translation is defined as semantic equivalence under policy, not lexical round-trip identity.

## 5. Layer 2 Policy Object

Layer 2 MUST include an explicit policy object:

```json
{
  "layer2_policy": {
    "policy_id": "L2POLICY_STRICT_1",
    "equivalence_mode": "strict",
    "float_precision": 6,
    "require_provenance": true,
    "provenance_canonical_mode": "exclude",
    "provenance_hash_algorithm": "sha256",
    "include_low_confidence": true,
    "allow_domain_relaxations": false,
    "domain_relaxations": []
  }
}
```

Required fields:
- policy_id
- equivalence_mode (strict | relaxed | domain)
- float_precision
- require_provenance
- provenance_canonical_mode (include | exclude | separate_hash)
- provenance_hash_algorithm (required when provenance_canonical_mode=separate_hash)
- include_low_confidence
- allow_domain_relaxations
- domain_relaxations (required when allow_domain_relaxations=true)

## 6. Deterministic Canonicalization Rules

For each Meaning Unit:
- nodes sorted by node_id
- edges sorted by edge_id
- overlays sorted by (relation, source, target, assertion_id)
- floats rounded to configured float_precision before serialization
- canonical JSON emitted with sorted keys and compact separators

Two units are equivalent if and only if canonical payload bytes are identical under the same policy context.

## 7. Equivalence Modes

### 7.1 strict

strict mode MUST include:
- all committed and non-committed overlays
- uncertainty_type values
- scope labels (relation vs proposition)
- provenance requirements

strict mode interpretation: everything that affects semantic state is part of equivalence.
Trade-off: highest safety and audit confidence, lowest merge rate.

### 7.2 relaxed

relaxed mode MAY ignore low-confidence overlays where confidence_band < tau_accept, if and only if include_low_confidence=false.

relaxed mode interpretation: low-confidence evidence can be excluded from equivalence when policy explicitly opts out.
Trade-off: higher merge rate and broader matching, with lower safety margin.

### 7.3 domain

domain mode MAY apply declared domain relaxations, but MUST declare every relaxation in policy metadata.

When `allow_domain_relaxations=true`, `domain_relaxations` MUST contain only declared, deterministic relaxations. Initial allowed examples:
- `ignore_alias_label_variants`
- `ignore_unit_scale_equivalence` (only when conversion is exact and declared)
- `ignore_domain_stopword_nodes`

Undeclared relaxations MUST NOT be applied.
Trade-off: best domain fit when rules are curated, but strongest governance burden.

## 8. Layer 2 Contradiction Classes

Layer 2 introduces:
- equivalence_scope_mismatch
- modality_projection_conflict
- uncertainty_alignment_failure
- canonicalization_collision_risk
- provenance_gap
- torsion_instability

These are additive to Layer 0 and Layer 1 contradiction classes.

### 8.1 Torsion and Anti-Lobe Integration

Layer 2 signature decisions MUST consume Layer 1 route-audit signals when present.
If canonicalization is attempted on inputs whose inherited torsion state crosses the declared Anti-Lobe threshold, implementations SHOULD emit `torsion_instability` and MUST avoid auto-equivalence promotion.

Recommended behavior:
- emit decision=`needs_input` when torsion is above threshold but conflict is unresolved
- emit decision=`non_equivalent` when torsion instability is committed and policy forbids relaxed handling

When torsion instability is detected during Layer 2 canonicalization, implementations SHOULD trigger Anti-Lobe rejection and return `needs_input` or `non_equivalent` according to policy severity.

## 9. Universal Translator Rule

Two expressions from different languages are semantically equivalent if and only if they produce the same Meaning Signature under the same Layer 2 policy and inherited Layer 1 configuration.

## 10. Audit Trace Contract

Each Layer 2 decision MUST emit an audit trace with:
- source artifacts
- canonical payload snapshot (or hash reference)
- signature
- applied policy
- contradiction classes (if any)
- final decision (equivalent | non_equivalent | needs_input)

## 11. Layer 2 Conformance Examples

### L2-C-018 Canonical Signature Equality

Input:
- two surface variants with identical normalized semantics

Expected:
- pass
- same signature
- equivalent=true

### L2-C-019 Modality Projection Conflict

Input:
- same Layer 0 core
- must vs may divergence under strict policy

Expected:
- pass
- equivalent=false
- contradiction class modality_projection_conflict

### L2-C-020 Scope Mismatch

Input:
- same core
- negates_relation vs negates_proposition mismatch

Expected:
- pass
- equivalent=false
- contradiction class equivalence_scope_mismatch

### L2-C-021 Uncertainty Alignment Failure

Input:
- same core
- uncertainty_type mismatch for same assertion target

Expected:
- pass
- equivalent=false
- contradiction class uncertainty_alignment_failure

### L2-C-022 Replay Signature Stability

Input:
- repeated canonicalization on fixed input

Expected:
- pass
- byte-identical signatures across runs

### L2-C-023 Canonical Ordering Invariance

Input:
- same meaning with shuffled node/edge ordering

Expected:
- pass
- identical signatures

### L2-C-024 Translation Contract Audit Completeness

Input:
- source and target units mapped to same signature

Expected:
- pass
- translation contract valid
- required audit fields present

## 12. Strict Conformance-First Rollout

Layer 2 rollout MUST remain blocked until all preconditions pass:

Phase A:
- Layer 0 pass: C-001..C-013

Phase B:
- Layer 1 pass: L1-C-014..L1-C-017

Phase C:
- Layer 0+1 replay stability pass with baseline matching

Phase D:
- Layer 2 pass: L2-C-018..L2-C-024

Phase E:
- Layer 2 replay stability + baseline drift check pass

Layer 2 production enablement MUST remain blocked unless Phases A..E pass.

## 13. Reference Serialization Skeleton

```json
{
  "layer0_version": "0.2",
  "layer1_version": "0.4",
  "layer2_version": "0.3",
  "layer1_config": {
    "config_version": "L1CFG_1",
    "tau_accept": 0.8,
    "tau_reject": 0.2,
    "uncertainty_policy": "deterministic_v1"
  },
  "layer2_policy": {
    "policy_id": "L2POLICY_STRICT_1",
    "equivalence_mode": "strict",
    "float_precision": 6,
    "require_provenance": true,
    "provenance_canonical_mode": "exclude",
    "provenance_hash_algorithm": "sha256",
    "include_low_confidence": true,
    "allow_domain_relaxations": false,
    "domain_relaxations": []
  },
  "meaning_unit": {
    "unit_id": "u_source",
    "layer0_subgraph": {
      "nodes": [
        {"node_id": "entity_1", "label": "door_1", "provenance": {"source": "example"}}
      ],
      "edges": [
        {
          "edge_id": "e1",
          "source_node": "entity_1",
          "relation": "has_state",
          "target_node": "state_open",
          "confidence_band": 0.9,
          "provenance": {"source": "example"}
        }
      ]
    },
    "layer1_overlays": [
      {
        "assertion_id": "a1",
        "relation": "may",
        "source": "entity_1",
        "target": "assertion_open",
        "scope": "proposition",
        "confidence_band": 0.6,
        "uncertainty_type": "low_confidence",
        "provenance": {"source": "example"}
      }
    ],
    "provenance": {"source": "example"}
  },
  "meaning_signature": "sha256:...",
  "audit_trace": {
    "decision": "equivalent",
    "contradiction_classes": []
  }
}
```

## 14. Appendix: End-to-End Translation Equivalence Example

Source expression (language A):
- "Door 1 must be open."

Target expression (language B):
- "La puerta 1 debe estar abierta."

### 14.1 Layer 0 + Layer 1 Representation (normalized)

```json
{
  "layer0_subgraph": {
    "nodes": [
      {"node_id": "door_1", "label": "door_1", "provenance": {"source": "A"}},
      {"node_id": "state_open", "label": "open", "provenance": {"source": "A"}}
    ],
    "edges": [
      {
        "edge_id": "e1",
        "source_node": "door_1",
        "relation": "has_state",
        "target_node": "state_open",
        "confidence_band": 0.9,
        "provenance": {"source": "A"}
      }
    ]
  },
  "layer1_overlays": [
    {
      "assertion_id": "a1",
      "relation": "must",
      "source": "door_1",
      "target": "assertion_door_open",
      "scope": "proposition",
      "confidence_band": 0.9,
      "uncertainty_type": "none",
      "provenance": {"source": "A"}
    }
  ]
}
```

### 14.2 Meaning Unit and Signature Generation

Policy context:
- policy_id: `L2POLICY_STRICT_1`
- equivalence_mode: `strict`
- float_precision: `6`

Process:
1. Normalize nodes/edges/overlays ordering.
2. Normalize floats to fixed precision.
3. Serialize canonical JSON with sorted keys and UTF-8.
4. Compute SHA-256 digest.

Example signature:
- `d21da30c9b8a1b50dc23844a911135b30cccdb6826e2a6554483d89dd2f0ab4b`

### 14.3 Translation Contract Decision

If source and target expressions both produce the same signature under the same policy:
- decision: `equivalent`
- translation contract: valid

If signatures differ or contradiction classes are present:
- decision: `non_equivalent` or `needs_input`
- translation contract: invalid until resolved

## 15. Recommended Default Policy (Quick Reference)

| Field | Recommended Default | Notes |
|---|---|---|
| policy_id | L2POLICY_STRICT_1 | Stable baseline policy identifier |
| equivalence_mode | strict | Safety-first default |
| float_precision | 6 | Good trade-off for deterministic replay |
| require_provenance | true | Auditability by default |
| provenance_canonical_mode | exclude | Prevent source-format drift in primary signature |
| provenance_hash_algorithm | sha256 | Use when provenance is separately hashed |
| include_low_confidence | true | Preserve uncertainty evidence in strict mode |
| allow_domain_relaxations | false | Prevent implicit domain drift |
| domain_relaxations | [] | Empty by default; explicit allow-list only |
