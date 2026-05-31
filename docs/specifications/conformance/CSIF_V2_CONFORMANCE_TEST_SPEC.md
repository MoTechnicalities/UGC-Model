# CSIF v2 Conformance Test Specification

This document defines executable conformance tests for:
- [CSIF_V2_ENGINE_SPEC.md](../csif/CSIF_V2_ENGINE_SPEC.md)
- [RWIF_V2_FIELD_SPEC.md](../rwif/RWIF_V2_FIELD_SPEC.md)

It includes required core tests and optional experimental profile tests.

## 1. Test Objectives

A CSIF implementation passes conformance when it demonstrates:
- deterministic, byte-stable query outputs
- correct phase wrapping and reverse traversal identity
- correct integer overflow handling under configured policy
- explicit stop reasons and contradiction gating
- append-only RWIF-compatible event emission

## 2. Conformance Levels

- L1 Core Required: Sections 4 through 15 of CSIF v2 spec.
- L2 Experimental Operator-Phase: Section 16.
- L3 Experimental Language/Lobe Geometry: Section 18.
- L4 Unit Crystal Profile: Section 17.
- L5 Logic Crystal Profile: Section 19.

Release gate recommendation:
- Production: L1 REQUIRED.
- Research profile: L1 + selected L2/L3/L4/L5 tests.

## Appendix A. L4 Unit Crystal Profile Tests

These tests validate the implemented unit-crystal profile and contradiction
propagation behavior.

### T-401 Unit Crystal Emission

Spec: CSIF v2 Engine Spec 17.1

Procedure:
1. Execute numeric eval in algebraic and geometric mode.
2. Inspect output payload for `unit_crystal`.

Pass criteria:
- `unit_crystal` exists with `unit_id`, `representation_system`,
  `phase_signature`, and `trajectory`.

### T-402 Conversion Morphism Chain Integrity

Spec: CSIF v2 Engine Spec 17.2, RWIF v2 Field Spec 10.3

Procedure:
1. Execute numeric eval.
2. Inspect `unit_crystal.conversion_morphisms`.

Pass criteria:
- At least one chain exists with source/target units.
- Hop records include `phase_drift`.
- `loop_metrics.loop_torsion_norm` and `loop_metrics.loop_resonance` exist.

### T-403 Loop Threshold Contradiction Signaling

Spec: CSIF v2 Engine Spec 17.3

Procedure:
1. Run with low threshold policy (for example env threshold 0).
2. Execute numeric eval and inspect output.

Pass criteria:
- Contradiction code `unit_conversion_loop_torsion_exceeded` emitted.
- Stop reason equals `unit_conversion_loop_torsion_exceeded`.
- Unit contradiction signal marks `triggered=true`.

### T-404 Bridge Consistency Propagation

Spec: CSIF v2 Engine Spec 17.4

Procedure:
1. Trigger unit conversion contradiction as in T-403.
2. Inspect `bridge_audit.consistency`.

Pass criteria:
- `bridge_audit.consistency.contradictions` contains propagated unit
  contradiction records.
- `bridge_audit.consistency.math_logic_alignment = Conflicted`.

### T-405 Final Outcome Lockstep

Spec: CSIF v2 Engine Spec 17.5

Procedure:
1. Trigger propagated contradictions as in T-403.
2. Inspect `bridge_audit.final_outcome`.

Pass criteria:
- `machine_summary.contradiction_count` equals propagated contradiction count.
- Final status is qualified when contradiction_count > 0.
- Final responder text includes contradiction qualification marker.

## Appendix B. L5 Logic Crystal Profile Tests

These tests validate the draft logic crystal and Modus Ponens trajectory
contract.

### T-501 logic_prop Schema Emission

Spec: CSIF v2 Engine Spec 19.1

Procedure:
1. Create or load proposition object.
2. Inspect emitted logic crystal payload.

Pass criteria:
- `node_type = logic_prop` is present.
- `phase_signature` contains `phase_theta`, `resonance`, `torsion_norm`.
- `trajectory` is append-only and monotonic.

### T-502 Connective Geometry Emission

Spec: CSIF v2 Engine Spec 19.2

Procedure:
1. Evaluate at least one connective case for `not`, `and`, `or`, `implies`.
2. Inspect emitted connective edge/event diagnostics.

Pass criteria:
- Connective operator id and input/output proposition ids are present.
- Phase update diagnostics are deterministic under replay.

### T-503 Modus Ponens Trajectory Contract

Spec: CSIF v2 Engine Spec 19.3

Procedure:
1. Provide `P` and `P -> Q` premises.
2. Trigger inference.
3. Inspect emitted inference morphism chain.

Pass criteria:
- `morphism_type = inference_modus_ponens`.
- Hop list includes implication bind and inference fire phases.
- Output proposition is `Q` with deterministic phase/torsion fields.

### T-504 Inference Contradiction Gating

Spec: CSIF v2 Engine Spec 19.4

Procedure:
1. Configure low inference torsion threshold.
2. Trigger inconsistent inference scenario.

Pass criteria:
- Stop reason `logic_inference_torsion_exceeded` is emitted.
- Contradiction propagated into shared consistency contradiction list.

### T-505 Truth Projection Consistency

Spec: CSIF v2 Engine Spec 19.5

Procedure:
1. Evaluate same proposition state repeatedly under fixed config.
2. Compare projected truth outputs.

Pass criteria:
- Truth labels remain deterministic projections from logic crystal state.
- No hidden nondeterministic truth path bypasses geometric state.

## 3. Canonical Test Output Contract

