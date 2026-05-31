# CSIF v2 Engine Specification

This document defines a clean, implementation-ready CSIF v2 engine contract for deterministic, auditable phase-geometric reasoning.

It is designed to pair with RWIF v2 storage semantics in [RWIF_V2_FIELD_SPEC.md](../rwif/RWIF_V2_FIELD_SPEC.md).

## 1. Purpose

CSIF defines the runtime reasoning model for:
- coherence detection
- contradiction detection
- deterministic semantic routing
- append-only audit traces

RWIF defines persistence and trajectory encoding. CSIF defines computation and behavioral guarantees.

## 2. Scope

This specification covers:
- numeric and state model
- graph and path composition model
- contradiction and cancellation metrics
- deterministic replay requirements
- compatibility requirements for engine behavior

This specification does not require one specific transport API or one specific storage engine.

## 3. Core Terms

- Node: semantic concept vertex.
- Edge: directed semantic relation from source node to target node.
- Phase: wrapped angular value in principal domain.
- Trajectory event: append-only evidence update for an edge.
- Intent: signed directional propagation memory used when amplitude crosses equilibrium.
- Residual: absolute disagreement metric between composed paths.

## 4. Normative Requirements

The keywords MUST, SHOULD, and MAY are normative.

### 4.1 Determinism

Given identical input graph state, query, and engine config, the engine MUST produce byte-stable:
- acceptance/rejection decision
- contradiction metrics
- selected trace path identifiers
- stop reason

### 4.2 Append-Only Auditability

Runtime updates MUST be representable as append-only trajectory events.

### 4.3 Reproducibility

Replay MUST be possible from persisted events and declared integration settings.

### 4.4 Graceful Degradation

If optional extensions are unavailable, engine MUST fall back to baseline phase-scalar behavior without changing safety guarantees.

## 5. Numeric Model

### 5.1 Phase Domain

The canonical phase wrapping function is:

$$
\mathrm{wrap}_{\pi}(\theta) = ((\theta + \pi) \bmod 2\pi) - \pi
$$

All phase composition and residual computations MUST use wrapped values.

### 5.2 Signed State Tuple

CSIF v2 defines a runtime state tuple for propagation:
- amplitude_signed
- intent_signed

Interpretation:
- amplitude_signed encodes signed displacement/polarity.
- intent_signed encodes directional memory across equilibrium crossings.

A single equilibrium amplitude zero is canonical. Directional history is represented by intent, not dual independent zero amplitudes.

### 5.3 Recommended Quantization

For CPU-first deterministic runtime:
- amplitude range SHOULD default to signed i8: [-127, 127]
- intent range SHOULD default to signed i8: [-127, 127]
- quantization step SHOULD be explicit and fixed
- integer wrap policy SHOULD be explicit (`clamp` or `overflow_modulo`)

Alternative ranges MAY be used if declared in metadata and replay remains deterministic.

## 6. Graph and Path Composition

### 6.1 Edge Orientation

Edges are directional. Reverse traversal MUST apply inverse composition semantics.

### 6.2 Reverse Relation Composition

For phase-only fallback mode, reverse traversal SHOULD use sign inversion under principal wrap:

$$
\theta_{reverse} = \mathrm{wrap}_{\pi}(-\theta)
$$

### 6.3 Path State Composition

For each traversed edge, the composed path state updates:
- composed phase
- composed amplitude
- composed intent
- uncertainty/confidence envelope

The exact integration rule MUST be named by configuration (for example legacy_scalar, leapfrog_v1).

## 7. Contradiction and Cancellation

### 7.1 Multi-Path Residual

When two valid paths connect source and target:

$$
r_{phase} = \left|\mathrm{wrap}_{\pi}(\theta_A - \theta_B)\right|
$$

A contradiction candidate exists when residual crosses configured threshold.

### 7.2 Threshold Contract

Default threshold model:

$$
T_{alarm} = \frac{\pi}{2} + c \cdot \sigma_{path}
$$

Where:
- c is a declared stability constant
- sigma_path is declared uncertainty envelope

### 7.3 Signed-State Cancellation

In signed-state mode, contradiction and cancellation SHOULD also evaluate:
- amplitude cancellation residual
- intent conflict residual

A strict cancellation identity case is:

$$
A + (-A) = 0
$$

Cross-origin traversal MUST preserve intent unless explicit damping policy applies.

## 8. Routing and Stop Reasons

The runtime MUST produce explicit stop reasons for each query route, including but not limited to:
- path_found
- no_supporting_path
- anti_lobe_negative_match
- contradiction_detected
- timeout_or_budget

## 9. Engine Configuration Surface

A compliant implementation MUST expose effective runtime configuration for audit:
- wrap_mode
- integer_wrap_mode
- integration_rule
- quantization_step
- numeric_range
- threshold strategy and constants
- deterministic seed or deterministic mode flag if used

