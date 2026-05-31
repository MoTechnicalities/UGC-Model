# Semantic Layer 0 Specification v0.2

This document defines a substrate-first semantic baseline for CSIF-compatible reasoning.

Layer 0 is intentionally minimal and language-agnostic. It captures what must be representable before any domain or natural-language interpretation is applied.

## 1. Purpose

Layer 0 standardizes semantic meaning at the level of spacetime existence, identity, and primitive relations.

It is designed to:
- anchor higher-layer semantics to invariant structure
- support deterministic replay and contradiction checking
- provide a conformance baseline across languages and species-level symbol systems

## 2. Scope

This spec defines:
- canonical Layer 0 graph primitives
- required invariants
- minimal relation set
- event ordering and identity rules
- conformance examples

This spec does not define:
- language parsing
- domain ontologies
- policy-specific truth systems

## 3. Normative Terms

The keywords MUST, SHOULD, and MAY are normative.

## 4. Layer 0 Core Primitives

### 4.1 Node Types

A Layer 0 implementation MUST support these node categories:

- entity: persistent object or agent candidate
- event: bounded occurrence/change
- state: bounded property condition (for example alive, open, active)
- region: spatial container/reference frame
- interval: temporal container/reference frame
- quantity: measured scalar/vector value token
- unknown_value: explicit placeholder for unobserved/underdetermined value

Required node fields:
- node_id (stable identifier)
- node_type (one of the categories above)
- label (implementation-defined text token)
- provenance (source metadata)

### 4.2 Edge Types

A Layer 0 implementation MUST support these edge relations:

- occurs_in(event -> interval)
- located_in(entity|event -> region)
- before(event|interval -> event|interval)
- causes(event -> event)
- part_of(entity|region -> entity|region)
- transforms_to(entity|state -> entity|state)
- has_state(entity -> state)
- same_as(node -> node)
- different_from(node -> node)
- negates(assertion|state -> assertion|state)
- interacts_with(entity -> entity|event)
- measures(quantity -> entity|event|region|interval)

Required edge fields:
- edge_id
- source_node
- relation
- target_node
- confidence_band
- provenance

### 4.3 Confidence Band Semantics

Layer 0 defines confidence_band as normalized assertion strength in [0.0, 1.0].

- 1.0: fully committed assertion strength
- 0.5: weak/provisional assertion strength
- 0.0: untrusted or explicitly unknown assertion strength

confidence_band is not required to be Bayesian probability. It is a deterministic
commitment strength used for ordering, gating, and contradiction resolution.

Implementations SHOULD declare operational thresholds:
- tau_accept for committed true assertions
- tau_reject for committed false/negated assertions
- intermediate zone mapped to NEEDS_INPUT

## 5. Invariants

### 5.1 Existence Anchoring

Any claim node SHOULD be anchored through at least one of:
- located_in(... -> region)
- occurs_in(... -> interval)

### 5.2 Identity Persistence

If an entity appears across multiple intervals, the implementation MUST either:
- preserve same node_id, or
- link snapshots with same_as

### 5.3 Temporal Acyclicity

The before relation MUST be acyclic within one consistent timeline context.

### 5.4 Causal Directionality

causes(A, B) SHOULD imply not before(B, A) unless marked as speculative and isolated from hard inference.

### 5.5 Part-Whole Consistency

part_of MUST be transitive within one ontology frame unless explicit frame boundaries are declared.

### 5.6 Distinction Safety

different_from(X, Y) MUST NOT co-exist with same_as(X, Y) at equal confidence in one committed state.

### 5.7 Unknown Handling

An implementation MUST support explicit unknown/unobserved representation through one or both:
- unknown_value nodes
- assertions with confidence_band = 0.0

Unknown MUST NOT be silently coerced into true or false in committed outputs.

### 5.8 Negation Consistency

If negates(A, B) is committed above tau_accept, then A and B MUST NOT both be committed as true in the same context window.

Implementations SHOULD expose tri-state outputs derived from Layer 0 status:
- YES: committed support above tau_accept with no blocking contradiction
- NO: committed negation or contradiction above tau_reject
- NEEDS_INPUT: unknown/underdetermined support or unresolved conflict

