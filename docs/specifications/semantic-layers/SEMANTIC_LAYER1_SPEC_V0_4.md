# Semantic Layer 1 Specification v0.4

This document defines Layer 1 semantic extensions built on top of Semantic Layer 0.

Layer 1 introduces controlled expressivity for modality, scoped negation, and typed uncertainty, while preserving deterministic replay and conformance-first rollout.

## 1. Purpose

Layer 1 extends Layer 0 with:
- modality primitives (can, may, must, impossible)
- scoped negation primitives (relation-level vs proposition-level)
- uncertainty typing for deterministic non-binary reasoning

Layer 1 MUST NOT redefine Layer 0 invariants. It must consume Layer 0 graphs and add constrained overlays.

## 2. Dependency on Layer 0

Layer 1 requires a Layer 0-compliant input graph.

Minimum assumptions from Layer 0:
- deterministic serialization contract
- core contradiction classes
- tri-state output discipline (YES/NO/NEEDS_INPUT)
- conformance baseline C-001..C-013

Layer 1 assertions MUST be grounded in Layer 0 structure. In practice, each assertion node SHOULD map to one or more Layer 0 edges (or edge bundles) that provide its evidentiary basis. Implementations SHOULD preserve this grounding via provenance links (for example, assertion_id -> supporting edge_id list) so Layer 1 verdicts remain auditable against Layer 0 facts.

Grounding example:
- Layer 0 edge: has_state(entity_door_1, state_open) with edge_id=e1
- Layer 1 assertion node: assertion_door_open
- Provenance link: assertion_door_open -> [e1]

This means the Layer 1 proposition is explicitly anchored to the Layer 0 state edge used as evidence.

## 3. Normative Terms

The keywords MUST, SHOULD, and MAY are normative.

## 4. Layer 1 Core Primitives

### 4.1 Additional Node Type

Layer 1 introduces:
- assertion: explicit proposition/relation claim node used as a first-class target for modality and scoped negation

Required assertion fields:
- node_id
- node_type = assertion
- label
- provenance

### 4.2 Additional Edge Relations

Layer 1 defines these additional relations:

- negates_relation(assertion -> assertion)
- negates_proposition(assertion -> assertion)
- can(entity|assertion -> assertion)
- may(entity|assertion -> assertion)
- must(entity|assertion -> assertion)
- impossible(entity|assertion -> assertion)

All Layer 1 edges MUST retain Layer 0 edge base fields:
- edge_id
- source_node
- relation
- target_node
- confidence_band
- provenance

## 5. Uncertainty Typing

Layer 1 requires explicit uncertainty_type for non-committed assertions.

Allowed values:
- unknown: no sufficient observation
- conflicting: simultaneous competing support/negation
- low_confidence: weak support below tau_accept
- none: committed assertion with no uncertainty flag

Mapping guidance:
- confidence_band = 0.0 SHOULD map to unknown
- unresolved contradiction windows SHOULD map to conflicting
- 0.0 < confidence_band < tau_accept SHOULD map to low_confidence

Given fixed inputs and thresholds, uncertainty_type assignment MUST be deterministic and replay-stable.

### 5.1 Formal Decision Boundaries

Implementations MUST apply the following deterministic boundary order for each evaluated assertion:

1. If contradiction state is active for the assertion context, set uncertainty_type=conflicting.
2. Else if confidence_band == 0.0, set uncertainty_type=unknown.
3. Else if 0.0 < confidence_band < tau_accept, set uncertainty_type=low_confidence.
4. Else set uncertainty_type=none.

Recommended threshold defaults:

| Symbol | Meaning | Default |
|---|---|---|
| tau_accept | committed support threshold | 0.8 |
| tau_reject | committed reject threshold | 0.2 |

Boundary policy:
- confidence_band >= tau_accept is committed support candidate.
- confidence_band <= tau_reject is reject/insufficient candidate unless contradiction routing overrides.
- tau_reject < confidence_band < tau_accept maps to NEEDS_INPUT unless other committed evidence resolves the claim.

### 5.2 Required Threshold Configuration Object

Implementations MUST include a versioned Layer 1 configuration object in serialized artifacts that participate in conformance or replay.

Required fields:
- config_version
- tau_accept
- tau_reject
- uncertainty_policy

Example:

```json
{
  "layer1_config": {
    "config_version": "L1CFG_1",
    "tau_accept": 0.8,
    "tau_reject": 0.2,
    "uncertainty_policy": "deterministic_v1"
  }
}
```

The same graph evaluated under different `layer1_config` values MUST be treated as a different replay context.

## 6. Scoped Negation Contract

Layer 1 MUST preserve negation scope:
- relation-level negation via negates_relation(assertion_rel, assertion_rel_target)
- proposition-level negation via negates_proposition(assertion_prop, assertion_prop_target)