## 10. CSIF-RWIF Boundary

CSIF computes; RWIF persists.

CSIF output events SHOULD map directly to RWIF v2 trajectory fields:
- phase
- confidence_band
- drift_delta
- event_type
- source
- amplitude_signed
- intent_signed
- phase_theta and phase_omega when available
- monotonic_index

## 11. Compatibility Modes

### 11.1 Legacy Scalar Mode

Mode name: phase_scalar_v1

Behavior:
- phase and confidence semantics as v1
- amplitude and intent optional or null
- deterministic behavior unchanged from v1

### 11.2 Signed Intent Mode

Mode name: signed_i8_plus_intent_v2

Behavior:
- amplitude and intent active in path composition
- single amplitude equilibrium with directional continuity
- deterministic replay requires declared integration rule and quantization

## 12. Minimal Query Result Contract

A query result SHOULD include:
- answer string
- decision label
- route audit object
- stop reason
- optional contradiction metrics
- optional residual components

## 13. Security and Integrity Invariants

The implementation MUST maintain:
- append-only update semantics for persisted trajectories
- full provenance on accepted and rejected updates
- deterministic trace reconstruction from stored artifacts

## 14. Compliance Checklist

A CSIF engine is v2-compliant if:
- it preserves deterministic outputs under fixed state and config
- it emits explicit stop reasons
- it supports phase wrap and contradiction residual checks
- it can operate in legacy scalar mode
- it declares integration settings for replay
- it interoperates with RWIF v2 additive schema

## 15. Suggested Next Steps

- Add executable conformance tests for the checklist above.
- Add a benchmark profile comparing legacy_scalar vs signed_i8_plus_intent_v2.
- Add a replay test that re-runs the same event stream and verifies byte-stable route audit outputs.

## 16. Operator-Phase Profile (Experimental)

This profile maps operations (not only results) into a phase trajectory to make
execution ordering geometrically auditable.

### 16.1 Canonical Operator Base Phases

Implementations MAY define canonical base phases for operators, for example:
- addition: `theta = 0.0`
- subtraction: `theta = pi/4`
- multiplication: `theta = pi/6`
- division: `theta = pi/2`
- division by zero: `theta = pi` with `stop_reason = contradiction_detected`

Implementations MUST publish the active operator map in engine configuration
when this profile is enabled.
### 16.2 Order-of-Operations Geometric Identity

For compound expressions, the engine SHOULD emit the composed operator-phase
trajectory as route audit metadata.

Two parse trees that differ only by precedence (for example `2 + 2 * 3` versus
`(2 + 2) * 3`) SHOULD yield distinct geometric trajectories unless reduced to an
equivalent canonical form by explicit rewrite rules.

### 16.3 Ambiguity Torsion

When multiple parse trees are valid, implementations SHOULD compute an ambiguity
torsion residual between path trajectories:

$$
r_{ambiguity} = \left|\mathrm{wrap}_{\pi}(\theta_{parseA} - \theta_{parseB})\right|
$$

The runtime SHOULD expose this as a graded warning signal (not only binary
error) when syntax remains legal but semantically under-specified.

## 17. Unit Crystal Profile (Implemented)

CSIF v2 now treats representation units as first-class geometric objects in
math evaluation outputs.

### 17.1 Unit Crystal Object

Implementations SHOULD emit a `unit_crystal` object containing:

- `unit_id`
- `node_type = "unit_crystal"`
- `representation_system`
- `phase_signature`
- `trajectory`

### 17.2 Conversion Morphism Chains

Implementations SHOULD emit deterministic `conversion_morphisms` chains.
Each chain SHOULD include:

- `source_unit`, `target_unit`, `morphism_type`
- ordered `hops`
- per-hop `phase_from`, `phase_to`, `phase_drift`
- per-hop confidence/torsion metadata
- `loop_metrics` with `loop_torsion_norm`

### 17.3 Threshold-Based Contradiction Signaling

Implementations SHOULD enforce a configurable loop torsion threshold for
conversion loop closure safety.

Default policy in this repository:

- `CSIF_UNIT_LOOP_TORSION_THRESHOLD` environment override
- fallback threshold = `0.2`

When `loop_torsion_norm` exceeds threshold, engines MUST emit explicit
contradiction signaling with stop reason:

- `unit_conversion_loop_torsion_exceeded`

### 17.4 Consistency Channel Propagation

Unit conversion contradictions MUST be propagated into the same contradiction
channel used by other engine conflicts:

- `bridge_audit.consistency.contradictions`

When propagated contradictions are non-empty, engines SHOULD mark:

- `bridge_audit.consistency.math_logic_alignment = Conflicted`

### 17.5 Final Outcome Lockstep

`final_outcome.machine_summary.contradiction_count` MUST equal the number of
propagated consistency contradictions for that response.

