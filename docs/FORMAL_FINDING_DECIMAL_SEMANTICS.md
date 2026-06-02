# Formal Finding: Decimal Truth Requires Decimal/Geometric Semantics

Date: 2026-06-01  
Status: Adopted (Chat reasoning path)

## Executive Finding

Binary IEEE-754 floating-point is an approximation substrate, not a truth substrate for decimal intent.
When user intent is decimal mathematics, truth evaluation must be performed in a decimal-consistent geometric representation rather than binary approximation space.

UGC treats decimal user input as decimal intent, evaluates in decimal/geometric semantics, and only uses binary trace mode when explicitly requested for hardware-faithful diagnostics.

## Problem Class

For literals such as 0.1, 0.2, and 0.3:

- These values are finite in base-10 intent but repeating in base-2 representation.
- Binary hardware computes with nearby representable values, not exact decimal literals.
- Equality outcomes can reflect representation artifacts rather than mathematical intent.

This creates semantic mismatch between:

- User proposition intent (decimal quantity identity)
- Hardware execution substrate (binary approximation identity)

## Formal Interpretation

UGC distinguishes two valid but different evaluation contracts:

1. Algebraic/Binary Trace Contract
- Purpose: preserve IEEE-754 traceability and exact hardware-level behavior.
- Identity meaning: binary representational equality.

2. Geometric Decimal Contract
- Purpose: preserve decimal-intent quantity identity.
- Identity meaning: exact decimal coordinate equivalence.

For decimal-intent chat reasoning, UGC adopts the Geometric Decimal Contract.

## Adopted Repository Policy (Chat Reasoning)

For conversational math and logic prompts in chat:

- Equality/inequality statements are evaluated under geometric decimal semantics.
- Decimal literals are interpreted as exact decimal coordinates.
- Equality is determined by exact decimal identity (not approximate epsilon tolerance).
- Operator-normalized conversational forms (for example, =, ==, !=, and Unicode variants) are mapped into deterministic parse-safe forms before evaluation.

### Restated Intent Contract

| Expression | UGC Interpretation |
| --- | --- |
| `0.1` | Decimal 0.1 (exact intent) |
| `0.2` | Decimal 0.2 (exact intent) |
| `0.1 + 0.2` | Decimal 0.3 (exact intent result) |
| `0.30000000000000004` | A different decimal literal, not 0.3 |

The engine does not reinterpret decimal intent as binary-equality truth in this policy path.

## Canonical Decimal Technicality Cases

Under geometric decimal semantics, the following results are normative:

- 0.1 + 0.2 = 0.3 -> true
- 0.1 + 0.2 = 0.30000000000000004 -> false
- 0.30000000000000004 = 0.3 -> false

Rationale:

- 0.30000000000000004 is a binary artifact literal, not the same decimal coordinate as 0.3.
- Decimal-intent equality compares decimal coordinates, not binary approximation neighborhoods.

Equivalent compact form:

- 0.1 (decimal) + 0.2 (decimal) = 0.3 (decimal)
- 0.3 (decimal) == 0.30000000000000004 (decimal) = false

Comparison reference:

| Comparison | Result | Why |
| --- | --- | --- |
| `0.3 = 0.3` | `true` | Same decimal number |
| `0.3 = 0.30000000000000004` | `false` | Different decimal numbers |

## Why This Is Structural, Not Cosmetic

This is not output formatting. It is substrate selection for truth evaluation.

- Binary trace mode remains available for hardware-faithful diagnostics.
- Geometric decimal mode is used when the proposition itself is decimal-intent mathematics.

Documentation doctrine sentence:

"UGC recognizes decimal input as decimal. 0.1 + 0.2 = 0.3, exactly. And 0.3 is not 0.30000000000000004, because that is a different decimal number."

## Scope and Practical Impact

This finding applies to any system where user intent is decimal mathematics but execution is binary floating point:

- finance and accounting
- simulation accumulators
- engineering tolerances
- data pipelines with repeated arithmetic composition

UGC position: representation mismatch must be resolved at evaluation semantics, not patched after the fact.

## Verification

Regression coverage in the Rust test suite locks this contract for chat reasoning behavior.
Benchmark and CI gates must preserve deterministic outputs for these decimal technicality cases.
