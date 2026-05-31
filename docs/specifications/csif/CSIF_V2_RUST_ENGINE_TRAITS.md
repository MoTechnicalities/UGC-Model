# CSIF v2 Rust Engine Trait Definitions

This document provides concrete Rust trait/interface definitions for implementing CSIF v2 routing deterministically.

Spec alignment:
- [CSIF_V2_ENGINE_SPEC.md](CSIF_V2_ENGINE_SPEC.md)
- [RWIF_V2_FIELD_SPEC.md](RWIF_V2_FIELD_SPEC.md)

## 1. Design Goals

The trait surface is built to satisfy:
- deterministic output under fixed input/config
- explicit numeric policy (float-strict or fixed-point)
- explicit phase wrapping and reverse traversal rules
- explicit stop reasons and route audit output
- replay-safe, byte-stable result encoding

## 2. Core Data Types (Rust)

```rust
use core::fmt::Debug;
use std::borrow::Cow;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StopReason {
    PathFound,
    NoSupportingPath,
    AntiLobeNegativeMatch,
    ContradictionDetected,
    TimeoutOrBudget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntegerWrapMode {
    Clamp,
    OverflowModulo,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngineMode {
    PhaseScalarV1,
    SignedI8PlusIntentV2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SignedState {
    pub amplitude_signed: i16,
    pub intent_signed: i16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteStep {
    pub edge_id: String,
    pub direction_forward: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteAudit {
    pub stop_reason: StopReason,
    pub selected_path: Vec<RouteStep>,
    pub considered_paths: Vec<Vec<RouteStep>>,
    pub contradiction_residual_quantized: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QueryResult {
        /// Keep heap allocation optional for hot-path labels.
        pub answer: Cow<'static, str>,
        pub decision_label: Cow<'static, str>,
    pub route_audit: RouteAudit,
}
```

Design note:
- `SignedState` remains `i16` to allow safe intermediate accumulation before
    applying the configured integer wrap policy back into the declared runtime
    domain (for example signed i8 bounds).

## 3. Deterministic Numeric Policy Trait

This trait isolates all nontrivial math behind a deterministic contract.

```rust
pub trait DeterministicNumericPolicy: Send + Sync + Debug {
    type Scalar: Copy + Debug + PartialEq;

    /// Canonical phase wrap to principal domain.
    fn wrap_pi(&self, theta: Self::Scalar) -> Self::Scalar;

    /// Compose phase along one directed edge.
    fn compose_phase(&self, lhs: Self::Scalar, rhs: Self::Scalar) -> Self::Scalar;

    /// Reverse relation phase: wrap_pi(-theta).
    fn reverse_phase(&self, theta: Self::Scalar) -> Self::Scalar;

    /// Absolute residual metric for contradiction checks.
    fn phase_residual_abs(&self, a: Self::Scalar, b: Self::Scalar) -> Self::Scalar;

    /// Quantize scalar to deterministic integer for byte-stable audit output.
    fn quantize_scalar(&self, x: Self::Scalar) -> i64;

    /// Optional signed-state composition for v2 mode.
    fn compose_signed_state(
        &self,
        current: SignedState,
        delta: SignedState,
        integer_wrap_mode: IntegerWrapMode,
    ) -> SignedState;
}
```

## 4. Contradiction Threshold Trait

```rust
pub trait ThresholdPolicy<S>: Send + Sync + Debug {
    /// T_alarm = pi/2 + c * sigma_path (or equivalent deterministic form).
    fn alarm_threshold(&self, sigma_path: S) -> S;

    fn is_contradiction(&self, residual_abs: S, sigma_path: S) -> bool;
}
```

## 5. Graph Access Trait