If contradiction_count is non-zero, responder status SHOULD be qualified and
responder text SHOULD carry contradiction qualification context.

## 18. Language/Lobe Geometry Profile (Experimental)

This profile extends CSIF routing to expression and sentence trajectories across
lobes/languages.

### 17.1 Cross-Lobe Resonance

Equivalent semantic structures encoded in different lobes/languages SHOULD be
comparable via geometric residual rather than string identity.

Implementations SHOULD report:
- intra-lobe residual
- cross-lobe residual
- calibration confidence

### 17.2 Soft Negation Continuum

Negation SHOULD be represented as a continuum in phase space rather than only
binary inversion, allowing intermediate hedge/soft-negation forms to occupy
measurable positions between coherent and anti-phase poles.

### 17.3 Experimental Metrics

Implementations MAY expose the following research metrics for language routing:
- synonym phase distance
- ambiguity torsion score
- metaphor resonance score
- setup/punchline surprise residual (humor-structure metric)

These metrics are advisory and MUST NOT weaken core safety gates in Sections 7,
8, and 13.

## 19. Logic Crystal Profile (Draft Contract)

This profile drafts first-class geometric logic objects and inference
trajectories using the same structural style as unit conversion morphism
chains.

### 19.1 logic_prop Crystal Schema

Implementations MAY represent propositions as crystals with class `logic_prop`.

Required fields:

- `logic_prop_id`: stable proposition identity key
- `node_type = "logic_prop"`
- `label`: proposition label or formula text
- `formula`: canonical expression string
- `phase_signature`:
	- `phase_theta`: context/model alignment phase
	- `resonance`: structural coherence in [0, 1]
	- `torsion_norm`: contradiction pressure in [0, 1+]
	- `context_frame_id`: model/frame signature
- `trajectory`: append-only logical evolution events
- `provenance`

Reference object skeleton:

```json
{
	"logic_prop_id": "prop.P",
	"node_type": "logic_prop",
	"label": "P",
	"formula": "P",
	"phase_signature": {
		"phase_theta": 0.0,
		"resonance": 1.0,
		"torsion_norm": 0.0,
		"context_frame_id": "frame.default"
	},
	"trajectory": [
		{
			"monotonic_index": 1,
			"op": "assume",
			"inputs": [],
			"output": "prop.P",
			"phase_theta": 0.0
		}
	],
	"provenance": {
		"source": "csif_logic_profile_v1"
	}
}
```

### 19.2 Connective Geometry Contract

Implementations MAY map core logical connectives to deterministic geometric
operators:

- `not(P)`: phase inversion/reflection under declared mapping.
- `and(P, Q)`: phase compatibility intersection.
- `or(P, Q)`: compatible trajectory union with torsion accumulation.
- `implies(P, Q)`: directed stabilization constraint edge.

Each connective SHOULD be emitted as a typed RWIF edge/event with:

- operator id
- input proposition ids
- output proposition id
- phase update and residual diagnostics

### 19.3 First Inference Contract: Modus Ponens

Modus Ponens trajectory contract:

- Inputs: `logic_prop(P)`, `logic_prop(P_implies_Q)`
- Rule: `infer_modus_ponens`
- Output: `logic_prop(Q)`

Trajectory chain SHOULD be emitted in morphism style:

```json
{
	"chain_id": "logic_mp_chain_v1",
	"source_unit": "prop.P",
	"target_unit": "prop.Q",
	"morphism_type": "inference_modus_ponens",
	"hops": [
		{
			"hop_index": 1,
			"source_unit": "prop.P",
			"target_unit": "prop.P_implies_Q",
			"transform": "implication_constraint_bind",
			"phase_from": 0.05,
			"phase_to": 0.08,
			"phase_drift": 0.03,
			"torsion_norm": 0.02,
			"confidence_band": 0.98
		},
		{
			"hop_index": 2,
			"source_unit": "prop.P_implies_Q",
			"target_unit": "prop.Q",
			"transform": "modus_ponens_fire",
			"phase_from": 0.08,
			"phase_to": 0.09,
			"phase_drift": 0.01,
			"torsion_norm": 0.03,
			"confidence_band": 0.97
		}
	],
	"loop_metrics": {
		"is_closed": false,
		"closure_target": null,
		"hop_count": 2,
		"loop_torsion": 0.0,
		"loop_torsion_norm": 0.0,
		"loop_resonance": 1.0,
		"exceeds_threshold": false
	}
}
```

### 19.4 Contradiction Gating for Inference

If inference torsion exceeds declared threshold, engines SHOULD:

- block inference commitment
- emit explicit stop reason
- propagate contradiction record into shared consistency channel

Suggested contradiction class for this profile:

- `logic_inference_torsion_exceeded`

### 19.5 Projection to Classical Truth

Classical truth labels SHOULD be emitted as deterministic projections from
logic crystal state rather than as primary hidden state.