## 6. Minimal Atomic Semantic Units

Layer 0 atomic units are represented as typed relation assertions:

- Exist(X): X has node identity
- Locate(X, R): located_in(X, R)
- Time(X, T): occurs_in(X, T)
- Order(A, B): before(A, B)
- Cause(A, B): causes(A, B)
- Compose(P, W): part_of(P, W)
- Transform(A, B): transforms_to(A, B)
- StateOf(X, S): has_state(X, S)
- Identity(A, B): same_as(A, B)
- Distinct(A, B): different_from(A, B)
- Negate(A, B): negates(A, B)
- Interact(A, B): interacts_with(A, B)

## 7. Deterministic Representation Contract

A conforming implementation MUST provide deterministic canonical ordering for serialization:
- nodes sorted by node_id
- edges sorted by edge_id
- relation names emitted exactly as declared
- confidence values quantized to declared precision

A serialized Layer 0 artifact SHOULD be byte-stable under replay with unchanged input.

## 8. Contradiction Baseline

The following are baseline contradiction classes:

- temporal_cycle: before(A, B) and before(B, A)
- identity_conflict: same_as(A, B) and different_from(A, B)
- causal_inversion: causes(A, B) while before(B, A)
- illegal_self_part: part_of(X, X) unless self-container mode explicitly enabled

Implementations MUST emit explicit stop_reason values for contradiction-triggered failures.

## 9. Mapping Guidance for Higher Layers

Higher layers (language, logic, domain) SHOULD map into Layer 0 first:
- parse expression/sentence
- produce candidate Layer 0 graph
- run Layer 0 invariant checks
- only then run advanced CSIF phase/torsion policies

## 10. Conformance Examples (13)

### C-001 Temporal Order Valid

Input assertions:
- before(event_birth, event_walk)
- before(event_walk, event_rest)

Expected:
- pass
- no temporal_cycle

### C-002 Temporal Cycle Rejected

Input assertions:
- before(a, b)
- before(b, a)

Expected:
- fail
- contradiction class: temporal_cycle

### C-003 Identity Persistence

Input assertions:
- same_as(entity_t1, entity_t2)
- occurs_in(entity_t1, interval_1)
- occurs_in(entity_t2, interval_2)

Expected:
- pass
- entity lineage accepted

### C-004 Identity Conflict

Input assertions:
- same_as(x, y)
- different_from(x, y)

Expected:
- fail
- contradiction class: identity_conflict

### C-005 Causal Forward Valid

Input assertions:
- causes(event_heat, event_expand)
- before(event_heat, event_expand)

Expected:
- pass

### C-006 Causal Inversion Rejected

Input assertions:
- causes(event_a, event_b)
- before(event_b, event_a)

Expected:
- fail
- contradiction class: causal_inversion

### C-007 Part-Whole Transitivity

Input assertions:
- part_of(cell, organ)
- part_of(organ, body)

Expected:
- pass
- inferred: part_of(cell, body)

### C-008 Illegal Self-Part

Input assertions:
- part_of(x, x)

Expected:
- fail unless self-container mode enabled

### C-009 Existence Anchoring

Input assertions:
- entity bird_1
- located_in(bird_1, region_forest)

Expected:
- pass
- anchoring satisfied

### C-010 Interaction + Measurement

Input assertions:
- interacts_with(predator, prey)
- measures(quantity_speed, predator)

Expected:
- pass
- quantity linkage valid

### C-011 State Assertion

Input assertions:
- has_state(door_7, state_open)

Expected:
- pass
- state binding valid

### C-012 Unknown Assertion Handling

Input assertions:
- measures(quantity_temp, reactor_1) with confidence_band=0.0

Expected:
- pass
- verdict defaults to NEEDS_INPUT for temperature claim

### C-013 Negation Consistency

Input assertions:
- has_state(light_2, state_on)
- negates(assertion_light_on, assertion_light_on)

Expected:
- fail or demote to NEEDS_INPUT depending on threshold policy
- contradiction class includes negation conflict when both are committed

## 11. Reference Serialization Skeleton

