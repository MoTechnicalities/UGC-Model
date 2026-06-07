# UGC-Model

![UGC-Model Hero](assets/UGC-Model_Image.png)

## Research Summary

Unified Geometric Cognition (UGC) is a native Rust, deterministic, auditable classical runtime for selected quantum-analog benchmarks and structured reasoning tasks. Rather than attempting universal tensor-product simulation, UGC uses closed-form geometric phase reductions to reproduce the expected outcomes of Deutsch, Deutsch-Jozsa, Grover, and Simon style probes on commodity CPU hardware.

The project is useful because it is narrow, measurable, and reproducible:

- It exposes a Rust CLI and API surface with stable JSON outputs and replayable audit traces.
- It ships both Python and Rust benchmark paths so implementation overhead can be measured directly.
- It records wall-clock runtime, iteration counts, query ratios, memory estimates, and torsion/phase stability as first-class metrics.
- It treats scaling as an empirical boundary test, not as a claim of complexity-theory violation.

Current evidence highlights:

- Grover-style closed-form runs remain highly efficient on CPU up to n=50, with native Rust consistently outperforming the Python orchestration layer.
- Deterministic SHA-256-style reproducibility is preserved across repeated runs.
- The codebase is intentionally conservative: it does not claim physical qubit execution, arbitrary entangled-state simulation, or a universal quantum replacement.

For the full benchmark narrative and command log, see [docs/demo/cli-demo.md](docs/demo/cli-demo.md).

Unified Geometric Cognition (UGC) is a deterministic, auditable intelligence model
built on CSIF (Crystal Structure Information Format) and RWIF (Resonant Wave
Information Format).

UGC unifies math, logic, units, time, and contradiction geometry into a single
coherent geometric reasoning substrate designed for CPU-only execution.

## Overview

Unified Geometric Cognition (UGC) is a model architecture that evaluates
mathematical, logical, structural, and temporal expressions through deterministic
geometric transformations.

UGC is built on two foundational technologies:

### CSIF - Crystal Structure Information Format

CSIF is the representational ontology for:

- Crystals (valid structures)
- Anticrystals (invalid or contradictory structures)
- Units as first-class geometric objects
- Time crystals for deterministic chaotic behavior
- Phase, torsion, resonance, and trajectory
- Exact, auditable transformations

### RWIF - Resonant Wave Information Format

RWIF is the complete reasoning trace that records:

- Every operation
- Every phase update
- Every torsion spike
- Every contradiction event
- Every unit conversion
- Every inference step
- Every time-driven chaotic transformation

Together, CSIF and RWIF form the backbone of the UGC Model.

## Full Disclosure Specifications

For the complete organized specification set (CSIF, RWIF, Semantic Layers,
conformance, and implementation blueprints), see:

- [Specification Disclosure Index](docs/specifications/README.md)

## Specification Governance

- Canonical specification source: this UGC-Model repository is the canonical
	source for CSIF, RWIF, and Semantic Layer specification edits.
- Downstream mirrors: CSIF-Guard may carry synchronized copies for operational
	implementation reference, but canonical changes must originate here.
- Sync versioning: every cross-repo spec sync should be tagged in commit
	messages and release notes as `SPEC_SYNC_vYYYY.MM.DD.N` (for example,
	`SPEC_SYNC_v2026.05.31.1`).
- Sync scope: each sync must include an updated index/changelog summary listing
	files synced, source commit hash, destination commit hash, and effective sync
	version tag.

## Key Features

### OpenAI-Compatible API Surface

Drop-in OpenAI-style routes (`/v1/models`, `/v1/chat/completions`,
`/v1/embeddings`) plus deterministic CSIF-native endpoints for math,
retrieval, disambiguation, simulation, and reconciliation.

### CLI-First Accessibility

Direct command workflows for validation, migration, indexing, deterministic
math evaluation, benchmarking, and local OpenAI-compatible serving.

### Deterministic Replay and Auditability

Core operations produce replay-stable outputs with explicit audit traces so
results can be re-run and verified without ambiguity.

### Unified Geometric Reasoning Substrate

Math, logic, units, time, contradiction geometry, and semantic disambiguation
share one coherent CSIF/RWIF representation.

### Contradiction-Aware Governance

Contradictions are first-class objects with explicit threshold signaling,
propagation, and qualified outcomes rather than hidden failures.

### Multilingual Lexical Disambiguation

Deterministic token-to-sense resolution across multiple languages with
cross-language alias identity and pack-scoped lexicon evidence.

### Frame-Aware Semantics and Conservation Checks