Implementations MUST NOT collapse proposition-level negation into relation-level negation or vice versa.

## 7. Modality Contract

For same scope and context:
- must(X) and impossible(X) MUST NOT both be committed above tau_accept
- impossible(X) above tau_accept MUST block YES verdicts for X
- may(X) MAY co-exist with can(X)
- may(X) MUST NOT imply must(X)

### 7.1 Modality Composition Rules

Layer 1 MUST use explicit composition outcomes for same-context modality pairs:

| Composition | Required Outcome |
|---|---|
| must(X) + impossible(X) | contradiction: modality_conflict |
| must(X) + may(X) | must(X) dominates; verdict cannot be weaker than must unless blocked by contradiction |
| can(X) + impossible(X) | contradiction unless can(X) is below tau_accept |
| may(X) + can(X) | admissible coexistence; no automatic promotion to must |
| may(X) + impossible(X) | NEEDS_INPUT or contradiction depending on confidence and thresholds |

When confidence differs, engines SHOULD evaluate only committed edges (>= tau_accept) for hard conflicts and keep weaker edges as advisory evidence.

## 8. Negation Scope Interaction with Torsion and Anti-Lobe

Layer 1 scope choices MUST influence torsion and Anti-Lobe routing deterministically:

- negates_relation affects only the targeted relation assertion trajectory.
- negates_proposition affects proposition-level claim trajectory and its derived route verdict.

Torsion guidance:
- relation-scope negation SHOULD add torsion residual only to the relation branch where negation is applied.
- proposition-scope negation SHOULD add torsion residual to proposition-level routing and downstream verdict branching.

Anti-Lobe trigger guidance:
- relation-scope negation MAY trigger Anti-Lobe on a localized branch.
- proposition-scope negation SHOULD trigger Anti-Lobe at proposition verdict level when committed above tau_accept.

Implementations MUST preserve these scope-to-torsion mappings in route audit output.

### 8.1 Torsion Contribution Example

Implementations SHOULD make torsion contributions explicit. One acceptable deterministic pattern is:

$$
T_{total} = T_{base} + w_r \cdot N_r + w_p \cdot N_p
$$

Where:
- $T_{base}$ is baseline trajectory torsion from non-negation routing.
- $N_r$ is count (or weighted sum) of committed `negates_relation` contributions.
- $N_p$ is count (or weighted sum) of committed `negates_proposition` contributions.
- $w_r$ and $w_p$ are fixed config weights with $w_p > w_r$ recommended.

Example defaults:
- $w_r = 0.15$
- $w_p = 0.35$

Anti-Lobe guidance:
- trigger when $T_{total} \ge T_{anti}$ where $T_{anti}$ is a declared threshold in `layer1_config`.

### 8.2 Torsion Residual Verdict Integration

Implementations MUST apply a deterministic verdict gate using torsion residual:
- if $T_{total} \ge anti\_lobe\_threshold$, trigger Anti-Lobe and emit stop_reason=anti_lobe_negative_match
- else continue normal Layer 1 verdict composition using modality, negation, and uncertainty rules

This gate MUST execute after torsion accumulation and before final verdict emission.

## 9. Contradiction Extensions

Layer 1 adds contradiction classes:
- modality_conflict: must(X) and impossible(X) both committed
- scoped_negation_mismatch: negation scope target type mismatch
- negation_conflict: negation committed against simultaneously committed target assertion

These are additive to Layer 0 contradiction classes.

## 10. Deterministic Representation

Layer 1 artifacts MUST preserve deterministic ordering and replay behavior from Layer 0:
- nodes sorted by node_id
- edges sorted by edge_id
- fixed threshold config emitted
- uncertainty_type emitted deterministically for evaluated assertions

## 11. Layer 1 Conformance Examples (4)

### L1-C-014 Modality Must-Impossible Conflict

Input assertions:
- must(agent_a, assertion_exit_open)
- impossible(agent_a, assertion_exit_open)

Expected:
- fail
- contradiction class: modality_conflict

### L1-C-015 Scoped Relation Negation

Input assertions:
- assertion_rel_1 encodes causes(event_a, event_b)
- assertion_rel_2 encodes before(event_a, event_b)
- negates_relation(assertion_rel_3, assertion_rel_2)

Expected:
- pass
- relation-level negation scope preserved
- torsion delta applied to relation branch only

### L1-C-016 Scoped Proposition Negation

Input assertions:
- assertion_prop_1 encodes proposition door_is_open
- negates_proposition(assertion_prop_2, assertion_prop_1)

Expected:
- pass
- proposition-level negation scope preserved
- proposition-level torsion and Anti-Lobe routing preserved

### L1-C-017 Uncertainty Typing Determinism

Input assertions:
- assertion_x with confidence_band=0.2
- fixed thresholds tau_accept=0.8, tau_reject=0.2

Expected:
- pass
- uncertainty_type = low_confidence deterministically assigned on replay