```json
{
  "layer0_version": "0.2",
  "nodes": [
    {
      "node_id": "entity_1",
      "node_type": "entity",
      "label": "entity_1",
      "provenance": {"source": "example"}
    },
    {
      "node_id": "state_open",
      "node_type": "state",
      "label": "open",
      "provenance": {"source": "example"}
    }
  ],
  "edges": [
    {
      "edge_id": "edge_1",
      "source_node": "entity_1",
      "relation": "located_in",
      "target_node": "region_1",
      "confidence_band": 1.0,
      "provenance": {"source": "example"}
    },
    {
      "edge_id": "edge_2",
      "source_node": "entity_1",
      "relation": "has_state",
      "target_node": "state_open",
      "confidence_band": 0.5,
      "provenance": {"source": "example"}
    }
  ],
  "stop_reason": "path_found"
}
```

## 12. Next Step Recommendations

- Create executable conformance tests for C-001..C-013.
- Add Layer 0 -> RWIF mapping profile for append-only event traces.
- Add Layer 1 draft for modality, negation, and uncertainty operators built on Layer 0 invariants.

## 13. Unit Crystal Extension (Implementation Profile)

This repository now uses an implementation profile where unit representation is
modeled as a first-class object (`unit_crystal`) in evaluation outputs.

This profile is additive and does not change Layer 0 core primitive
requirements above.

### 13.1 Additional Relation (Profile)

Implementations MAY emit:

- represented_in_unit(value_node -> unit_crystal)

to encode representation/unit projection explicitly.

### 13.2 Conversion Morphism Chains (Profile)

Implementations MAY include conversion chains with hop-by-hop phase drift and
closed-loop metrics:

- per-hop `phase_drift`
- loop `loop_torsion_norm`
- loop `loop_resonance`

### 13.3 Contradiction Class (Profile)

When conversion loop torsion exceeds configured threshold, implementations MAY
emit contradiction class:

- unit_conversion_loop_torsion_exceeded

with explicit stop reason of the same value.

### 13.4 Consistency Propagation (Profile)

When emitted, unit conversion contradictions SHOULD be propagated into shared
consistency contradiction channels so summary contradiction counts and detailed
contradiction records stay in lockstep.

### 13.5 Anticrystal Lobe (Profile)

Implementations MAY emit an explicit `anticrystal_lob` as a first-class store
for negative knowledge (for example: "this is wrong" / "this does not work
because ...").

Recommended fields:

- `lobe`: fixed identifier (for example `anticrystal`)
- `entry_count`: deterministic count of negative entries
- `entries[*].contradiction_id`: stable contradiction reference
- `entries[*].this_is_wrong_because`: human-readable contradiction rationale
- `entries[*].severity`: normalized severity label

When present, `anticrystal_lob.entry_count` SHOULD remain consistent with the
active contradiction set emitted in consistency channels.

### 13.6 Time-Crystal Auditable Randomness Appearance (Profile)

Implementations MAY derive apparent randomness from a deterministic time
coordinate embedded in the geometric trajectory.

This profile treats time as a first-class trajectory dimension, not merely a
timestamp. Time-driven outputs MAY appear random to external observers while
remaining deterministic and replayable under fixed inputs.

Recommended fields:

- `time_crystal.t_ns`: captured evaluation-time coordinate used for the run
- `time_crystal.phase_theta`: phase projection of time coordinate
- `time_crystal.torsion_norm`: normalized torsion contribution from time axis
- `randomness_appearance.mode`: fixed label, for example `deterministic_time_chaos`
- `randomness_appearance.replay_key`: stable replay identifier over function + time crystal + options
- `randomness_appearance.audit_trace_id`: stable identifier for stepwise reconstruction

Determinism and replay requirements:

- With identical expression, options, time crystal, and function version,
  output MUST be identical.
- Implementations SHOULD record enough trajectory detail to explain why a
  specific result occurred.
- Apparent randomness derived from this profile MUST be reconstructible from
  persisted trace artifacts.

Safety and claim boundaries:

- This profile MUST NOT be labeled true randomness.
- Cryptographic or regulated-gaming suitability MUST NOT be claimed without
  separate domain validation and certification.