```rust
pub trait CsifGraphView: Send + Sync + Debug {
    type NodeId: Clone + Debug + Eq + core::hash::Hash;
    type EdgeId: Clone + Debug + Eq + core::hash::Hash;
    type NeighborIter<'a>: Iterator<Item = Self::EdgeId> + 'a where Self: 'a;

    /// Allocation-free neighbor traversal for routing hot paths.
    fn neighbors<'a>(&'a self, node: &Self::NodeId) -> Self::NeighborIter<'a>;
    fn edge_endpoints(&self, edge: &Self::EdgeId) -> (Self::NodeId, Self::NodeId);

    /// Base phase for this edge in forward direction.
    fn edge_phase(&self, edge: &Self::EdgeId) -> i64;

    /// Optional signed-state delta in quantized integer domain.
    fn edge_signed_delta(&self, edge: &Self::EdgeId) -> Option<SignedState>;

    fn edge_is_negative_match(&self, edge: &Self::EdgeId) -> bool;
}
```

## 6. Router Trait (Primary Interface)

```rust
#[derive(Clone, Debug)]
pub struct RouterConfig {
    pub mode: EngineMode,
    pub integer_wrap_mode: IntegerWrapMode,
    pub budget_max_steps: usize,
    pub quantization_step: i64,
}

pub trait CsifRouter: Send + Sync + Debug {
    type NodeId: Clone + Debug + Eq + core::hash::Hash;

    fn config(&self) -> &RouterConfig;

    fn query(
        &self,
        source: &Self::NodeId,
        target: &Self::NodeId,
    ) -> QueryResult;
}
```

## 7. Replay/Determinism Trait

This isolates byte-stable serialization for conformance tests.

```rust
pub trait DeterministicEncoder: Send + Sync + Debug {
    /// Deterministic canonical bytes (field order fixed, no map-order dependence).
    fn encode_query_result(&self, result: &QueryResult) -> Vec<u8>;

    fn hash_bytes(&self, bytes: &[u8]) -> [u8; 32];
}
```

## 8. Required Invariants for Implementers

1. Phase principal wrapping:
- `reverse_phase(theta) == wrap_pi(-theta)`

2. Round-trip identity:
- `wrap_pi(theta + reverse_phase(theta)) == 0` after one final rounding pass

3. Signed-state integer behavior:
- all amplitude/intent updates MUST use configured `integer_wrap_mode`
- no implicit panic-on-overflow behavior in hot path

4. Determinism:
- same graph + config + query MUST produce byte-identical encoded results

## 9. Recommended Rust Implementation Profiles

### Profile A: Strict Fixed-Point (Preferred for Cross-CPU Byte Stability)

- represent phase as fixed-point i64
- implement `wrap_pi` in integer domain
- quantize once per edge or once per composed path per policy

### Profile B: Strict Float with Deterministic Constraints

- explicit deterministic software math path
- disallow non-deterministic optimizations in release profile
- always quantize before audit/result encoding

## 10. Conformance Test Matrix (Minimum)

1. Determinism replay:
- run same query 100x, compare encoded hashes

2. Reverse traversal identity:
- random edge phases satisfy round-trip to quantized zero

3. Integer overflow mode:
- clamp mode saturates boundaries
- overflow_modulo mode wraps correctly

4. Stop reason contract:
- each terminal path emits one canonical stop reason

5. Contradiction gate:
- injected conflicting multi-path case yields contradiction_detected

6. Allocation profile (recommended):
- neighbor traversal executes without per-hop heap allocation
- static labels (`Cow::Borrowed`) avoid allocator churn for common decisions

## 11. Integration with RWIF v2

When writing trajectory events, map router output to RWIF fields:
- `phase`
- `confidence_band`
- `drift_delta`
- `event_type`
- `source`
- `amplitude_signed`
- `intent_signed`
- `phase_theta` / `phase_omega` (if used)
- `monotonic_index`

Use edge metadata values from RWIF as runtime defaults:
- `state_encoding`
- `numeric_range`
- `wrap_mode`
- `integer_wrap_mode`
- `integration_rule`