## 12. Strict Conformance-First Rollout

Rollout MUST remain gate-driven:

Phase A:
- Layer 0 required pass: C-001..C-013

Phase B:
- Layer 1 required pass: L1-C-014..L1-C-017

Phase C:
- replay all Layer 0 + Layer 1 conformance cases with byte-stable outputs
- require deterministic uncertainty_type assignment

Phase D (stability replay + small-refactor safety):
- execute repeated replay runs (RECOMMENDED: N >= 5) of the combined conformance gate
- require identical Layer 0, Layer 1, and combined summary hashes across all runs
- after small refactors, require no unexpected drift versus pre-refactor baseline summaries unless intentionally approved

A build MAY only enable Layer 1 production behavior when all phases pass.
Layer 2 work MUST remain blocked until Phase D passes.

## 13. Reference Serialization Skeleton

```json
{
  "layer0_version": "0.2",
  "layer1_version": "0.4",
  "layer1_config": {
    "config_version": "L1CFG_1",
    "tau_accept": 0.8,
    "tau_reject": 0.2,
    "uncertainty_policy": "deterministic_v1",
    "torsion_weights": {
      "relation_negation": 0.15,
      "proposition_negation": 0.35
    },
    "anti_lobe_threshold": 0.7
  },
  "nodes": [
    {
      "node_id": "entity_1",
      "node_type": "entity",
      "label": "entity_1",
      "provenance": {"source": "example"}
    },
    {
      "node_id": "assertion_open",
      "node_type": "assertion",
      "label": "door_is_open",
      "provenance": {"source": "example"}
    }
  ],
  "edges": [
    {
      "edge_id": "edge_1",
      "source_node": "entity_1",
      "relation": "may",
      "target_node": "assertion_open",
      "confidence_band": 0.5,
      "uncertainty_type": "low_confidence",
      "provenance": {"source": "example"}
    }
  ],
  "stop_reason": "path_found"
}
```

## 14. Appendix: Full Layer 0 + Layer 1 Graph Example

```json
{
  "layer0_version": "0.2",
  "layer1_version": "0.4",
  "layer1_config": {
    "config_version": "L1CFG_1",
    "tau_accept": 0.8,
    "tau_reject": 0.2,
    "uncertainty_policy": "deterministic_v1",
    "torsion_weights": {
      "relation_negation": 0.15,
      "proposition_negation": 0.35
    },
    "anti_lobe_threshold": 0.7
  },
  "nodes": [
    {
      "node_id": "entity_door_1",
      "node_type": "entity",
      "label": "door_1",
      "provenance": {"source": "example"}
    },
    {
      "node_id": "state_open",
      "node_type": "state",
      "label": "open",
      "provenance": {"source": "example"}
    },
    {
      "node_id": "assertion_door_open",
      "node_type": "assertion",
      "label": "door_1_is_open",
      "provenance": {"source": "example"}
    },
    {
      "node_id": "assertion_not_open",
      "node_type": "assertion",
      "label": "door_1_is_not_open",
      "provenance": {"source": "example"}
    }
  ],
  "edges": [
    {
      "edge_id": "e1",
      "source_node": "entity_door_1",
      "relation": "has_state",
      "target_node": "state_open",
      "confidence_band": 0.9,
      "provenance": {"source": "example"}
    },
    {
      "edge_id": "e2",
      "source_node": "entity_door_1",
      "relation": "may",
      "target_node": "assertion_door_open",
      "confidence_band": 0.6,
      "uncertainty_type": "low_confidence",
      "provenance": {"source": "example"}
    },
    {
      "edge_id": "e3",
      "source_node": "assertion_not_open",
      "relation": "negates_proposition",
      "target_node": "assertion_door_open",
      "confidence_band": 0.85,
      "uncertainty_type": "none",
      "provenance": {"source": "example"}
    }
  ],
  "route_audit": {
    "torsion_relation_branch": 0.12,
    "torsion_proposition_branch": 0.41,
    "torsion_residual_total": 0.76,
    "anti_lobe_triggered": true
  },
  "stop_reason": "needs_input"
}
```

## 15. Next Step Recommendations

- Add executable Layer 1 fixtures and gates under tests/conformance/semantic_layer1.
- Keep Layer 0 and Layer 1 fixture suites physically separated.
- Add a combined gate command that executes Layer 0 first, then Layer 1.

## 16. Recommended Default Configuration

Quick-reference default Layer 1 configuration:

```json
{
  "layer1_config": {
    "config_version": "L1CFG_1",
    "tau_accept": 0.8,
    "tau_reject": 0.2,
    "uncertainty_policy": "deterministic_v1",
    "torsion_weights": {
      "relation_negation": 0.15,
      "proposition_negation": 0.35
    },
    "anti_lobe_threshold": 0.7
  }
}
```