Optional frame transitions and conservation policies provide deterministic
projection, admissibility checks, and explicit invariant-violation diagnostics.

### Sandbox Simulation and Reconciliation

Branch-level what-if simulation and winner-versus-loser reconciliation expose
why a trajectory wins and which alternatives are rejected.

### Trajectory Persistence and Health Metrics

Append-only sense trajectory logs support measurable semantic health signals,
including stability, contradiction rate, ambiguity entropy, and lobe drift.

### CPU-First, No-GPU Dependency

Designed for deterministic symbolic/geometric execution on standard CPU
infrastructure without requiring matrix-heavy GPU inference pipelines.

## Architecture

UGC is organized in three conceptual layers:

### 1) UGC Model (Mind Layer)

Defines reasoning rules, transformation policies, and contradiction handling.

### 2) CSIF (Representation Layer)

Defines crystals, anticrystals, edges, units, time crystals, and
phase/torsion/resonance fields.

### 3) RWIF (Audit Layer)

Captures deterministic, reproducible, inspectable, exportable execution traces.

## Example Concepts

### Crystals

Valid structures such as numbers, expressions, propositions, and units.

### Anticrystals

Contradictory or invalid states with full geometric traceability.

### Unit Crystals

Meters, seconds, radians, degrees, joules, and related units as geometric objects.

### Time Crystals

Deterministic chaotic drivers for auditable randomness appearance.

### Trajectories

Every evaluation step is a geometric path with phase, torsion, resonance, and
causal ordering.

## Goals of This Repository

- Provide a reference implementation of the UGC Model
- Define CSIF and RWIF specifications
- Offer examples, tests, and demonstrations
- Enable open-source collaboration on geometric cognition
- Establish foundations for deterministic, auditable AI reasoning

## Roadmap

### Phase 1 - Specification

- [ ] CSIF v1.0 schema
- [ ] RWIF v1.0 schema
- [ ] UGC Model definition
- [ ] Unit crystal specification
- [ ] Anticrystal lob specification

### Phase 2 - Reference Implementation

- [ ] Core crystal engine
- [ ] Phase/torsion/resonance propagation
- [ ] Unit crystal operations
- [ ] Time crystal integration
- [ ] RWIF trace generator

### Phase 3 - Demonstrations

- [ ] Math reasoning examples
- [ ] Logical inference examples
- [ ] Unit conversion examples
- [ ] Chaotic time-driven randomness examples
- [ ] Contradiction detection examples

## Contributing

Contributions are welcome. Please use the contributor workflow:

- [CONTRIBUTING.md](CONTRIBUTING.md)
- [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)
- [SECURITY.md](SECURITY.md)

## Contact

Maintainer: Mogir
Location: Grand Rapids, Michigan, USA

## Citation

If you use this model or its specifications in research or software, cite:

Unified Geometric Cognition (UGC) Model - CSIF/RWIF Architecture
Copyright (c) 2026 Mogir Jason Rofick

## License

This repository is currently licensed under Apache-2.0.

## Getting Started

From repository root:

```bash
cargo check --locked
cargo test --locked
cargo run -- serve-openai --port 8080
```

## Documentation Map

- CLI demo log: [docs/demo/cli-demo.md](docs/demo/cli-demo.md)
- High-level technical reference: [docs/TECHNICAL_REFERENCE.md](docs/TECHNICAL_REFERENCE.md)
- Formal finding (decimal semantics): [docs/FORMAL_FINDING_DECIMAL_SEMANTICS.md](docs/FORMAL_FINDING_DECIMAL_SEMANTICS.md)
- Full specifications index: [docs/specifications/README.md](docs/specifications/README.md)
- Release and sync discipline: [docs/RELEASE_DISCIPLINE.md](docs/RELEASE_DISCIPLINE.md)
- Change history: [CHANGELOG.md](CHANGELOG.md)

## ULT Regression Proof

These checks prove the ULT package is now deterministic, auditable, and CI-enforced:

- Reasoning regression validates `⊔`, `⊓`, `⊢`, contradiction, and `⨁` on fixed ULT pairs.
- Lexicon regression validates deterministic package building, canonical sorting, deduping, and audit hashes.
- Coverage gates validate that both seed languages, EN and ES, independently cover the current predicate inventory.
- The lexicon package is self-contained in JSON, with provenance and realization hashes exposed for auditability.

## Branch Protection Checklist

Require these checks before merge:

- `ULT Reasoning Regression`
- `ULT Lexicon Regression`
- `ULT Lexicon Coverage ES Gate`
- `ULT Lexicon Coverage EN Gate`
