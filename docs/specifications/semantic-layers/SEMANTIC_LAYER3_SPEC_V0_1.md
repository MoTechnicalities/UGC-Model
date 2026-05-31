# Semantic Layer 3 Specification v0.1

This document defines Layer 3 deterministic derivation behavior on top of Semantic Layer 2 canonical meaning.

Layer 3 introduces formal inference over Layer 0 facts, Layer 1 operators, and Layer 2 equivalence classes.

## 1. Purpose

Layer 3 extends Layer 2 with:
- deterministic logic derivation rules
- quantified reasoning over sets (all/some/none)
- temporal and causal chain derivation
- mandatory derivation audit trails for every derived fact

Layer 3 MUST NOT redefine Layer 0, Layer 1, or Layer 2 invariants.

## 2. Dependencies

Layer 3 requires:
- Layer 0 pass: C-001..C-013
- Layer 1 pass: L1-C-014..L1-C-017
- Layer 2 pass: L2-C-018..L2-C-024
- Layer 2 stability + baseline pass (Phase E)

## 3. Normative Terms

The keywords MUST, SHOULD, and MAY are normative.

## 4. Core Derivation Domains

### 4.1 Logic

Layer 3 MUST support deterministic derivation rules, including:
- `modus_ponens_v1`: from `holds(A)` and `implies(A,B)` derive `holds(B)`

### 4.2 Quantification

Layer 3 MUST support quantified constraints over explicit domains:
- `all(X)`
- `some(X)`
- `none(X)`

Initial required quantifier rule:
- `universal_instantiation_v1`: from all-elements policy and explicit domain list, derive per-entity constraints.

### 4.3 Temporal and Causal

Layer 3 MUST support deterministic transitive closure rules:
- `transitive_causes_v1`: from `causes(A,B)` and `causes(B,C)` derive `causes(A,C)`
- `transitive_before_v1`: from `before(T1,T2)` and `before(T2,T3)` derive `before(T1,T3)`

## 5. Derivation Audit Trail (Normative)

Every derived fact MUST carry derivation metadata:
- `derived_from`: list of source edge/assertion identifiers
- `derivation_rule`: deterministic rule identifier
- `layer3_config`: config version used for derivation

A derived fact without these fields MUST be considered invalid.

## 6. Layer 3 Configuration Object

```json
{
  "layer3_config": {
    "config_version": "L3CFG_1",
    "enable_logic": true,
    "enable_quantification": true,
    "enable_temporal_causal": true,
    "max_derivation_depth": 8,
    "derivation_policy": "deterministic_v1"
  }
}
```

Required fields:
- config_version
- enable_logic
- enable_quantification
- enable_temporal_causal
- max_derivation_depth
- derivation_policy

## 7. Contradiction Classes (Layer 3)

Layer 3 introduces:
- `derivation_rule_mismatch`
- `quantifier_domain_gap`
- `temporal_causal_inconsistency`
- `derivation_provenance_gap`

## 8. Conformance Cases

- L3-C-025: logic derivation (modus ponens)
- L3-C-026: quantified derivation (universal instantiation)
- L3-C-027: temporal/causal transitive derivation
- L3-C-028: derivation audit trail completeness

## 9. Rollout Policy

Layer 3 rollout MUST remain blocked until:
- Layer 0, 1, and 2 gates pass
- Layer 2 stability-baseline gate passes
- Layer 3 conformance passes
- Layer 3 stability-baseline gate is introduced and passing

## 10. Reference Derived Fact Skeleton

```json
{
  "derived_fact": {
    "fact_id": "d1",
    "relation": "causes",
    "source": "rain",
    "target": "slippery",
    "confidence_band": 1.0,
    "derived_from": ["e_rain_wet", "e_wet_slippery"],
    "derivation_rule": "transitive_causes_v1",
    "layer3_config": "L3CFG_1"
  }
}
```
