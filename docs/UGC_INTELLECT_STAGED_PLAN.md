# UGC Intellect Staged Implementation Plan

## Goal
Build a layered intellect around the existing deterministic math engine, prioritizing safe capability growth over core arithmetic changes.

## v1 (Smallest, Immediate)
Status: in progress

Scope:
- Add a symbolic equation orchestrator that recognizes canonical equation families and returns deterministic structured outputs.
- Keep core numeric evaluator unchanged.
- Ensure both chat and /v1/csif/math can route recognized symbolic templates.

Ship Items:
- Family recognizers for:
  - Crystal Harmonic Constraint: nabla^2(phi) = k^2 phi
  - Dipole Flux Balance: nabla · F = rho_torsion
  - Resonance Eigenmode: omega_n = n*pi*c/L
  - Anticrystal Inversion: A^{-1}(x) = -A(x)
  - Unit-Time Coupling: dU/dt = alpha*U*(1-U)
- Structured payload contract:
  - family id/label
  - canonical form
  - assumptions
  - solution family
  - verification contract
  - deterministic marker
- Regression tests for all five equation families.
- Chat test for Unicode symbolic prompt routing.

Acceptance Criteria:
- All five templates return deterministic symbolic payloads.
- Existing math tests stay green.
- Chat no longer falls through on supported Unicode symbolic equation prompts.

## v2 (Medium)
Status: planned

Scope:
- Add constraint and assumption management, plus numeric/structural verification hooks.

Ship Items:
- Assumption resolver:
  - boundary condition placeholders
  - domain constraints (positivity, integrality, smoothness)
- Verification module:
  - substitution residual checks
  - dimensional/unit consistency checks (where applicable)
- Confidence stratification:
  - template_matched
  - verified_symbolic
  - underdetermined
- Enhanced response contract with unresolved fields and required inputs.

Acceptance Criteria:
- Symbolic outputs explicitly report underdetermined dimensions.
- Verification status and residual diagnostics present when checks are available.
- No false certainty for unconstrained PDE/field problems.

## v3 (Largest)
Status: planned

Scope:
- Multi-tool intellect arbitration and persistent learning from solved traces.

Ship Items:
- Tool arbitration pipeline:
  - symbolic transform lane
  - numeric simulation lane
  - contradiction arbitration lane
- Long-term solver memory:
  - solved pattern cache
  - failure mode catalog
  - replay tests generated from real misses
- Adaptive orchestrator policy:
  - route by confidence and required precision
  - retry with alternate derivation strategy when mismatches appear

Acceptance Criteria:
- End-to-end solve quality increases on held-out symbolic + numeric benchmark prompts.
- Regression packs auto-grow from failed traces.
- Deterministic contract remains stable and auditable.

## Implementation Order
1. Land v1 symbolic template recognizers and payload schema.
2. Add v1 tests and stabilize.
3. Add v2 assumption/verification envelope.
4. Add v3 arbitration + memory-backed improvement loop.