Each test case MUST emit a machine-readable result object with:
- test_id
- spec_section
- pass (bool)
- observed
- expected
- notes

A full run MUST emit:
- total
- passed
- failed
- conformance_level

## 4. L1 Core Required Tests

### T-001 Determinism Replay

Spec: 4.1, 4.3, 14

Procedure:
1. Load fixed graph and config.
2. Execute same query N times (N >= 100).
3. Canonically encode query result bytes each run.
4. Compare hashes.

Pass criteria:
- All hashes identical.

### T-002 wrap_pi Principal Interval

Spec: 5.1

Procedure:
1. Evaluate wrap_pi for boundary and overflow angles:
- -4pi, -3pi, -2pi, -pi, 0, pi, 2pi, 3pi, 4pi
2. Verify output bounded to [-pi, pi).

Pass criteria:
- All values inside principal domain.
- Known boundary expectations satisfied.

### T-003 Reverse Traversal Identity

Spec: 6.2, 8

Procedure:
1. Sample edge phases theta_i.
2. Compute reverse phase r_i = wrap_pi(-theta_i).
3. Verify wrap_pi(theta_i + r_i) == 0 under final rounding policy.

Pass criteria:
- Residual equals configured zero tolerance.

### T-004 Integer Wrap Mode Clamp

Spec: 5.3, 9

Procedure:
1. Run signed-state composition with integer_wrap_mode=clamp.
2. Inject over/underflow updates beyond range.

Pass criteria:
- Values saturate to configured min/max.
- No panic/overflow abort.

### T-005 Integer Wrap Mode Overflow Modulo

Spec: 5.3, 9

Procedure:
1. Run signed-state composition with integer_wrap_mode=overflow_modulo.
2. Inject over/underflow updates beyond range.

Pass criteria:
- Values wrap modulo configured signed domain.
- No panic/overflow abort.

### T-006 Stop Reason Completeness

Spec: 8, 12, 14

Procedure:
1. Execute scenario set covering:
- path found
- no path
- anti-lobe match
- contradiction
- timeout/budget
2. Verify route audit stop_reason.

Pass criteria:
- Correct stop reason per scenario.
- No missing stop_reason.

### T-007 Contradiction Threshold Gate

Spec: 7.1, 7.2, 7.3

Procedure:
1. Construct multi-path pair with known residual below threshold.
2. Construct multi-path pair above threshold.

Pass criteria:
- Below threshold: no contradiction gate.
- Above threshold: contradiction_detected.

### T-008 RWIF Event Mapping Integrity

Spec: 10, 13

Procedure:
1. Execute query/update cycle.
2. Persist trajectory events.
3. Validate required field mapping:
- phase, confidence_band, drift_delta, event_type, source
4. Validate optional v2 state fields are present when enabled.

Pass criteria:
- Required mapping complete.
- No destructive mutation of prior events.

## 5. L2 Experimental Operator-Phase Tests

### T-101 Operator Map Publication

Spec: 16.1

Procedure:
1. Enable operator-phase profile.
2. Read engine config surface.

Pass criteria:
- Active operator phase map present and explicit.

### T-102 Precedence Trajectory Separation

Spec: 16.2

Procedure:
1. Parse/execute `2 + 2 * 3`.
2. Parse/execute `(2 + 2) * 3`.
3. Compare operator-phase trajectories.

Pass criteria:
- Distinct trajectories OR explicit canonical rewrite justification.

### T-103 Ambiguity Torsion Score

Spec: 16.3

Procedure:
1. Select expression/sentence with >=2 valid parses.
2. Compute r_ambiguity between parse trajectories.

Pass criteria:
- Non-zero graded ambiguity signal emitted.
- Warning includes residual magnitude.

## 6. L3 Experimental Language/Lobe Tests

### T-201 Cross-Lobe Structural Equivalence

Spec: 17.1

Procedure:
1. Encode semantically equivalent statements in >=2 lobes/languages.
2. Compute intra-lobe and cross-lobe residuals.

Pass criteria:
- Cross-lobe residual below configured equivalence threshold.
- Calibration confidence reported.

### T-202 Soft Negation Ordering

Spec: 17.2

Procedure:
1. Evaluate phrase set from positive to negative continuum.
2. Compare phase positions/residual ranking.

Pass criteria:
- Ordering follows configured soft-negation monotonic direction.

### T-203 Experimental Metric Emission

Spec: 17.3

Procedure:
1. Run synonym, ambiguity, metaphor, humor benchmark set.
2. Verify metric object outputs.

Pass criteria:
- Metric fields emitted with deterministic schema.
- Advisory metrics do not override core safety gates.

## 7. Recommended Test Artifacts

- `fixtures/core_graph.json`
- `fixtures/contradiction_cases.json`
- `fixtures/operator_phase_cases.json`
- `fixtures/language_cases.json`
- `results/conformance_summary.json`

## 8. CI Gate Policy

Recommended CI blocking rules:
- FAIL build if any L1 test fails.
- Warn (non-blocking) for L2/L3 unless explicitly promoted.

## 9. Minimal Runner Interface (Pseudo)

```text
run_conformance --level L1 --fixtures fixtures/ --out results/conformance_summary.json
run_conformance --level L2 --fixtures fixtures/ --out results/conformance_summary_l2.json
run_conformance --level L3 --fixtures fixtures/ --out results/conformance_summary_l3.json
```

## 10. Conformance Verdict Format

```json
{
  "suite": "csif_v2_conformance",
  "level": "L1",
  "total": 8,
  "passed": 8,
  "failed": 0,
  "verdict": "PASS",
  "tests": []
}
```
