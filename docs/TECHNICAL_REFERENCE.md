# UGC-Model Technical Reference

This document contains the detailed runtime/API/contract reference previously embedded in the root README.

## Current Commands


- `validate-bank <bank_path>`
- `migrate-bank <input_path> <output_path>`
- `index-bank <bank_path> <output_path>`
- `math-eval [--mode algebraic|geometric] [--angle-unit radians|degrees] <expression>`
- `benchmark-determinism [--iterations 100]`
- `serve-openai [--host 127.0.0.1] [--port 8080] [--bank-path /path/to/bank.json] [--sense-log-path /path/to/sense_trajectories.jsonl]`

## Math Output Contract (CSIF Upgrade)

`math-eval` now emits explicit policy and contradiction semantics for numeric reasoning:

- `reasoning_policy.numeric_determination`
	- `ieee754_binary_trace` in algebraic mode
	- `geometric_decimal_scaling` in geometric mode
- `unit_crystal` first-class object with:
	- `phase_signature`
	- `trajectory`
	- `conversion_morphisms` chain records (`source_unit -> target_unit` hops)
	- per-hop `phase_drift`
	- per-chain `loop_metrics` (`loop_torsion`, `loop_torsion_norm`, `loop_resonance`)
- `unit_contradiction_signal` top-level summary for unit-loop threshold checks
- `logic_contradiction_signal` top-level summary for logic-inference threshold checks
- `anticrystal_lob` first-class contradiction lobe for explicit negative memory:
	- `entries[*].contradiction_id`
	- `entries[*].this_is_wrong_because`
	- `entries[*].severity`
- optional time-chaos profile (opt-in):
	- `time_crystal` (`t_ns`, `phase_theta`, `torsion_norm`)
	- `randomness_appearance` (`mode`, `replay_key`, `audit_trace_id`)
- `bridge_audit.consistency.contradictions` includes propagated unit loop threshold breaches
- `final_outcome.machine_summary.contradiction_count` is kept in lockstep with propagated contradiction count
- `final_outcome.responder_text` is contradiction-qualified when contradictions are present

### Unit Loop Threshold Policy

The unit conversion loop contradiction threshold is configurable by environment variable:

- `CSIF_UNIT_LOOP_TORSION_THRESHOLD`

Default threshold when unset or invalid:

- `0.2` (normalized by `pi`)

When a loop exceeds threshold, the payload includes explicit contradiction signaling:

- contradiction code: `unit_conversion_loop_torsion_exceeded`
- stop reason: `unit_conversion_loop_torsion_exceeded`
- qualified final responder text and status

### Logic Inference Threshold Policy

The logic inference contradiction threshold is configurable by environment variable:

- `CSIF_LOGIC_INFERENCE_TORSION_THRESHOLD`

Default threshold when unset or invalid:

- `0.2` (normalized by `pi`)

When a logic inference chain exceeds threshold, the payload includes explicit
contradiction signaling and bridge propagation:

- contradiction code: `logic_inference_torsion_exceeded`
- stop reason: `logic_inference_torsion_exceeded`
- propagated contradiction IDs in `bridge_audit.consistency.contradictions`

### Time-Crystal Randomness Appearance (Optional)

Enable optional time-crystal output fields for auditable randomness appearance:

- `CSIF_EMIT_TIME_CRYSTAL_RANDOMNESS=true`

Optional deterministic replay override for testing/replay:

- `CSIF_TIME_CRYSTAL_T_NS=<fixed_nanosecond_coordinate>`

When enabled, `math-eval` emits:

- `time_crystal` with deterministic time-coordinate geometry metadata
- `randomness_appearance` with deterministic `replay_key` and `audit_trace_id`

## Logic Crystal Implementation Checklist (Build Gating)

This checklist maps CSIF v2 Engine Spec Section 19 clauses to concrete output
keys and conformance test IDs.

Source spec anchors:

- Logic profile: [CSIF_V2_ENGINE_SPEC.md](docs/specifications/csif/CSIF_V2_ENGINE_SPEC.md#19-logic-crystal-profile-draft-contract)
- Conformance appendix: [CSIF_V2_CONFORMANCE_TEST_SPEC.md](docs/specifications/conformance/CSIF_V2_CONFORMANCE_TEST_SPEC.md#appendix-b-l5-logic-crystal-profile-tests)

Build gate recommendation:

- Consider Section 19 as L5 profile gates.
- Mark a clause as build-pass only when all mapped keys are emitted and the
	mapped test ID passes.

| Clause | Required Output Keys (math-eval or bridge payload) | Conformance Test ID | Gate Condition |
| --- | --- | --- | --- |
| 19.1 logic_prop Crystal Schema | `logic_crystal.logic_prop_id`, `logic_crystal.node_type`, `logic_crystal.phase_signature.phase_theta`, `logic_crystal.phase_signature.resonance`, `logic_crystal.phase_signature.torsion_norm`, `logic_crystal.trajectory` | `T-501` | Pass when all keys exist with deterministic values under replay |
| 19.2 Connective Geometry Contract | `logic_connectives[*].operator`, `logic_connectives[*].inputs`, `logic_connectives[*].output`, `logic_connectives[*].phase_update`, `logic_connectives[*].residual` | `T-502` | Pass when each connective emits deterministic geometry diagnostics |
| 19.3 Modus Ponens Trajectory Contract | `inference_morphisms[*].chain_id`, `inference_morphisms[*].morphism_type=inference_modus_ponens`, `inference_morphisms[*].hops[*].phase_drift`, `inference_morphisms[*].hops[*].torsion_norm`, `inference_morphisms[*].target_unit` | `T-503` | Pass when MP chain emits expected hop structure and stable target proposition |
| 19.4 Inference Contradiction Gating | `logic_contradiction_signal.triggered`, `logic_contradiction_signal.stop_reason`, `bridge_audit.consistency.contradictions[*].contradiction_id` | `T-504` | Pass when threshold breach blocks commit and propagates contradiction |
| 19.5 Classical Truth Projection | `result` (truth projection), `bridge_audit.final_outcome.machine_summary.final_truth`, `bridge_audit.final_outcome.status` | `T-505` | Pass when truth labels are deterministic projections from logic crystal state |

### CI Wiring Suggestion

Use the test IDs above as named gates in CI reports:

- `L5-T-501`, `L5-T-502`, `L5-T-503`, `L5-T-504`, `L5-T-505`

Promote L5 to blocking only after all output keys above are implemented in
runtime payloads.

Math mode supports calculator-style syntax inspired by the CSIF scientific calculator demo:

- Real and complex literals, including `i`
- Implicit multiplication such as `3i` and `2(3+4)`
- Postfix factorial, such as `5!`
- Scientific functions: `sin`, `cos`, `tan`, `atan2`, `sinh`, `cosh`, `tanh`, `asinh`, `acosh`, `atanh`, `sqrt`, `ln`, `log`, `exp`, `gamma`, `zeta`, `lambertw`, `polylog`, `gammainc`, `j_sph`, `abs`, `arg`, `conj`, `comb`/`C`, `J0`, `J1`, `J2`, `J3`
- Constants: `pi`, `tau`, `e`
- Geometric mode with explicit angle units for trig: `--mode geometric --angle-unit degrees`

Quick reference:

| Kind | Syntax | Notes |
| --- | --- | --- |
| Add / subtract | `+`, `-` | Binary operators |
| Multiply / divide | `*`, `/` | Binary operators |
| Power | `^` | Right-associative |
| Factorial | `!` | Postfix, real integers only |
| Parentheses | `(`, `)` | Grouping |
| Imaginary unit | `i` | Complex literal support |
| Constants | `pi`, `tau`, `e` | Built-in constants |
| Trig | `sin(x)`, `cos(x)`, `tan(x)` | Use `--mode geometric` for degree/radian-aware traces |
| Two-arg trig | `atan2(y, x)` | Deterministic quadrant-aware arctangent |
| Hyperbolic | `sinh(x)`, `cosh(x)`, `tanh(x)`, `asinh(x)`, `acosh(x)`, `atanh(x)` | Deterministic real/complex-compatible function support |
| Special functions | `gamma(x)`, `zeta(s)`, `lambertw(x)`, `polylog(s, z)`, `gammainc(a, z)`, `j_sph(n, z)` | Deterministic complex gamma, zeta (analytic continuation with pole at s=1), principal-branch Lambert W, polylog for `|z| < 1`, lower incomplete gamma, and spherical Bessel |
| Combinatorics | `comb(n, k)`, `C(n, k)` | Integer-domain binomial coefficient |
| Bessel (first kind) | `J0(x)`, `J1(x)`, `J2(x)`, `J3(x)` | Deterministic series approximation with complex input support |
| Roots / logs | `sqrt(x)`, `ln(x)`, `log(x)`, `exp(x)` | `log` is base-10 style in the calculator demo |
| Complex ops | `abs(z)`, `arg(z)`, `conj(z)` | Complex-friendly calculator functions |

## Run

```bash
cargo run -- validate-bank /path/to/bank.json
cargo run -- migrate-bank /path/to/input.json /path/to/output.json
cargo run -- index-bank /path/to/rwif_v2_bank.json /path/to/index.json
cargo run -- math-eval '2*(3+4)^2'
cargo run -- math-eval --mode geometric --angle-unit degrees 'sin(30) + cos(60)'
cargo run -- benchmark-determinism --iterations 100
cargo run -- math-eval '(2+3i)^2 + conj(4-5i) + arg(1+i) + 5!'
cargo run -- math-eval 'exp(i*pi) + 1'
cargo run -- math-eval --mode geometric '-0.1i - -0.2'
CSIF_UNIT_LOOP_TORSION_THRESHOLD=0 cargo run -- math-eval --mode geometric '-0.1i - -0.2'
cargo run -- serve-openai --port 8080 --bank-path /path/to/bank.json
```

OpenAI-compatible routes:

- `GET /health`
- `GET /v1/models`
- `POST /v1/chat/completions`
- `POST /v1/embeddings`
- `GET /v1/csif/index`
- `POST /v1/csif/retrieve`
- `POST /v1/csif/math`
- `POST /v1/csif/disambiguate`
- `POST /v1/csif/simulate`
- `POST /v1/csif/reconcile`
- `GET /v1/csif/disambiguation/trajectories`
- `GET /v1/csif/disambiguation/summary`

Chat behavior notes:

- `POST /v1/chat/completions` uses the provided message history and responds with contextual conversation output.
- Chat replies now include deterministic intent-aware phrasing (`greeting`, `identity`, `help`, `troubleshooting`, `math`, `general`) for more natural user interaction.
- Decimal truth semantics for chat reasoning are documented as a formal adopted finding: [FORMAL_FINDING_DECIMAL_SEMANTICS.md](FORMAL_FINDING_DECIMAL_SEMANTICS.md).
- Chat replies support deterministic style adaptation (`concise` vs `standard`) based on user wording such as `brief` or `concise`.
- Greeting openings now support time-crystal-driven variant selection so introduction responses feel less repetitive while remaining auditable/replayable.
- The same time-crystal variation pattern now applies to the no-retrieval fallback sentence and the closing `Next options` heading.
- Conversation metadata includes:
	- `csif_meta.conversation.opening_randomness`
	- `csif_meta.conversation.retrieval_fallback_randomness`
	- `csif_meta.conversation.next_options_randomness`
  Each includes selected variant information and time-crystal fields when enabled.
- Greeting variation controls:
	- `CSIF_CHAT_TIME_CRYSTAL_OPENING_VARIATION` (default enabled; set `false` to disable)
	- `CSIF_CHAT_GREETING_WARMTH_CEILING` (`subtle|balanced|expressive`, default `balanced`)
	- `CSIF_TIME_CRYSTAL_T_NS` (optional fixed coordinate for deterministic replay)
- `POST /v1/chat/completions` accepts optional `preferences` payload fields:
	- `response_style`: `concise | standard`
	- `depth`: `shallow | standard | deep`
	- `tone`: `friendly | professional | direct`
	- `warmth_ceiling`: `subtle | balanced | expressive` (caps greeting variation intensity)
	- `retrieval_summary`: boolean toggle for natural-language evidence summary
	- `retrieval_top_k`: retrieval evidence count cap for chat synthesis (`1..12`)
- Chat responses include RWIF retrieval evidence when prompt terms match indexed bank content.
- Retrieval metadata includes deterministic rewrite diagnostics (`rewritten_query`, rewrite reasons, and miss diagnostics when no hits are found).
- Retrieval metadata now includes deterministic readability scoring under `csif_meta.retrieval.summary` (`readability_score`, `summary_quality`, `summary_text`) so high-match evidence remains readable.
- You can request inline calculator solves from chat with `/math <expression>`, `math:<expression>`, or `calc:<expression>`.
- Chat math metadata now includes deterministic status fields: `status`, `error_code`, and `error_message` for failure classification.
- Chat responses also include a top-level `csif_meta` object with machine-readable context, retrieval matches, and optional math result details.
- `csif_meta.conversation` exposes conversation intent, response style, and deterministic next-step suggestions for downstream clients.
- `csif_meta.schema_version` is currently `csif_chat_meta_v1` for stable client-side parsing.

Lexical disambiguation notes:

- `POST /v1/csif/disambiguate` performs deterministic token-to-sense resolution with an audit trace.
- This pass includes cross-language alias identity for `light`, `luz`, `光`, and `lumière`, mapped to one semantic identity node.
- The response includes candidate rankings, feature contributions, margin threshold, and selected sense (or `ambiguous` / `unknown_lexeme`).
- A compact deterministic lexicon pack (`csif_compact_lexicon_v1`) is now applied for `en`, `es`, `fr`, and `zh` to provide explicit concept-edge evidence from context tokens.
- Two additional deterministic packs now strengthen the same language set (`en`, `es`, `fr`, `zh`) with less-common vocabulary: `csif_compact_lexicon_legacy_v2` (older illumination terms) and `csif_compact_lexicon_reasoning_v2` (reasoning-oriented uncommon terms, including `individual`/`dividual` style distinctions).
- A compact third pack, `csif_compact_lexicon_etymology_v3`, adds language-specific etymological families (for example luc/clar families such as `lucidity`, `elucidate`, `clarify`) while preserving the same deterministic scoring path.
- Responses include `lexicon` diagnostics with `coverage` metrics (`context_token_count`, `matched_token_count`, `coverage_ratio`), `matched_lexicon_edges`, and `unknown_due_to_lexicon_gap` for unknown lexeme cases.
- Lexicon diagnostics also include `packs`, deterministic `pack_weights`, and each matched edge now carries its source `pack` for deterministic auditability.
- Candidate feature vectors now include deterministic `lexicon_support`, and resolver weights expose `lexicon_support: 0.4`.
- `POST /v1/csif/disambiguate`, `POST /v1/csif/simulate`, and `POST /v1/csif/reconcile` now accept optional `lexicon_packs` (list of pack IDs) to deterministically scope active lexicon evidence.
- The first concrete phrase layer pass is enabled in disambiguation output as `phrase_layer`.
- Phrase candidates are deterministic compositional objects and now include multilingual alias identity for shared phrase nodes (for example `light speed`, `velocidad de la luz`, `vitesse de la lumière`, `光速` -> `phrase_light_speed`).
- Phrase candidates emit overlap-aware chunk topology metadata (`chunk_topology` and phrase lattice groups) for deterministic competing chunk analysis.
- Phrase evidence contributes a deterministic `phrase_boost` feature to matching token-sense candidates.
- Frame-relative semantics are now supported via optional request fields `frame` and `prior_frame` with dimensions: `observer_frame`, `ontology_frame`, `temporal_frame`, `modality_frame`, `epistemic_source_frame`.
- Responses include `frame_semantics` with deterministic transition audit (`prior_frame_signature`, `candidate_frame_signature`, `frame_alignment_score`, `transition_cost`, `frame_reconciliation_status`) plus typed contradiction class.
- Disambiguation now emits deterministic cross-frame translation operators in `frame_operators` with `eligible_operators` and `projected_candidates` (strict policy: `implicit_transforms=false`, `allow_chaining=false`, single-step projections only).
- Optional `conservation_policy` now constrains transform admissibility with `required_invariants`, `allow_lossy`, and `max_total_loss`.
- Projected transform candidates include `conservation_profile` (invariant-by-invariant preservation status, total loss, violated invariants, admissibility).
- When frame conflict is detected, unresolved torsion is preserved (`unresolved_torsion.kind=frame_conflict`, `collapse_allowed=false`, `recommended_action=frame_reconcile`) instead of forcing a remap.
- When sense logging is enabled, each disambiguation response includes `trajectory_event` with persisted evolution metadata: prior selection, rejected candidates, contradiction/correction flags, and confidence narrowing.
- Responses include `semantic_identity` fields that separate symbol alias from canonical meaning identity.
- Semantic inertia is enabled by default and increases reassignment resistance for crystallized senses.

Semantic inertia model:

- `effective_threshold = base_threshold + (crystallization_depth * inertia_coefficient)`
- Reassignment pressure is only applied when top candidate differs from prior resolved sense.
- When inertia blocks reassignment, response includes `inertia_decision.blocked=true` with suggested action (`sandbox_review` by default).
- Request knobs on `POST /v1/csif/disambiguate`:
	- `inertia_coefficient` (optional number, default `0.35`)
	- `sandbox_on_inertia_block` (optional bool, default `true`)

Sandbox cognitive layer:

- `POST /v1/csif/simulate` runs deterministic hypothetical remappings without mutating trajectory state.
- Simulation branches are scored from the same resolver inputs but marked as sandbox-only.
- Response branches include modal semantics fields for possibility, necessity, belief source, certainty mode, and counterfactual status.
- Phrase candidates from `phrase_layer` are promoted as first-class `semantic_phrase` branches (`sandbox-phrase-*`) and compete in the same deterministic ranking pipeline.
- Phrase branches include overlap-sensitive penalties so competing chunk topologies are ranked deterministically without recursive parsing.
- Sandbox branch coherence is now frame-aware, including deterministic `frame_transition_cost` penalty and `frame_alignment_bonus` contribution.
- Sandbox includes explicit transform branches (`torsion=frame_transform`) derived from `frame_operators.projected_candidates` so direct and transformed interpretations compete in one deterministic arbitration space.
- Sandbox coherence now applies deterministic conservation penalties (`conservation_loss`) and marks blocked transforms with `conservation_violation` in rejection causes.
- The sandbox now performs competitive semantic future selection: branches receive a `coherence.trajectory_coherence_score`, are ranked, and the top branch becomes the deterministic sandbox winner.
- Rejected branches include explicit `rejection_causes` such as `lower_trajectory_coherence`, `high_inertia_break_cost`, `lobe_drift`, `identity_instability`, and `causal_conflict`.
- Optional request fields:
	- `branch_limit` limits the number of alternate branches returned.
	- `forced_sense_node` evaluates a specific counterfactual remap.
	- `inertia_coefficient` uses the same semantic inertia math as live resolution.

Reconciliation layer:

- `POST /v1/csif/reconcile` explains why the winning sandbox trajectory beat a losing branch.
- It reuses sandbox branch metrics and emits winner-vs-loser topology comparisons rather than opaque branch choice alone.
- Response includes `topology_explanation` fields such as `identity_persistence`, `causal_alignment`, `historical_resonance`, `inertia_break_cost`, and `lobe_stability`.
- Reconciliation is phrase-aware: `topology_explanation` now includes `winner_node_type`, `loser_node_type`, `phrase_reconciliation_mode`, and optional winner/loser phrase node IDs.
- Reconciliation also includes winner/loser phrase `chunk_topology` snapshots when phrase branches are involved.
- Reconciliation includes frame transition topology (`prior_frame_signature`, `candidate_frame_signature`, `frame_alignment_score`, `frame_transition_cost`, `frame_reconciliation_status`) and contradiction typing.
- Reconciliation includes operator audit fields (`winner_operator_audit`, `loser_operator_audit`) when transformed branches participate.
- Reconciliation includes conservation audit (`winner_conservation_profile`, `loser_conservation_profile`, `violated_invariants`) for transformed branch comparisons.
- Response also includes `rejected_topology` fields such as `contradiction_pressure`, `semantic_drift`, and `identity_fragmentation`.
- Optional request field `losing_branch_id` lets you reconcile against a specific losing branch; otherwise the first non-winning branch is selected.

Sense trajectory persistence notes:

- `serve-openai` accepts `--sense-log-path`; if omitted and `--bank-path` is set, default log path is `<bank_path>.sense_trajectories.jsonl`.
- Events are append-only JSONL records with schema `csif_disambiguation_event_v1`.
- `GET /v1/csif/disambiguation/trajectories` supports optional query filters: `language`, `token`, and `limit`.
- `GET /v1/csif/disambiguation/summary` returns compact semantic health metrics for filtered events.

Trajectory summary metrics:

- `stability_score`: resistance to meaning reassignment over the filtered event window.
- `contradiction_rate`: fraction of events that record contradiction encounters.
- `crystallization_depth`: normalized longest same-sense run among resolved events.
- `ambiguity_entropy`: normalized spread across competing resolved senses.
- `lobe_drift`: frequency of transitions between semantic lobes.
- `resonance_persistence`: average ambiguity-margin strength over time.

`csif_meta` JSON Schema (compact v1):

```json
{
	"$schema": "https://json-schema.org/draft/2020-12/schema",
	"$id": "https://csif.local/schemas/csif_chat_meta_v1.json",
	"title": "csif_chat_meta_v1",
	"type": "object",
	"required": [
		"schema_version",
		"generated_at",
		"mode",
		"prompt",
		"context",
		"bank",
		"retrieval",
		"math"
	],
	"properties": {
		"schema_version": {
			"const": "csif_chat_meta_v1"
		},
		"generated_at": {
			"type": "integer",
			"minimum": 0
		},
		"mode": {
			"type": "string"
		},
		"prompt": {
			"type": "string"
		},
		"context": {
			"type": "object",
			"required": ["recent_user_prompts"],
			"properties": {
				"recent_user_prompts": {
					"type": "array",
					"items": {"type": "string"}
				}
			},
			"additionalProperties": false
		},
		"bank": {
			"type": "object",
			"required": ["loaded", "bank_id", "crystal_count", "edge_count", "event_count"],
			"properties": {
				"loaded": {"type": "boolean"},
				"bank_id": {"type": ["string", "null"]},
				"crystal_count": {"type": "integer", "minimum": 0},
				"edge_count": {"type": "integer", "minimum": 0},
				"event_count": {"type": "integer", "minimum": 0}
			},
			"additionalProperties": false
		},
		"retrieval": {
			"type": "object",
			"required": ["match_count", "rewritten_query", "matches", "miss_diagnostics"],
			"properties": {
				"match_count": {"type": "integer", "minimum": 0},
				"rewritten_query": {"type": "string"},
				"matches": {
					"type": "array",
					"items": {
						"type": "object",
						"required": [
							"score",
							"crystal_id",
							"edge_id",
							"source_node",
							"relation",
							"target_node",
							"searchable_text"
						],
						"properties": {
							"score": {"type": "number"},
							"crystal_id": {"type": "string"},
							"edge_id": {"type": "string"},
							"source_node": {"type": "string"},
							"relation": {"type": "string"},
							"target_node": {"type": "string"},
							"searchable_text": {"type": "string"}
						},
						"additionalProperties": false
					}
				},
				"miss_diagnostics": {
					"type": ["object", "null"]
				}
			},
			"additionalProperties": false
		},
		"math": {
			"type": ["object", "null"],
			"properties": {
				"status": {"type": "string"},
				"expression": {"type": "string"},
				"result": {},
				"result_latex": {"type": ["string", "null"]},
				"phase_signature": {},
				"error_code": {"type": ["string", "null"]},
				"error_message": {"type": ["string", "null"]}
			},
			"additionalProperties": false
		}
	},
	"additionalProperties": false
}
```

Example:

```bash
curl -sS http://127.0.0.1:8080/v1/models
curl -sS http://127.0.0.1:8080/v1/csif/index
curl -sS http://127.0.0.1:8080/v1/csif/retrieve \
	-H 'content-type: application/json' \
	-d '{"query":"light darkness","top_k":3}'
curl -sS http://127.0.0.1:8080/v1/csif/math \
	-H 'content-type: application/json' \
	-d '{"expression":"2*(3+4)^2"}'
curl -sS http://127.0.0.1:8080/v1/csif/math \
	-H 'content-type: application/json' \
	-d '{"expression":"sin(30)+cos(60)","mode":"geometric","angle_unit":"degrees"}'
curl -sS http://127.0.0.1:8080/v1/csif/math \
	-H 'content-type: application/json' \
	-d '{"expression":"(2+3i)^2 + conj(4-5i) + arg(1+i) + 5!"}'
curl -sS http://127.0.0.1:8080/v1/csif/math \
	-H 'content-type: application/json' \
	-d '{"expression":"exp(i*pi) + 1"}'
curl -sS http://127.0.0.1:8080/v1/csif/disambiguate \
	-H 'content-type: application/json' \
	-d '{"language":"en","token":"light","context":"the light helped me see with my eyes","margin":0.75}'
curl -sS http://127.0.0.1:8080/v1/csif/disambiguate \
	-H 'content-type: application/json' \
	-d '{"language":"en","token":"light","context":"the lecture gave me understanding and insight","margin":0.75}'
curl -sS http://127.0.0.1:8080/v1/csif/disambiguate \
	-H 'content-type: application/json' \
	-d '{"language":"es","token":"luz","context":"la luz me ayuda a ver con mis ojos","margin":0.75}'
curl -sS http://127.0.0.1:8080/v1/csif/disambiguate \
	-H 'content-type: application/json' \
	-d '{"language":"es","token":"luz","context":"la velocidad de la luz es constante","margin":0.75}'
curl -sS http://127.0.0.1:8080/v1/csif/disambiguate \
	-H 'content-type: application/json' \
	-d '{"language":"en","token":"light","context":"the light speed engine prototype is unstable","margin":0.75}'
curl -sS http://127.0.0.1:8080/v1/csif/disambiguate \
	-H 'content-type: application/json' \
	-d '{"language":"en","token":"light","context":"the lecture gave me understanding and insight","margin":0.75,"inertia_coefficient":1.2,"sandbox_on_inertia_block":true}'
curl -sS http://127.0.0.1:8080/v1/csif/disambiguate \
	-H 'content-type: application/json' \
	-d '{"language":"en","token":"light","context":"light behaves as a wave and particle in context","margin":0.75,"frame":{"observer_frame":"third_person_external","ontology_frame":"quantum_interaction","temporal_frame":"present","modality_frame":"assertive","epistemic_source_frame":"instrumented_measurement"},"prior_frame":{"observer_frame":"internal_observer","ontology_frame":"general_ontology","temporal_frame":"present","modality_frame":"assertive","epistemic_source_frame":"user_reported"}}'
curl -sS http://127.0.0.1:8080/v1/csif/disambiguate \
	-H 'content-type: application/json' \
	-d '{"language":"en","token":"light","context":"light behaves as both wave and particle","margin":0.75,"frame":{"observer_frame":"third_person_external","ontology_frame":"quantum_interaction","temporal_frame":"present","modality_frame":"assertive","epistemic_source_frame":"instrumented_measurement"},"prior_frame":{"observer_frame":"internal_observer","ontology_frame":"classical_optics","temporal_frame":"present","modality_frame":"assertive","epistemic_source_frame":"instrumented_measurement"},"conservation_policy":{"required_invariants":["observer_consistency","identity_continuity"],"allow_lossy":false,"max_total_loss":0.2}}'
curl -sS http://127.0.0.1:8080/v1/csif/simulate \
	-H 'content-type: application/json' \
	-d '{"language":"en","token":"light","context":"the lecture gave me understanding and insight","margin":0.75,"inertia_coefficient":1.2,"branch_limit":3,"forced_sense_node":"sense_light_insight"}'
curl -sS http://127.0.0.1:8080/v1/csif/simulate \
	-H 'content-type: application/json' \
	-d '{"language":"en","token":"light","context":"light behaves as both wave and particle","margin":0.75,"frame":{"observer_frame":"internal_observer","ontology_frame":"quantum_interaction","temporal_frame":"present","modality_frame":"assertive","epistemic_source_frame":"instrumented_measurement"},"prior_frame":{"observer_frame":"internal_observer","ontology_frame":"classical_optics","temporal_frame":"present","modality_frame":"assertive","epistemic_source_frame":"instrumented_measurement"}}'
curl -sS http://127.0.0.1:8080/v1/csif/reconcile \
	-H 'content-type: application/json' \
	-d '{"language":"en","token":"light","context":"the lecture gave me understanding and insight","margin":0.75,"inertia_coefficient":1.2,"branch_limit":3}'
curl -sS http://127.0.0.1:8080/v1/csif/reconcile \
	-H 'content-type: application/json' \
	-d '{"language":"en","token":"light","context":"light behaves as both wave and particle","margin":0.75,"frame":{"observer_frame":"internal_observer","ontology_frame":"quantum_interaction","temporal_frame":"present","modality_frame":"assertive","epistemic_source_frame":"instrumented_measurement"},"prior_frame":{"observer_frame":"internal_observer","ontology_frame":"classical_optics","temporal_frame":"present","modality_frame":"assertive","epistemic_source_frame":"instrumented_measurement"}}'
curl -sS 'http://127.0.0.1:8080/v1/csif/disambiguation/trajectories?language=en&token=light&limit=10'
curl -sS 'http://127.0.0.1:8080/v1/csif/disambiguation/summary?language=en&token=light&limit=200'
curl -sS http://127.0.0.1:8080/v1/chat/completions \
	-H 'content-type: application/json' \
	-d '{"model":"ugc-model","messages":[{"role":"user","content":"status"}]}'
curl -sS http://127.0.0.1:8080/v1/chat/completions \
	-H 'content-type: application/json' \
	-d '{"model":"ugc-model","messages":[{"role":"user","content":"hey"},{"role":"user","content":"/math (2+3i)^2 + conj(4-5i) + arg(1+i) + 5!"}]}'
curl -sS http://127.0.0.1:8080/v1/embeddings \
	-H 'content-type: application/json' \
	-d '{"model":"ugc-model","input":"semantic status"}'
```

## Test

```bash
cargo test --locked
```

Validation checks RWIF v2 compatibility and deterministic replay field presence.
Migration is additive and preserves unknown fields.
