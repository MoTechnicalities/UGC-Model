use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::env;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

const RWIF_SCHEMA_VERSION: &str = "RWIF_V2";
const RWIF_EDGE_SCHEMA_VERSION: &str = "RWIF_EDGE_V2";
const RWIF_EVENT_SCHEMA_VERSION: &str = "RWIF_EVENT_V2";
const OPENAI_MODEL_ID: &str = "ugc-model";
const EMBEDDING_DIM: usize = 64;

#[derive(Clone, Debug)]
struct BankSummary {
    bank_id: String,
    crystal_count: usize,
    edge_count: usize,
    event_count: usize,
}

#[derive(Clone)]
struct AppState {
    bank_summary: Option<BankSummary>,
    bank_index: Option<Arc<BankIndex>>,
    sense_trajectory_log_path: Option<String>,
}

#[derive(Debug)]
struct ServeConfig {
    host: String,
    port: u16,
    bank_path: Option<String>,
    sense_log_path: Option<String>,
}

#[derive(Deserialize)]
struct ChatMessage {
    role: String,
    content: Value,
}

#[derive(Deserialize)]
struct ChatCompletionsRequest {
    model: Option<String>,
    messages: Vec<ChatMessage>,
    preferences: Option<ChatPreferencesRequest>,
}

#[derive(Deserialize, Clone, Debug, Default)]
struct ChatPreferencesRequest {
    response_style: Option<String>,
    depth: Option<String>,
    tone: Option<String>,
    warmth_ceiling: Option<String>,
    retrieval_summary: Option<bool>,
    retrieval_top_k: Option<usize>,
}

#[derive(Clone, Debug)]
struct ChatPreferences {
    response_style: &'static str,
    depth: &'static str,
    tone: &'static str,
    warmth_ceiling: &'static str,
    retrieval_summary: bool,
    retrieval_top_k: usize,
}

#[derive(Clone, Debug)]
struct ChatTimeCrystalContext {
    t_ns: u128,
    phase_theta: f64,
    torsion_norm: f64,
    coordinate_source: String,
}

#[derive(Deserialize)]
struct EmbeddingsRequest {
    model: Option<String>,
    input: Value,
}

#[derive(Deserialize)]
struct RetrieveRequest {
    query: String,
    top_k: Option<usize>,
}

#[derive(Deserialize)]
struct MathRequest {
    expression: String,
    mode: Option<String>,
    angle_unit: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct Layer0Node {
    node_id: String,
    node_type: String,
    label: String,
    provenance: Value,
}

#[derive(Clone, Debug, Deserialize)]
struct Layer0Edge {
    edge_id: String,
    source_node: String,
    relation: String,
    target_node: String,
    confidence_band: f64,
    provenance: Value,
}

#[derive(Clone, Debug, Deserialize)]
struct Layer0Graph {
    layer0_version: Option<String>,
    nodes: Vec<Layer0Node>,
    edges: Vec<Layer0Edge>,
}

#[derive(Clone, Debug)]
struct Layer0Policy {
    tau_accept: f64,
    tau_reject: f64,
}

#[derive(Clone, Debug, Serialize)]
struct Layer0Issue {
    code: String,
    message: String,
}

#[derive(Clone, Debug, Serialize)]
struct Layer0Report {
    valid: bool,
    contradictions: Vec<Layer0Issue>,
    warnings: Vec<Layer0Issue>,
    stop_reason: String,
    verdict: String,
    node_count: usize,
    edge_count: usize,
}

#[derive(Deserialize)]
struct DisambiguateRequest {
    token: String,
    context: Option<String>,
    language: Option<String>,
    top_k: Option<usize>,
    margin: Option<f64>,
    inertia_coefficient: Option<f64>,
    sandbox_on_inertia_block: Option<bool>,
    frame: Option<FrameContextRequest>,
    prior_frame: Option<FrameContextRequest>,
    conservation_policy: Option<ConservationPolicyRequest>,
    lexicon_packs: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct SimulateRequest {
    token: String,
    context: Option<String>,
    language: Option<String>,
    top_k: Option<usize>,
    margin: Option<f64>,
    inertia_coefficient: Option<f64>,
    branch_limit: Option<usize>,
    forced_sense_node: Option<String>,
    frame: Option<FrameContextRequest>,
    prior_frame: Option<FrameContextRequest>,
    conservation_policy: Option<ConservationPolicyRequest>,
    lexicon_packs: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct ReconcileRequest {
    token: String,
    context: Option<String>,
    language: Option<String>,
    top_k: Option<usize>,
    margin: Option<f64>,
    inertia_coefficient: Option<f64>,
    branch_limit: Option<usize>,
    forced_sense_node: Option<String>,
    losing_branch_id: Option<String>,
    frame: Option<FrameContextRequest>,
    prior_frame: Option<FrameContextRequest>,
    conservation_policy: Option<ConservationPolicyRequest>,
    lexicon_packs: Option<Vec<String>>,
}

#[derive(Deserialize, Clone, Debug, Default)]
struct FrameContextRequest {
    observer_frame: Option<String>,
    ontology_frame: Option<String>,
    temporal_frame: Option<String>,
    modality_frame: Option<String>,
    epistemic_source_frame: Option<String>,
}

#[derive(Clone, Debug)]
struct FrameContext {
    observer_frame: String,
    ontology_frame: String,
    temporal_frame: String,
    modality_frame: String,
    epistemic_source_frame: String,
}

#[derive(Deserialize, Clone, Debug, Default)]
struct ConservationPolicyRequest {
    required_invariants: Option<Vec<String>>,
    allow_lossy: Option<bool>,
    max_total_loss: Option<f64>,
}

#[derive(Clone, Debug)]
struct ConservationPolicy {
    required_invariants: Vec<String>,
    allow_lossy: bool,
    max_total_loss: f64,
}

#[derive(Clone, Debug)]
struct LexiconControl {
    active_packs: HashSet<String>,
    pack_weights: HashMap<String, f64>,
}

#[derive(Deserialize)]
struct TrajectoryQuery {
    language: Option<String>,
    token: Option<String>,
    limit: Option<usize>,
}

#[derive(Deserialize)]
struct TrajectorySummaryQuery {
    language: Option<String>,
    token: Option<String>,
    limit: Option<usize>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct ComplexValue {
    re: f64,
    im: f64,
}

impl ComplexValue {
    fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }

    fn abs(self) -> f64 {
        self.re.hypot(self.im)
    }

    fn arg(self) -> f64 {
        self.im.atan2(self.re)
    }

    fn conj(self) -> Self {
        Self::new(self.re, -self.im)
    }

    fn is_real(self) -> bool {
        self.im.abs() < 1e-12
    }

    fn is_zero(self) -> bool {
        self.re.abs() < 1e-12 && self.im.abs() < 1e-12
    }

    fn is_finite(self) -> bool {
        self.re.is_finite() && self.im.is_finite()
    }
}

use std::ops::{Add, Div, Mul, Neg, Sub};

impl Add for ComplexValue {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.re + rhs.re, self.im + rhs.im)
    }
}

impl Sub for ComplexValue {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.re - rhs.re, self.im - rhs.im)
    }
}

impl Mul for ComplexValue {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self::new(
            self.re * rhs.re - self.im * rhs.im,
            self.re * rhs.im + self.im * rhs.re,
        )
    }
}

impl Div for ComplexValue {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        let den = rhs.re * rhs.re + rhs.im * rhs.im;
        Self::new(
            (self.re * rhs.re + self.im * rhs.im) / den,
            (self.im * rhs.re - self.re * rhs.im) / den,
        )
    }
}

impl Neg for ComplexValue {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self::new(-self.re, -self.im)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MathMode {
    Algebraic,
    Geometric,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AngleUnit {
    Radians,
    Degrees,
}

#[derive(Clone, Copy, Debug)]
struct MathOptions {
    mode: MathMode,
    angle_unit: AngleUnit,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct EvaluationEnvelope {
    envelope_id: String,
    timestamp_unix_ms: u64,
    timeout_ms: Option<u64>,
    source_text: String,
    source_kind: SourceKind,
    intent: IntentDescriptor,
    semantic_context: EvalSemanticContext,
    active_frame: EvalFrameContext,
    assumptions: Vec<Assumption>,
    parsed_units: Vec<ParsedUnit>,
    symbol_table: SymbolTable,
    semantic_jobs: Vec<SemanticJobRecord>,
    math_jobs: Vec<MathJobRecord>,
    logic_jobs: Vec<LogicJobRecord>,
    consistency: ConsistencyReport,
    routing_trace: Vec<RouteEvent>,
    job_influence_audit: Vec<JobInfluenceRecord>,
    diagnostics: Vec<DiagnosticEvent>,
    final_outcome: FinalOutcome,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum SourceKind {
    NaturalLanguage,
    MathExpression,
    LogicExpression,
    Mixed,
    ApiStructured,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct IntentDescriptor {
    intent_id: String,
    primary_goal: PrimaryGoal,
    secondary_goals: Vec<PrimaryGoal>,
    requested_output_mode: OutputMode,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum PrimaryGoal {
    EvaluateNumeric,
    CheckTruth,
    SolveConstraint,
    CompareExpressions,
    ValidateDomain,
    ExplainReasoning,
    MixedReasoning,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum OutputMode {
    Text,
    Structured,
    Trace,
    TextAndStructured,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum InvocationMode {
    UserRequested,
    InternallyTriggered,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum ReasoningScope {
    Committed,
    Provisional,
    Sandbox,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct TriggerContext {
    trigger_reason: String,
    triggered_by_job_id: Option<String>,
    routed_from_stage: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct EvalSemanticContext {
    resolved_entities: Vec<SemanticBinding>,
    ambiguity_state: AmbiguityState,
    semantic_identity_signature: Option<String>,
    confidence: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SemanticBinding {
    symbol: String,
    canonical_id: String,
    role: String,
    confidence: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum AmbiguityState {
    Resolved,
    Ambiguous,
    NeedsInput,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct EvalFrameContext {
    observer_frame: Option<String>,
    ontology_frame: Option<String>,
    temporal_frame: Option<String>,
    modality_frame: Option<String>,
    epistemic_source_frame: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Assumption {
    assumption_id: String,
    text: String,
    binding: AssumptionBinding,
    confidence: f64,
    source: AssumptionSource,
    provenance: Provenance,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum AssumptionBinding {
    SymbolValue { symbol: String, value: String },
    DomainClaim { symbol: String, condition: String },
    RelationClaim { relation: String, lhs: String, rhs: String },
    Freeform(String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum AssumptionSource {
    User,
    SemanticInference,
    LogicInference,
    MathInference,
    SystemDefault,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Provenance {
    source: String,
    timestamp_unix_ms: Option<u64>,
    reference: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum ParsedUnit {
    ScalarExpr(AstNode),
    LogicExpr(LogicExprNode),
    RelationExpr(RelationExprNode),
    ConstraintSet(Vec<ConstraintNode>),
    Query(QueryNode),
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
struct SymbolTable {
    symbols: BTreeMap<String, SymbolBinding>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SymbolBinding {
    symbol: String,
    declared_type: SymbolType,
    bound_value: Option<String>,
    confidence: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum SymbolType {
    Scalar,
    Complex,
    Boolean,
    Matrix,
    SemanticEntity,
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum LogicExprNode {
    BoolLiteral(bool),
    Predicate { name: String, args: Vec<AstNode> },
    Comparison { op: ComparisonOp, left: AstNode, right: AstNode },
    Not(Box<LogicExprNode>),
    And(Vec<LogicExprNode>),
    Or(Vec<LogicExprNode>),
    Xor(Vec<LogicExprNode>),
    Implies(Box<LogicExprNode>, Box<LogicExprNode>),
    Equivalent(Box<LogicExprNode>, Box<LogicExprNode>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum ComparisonOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RelationExprNode {
    relation: String,
    left: String,
    right: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ConstraintNode {
    constraint_id: String,
    expression: LogicExprNode,
    description: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct QueryNode {
    prompt: String,
    target: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct MathJobRecord {
    job_id: String,
    invocation_mode: InvocationMode,
    reasoning_scope: ReasoningScope,
    trigger_context: TriggerContext,
    input_expr: AstNode,
    normalized_expression: String,
    requested_operation: MathOperation,
    domain_checks: Vec<DomainCheck>,
    assumptions_used: Vec<String>,
    result: MathJobResult,
    trace: Vec<MathTraceStep>,
    influenced_final_answer: bool,
    influence_notes: Vec<String>,
    diagnostics: Vec<DiagnosticEvent>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum MathOperation {
    Evaluate,
    SimplifyNumeric,
    IntegrateNumeric,
    SolveNumeric,
    MatrixFunction,
    SpecialFunction,
    ResponseQualification,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct MathJobResult {
    status: MathStatus,
    value: Option<MathValue>,
    value_type: MathValueType,
    domain_ok: bool,
    deterministic: bool,
    precision_class: PrecisionClass,
    error: Option<EngineError>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum MathStatus {
    Success,
    DomainError,
    Unsupported,
    AmbiguousSymbol,
    NonFinite,
    ContradictedByAssumptions,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum MathValue {
    Real(f64),
    Complex { re: f64, im: f64 },
    Matrix(Vec<Vec<MathValue>>),
    Interval { lo: f64, hi: f64 },
    SymbolicResidual(String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum MathValueType {
    Real,
    Complex,
    Matrix,
    Interval,
    Symbolic,
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum PrecisionClass {
    ExactDeterministic,
    ExactDeterministicComplex,
    NumericalApproximation,
    Mixed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct DomainCheck {
    rule_id: String,
    description: String,
    passed: bool,
    severity: CheckSeverity,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum CheckSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct MathTraceStep {
    rule: String,
    expression: String,
    result: Option<MathValue>,
    note: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct LogicJobRecord {
    job_id: String,
    invocation_mode: InvocationMode,
    reasoning_scope: ReasoningScope,
    trigger_context: TriggerContext,
    input_expr: LogicExprNode,
    requested_operation: LogicOperation,
    assumptions_used: Vec<String>,
    constraint_context: Vec<ConstraintNode>,
    result: LogicResult,
    trace: Vec<LogicTraceStep>,
    influenced_final_answer: bool,
    influence_notes: Vec<String>,
    diagnostics: Vec<DiagnosticEvent>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SemanticJobRecord {
    job_id: String,
    invocation_mode: InvocationMode,
    reasoning_scope: ReasoningScope,
    trigger_context: TriggerContext,
    requested_operation: SemanticOperation,
    input_summary: String,
    result: SemanticJobResult,
    trace: Vec<SemanticTraceStep>,
    influenced_final_answer: bool,
    influence_notes: Vec<String>,
    diagnostics: Vec<DiagnosticEvent>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum SemanticOperation {
    RouteRequest,
    SynthesizeResponse,
    PreserveFailureContext,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SemanticJobResult {
    status: SemanticStatus,
    interpretation: String,
    confidence: f64,
    error: Option<EngineError>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum SemanticStatus {
    Success,
    NeedsInput,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SemanticTraceStep {
    stage: String,
    note: String,
    confidence: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum LogicOperation {
    EvaluateTruth,
    CheckSatisfiable,
    CheckEquivalence,
    CheckImplication,
    ValidateConstraintSet,
    DomainGate,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct LogicResult {
    status: LogicStatus,
    truth: TruthValue,
    modality: Modality,
    satisfiable: Option<bool>,
    models_found: Option<usize>,
    blocking_conditions: Vec<BlockingCondition>,
    error: Option<EngineError>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum LogicStatus {
    Success,
    Unknown,
    Contradiction,
    Unsupported,
    IncompleteBindings,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum TruthValue {
    True,
    False,
    Unknown,
    NeedsInput,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum Modality {
    Must,
    May,
    Impossible,
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct BlockingCondition {
    condition_id: String,
    description: String,
    severity: CheckSeverity,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct LogicTraceStep {
    rule: String,
    expression: String,
    truth: TruthValue,
    note: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct EngineError {
    code: String,
    message: String,
    class: ErrorClass,
    retryable: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum ErrorClass {
    Parse,
    Domain,
    Unsupported,
    Contradiction,
    MissingBinding,
    Internal,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ConsistencyReport {
    semantic_math_alignment: AlignmentStatus,
    semantic_logic_alignment: AlignmentStatus,
    math_logic_alignment: AlignmentStatus,
    contradictions: Vec<ContradictionRecord>,
    unresolved_ambiguities: Vec<UnresolvedAmbiguity>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum AlignmentStatus {
    Aligned,
    Qualified,
    Conflicted,
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ContradictionRecord {
    contradiction_id: String,
    description: String,
    severity: CheckSeverity,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct UnresolvedAmbiguity {
    ambiguity_id: String,
    description: String,
    candidates: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RouteEvent {
    stage: String,
    decision: String,
    rationale: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct JobInfluenceRecord {
    job_id: String,
    job_kind: JobKind,
    invocation_mode: InvocationMode,
    reasoning_scope: ReasoningScope,
    used_in_final_answer: bool,
    influence_role: InfluenceRole,
    explanation: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum JobKind {
    Math,
    Logic,
    Semantic,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum InfluenceRole {
    DirectEvidence,
    DomainGate,
    ConsistencyCheck,
    CandidateRejection,
    CandidateRanking,
    FinalComputation,
    BackgroundOnly,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct DiagnosticEvent {
    code: String,
    message: String,
    severity: CheckSeverity,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct FinalOutcome {
    status: FinalStatus,
    responder_text: String,
    supporting_job_ids: Vec<String>,
    hidden_job_ids_used: Vec<String>,
    machine_summary: MachineSummary,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum FinalStatus {
    Success,
    QualifiedSuccess,
    NeedsInput,
    Contradiction,
    Unsupported,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct MachineSummary {
    final_value: Option<MathValue>,
    final_truth: Option<TruthValue>,
    assumptions_applied: Vec<String>,
    contradiction_count: usize,
    confidence: f64,
}

impl Default for MathOptions {
    fn default() -> Self {
        Self {
            mode: MathMode::Algebraic,
            angle_unit: AngleUnit::Radians,
        }
    }
}

#[derive(Clone, Debug)]
struct MathStep {
    rule: String,
    expression: String,
    latex: String,
    result: f64,
    complex_result: ComplexValue,
    crystal_traces: Vec<Value>,
    geometry: Option<Value>,
}

#[derive(Clone, Debug)]
struct PhaseStep {
    op: String,
    inputs: Vec<String>,
    output: String,
    phase_theta: f64,
}

fn complex_to_json(value: ComplexValue) -> Value {
    json!({"re": value.re, "im": value.im})
}

fn complex_to_text(value: ComplexValue) -> String {
    complex_to_text_with_mode(value, MathMode::Geometric)
}

fn format_number_with_mode(n: f64, mode: MathMode) -> String {
    match mode {
        MathMode::Algebraic => n.to_string(),
        MathMode::Geometric => format_number(n),
    }
}

fn complex_to_text_with_mode(value: ComplexValue, mode: MathMode) -> String {
    let re = if value.re.abs() < 1e-12 { 0.0 } else { value.re };
    let im = if value.im.abs() < 1e-12 { 0.0 } else { value.im };

    if im.abs() < 1e-12 {
        format_number_with_mode(re, mode)
    } else if re.abs() < 1e-12 {
        let coeff = format_number_with_mode(im, mode);
        if coeff == "1" {
            "i".to_string()
        } else if coeff == "-1" {
            "-i".to_string()
        } else {
            format!("{}i", coeff)
        }
    } else {
        let sign = if im >= 0.0 { "+" } else { "" };
        format!(
            "{}{}{}i",
            format_number_with_mode(re, mode),
            sign,
            format_number_with_mode(im, mode)
        )
    }
}

fn complex_phase(value: ComplexValue) -> f64 {
    if value.is_zero() {
        0.0
    } else {
        value.arg()
    }
}

fn wrap_to_pi(theta: f64) -> f64 {
    let two_pi = 2.0 * std::f64::consts::PI;
    (theta + std::f64::consts::PI).rem_euclid(two_pi) - std::f64::consts::PI
}

fn phase_drift(from_phase: f64, to_phase: f64) -> f64 {
    wrap_to_pi(to_phase - from_phase)
}

fn unit_loop_torsion_threshold_policy() -> (f64, String) {
    const DEFAULT_THRESHOLD: f64 = 0.2;
    match std::env::var("CSIF_UNIT_LOOP_TORSION_THRESHOLD") {
        Ok(raw) => match raw.parse::<f64>() {
            Ok(parsed) if parsed.is_finite() && parsed >= 0.0 => (
                parsed,
                "env:CSIF_UNIT_LOOP_TORSION_THRESHOLD".to_string(),
            ),
            _ => (
                DEFAULT_THRESHOLD,
                "default_invalid_env_fallback".to_string(),
            ),
        },
        Err(_) => (DEFAULT_THRESHOLD, "default".to_string()),
    }
}

fn logic_inference_torsion_threshold_policy() -> (f64, String) {
    const DEFAULT_THRESHOLD: f64 = 0.2;
    match std::env::var("CSIF_LOGIC_INFERENCE_TORSION_THRESHOLD") {
        Ok(raw) => match raw.parse::<f64>() {
            Ok(parsed) if parsed.is_finite() && parsed >= 0.0 => (
                parsed,
                "env:CSIF_LOGIC_INFERENCE_TORSION_THRESHOLD".to_string(),
            ),
            _ => (
                DEFAULT_THRESHOLD,
                "default_invalid_env_fallback".to_string(),
            ),
        },
        Err(_) => (DEFAULT_THRESHOLD, "default".to_string()),
    }
}

fn check_severity_from_str(raw: Option<&str>) -> CheckSeverity {
    match raw.unwrap_or("error").to_ascii_lowercase().as_str() {
        "info" => CheckSeverity::Info,
        "warning" => CheckSeverity::Warning,
        _ => CheckSeverity::Error,
    }
}

fn unit_contradictions_from_payload(unit_crystal: &Value) -> Vec<ContradictionRecord> {
    unit_crystal
        .get("contradictions")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    let code = item
                        .get("code")
                        .and_then(Value::as_str)
                        .unwrap_or("unit_conversion_contradiction")
                        .to_string();
                    let chain_id = item
                        .get("chain_id")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown_chain");
                    let loop_torsion_norm = item
                        .get("loop_torsion_norm")
                        .and_then(Value::as_f64)
                        .unwrap_or(0.0);
                    let threshold = item
                        .get("threshold")
                        .and_then(Value::as_f64)
                        .unwrap_or(0.0);

                    ContradictionRecord {
                        contradiction_id: format!("{}:{}", code, chain_id),
                        description: format!(
                            "unit conversion chain '{}' exceeded torsion threshold: loop_torsion_norm={} threshold={}",
                            chain_id, loop_torsion_norm, threshold
                        ),
                        severity: check_severity_from_str(item.get("severity").and_then(Value::as_str)),
                    }
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn logic_contradictions_from_payload(logic_contradiction_signal: &Value) -> Vec<ContradictionRecord> {
    logic_contradiction_signal
        .get("contradictions")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    let code = item
                        .get("code")
                        .and_then(Value::as_str)
                        .unwrap_or("logic_inference_contradiction")
                        .to_string();
                    let chain_id = item
                        .get("chain_id")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown_chain");
                    let loop_torsion_norm = item
                        .get("loop_torsion_norm")
                        .and_then(Value::as_f64)
                        .unwrap_or(0.0);
                    let threshold = item
                        .get("threshold")
                        .and_then(Value::as_f64)
                        .unwrap_or(0.0);

                    ContradictionRecord {
                        contradiction_id: format!("{}:{}", code, chain_id),
                        description: format!(
                            "logic inference chain '{}' exceeded torsion threshold: loop_torsion_norm={} threshold={}",
                            chain_id, loop_torsion_norm, threshold
                        ),
                        severity: check_severity_from_str(item.get("severity").and_then(Value::as_str)),
                    }
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn build_anticrystal_lob(contradictions: &[ContradictionRecord]) -> Value {
    json!({
        "lobe": "anticrystal",
        "entry_count": contradictions.len(),
        "entries": contradictions
            .iter()
            .enumerate()
            .map(|(idx, c)| {
                json!({
                    "entry_id": format!("anticrystal:{}", idx + 1),
                    "contradiction_id": c.contradiction_id,
                    "severity": format!("{:?}", c.severity),
                    "this_is_wrong_because": c.description,
                })
            })
            .collect::<Vec<_>>()
    })
}

fn build_unit_crystal_payload(
    options: MathOptions,
    angle_unit: &str,
    final_theta: f64,
    torsion_norm: f64,
) -> Value {
    let (loop_torsion_threshold, threshold_source) = unit_loop_torsion_threshold_policy();
    let (unit_id, label, representation) = match options.mode {
        MathMode::Algebraic => (
            "unit.binary.ieee754.f64",
            "IEEE-754 Binary64",
            "binary_floating_point",
        ),
        MathMode::Geometric => (
            "unit.decimal.geometric",
            "Geometric Decimal",
            "decimal_geometric_scaling",
        ),
    };

    let base_phase = match options.mode {
        MathMode::Algebraic => 0.7853981633974483_f64,
        MathMode::Geometric => 0.0_f64,
    };

    let raw_phase = base_phase + final_theta * 0.125;
    let unit_phase = wrap_to_pi(raw_phase);

    let repr_target = if options.mode == MathMode::Algebraic {
        "unit.decimal.geometric"
    } else {
        "unit.binary.ieee754.f64"
    };
    let repr_target_phase = wrap_to_pi(unit_phase + if options.mode == MathMode::Algebraic {
        -std::f64::consts::PI / 16.0
    } else {
        std::f64::consts::PI / 16.0
    });
    let repr_hop1_drift = phase_drift(unit_phase, repr_target_phase);
    let repr_hop2_drift = phase_drift(repr_target_phase, unit_phase);
    let repr_loop_torsion =
        (repr_hop1_drift + repr_hop2_drift).abs() + torsion_norm * std::f64::consts::PI * 0.25;
    let repr_loop_torsion_norm = repr_loop_torsion / std::f64::consts::PI;
    let repr_loop_resonance = (1.0 - repr_loop_torsion_norm).clamp(0.0, 1.0);
    let repr_exceeds_threshold = repr_loop_torsion_norm > loop_torsion_threshold;

    let (angle_source, angle_target, forward_factor, reverse_factor, angle_offset) =
        if angle_unit == "degrees" {
            (
                "unit.angle.degree",
                "unit.angle.radian",
                std::f64::consts::PI / 180.0,
                180.0 / std::f64::consts::PI,
                std::f64::consts::PI / 24.0,
            )
        } else {
            (
                "unit.angle.radian",
                "unit.angle.degree",
                180.0 / std::f64::consts::PI,
                std::f64::consts::PI / 180.0,
                -std::f64::consts::PI / 24.0,
            )
        };
    let angle_target_phase = wrap_to_pi(unit_phase + angle_offset);
    let angle_hop1_drift = phase_drift(unit_phase, angle_target_phase);
    let angle_hop2_drift = phase_drift(angle_target_phase, unit_phase);
    let angle_loop_torsion =
        (angle_hop1_drift + angle_hop2_drift).abs() + torsion_norm * std::f64::consts::PI * 0.125;
    let angle_loop_torsion_norm = angle_loop_torsion / std::f64::consts::PI;
    let angle_loop_resonance = (1.0 - angle_loop_torsion_norm).clamp(0.0, 1.0);
    let angle_exceeds_threshold = angle_loop_torsion_norm > loop_torsion_threshold;

    let conversion_morphisms = vec![
        json!({
            "chain_id": "repr_round_trip_v1",
            "source_unit": unit_id,
            "target_unit": repr_target,
            "morphism_type": "representation_change",
            "hops": [
                {
                    "hop_index": 1,
                    "source_unit": unit_id,
                    "target_unit": repr_target,
                    "transform": "representation_forward",
                    "phase_from": unit_phase,
                    "phase_to": repr_target_phase,
                    "phase_drift": repr_hop1_drift,
                    "torsion_norm": torsion_norm,
                    "confidence_band": resonance(repr_target_phase)
                },
                {
                    "hop_index": 2,
                    "source_unit": repr_target,
                    "target_unit": unit_id,
                    "transform": "representation_inverse",
                    "phase_from": repr_target_phase,
                    "phase_to": unit_phase,
                    "phase_drift": repr_hop2_drift,
                    "torsion_norm": torsion_norm,
                    "confidence_band": resonance(unit_phase)
                }
            ],
            "loop_metrics": {
                "is_closed": true,
                "closure_target": unit_id,
                "hop_count": 2,
                "loop_torsion": repr_loop_torsion,
                "loop_torsion_norm": repr_loop_torsion_norm,
                "loop_resonance": repr_loop_resonance,
                "exceeds_threshold": repr_exceeds_threshold
            }
        }),
        json!({
            "chain_id": "angle_round_trip_v1",
            "source_unit": angle_source,
            "target_unit": angle_target,
            "morphism_type": "angle_coordinate_change",
            "hops": [
                {
                    "hop_index": 1,
                    "source_unit": angle_source,
                    "target_unit": angle_target,
                    "transform": "angle_forward",
                    "scale_factor": forward_factor,
                    "phase_from": unit_phase,
                    "phase_to": angle_target_phase,
                    "phase_drift": angle_hop1_drift,
                    "torsion_norm": torsion_norm,
                    "confidence_band": resonance(angle_target_phase)
                },
                {
                    "hop_index": 2,
                    "source_unit": angle_target,
                    "target_unit": angle_source,
                    "transform": "angle_inverse",
                    "scale_factor": reverse_factor,
                    "phase_from": angle_target_phase,
                    "phase_to": unit_phase,
                    "phase_drift": angle_hop2_drift,
                    "torsion_norm": torsion_norm,
                    "confidence_band": resonance(unit_phase)
                }
            ],
            "loop_metrics": {
                "is_closed": true,
                "closure_target": angle_source,
                "hop_count": 2,
                "loop_torsion": angle_loop_torsion,
                "loop_torsion_norm": angle_loop_torsion_norm,
                "loop_resonance": angle_loop_resonance,
                "exceeds_threshold": angle_exceeds_threshold
            }
        }),
    ];

    let mut contradictions = Vec::<Value>::new();
    if repr_exceeds_threshold {
        contradictions.push(json!({
            "code": "unit_conversion_loop_torsion_exceeded",
            "chain_id": "repr_round_trip_v1",
            "loop_torsion_norm": repr_loop_torsion_norm,
            "threshold": loop_torsion_threshold,
            "severity": "Error"
        }));
    }
    if angle_exceeds_threshold {
        contradictions.push(json!({
            "code": "unit_conversion_loop_torsion_exceeded",
            "chain_id": "angle_round_trip_v1",
            "loop_torsion_norm": angle_loop_torsion_norm,
            "threshold": loop_torsion_threshold,
            "severity": "Error"
        }));
    }

    let max_loop_torsion_norm = repr_loop_torsion_norm.max(angle_loop_torsion_norm);
    let contradiction_triggered = !contradictions.is_empty();

    json!({
        "unit_id": unit_id,
        "node_type": "unit_crystal",
        "label": label,
        "representation_system": representation,
        "phase_signature": {
            "phase_theta": unit_phase,
            "torsion_norm": torsion_norm,
            "resonance": resonance(unit_phase),
            "angle_unit": angle_unit
        },
        "trajectory": [
            {
                "monotonic_index": 1,
                "op": "unit_basis_select",
                "inputs": [representation],
                "output": unit_id,
                "phase_theta": unit_phase
            }
        ],
        "conversion_morphisms": conversion_morphisms,
        "policy": {
            "loop_torsion_norm_threshold": loop_torsion_threshold,
            "threshold_source": threshold_source
        },
        "contradictions": contradictions,
        "contradiction_signal": {
            "triggered": contradiction_triggered,
            "stop_reason": if contradiction_triggered {
                Some("unit_conversion_loop_torsion_exceeded")
            } else {
                None
            },
            "max_loop_torsion_norm": max_loop_torsion_norm,
            "threshold": loop_torsion_threshold
        }
    })
}

fn pow10_i128(exp: u32) -> Option<i128> {
    let mut value: i128 = 1;
    for _ in 0..exp {
        value = value.checked_mul(10)?;
    }
    Some(value)
}

fn decimal_components(value: f64) -> Option<(i128, u32)> {
    if !value.is_finite() {
        return None;
    }

    let raw = value.to_string();
    let (negative, body) = if let Some(stripped) = raw.strip_prefix('-') {
        (true, stripped)
    } else {
        (false, raw.as_str())
    };

    let (mantissa, exponent) = if let Some((m, e)) = body.split_once(['e', 'E']) {
        let parsed_exp = e.parse::<i32>().ok()?;
        (m, parsed_exp)
    } else {
        (body, 0)
    };

    let (whole, frac) = if let Some((w, f)) = mantissa.split_once('.') {
        (w, f)
    } else {
        (mantissa, "")
    };

    let mut digits = String::new();
    digits.push_str(whole);
    digits.push_str(frac);
    if digits.is_empty() {
        return None;
    }

    let mut coefficient = digits.parse::<i128>().ok()?;
    if negative {
        coefficient = -coefficient;
    }

    let mut scale: i32 = frac.len() as i32 - exponent;
    if scale < 0 {
        let factor = pow10_i128((-scale) as u32)?;
        coefficient = coefficient.checked_mul(factor)?;
        scale = 0;
    }

    let mut out_scale = scale as u32;
    while out_scale > 0 && coefficient % 10 == 0 {
        coefficient /= 10;
        out_scale -= 1;
    }

    Some((coefficient, out_scale))
}

fn decimal_to_f64(coefficient: i128, scale: u32) -> Option<f64> {
    if scale == 0 {
        return coefficient.to_string().parse::<f64>().ok();
    }

    let sign = if coefficient < 0 { "-" } else { "" };
    let digits = coefficient.abs().to_string();
    let scale_usize = scale as usize;
    let body = if digits.len() <= scale_usize {
        format!("0.{}{}", "0".repeat(scale_usize - digits.len()), digits)
    } else {
        let split = digits.len() - scale_usize;
        format!("{}.{}", &digits[..split], &digits[split..])
    };

    format!("{}{}", sign, body).parse::<f64>().ok()
}

fn geometric_decimal_binary(op: char, left: f64, right: f64) -> Option<f64> {
    let (l_coeff, l_scale) = decimal_components(left)?;
    let (r_coeff, r_scale) = decimal_components(right)?;
    let scale = l_scale.max(r_scale);
    let l_factor = pow10_i128(scale.checked_sub(l_scale)?)?;
    let r_factor = pow10_i128(scale.checked_sub(r_scale)?)?;
    let l_adj = l_coeff.checked_mul(l_factor)?;
    let r_adj = r_coeff.checked_mul(r_factor)?;

    let result_coeff = match op {
        '+' => l_adj.checked_add(r_adj)?,
        '-' => l_adj.checked_sub(r_adj)?,
        _ => return None,
    };

    decimal_to_f64(result_coeff, scale)
}

fn geometric_decimal_complex_binary(op: char, left: ComplexValue, right: ComplexValue) -> ComplexValue {
    let re = geometric_decimal_binary(op, left.re, right.re).unwrap_or_else(|| match op {
        '+' => left.re + right.re,
        '-' => left.re - right.re,
        _ => left.re,
    });

    let im = geometric_decimal_binary(op, left.im, right.im).unwrap_or_else(|| match op {
        '+' => left.im + right.im,
        '-' => left.im - right.im,
        _ => left.im,
    });

    ComplexValue::new(re, im)
}

fn c_add(a: ComplexValue, b: ComplexValue) -> ComplexValue {
    a + b
}

fn c_sub(a: ComplexValue, b: ComplexValue) -> ComplexValue {
    a - b
}

fn c_mul(a: ComplexValue, b: ComplexValue) -> ComplexValue {
    a * b
}

fn c_div(a: ComplexValue, b: ComplexValue) -> Result<ComplexValue, String> {
    if b.is_zero() {
        Err("division by zero".to_string())
    } else {
        Ok(a / b)
    }
}

fn c_exp(value: ComplexValue) -> ComplexValue {
    let e = value.re.exp();
    ComplexValue::new(e * value.im.cos(), e * value.im.sin())
}

fn c_log(value: ComplexValue) -> Result<ComplexValue, String> {
    if value.is_zero() {
        Err("log(0) undefined".to_string())
    } else {
        Ok(ComplexValue::new(value.abs().ln(), value.arg()))
    }
}

fn c_pow(base: ComplexValue, exponent: ComplexValue) -> Result<ComplexValue, String> {
    // 0^n = 0 for Re(n) > 0; 0^0 = 1 by convention
    if base.is_zero() {
        if exponent.is_zero() {
            return Ok(ComplexValue::new(1.0, 0.0));
        }
        if exponent.re > 0.0 {
            return Ok(ComplexValue::new(0.0, 0.0));
        }
        return Err("0 raised to a non-positive power is undefined".to_string());
    }
    let log_base = c_log(base)?;
    Ok(c_exp(c_mul(exponent, log_base)))
}

fn c_sqrt(value: ComplexValue) -> ComplexValue {
    let r = value.abs();
    let t = value.arg() / 2.0;
    ComplexValue::new(r.sqrt() * t.cos(), r.sqrt() * t.sin())
}

fn c_abs(value: ComplexValue) -> ComplexValue {
    ComplexValue::new(value.abs(), 0.0)
}

fn c_arg(value: ComplexValue) -> ComplexValue {
    ComplexValue::new(value.arg(), 0.0)
}

fn c_conj(value: ComplexValue) -> ComplexValue {
    value.conj()
}

fn c_sin(value: ComplexValue) -> ComplexValue {
    ComplexValue::new(
        value.re.sin() * value.im.cosh(),
        value.re.cos() * value.im.sinh(),
    )
}

fn c_cos(value: ComplexValue) -> ComplexValue {
    ComplexValue::new(
        value.re.cos() * value.im.cosh(),
        -value.re.sin() * value.im.sinh(),
    )
}

fn c_gamma(value: ComplexValue) -> Result<ComplexValue, String> {
    // Poles at non-positive integers.
    if value.is_real() {
        let n = value.re.round();
        if value.re <= 0.0 && (value.re - n).abs() < 1e-12 {
            return Err("gamma undefined at non-positive integers".to_string());
        }
    }

    const G: f64 = 7.0;
    const COEFFS: [f64; 9] = [
        0.999_999_999_999_809_9,
        676.520_368_121_885_1,
        -1_259.139_216_722_402_8,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        0.000_009_984_369_578_019_572,
        0.000_000_150_563_273_514_931_16,
    ];

    // Reflection formula for better stability in left half-plane.
    if value.re < 0.5 {
        let pi_z = ComplexValue::new(std::f64::consts::PI, 0.0) * value;
        let sin_pi_z = c_sin(pi_z);
        if sin_pi_z.is_zero() {
            return Err("gamma undefined at reflection pole".to_string());
        }
        let one_minus_z = ComplexValue::new(1.0, 0.0) - value;
        let gamma_reflected = c_gamma(one_minus_z)?;
        return c_div(
            ComplexValue::new(std::f64::consts::PI, 0.0),
            sin_pi_z * gamma_reflected,
        );
    }

    let z = value - ComplexValue::new(1.0, 0.0);
    let mut x = ComplexValue::new(COEFFS[0], 0.0);
    for (i, coeff) in COEFFS.iter().enumerate().skip(1) {
        let denom = z + ComplexValue::new(i as f64, 0.0);
        x = x + c_div(ComplexValue::new(*coeff, 0.0), denom)?;
    }

    let t = z + ComplexValue::new(G + 0.5, 0.0);
    let sqrt_two_pi = ComplexValue::new((2.0 * std::f64::consts::PI).sqrt(), 0.0);
    let pow = c_pow(t, z + ComplexValue::new(0.5, 0.0))?;
    let exp_term = c_exp(-t);
    Ok(sqrt_two_pi * pow * exp_term * x)
}

fn c_lambertw(value: ComplexValue) -> Result<ComplexValue, String> {
    if value.is_zero() {
        return Ok(ComplexValue::new(0.0, 0.0));
    }

    // Principal branch deterministic initialization.
    let mut w = c_log(value + ComplexValue::new(1.0, 0.0))?;
    for _ in 0..60 {
        let ew = c_exp(w);
        let wew = w * ew;
        let f = wew - value;
        let wp1 = w + ComplexValue::new(1.0, 0.0);
        if wp1.abs() < 1e-15 {
            return Err("lambertw iteration singular near branch point".to_string());
        }

        // Halley update: w_{n+1} = w - f / (e^w(w+1) - (w+2)f/(2w+2)).
        let denom_left = ew * wp1;
        let denom_right = c_div((w + ComplexValue::new(2.0, 0.0)) * f, ComplexValue::new(2.0, 0.0) * wp1)?;
        let denom = denom_left - denom_right;
        if denom.abs() < 1e-18 {
            return Err("lambertw iteration denominator underflow".to_string());
        }

        let delta = c_div(f, denom)?;
        let next = w - delta;
        if delta.abs() < 1e-13 {
            return Ok(next);
        }
        w = next;
    }
    Ok(w)
}

fn c_zeta_positive_half_plane(value: ComplexValue) -> Result<ComplexValue, String> {
    // Deterministic implementation for Re(s) > 0 via Dirichlet eta:
    // zeta(s) = eta(s) / (1 - 2^(1-s)), eta(s)=sum_{n>=1} (-1)^(n-1)/n^s
    let mut eta_sum = ComplexValue::new(0.0, 0.0);
    for n in 1..=20000usize {
        let ln_n = (n as f64).ln();
        let n_pow_neg_s = c_exp(ComplexValue::new(-value.re * ln_n, -value.im * ln_n));
        let signed = if n % 2 == 1 { n_pow_neg_s } else { -n_pow_neg_s };
        eta_sum = eta_sum + signed;
        if signed.abs() < 1e-14 {
            break;
        }
    }

    let two_pow_one_minus_s =
        c_exp((ComplexValue::new(1.0, 0.0) - value) * ComplexValue::new(2.0_f64.ln(), 0.0));
    let denom = ComplexValue::new(1.0, 0.0) - two_pow_one_minus_s;
    if denom.abs() < 1e-12 {
        return Err("zeta undefined at singular denominator".to_string());
    }
    c_div(eta_sum, denom)
}

fn c_zeta(value: ComplexValue) -> Result<ComplexValue, String> {
    if (value.re - 1.0).abs() < 1e-12 && value.im.abs() < 1e-12 {
        return Err("zeta has a pole at s = 1".to_string());
    }

    if value.re > 0.0 {
        return c_zeta_positive_half_plane(value);
    }

    // Analytic continuation via functional equation:
    // zeta(s) = 2^s * pi^(s-1) * sin(pi s / 2) * gamma(1-s) * zeta(1-s)
    let one = ComplexValue::new(1.0, 0.0);
    let two = ComplexValue::new(2.0, 0.0);
    let pi = ComplexValue::new(std::f64::consts::PI, 0.0);
    let s = value;
    let one_minus_s = one - s;
    let zeta_one_minus_s = c_zeta_positive_half_plane(one_minus_s)?;
    let two_pow_s = c_exp(s * ComplexValue::new(2.0_f64.ln(), 0.0));
    let pi_pow_s_minus_one = c_pow(pi, s - one)?;
    let sin_term = c_sin((pi * s) / two);
    let gamma_term = c_gamma(one_minus_s)?;
    Ok(two_pow_s * pi_pow_s_minus_one * sin_term * gamma_term * zeta_one_minus_s)
}

fn c_fact(value: ComplexValue) -> Result<ComplexValue, String> {
    if !value.is_real() {
        return Err("Factorial requires real value".to_string());
    }
    let n = value.re.round();
    if (value.re - n).abs() > 1e-12 {
        return Err("Factorial requires integer".to_string());
    }
    if n < 0.0 {
        return Err("Factorial requires n >= 0".to_string());
    }
    if n > 170.0 {
        return Err("Factorial overflow".to_string());
    }
    let mut r = 1.0;
    let mut i = 2.0;
    while i <= n {
        r *= i;
        i += 1.0;
    }
    Ok(ComplexValue::new(r, 0.0))
}

fn c_comb(n: ComplexValue, k: ComplexValue) -> Result<ComplexValue, String> {
    if !n.is_real() || !k.is_real() {
        return Err("comb requires real arguments".to_string());
    }
    let n_rounded = n.re.round();
    let k_rounded = k.re.round();
    if (n.re - n_rounded).abs() > 1e-12 || (k.re - k_rounded).abs() > 1e-12 {
        return Err("comb requires integer arguments".to_string());
    }
    if n_rounded < 0.0 || k_rounded < 0.0 {
        return Err("comb requires n >= 0 and k >= 0".to_string());
    }
    if k_rounded > n_rounded {
        return Err("comb requires k <= n".to_string());
    }

    let n_u = n_rounded as u64;
    let mut k_u = k_rounded as u64;
    if k_u > n_u - k_u {
        k_u = n_u - k_u;
    }

    let mut value = 1.0_f64;
    for i in 1..=k_u {
        value = value * ((n_u - k_u + i) as f64) / (i as f64);
    }
    Ok(ComplexValue::new(value, 0.0))
}

fn c_bessel_j(order: usize, z: ComplexValue) -> Result<ComplexValue, String> {
    if z.is_zero() {
        return Ok(if order == 0 {
            ComplexValue::new(1.0, 0.0)
        } else {
            ComplexValue::new(0.0, 0.0)
        });
    }

    let n_fact = (1..=order).fold(1.0_f64, |acc, v| acc * v as f64);
    let z_over_2 = z / ComplexValue::new(2.0, 0.0);
    let z_over_2_sq = z_over_2 * z_over_2;

    let mut term = c_pow(z_over_2, ComplexValue::new(order as f64, 0.0))?
        / ComplexValue::new(n_fact, 0.0);
    let mut sum = term;

    for m in 0..80 {
        // term_{m+1} = term_m * (-(z/2)^2) / ((m+1)(m+n+1))
        let denom = ((m + 1) * (m + order + 1)) as f64;
        let factor = c_div(-z_over_2_sq, ComplexValue::new(denom, 0.0))?;
        term = term * factor;
        sum = sum + term;
        if term.abs() < 1e-15 {
            break;
        }
    }

    Ok(sum)
}

fn c_beta(a: ComplexValue, b: ComplexValue) -> Result<ComplexValue, String> {
    let gamma_a = c_gamma(a)?;
    let gamma_b = c_gamma(b)?;
    let gamma_apb = c_gamma(a + b)?;
    if gamma_apb.abs() < 1e-15 {
        return Err("beta(a,b) undefined when gamma(a+b)=0".to_string());
    }
    c_div(gamma_a * gamma_b, gamma_apb)
}

fn c_erf(z: ComplexValue) -> ComplexValue {
    // Series expansion: erf(z) = (2/√π) * z * sum_{n=0}^∞ ((-z²)^n / (n! * (2n+1)))
    let sqrt_pi = std::f64::consts::PI.sqrt();
    let z2 = z * z;
    let mut sum = z;
    let mut z2_pow = z2;
    for n in 1..150 {
        let fact_n = (1..=n).fold(1.0, |a, v| a * v as f64);
        let coeff = (-1.0_f64).powi(n as i32) / (fact_n * (2.0 * n as f64 + 1.0));
        let term = ComplexValue::new(coeff, 0.0) * z2_pow * z;
        sum = sum + term;
        if term.abs() < 1e-14 {
            break;
        }
        z2_pow = z2_pow * z2;
    }
    ComplexValue::new(2.0 / sqrt_pi, 0.0) * sum
}

fn c_erfc(z: ComplexValue) -> ComplexValue {
    ComplexValue::new(1.0, 0.0) - c_erf(z)
}

fn c_si(x: ComplexValue) -> Result<ComplexValue, String> {
    if !x.is_real() {
        return Err("Si currently supports real inputs only".to_string());
    }
    let x_val = x.re;
    let mut sum = ComplexValue::new(0.0, 0.0);
    for n in 0..200 {
        let coeff = if n % 2 == 0 { -1.0 } else { 1.0 };
        let k = (2 * n + 1) as f64;
        let fact = (1..=(2*n+1)).fold(1.0, |a, v| a * v as f64);
        let x_pow = x_val.powi((2*n + 1) as i32);
        let term = ComplexValue::new(coeff * x_pow / (fact * k), 0.0);
        sum = sum + term;
        if term.abs() < 1e-14 {
            break;
        }
    }
    Ok(sum)
}

fn c_ci(x: ComplexValue) -> Result<ComplexValue, String> {
    if !x.is_real() {
        return Err("Ci currently supports real inputs only".to_string());
    }
    if x.re <= 0.0 {
        return Err("Ci undefined for x <= 0".to_string());
    }
    let x_val = x.re;
    let euler_gamma = 0.5772156649015329;
    let ln_x = x_val.ln();
    let mut sum = ComplexValue::new(euler_gamma + ln_x, 0.0);
    for n in 1..100 {
        let coeff = if n % 2 == 0 { 1.0 } else { -1.0 };
        let fact = (1..=n).fold(1.0, |a, v| a * v as f64);
        let x_pow = x_val.powi(n as i32 * 2);
        let denom = (2.0 * n as f64) * fact * fact;
        let term = ComplexValue::new(coeff * x_pow / denom, 0.0);
        sum = sum + term;
        if term.abs() < 1e-14 {
            break;
        }
    }
    Ok(sum)
}

fn c_fresnel_c(x: ComplexValue) -> Result<ComplexValue, String> {
    if !x.is_real() {
        return Err("FresnelC currently supports real inputs only".to_string());
    }
    let x_val = x.re;
    let mut sum = ComplexValue::new(0.0, 0.0);
    for n in 0..100 {
        let k = (4.0 * n as f64) + 1.0;
        let fact = (1..=(2*n)).fold(1.0, |a, v| a * v as f64);
        let x_pow = x_val.powi((4*n + 1) as i32);
        let denom = (2.0_f64.powi(2*n as i32)) * fact * k;
        let coeff = if n % 2 == 0 { 1.0 } else { -1.0 };
        let term = ComplexValue::new(coeff * x_pow / denom, 0.0);
        sum = sum + term;
        if term.abs() < 1e-14 {
            break;
        }
    }
    Ok(ComplexValue::new(std::f64::consts::PI.sqrt() / (2.0 * std::f64::consts::PI), 0.0) * sum)
}

fn c_fresnel_s(x: ComplexValue) -> Result<ComplexValue, String> {
    if !x.is_real() {
        return Err("FresnelS currently supports real inputs only".to_string());
    }
    let x_val = x.re;
    let mut sum = ComplexValue::new(0.0, 0.0);
    for n in 0..100 {
        let k = (4.0 * n as f64) + 3.0;
        let fact = (1..=(2*n + 1)).fold(1.0, |a, v| a * v as f64);
        let x_pow = x_val.powi((4*n + 3) as i32);
        let denom = (2.0_f64.powi(2*n as i32)) * fact * k;
        let coeff = if n % 2 == 0 { 1.0 } else { -1.0 };
        let term = ComplexValue::new(coeff * x_pow / denom, 0.0);
        sum = sum + term;
        if term.abs() < 1e-14 {
            break;
        }
    }
    Ok(ComplexValue::new(std::f64::consts::PI.sqrt() / (2.0 * std::f64::consts::PI), 0.0) * sum)
}

fn c_ei(x: ComplexValue) -> Result<ComplexValue, String> {
    if !x.is_real() {
        return Err("Ei currently supports real inputs only".to_string());
    }
    if x.re.abs() < 1e-12 {
        return Err("Ei undefined at x=0".to_string());
    }
    let x_val = x.re;
    let euler_gamma = 0.5772156649015329;
    let ln_abs_x = x_val.abs().ln();
    let mut sum = ComplexValue::new(euler_gamma + ln_abs_x, 0.0);
    for n in 1..100 {
        let x_pow = x_val.powi(n as i32);
        let fact = (1..=n).fold(1.0, |a, v| a * v as f64);
        let term = ComplexValue::new(x_pow / (n as f64 * fact), 0.0);
        sum = sum + term;
        if term.abs() < 1e-14 {
            break;
        }
    }
    Ok(sum)
}

fn c_li(x: ComplexValue) -> Result<ComplexValue, String> {
    if x.abs() <= 1e-12 {
        return Ok(ComplexValue::new(0.0, 0.0));
    }
    if (x.re - 1.0).abs() < 1e-12 && x.im.abs() < 1e-12 {
        return Ok(ComplexValue::new(0.0, 0.0));
    }
    if !x.is_real() || x.re <= 0.0 {
        return Err("li currently supports real x > 0".to_string());
    }
    let ln_x = x.re.ln();
    let ln_ln_x = ln_x.ln();
    let euler_gamma = 0.5772156649015329;
    let mut sum = ComplexValue::new(euler_gamma + ln_ln_x, 0.0);
    let mut ln_pow = ln_x;
    for n in 1..100 {
        let fact = (1..=n).fold(1.0, |a, v| a * v as f64);
        let term = ComplexValue::new(ln_pow / (n as f64 * fact), 0.0);
        sum = sum + term;
        if term.abs() < 1e-14 {
            break;
        }
        ln_pow = ln_pow * ln_x;
    }
    Ok(sum)
}

fn c_sinc(x: ComplexValue) -> ComplexValue {
    if x.abs() < 1e-12 {
        return ComplexValue::new(1.0, 0.0);
    }
    c_div(c_sin(x), x).unwrap_or(ComplexValue::new(1.0, 0.0))
}

fn c_ai(x: ComplexValue) -> Result<ComplexValue, String> {
    if !x.is_real() {
        return Err("Ai currently supports real inputs only".to_string());
    }
    let x_val = x.re;
    let mut sum = ComplexValue::new(0.0, 0.0);
    for n in 0..100 {
        let fact3n = (1..=(3*n)).fold(1.0, |a, v| a * v as f64);
        let coeff = if n % 2 == 0 { 1.0 } else { -1.0 };
        let x_pow = x_val.powi((3*n) as i32);
        let denom = fact3n * (3.0_f64.powi(n as i32));
        let term = ComplexValue::new(coeff * x_pow / denom, 0.0);
        sum = sum + term;
        if term.abs() < 1e-14 {
            break;
        }
    }
    Ok(sum / ComplexValue::new(std::f64::consts::PI, 0.0))
}

fn c_bi(x: ComplexValue) -> Result<ComplexValue, String> {
    if !x.is_real() {
        return Err("Bi currently supports real inputs only".to_string());
    }
    let x_val = x.re;
    let sqrt3 = 3.0_f64.sqrt();
    let mut sum = ComplexValue::new(0.0, 0.0);
    for n in 0..100 {
        let fact3n = (1..=(3*n)).fold(1.0, |a, v| a * v as f64);
        let x_pow = x_val.powi((3*n) as i32);
        let denom = fact3n * (3.0_f64.powi(n as i32));
        let term = ComplexValue::new(x_pow / denom, 0.0);
        sum = sum + term;
        if term.abs() < 1e-14 {
            break;
        }
    }
    Ok(sum * ComplexValue::new(sqrt3 / std::f64::consts::PI, 0.0))
}

fn c_theta4(q: ComplexValue, z: ComplexValue) -> Result<ComplexValue, String> {
    // Jacobi theta function: theta4(z, q) = 1 + 2*sum_{n=1}^inf q^{n²} * cos(2nz)
    // For convergence, we need |q| < 1
    if q.abs() >= 1.0 {
        return Err("theta4(q,z) requires |q| < 1 for convergence".to_string());
    }
    let mut sum = ComplexValue::new(1.0, 0.0);
    for n in 1..100 {
        let q_n2 = c_pow(q, ComplexValue::new((n * n) as f64, 0.0))?;
        let cos_2nz = c_cos(z * ComplexValue::new(2.0 * n as f64, 0.0));
        let term = ComplexValue::new(2.0, 0.0) * q_n2 * cos_2nz;
        sum = sum + term;
        if term.abs() < 1e-14 {
            break;
        }
    }
    Ok(sum)
}

fn c_spherical_bessel_j(order: usize, z: ComplexValue) -> Result<ComplexValue, String> {
    if order == 0 {
        if z.is_zero() {
            return Ok(ComplexValue::new(1.0, 0.0));
        }
        return c_div(c_sin(z), z);
    }

    if order == 1 {
        if z.is_zero() {
            return Ok(ComplexValue::new(0.0, 0.0));
        }
        let z2 = z * z;
        return Ok(c_div(c_sin(z), z2)? - c_div(c_cos(z), z)?);
    }

    if z.is_zero() {
        return Ok(ComplexValue::new(0.0, 0.0));
    }

    let mut jm1 = c_div(c_sin(z), z)?;
    let mut j = c_div(c_sin(z), z * z)? - c_div(c_cos(z), z)?;
    for n in 1..order {
        let coeff = ComplexValue::new((2 * n + 1) as f64, 0.0);
        let jp1 = c_div(coeff * j, z)? - jm1;
        jm1 = j;
        j = jp1;
    }
    Ok(j)
}

fn c_polylog(s: ComplexValue, z: ComplexValue) -> Result<ComplexValue, String> {
    if s.re <= 0.0 {
        return Err("polylog currently supports Re(s) > 0".to_string());
    }
    if z.is_zero() {
        return Ok(ComplexValue::new(0.0, 0.0));
    }

    // Fast convergent defining series when |z| is strictly inside the unit circle.
    if z.abs() < 0.9 {
        let mut sum = ComplexValue::new(0.0, 0.0);
        let mut z_pow = z;
        for n in 1..=20000usize {
            let ln_n = (n as f64).ln();
            let n_pow_neg_s = c_exp(ComplexValue::new(-s.re * ln_n, -s.im * ln_n));
            let current = z_pow * n_pow_neg_s;
            sum = sum + current;
            if current.abs() < 1e-14 {
                break;
            }
            z_pow = z_pow * z;
        }
        return Ok(sum);
    }

    // Deterministic continuation via integral representation:
    // Li_s(z) = 1/Gamma(s) * integral_0^inf t^(s-1) * z/(exp(t)-z) dt
    // Works well for complex z away from the positive-real pole line.
    let gamma_s = c_gamma(s)?;
    if gamma_s.abs() < 1e-15 {
        return Err("polylog gamma(s) underflow".to_string());
    }

    let eps = 1e-8;
    let upper = 40.0;
    let n_steps = 1200usize;
    let h = (upper - eps) / (n_steps as f64);
    let one = ComplexValue::new(1.0, 0.0);
    let s_minus_one = s - one;

    let integrand = |t: f64| -> Result<ComplexValue, String> {
        let t_pow = c_exp(s_minus_one * ComplexValue::new(t.ln(), 0.0));
        let denom = ComplexValue::new(t.exp(), 0.0) - z;
        if denom.abs() < 1e-12 {
            return Err("polylog integral encountered pole on contour".to_string());
        }
        Ok(t_pow * c_div(z, denom)?)
    };

    let mut acc = ComplexValue::new(0.0, 0.0);
    for i in 0..=n_steps {
        let t = eps + (i as f64) * h;
        let weight = if i == 0 || i == n_steps {
            1.0
        } else if i % 2 == 0 {
            2.0
        } else {
            4.0
        };
        acc = acc + integrand(t)? * ComplexValue::new(weight, 0.0);
    }
    let integral = acc * ComplexValue::new(h / 3.0, 0.0);
    c_div(integral, gamma_s)
}

fn c_gammainc(a: ComplexValue, z: ComplexValue) -> Result<ComplexValue, String> {
    if a.re <= 0.0 {
        return Err("gammainc currently supports Re(a) > 0".to_string());
    }
    if a.is_zero() {
        return Err("gammainc undefined for a = 0".to_string());
    }

    let mut series = ComplexValue::new(1.0, 0.0) / a;
    let mut term = series;
    for n in 0..500usize {
        let denom = a + ComplexValue::new((n + 1) as f64, 0.0);
        term = c_div(term * z, denom)?;
        series = series + term;
        if term.abs() < 1e-14 {
            break;
        }
    }
    Ok(c_pow(z, a)? * c_exp(-z) * series)
}

#[derive(Clone, Debug)]
enum Token {
    Number(f64),
    Identifier(String),
    Imaginary,
    Comma,
    Bang,
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    LParen,
    RParen,
    LBracket,
    RBracket,
    EqEq,
    NotEq,
    Lt,
    Le,
    Gt,
    Ge,
    AndAnd,
    OrOr,
    XorCaret,
    ImpliesArrow,
    EquivArrow,
}

#[derive(Clone, Debug)]
enum ParsedMathInput {
    Scalar(AstNode),
    Logic(LogicExprNode),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum AstNode {
    Number(f64),
    Variable(String),
    Matrix(Vec<Vec<AstNode>>),
    UnaryNeg(Box<AstNode>),
    Function {
        name: String,
        args: Vec<AstNode>,
    },
    Binary {
        op: char,
        left: Box<AstNode>,
        right: Box<AstNode>,
    },
}

#[derive(Clone, Debug)]
struct IndexEntry {
    crystal_id: String,
    edge_id: String,
    source_node: String,
    relation: String,
    target_node: String,
    searchable_text: String,
}

#[derive(Clone, Debug)]
struct BankIndex {
    entries: Vec<IndexEntry>,
    postings: HashMap<String, Vec<usize>>,
}

#[derive(Clone)]
struct SenseSpec {
    node_id: &'static str,
    label: &'static str,
    anchor_terms: &'static [&'static str],
}

#[derive(Clone, Copy)]
struct LexiconEntry {
    pack: &'static str,
    language: &'static str,
    lemma: &'static str,
    concept_node: &'static str,
    relation: &'static str,
    related_terms: &'static [&'static str],
}

#[derive(Clone)]
struct MatchedLexiconEdge {
    entry: LexiconEntry,
    matched_token: String,
    via: &'static str,
}

#[derive(Clone, Copy)]
struct LexemeIdentity {
    canonical_language: &'static str,
    canonical_token: &'static str,
    canonical_lexeme_node: &'static str,
    aliases: &'static [&'static str],
}

#[derive(Clone, Debug, Default)]
struct LexemeInertiaProfile {
    crystallization_depth: f64,
    last_selected_sense: Option<String>,
    current_streak: usize,
    resolved_count: usize,
}

#[derive(Clone, Copy)]
struct PhraseAliasSpec {
    language: &'static str,
    surface: &'static str,
    pattern: &'static [&'static str],
}

#[derive(Clone, Copy)]
struct PhraseSpec {
    node_id: &'static str,
    label: &'static str,
    aliases: &'static [PhraseAliasSpec],
    preferred_sense: &'static str,
    lobe: &'static str,
}

#[derive(Clone)]
struct PhraseMatch {
    spec: PhraseSpec,
    matched_language: &'static str,
    matched_surface: &'static str,
    matched_pattern: &'static [&'static str],
    phrase_text: String,
    phrase_coherence_score: f64,
    token_composition_score: f64,
    composition_depth: usize,
    span_start: Option<usize>,
    span_end_exclusive: Option<usize>,
    overlap_group: usize,
    overlaps_with: Vec<String>,
}

fn unix_time_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn unix_time_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

fn emit_time_crystal_randomness_profile() -> bool {
    match std::env::var("CSIF_EMIT_TIME_CRYSTAL_RANDOMNESS") {
        Ok(raw) => {
            let normalized = raw.trim().to_ascii_lowercase();
            matches!(normalized.as_str(), "1" | "true" | "yes" | "on")
        }
        Err(_) => false,
    }
}

fn time_crystal_coordinate() -> (u128, String) {
    match std::env::var("CSIF_TIME_CRYSTAL_T_NS") {
        Ok(raw) => match raw.parse::<u128>() {
            Ok(parsed) => (parsed, "env:CSIF_TIME_CRYSTAL_T_NS".to_string()),
            Err(_) => (unix_time_nanos(), "system_clock_invalid_env_fallback".to_string()),
        },
        Err(_) => (unix_time_nanos(), "system_clock".to_string()),
    }
}

fn build_time_crystal_randomness_payload(
    expr: &str,
    mode_txt: &str,
    angle_txt: &str,
) -> Option<(Value, Value)> {
    if !emit_time_crystal_randomness_profile() {
        return None;
    }

    let (t_ns, coordinate_source) = time_crystal_coordinate();
    let cycle = 1_000_000_000_u128;
    let phase_frac = (t_ns % cycle) as f64 / cycle as f64;
    let phase_theta = wrap_to_pi(phase_frac * 2.0 * std::f64::consts::PI);
    let torsion_norm = ((t_ns / 97) % 1000) as f64 / 1000.0;

    let replay_seed = format!(
        "expr={}::mode={}::angle={}::t_ns={}::engine=deterministic_math_v2",
        expr, mode_txt, angle_txt, t_ns
    );
    let replay_key = stable_bridge_id("time_replay", &replay_seed);
    let audit_trace_id = stable_bridge_id("time_rand_audit", &replay_seed);

    let time_crystal = json!({
        "t_ns": t_ns,
        "phase_theta": phase_theta,
        "torsion_norm": torsion_norm,
        "coordinate_source": coordinate_source,
    });

    let randomness_appearance = json!({
        "mode": "deterministic_time_chaos",
        "replay_key": replay_key,
        "audit_trace_id": audit_trace_id,
        "deterministic": true,
    });

    Some((time_crystal, randomness_appearance))
}

fn token_count(text: &str) -> usize {
    text.split_whitespace().count().max(1)
}

fn extract_text_content(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(parts) => {
            let mut out = Vec::new();
            for p in parts {
                if let Some(text) = p.get("text").and_then(Value::as_str) {
                    out.push(text.to_string());
                }
            }
            out.join(" ")
        }
        _ => String::new(),
    }
}

fn last_user_prompt(messages: &[ChatMessage]) -> String {
    for msg in messages.iter().rev() {
        if msg.role == "user" {
            let text = extract_text_content(&msg.content);
            if !text.is_empty() {
                return text;
            }
        }
    }
    messages
        .last()
        .map(|m| extract_text_content(&m.content))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "No user prompt provided.".to_string())
}

fn recent_user_prompts(messages: &[ChatMessage], max_count: usize) -> Vec<String> {
    let mut prompts = Vec::new();
    for msg in messages.iter().rev() {
        if msg.role != "user" {
            continue;
        }
        let text = extract_text_content(&msg.content);
        if text.is_empty() {
            continue;
        }
        prompts.push(text);
        if prompts.len() >= max_count {
            break;
        }
    }
    prompts.reverse();
    prompts
}

fn looks_like_math_expression(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed.starts_with("/math ") || trimmed.starts_with("math:") || trimmed.starts_with("calc:") {
        return true;
    }

    let has_digit = trimmed.chars().any(|c| c.is_ascii_digit());
    let has_operator = trimmed
        .chars()
        .any(|c| matches!(c, '+' | '-' | '*' | '/' | '^' | '!' | '(' | ')' | '='));
    let has_alpha = trimmed.chars().any(|c| c.is_ascii_alphabetic());

    // Avoid classifying plain-language text as math if it lacks explicit math syntax.
    if !has_digit && !has_operator {
        return false;
    }
    if has_alpha && !has_operator && !trimmed.contains('(') {
        return false;
    }

    // Heuristic: allow direct calculator-style prompts such as "(2+3i)^2".
    trimmed.chars().all(|c| {
        c.is_ascii_alphanumeric()
            || matches!(c, '+' | '-' | '*' | '/' | '^' | '!' | '(' | ')' | '.' | ',' | '_' | ' ')
    })
}

fn normalize_chat_math_candidate(prompt: &str) -> &str {
    let trimmed = prompt.trim();
    if let Some(rest) = trimmed.strip_prefix("/math ") {
        return rest.trim();
    }
    if let Some(rest) = trimmed.strip_prefix("math:") {
        return rest.trim();
    }
    if let Some(rest) = trimmed.strip_prefix("calc:") {
        return rest.trim();
    }
    trimmed
}

fn detect_chat_intent(prompt: &str) -> &'static str {
    let p = prompt.to_ascii_lowercase();
    if p.contains("hello") || p.contains("hi") || p.contains("hey") {
        "greeting"
    } else if p.contains("who are you") || p.contains("what are you") {
        "identity"
    } else if p.contains("thank") {
        "thanks"
    } else if p.contains("help") || p.contains("how do i") || p.contains("how can i") {
        "help"
    } else if p.contains("not working")
        || p.contains("failed")
        || p.contains("failing")
        || p.contains("cannot")
        || p.contains("can't")
    {
        "troubleshooting"
    } else if looks_like_math_expression(prompt) {
        "math"
    } else {
        "general"
    }
}

fn prefers_concise_reply(prompt: &str) -> bool {
    let p = prompt.to_ascii_lowercase();
    p.contains("brief")
        || p.contains("concise")
        || p.contains("short")
        || p.contains("quick answer")
        || p.contains("tldr")
}

fn normalize_response_style(value: Option<&str>, prompt: &str) -> &'static str {
    match value
        .map(|v| v.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("concise") | Some("brief") | Some("short") => "concise",
        Some("standard") | Some("balanced") | Some("normal") => "standard",
        _ => {
            if prefers_concise_reply(prompt) {
                "concise"
            } else {
                "standard"
            }
        }
    }
}

fn normalize_depth(value: Option<&str>) -> &'static str {
    match value
        .map(|v| v.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("shallow") | Some("brief") => "shallow",
        Some("deep") | Some("detailed") => "deep",
        _ => "standard",
    }
}

fn normalize_tone(value: Option<&str>) -> &'static str {
    match value
        .map(|v| v.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("professional") => "professional",
        Some("direct") => "direct",
        _ => "friendly",
    }
}

fn normalize_warmth_ceiling(value: Option<&str>) -> &'static str {
    match value
        .map(|v| v.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("subtle") | Some("low") | Some("minimal") => "subtle",
        Some("expressive") | Some("high") | Some("vivid") => "expressive",
        Some("balanced") | Some("medium") | Some("normal") => "balanced",
        _ => "balanced",
    }
}

fn env_chat_greeting_warmth_ceiling() -> &'static str {
    normalize_warmth_ceiling(std::env::var("CSIF_CHAT_GREETING_WARMTH_CEILING").ok().as_deref())
}

fn resolve_chat_preferences(pref: Option<&ChatPreferencesRequest>, prompt: &str) -> ChatPreferences {
    let requested_top_k = pref.and_then(|p| p.retrieval_top_k);
    let response_style = normalize_response_style(pref.and_then(|p| p.response_style.as_deref()), prompt);
    let depth = normalize_depth(pref.and_then(|p| p.depth.as_deref()));
    let tone = normalize_tone(pref.and_then(|p| p.tone.as_deref()));
    let warmth_ceiling = normalize_warmth_ceiling(
        pref.and_then(|p| p.warmth_ceiling.as_deref())
            .or(Some(env_chat_greeting_warmth_ceiling())),
    );

    let default_top_k = match depth {
        "deep" => 6,
        "shallow" => 2,
        _ => {
            if response_style == "concise" {
                2
            } else {
                3
            }
        }
    };

    ChatPreferences {
        response_style,
        depth,
        tone,
        warmth_ceiling,
        retrieval_summary: pref.and_then(|p| p.retrieval_summary).unwrap_or(true),
        retrieval_top_k: requested_top_k.unwrap_or(default_top_k).clamp(1, 12),
    }
}

fn emit_chat_time_crystal_opening_variation() -> bool {
    match std::env::var("CSIF_CHAT_TIME_CRYSTAL_OPENING_VARIATION") {
        Ok(raw) => {
            let normalized = raw.trim().to_ascii_lowercase();
            !matches!(normalized.as_str(), "0" | "false" | "no" | "off")
        }
        Err(_) => true,
    }
}

fn build_chat_time_crystal_context() -> Option<ChatTimeCrystalContext> {
    if !emit_chat_time_crystal_opening_variation() {
        return None;
    }

    let (t_ns, coordinate_source) = time_crystal_coordinate();
    let cycle = 1_000_000_000_u128;
    let phase_frac = (t_ns % cycle) as f64 / cycle as f64;
    let phase_theta = wrap_to_pi(phase_frac * 2.0 * std::f64::consts::PI);
    let torsion_norm = ((t_ns / 97) % 1000) as f64 / 1000.0;

    Some(ChatTimeCrystalContext {
        t_ns,
        phase_theta,
        torsion_norm,
        coordinate_source,
    })
}

fn select_time_crystal_variant(
    slot: &str,
    variants: &[(&'static str, &'static str)],
    context: &ChatTimeCrystalContext,
) -> (&'static str, &'static str, usize) {
    let seed = value_hash(&json!({
        "slot": slot,
        "t_ns": context.t_ns,
        "phase_theta": context.phase_theta,
        "torsion_norm": context.torsion_norm,
    }));
    let variant_index = (seed as usize) % variants.len();
    let (variant_id, text) = variants[variant_index];
    (variant_id, text, variant_index)
}

fn time_crystal_variant_meta(
    enabled: bool,
    reason: &str,
    mode: &str,
    variant_id: Option<&str>,
    variant_index: Option<usize>,
    variant_count: usize,
    context: Option<&ChatTimeCrystalContext>,
) -> Value {
    if enabled {
        json!({
            "enabled": true,
            "reason": reason,
            "mode": mode,
            "variant_id": variant_id,
            "variant_index": variant_index,
            "variant_count": variant_count,
            "time_crystal": {
                "t_ns": context.map(|c| c.t_ns),
                "phase_theta": context.map(|c| c.phase_theta),
                "torsion_norm": context.map(|c| c.torsion_norm),
                "coordinate_source": context.map(|c| c.coordinate_source.clone()),
            }
        })
    } else {
        json!({
            "enabled": false,
            "reason": reason,
        })
    }
}

fn default_conversational_opening(intent: &str, tone: &str) -> &'static str {
    match tone {
        "professional" => match intent {
            "greeting" => "Hello. I am ready to assist.",
            "identity" => "I am UGC-Model, a deterministic assistant built on CSIF and RWIF.",
            "thanks" => "You are welcome. I am available for the next task.",
            "help" => "Please share your target outcome and I will provide a structured plan.",
            "troubleshooting" => {
                "Understood. We can isolate the issue with a deterministic troubleshooting pass."
            }
            "math" => "Acknowledged. I will solve it deterministically.",
            _ => "Understood. I will provide a clear and structured response.",
        },
        "direct" => match intent {
            "greeting" => "Ready.",
            "identity" => "I am UGC-Model, deterministic and CSIF/RWIF-backed.",
            "thanks" => "Anytime.",
            "help" => "Share the goal and I will give exact steps.",
            "troubleshooting" => "Let us isolate the fault quickly.",
            "math" => "Let us solve it now.",
            _ => "Proceeding with your request.",
        },
        _ => match intent {
            "greeting" => "Hey, great to connect. I am ready to help.",
            "identity" => "I am UGC-Model, a deterministic assistant built on CSIF and RWIF.",
            "thanks" => "You are welcome. Happy to keep going with you.",
            "help" => {
                "Absolutely. Tell me the outcome you want, and I will help you get there step by step."
            }
            "troubleshooting" => {
                "Thanks for flagging that. We can isolate the issue quickly with a focused check."
            }
            "math" => "Great, let us solve it deterministically.",
            _ => "I am with you. Let us work through it clearly.",
        },
    }
}

fn greeting_opening_variants(tone: &str) -> &'static [(&'static str, &'static str, &'static str)] {
    const FRIENDLY: [(&str, &str, &str); 5] = [
        ("friendly_g1", "Hey, great to connect. I am ready to help.", "balanced"),
        ("friendly_g2", "Hi there. Glad you are here, let us build something useful.", "expressive"),
        ("friendly_g3", "Hello. I am online and ready when you are.", "subtle"),
        ("friendly_g4", "Hey. Good to see you. We can start anywhere you want.", "expressive"),
        ("friendly_g5", "Hi. I am active and ready to jump in.", "subtle"),
    ];
    const PROFESSIONAL: [(&str, &str, &str); 4] = [
        ("professional_g1", "Hello. I am ready to assist.", "subtle"),
        ("professional_g2", "Greetings. I am prepared to help with your objective.", "balanced"),
        ("professional_g3", "Hello. I am online and available for your next task.", "subtle"),
        ("professional_g4", "Good day. I am ready for a structured working session.", "expressive"),
    ];
    const DIRECT: [(&str, &str, &str); 4] = [
        ("direct_g1", "Ready.", "subtle"),
        ("direct_g2", "Online. Send the task.", "subtle"),
        ("direct_g3", "Active. What do you want to do first?", "balanced"),
        ("direct_g4", "Ready to execute. Share the target.", "expressive"),
    ];

    match tone {
        "professional" => &PROFESSIONAL,
        "direct" => &DIRECT,
        _ => &FRIENDLY,
    }
}

fn greeting_variant_rank(level: &str) -> u8 {
    match level {
        "subtle" => 1,
        "balanced" => 2,
        "expressive" => 3,
        _ => 2,
    }
}

fn greeting_variants_for_warmth(
    tone: &str,
    warmth_ceiling: &str,
) -> Vec<(&'static str, &'static str)> {
    let max_rank = greeting_variant_rank(warmth_ceiling);
    let mut selected = greeting_opening_variants(tone)
        .iter()
        .filter(|(_, _, level)| greeting_variant_rank(level) <= max_rank)
        .map(|(id, text, _)| (*id, *text))
        .collect::<Vec<_>>();

    if selected.is_empty() {
        if let Some((id, text, _)) = greeting_opening_variants(tone).first() {
            selected.push((*id, *text));
        }
    }

    selected
}

fn conversational_opening(
    intent: &str,
    tone: &str,
    warmth_ceiling: &str,
    time_crystal_context: Option<&ChatTimeCrystalContext>,
) -> (String, Value) {
    if intent != "greeting" {
        return (
            default_conversational_opening(intent, tone).to_string(),
            time_crystal_variant_meta(
                false,
                "non_greeting_intent",
                "deterministic_time_crystal_greeting_variation",
                None,
                None,
                0,
                None,
            ),
        );
    }

    let Some(context) = time_crystal_context else {
        return (
            default_conversational_opening(intent, tone).to_string(),
            time_crystal_variant_meta(
                false,
                "disabled_by_env",
                "deterministic_time_crystal_greeting_variation",
                None,
                None,
                0,
                None,
            ),
        );
    };

    let variants = greeting_variants_for_warmth(tone, warmth_ceiling);
    let (variant_id, opening_text, variant_index) =
        select_time_crystal_variant("opening", variants.as_slice(), context);

    (
        opening_text.to_string(),
        time_crystal_variant_meta(
            true,
            "applied",
            "deterministic_time_crystal_greeting_variation",
            Some(variant_id),
            Some(variant_index),
            variants.len(),
            Some(context),
        ),
    )
}

fn retrieval_fallback_variants(kind: &str, tone: &str, concise: bool) -> &'static [(&'static str, &'static str)] {
    match (kind, tone, concise) {
        ("no_index_loaded", "professional", _) => &[
            (
                "no_index_prof_1",
                "RWIF retrieval is currently offline because no bank index is loaded; I can still provide direct guidance.",
            ),
            (
                "no_index_prof_2",
                "No RWIF index is loaded yet, so retrieval evidence is unavailable; I can continue with direct deterministic guidance.",
            ),
        ],
        ("no_index_loaded", "direct", _) => &[
            (
                "no_index_direct_1",
                "No RWIF bank index is loaded right now. I can still guide directly.",
            ),
            (
                "no_index_direct_2",
                "Retrieval is offline until a RWIF index is loaded. Direct guidance is still available.",
            ),
        ],
        ("no_index_loaded", _, true) => &[
            (
                "no_index_friendly_concise_1",
                "No RWIF bank index is loaded yet; I can still guide directly.",
            ),
            (
                "no_index_friendly_concise_2",
                "RWIF retrieval is offline right now, but direct help is still available.",
            ),
        ],
        ("no_index_loaded", _, false) => &[
            (
                "no_index_friendly_1",
                "RWIF retrieval is currently offline because no bank index is loaded; I can still provide direct guidance.",
            ),
            (
                "no_index_friendly_2",
                "I do not have a loaded RWIF index yet, so retrieval evidence is offline, but I can still help with direct reasoning.",
            ),
        ],
        ("no_match_hits", "professional", _) => &[
            (
                "no_match_prof_1",
                "I could not find matching indexed RWIF facts for that prompt.",
            ),
            (
                "no_match_prof_2",
                "Indexed RWIF retrieval returned no matching evidence for that prompt.",
            ),
        ],
        ("no_match_hits", "direct", _) => &[
            (
                "no_match_direct_1",
                "No matching RWIF facts were found for that prompt.",
            ),
            (
                "no_match_direct_2",
                "No indexed evidence matched that prompt.",
            ),
        ],
        ("no_match_hits", _, _) => &[
            (
                "no_match_friendly_1",
                "I could not find matching indexed RWIF facts for that prompt.",
            ),
            (
                "no_match_friendly_2",
                "I checked the current RWIF index and did not find matching evidence for that prompt.",
            ),
        ],
        _ => &[("fallback_default", "I can still continue with deterministic guidance.")],
    }
}

fn retrieval_fallback_text(
    kind: &str,
    tone: &str,
    concise: bool,
    time_crystal_context: Option<&ChatTimeCrystalContext>,
) -> (String, Value) {
    let variants = retrieval_fallback_variants(kind, tone, concise);
    if let Some(context) = time_crystal_context {
        let (variant_id, text, variant_index) =
            select_time_crystal_variant(&format!("retrieval_fallback:{}", kind), variants, context);
        (
            text.to_string(),
            time_crystal_variant_meta(
                true,
                "applied",
                "deterministic_time_crystal_retrieval_fallback_variation",
                Some(variant_id),
                Some(variant_index),
                variants.len(),
                Some(context),
            ),
        )
    } else {
        (
            variants[0].1.to_string(),
            time_crystal_variant_meta(
                false,
                "disabled_by_env",
                "deterministic_time_crystal_retrieval_fallback_variation",
                None,
                None,
                variants.len(),
                None,
            ),
        )
    }
}

fn next_options_heading_variants(tone: &str) -> &'static [(&'static str, &'static str)] {
    match tone {
        "professional" => &[
            ("next_prof_1", "Next options:"),
            ("next_prof_2", "Next options: evaluate these paths."),
            ("next_prof_3", "Next options: continue this session with one of these."),
        ],
        "direct" => &[
            ("next_direct_1", "Next options:"),
            ("next_direct_2", "Next options: execute one of these now."),
            ("next_direct_3", "Next options: pick a direction now."),
        ],
        _ => &[
            ("next_friendly_1", "Next options:"),
            ("next_friendly_2", "Next options: choose whichever feels best."),
            ("next_friendly_3", "Next options: keep momentum with one of these."),
        ],
    }
}

fn next_options_heading(
    tone: &str,
    time_crystal_context: Option<&ChatTimeCrystalContext>,
) -> (String, Value) {
    let variants = next_options_heading_variants(tone);
    if let Some(context) = time_crystal_context {
        let (variant_id, text, variant_index) =
            select_time_crystal_variant("next_options_heading", variants, context);
        (
            text.to_string(),
            time_crystal_variant_meta(
                true,
                "applied",
                "deterministic_time_crystal_next_options_variation",
                Some(variant_id),
                Some(variant_index),
                variants.len(),
                Some(context),
            ),
        )
    } else {
        (
            "Next options:".to_string(),
            time_crystal_variant_meta(
                false,
                "disabled_by_env",
                "deterministic_time_crystal_next_options_variation",
                None,
                None,
                variants.len(),
                None,
            ),
        )
    }
}

fn context_bridge_text(context_items: &[String], concise: bool) -> Option<String> {
    if context_items.is_empty() {
        return None;
    }

    if concise {
        if let Some(last) = context_items.last() {
            return Some(format!("Context carryover: you previously asked about '{}'.", last));
        }
        return None;
    }

    let sample = context_items
        .iter()
        .rev()
        .take(2)
        .cloned()
        .collect::<Vec<_>>();
    let summary = sample
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join(" | ");
    Some(format!(
        "Context carryover: I am tracking your recent thread: {}.",
        summary
    ))
}

fn follow_up_suggestions(
    intent: &str,
    has_retrieval_hits: bool,
    has_math_result: bool,
    concise: bool,
) -> Vec<&'static str> {
    let mut items = Vec::new();

    match intent {
        "help" | "troubleshooting" => {
            items.push("Share your exact goal and I will produce a concrete execution plan.");
            items.push("Paste a failing command, API payload, or output and I will debug it with you.");
        }
        "math" => {
            items.push("Send another expression for a deterministic solve.");
            items.push("Ask for geometric mode or angle-unit control if you want trace-aware trig behavior.");
        }
        _ => {
            items.push("Ask for a concise answer or a deep walkthrough, and I will adapt.");
        }
    }

    if has_retrieval_hits {
        items.push("Ask me to summarize retrieval evidence into plain-language conclusions.");
    } else {
        items.push("If you load a RWIF bank index, I can cite evidence directly from your data.");
    }

    if has_math_result {
        items.push("I can explain each step behind the computed result if you want the full reasoning path.");
    }

    if concise && items.len() > 2 {
        items.truncate(2);
    }

    items
}

fn capability_hint_text(concise: bool) -> &'static str {
    if concise {
        "I can handle chat, math, retrieval, and semantic disambiguation in one flow."
    } else {
        "I can help across conversational guidance, deterministic math, RWIF-backed retrieval, and semantic disambiguation, then turn that into practical next actions."
    }
}

fn summarize_retrieval_readability(matches: &[Value], top_k: usize) -> Value {
    if matches.is_empty() {
        return json!({
            "enabled": true,
            "coverage": 0.0,
            "mean_score": 0.0,
            "readability_score": 1.0,
            "summary_quality": "none",
            "summary_text": "No indexed evidence matched this prompt.",
        });
    }

    let score_sum = matches
        .iter()
        .filter_map(|m| m.get("score").and_then(Value::as_f64))
        .sum::<f64>();
    let mean_score = score_sum / matches.len() as f64;
    let coverage = matches.len() as f64 / top_k.max(1) as f64;
    let compactness_penalty = (matches.len().saturating_sub(3) as f64 * 0.08).min(0.35);
    let readability_score = (0.62 * mean_score + 0.38 * coverage - compactness_penalty).clamp(0.0, 1.0);
    let summary_quality = if readability_score >= 0.80 {
        "high"
    } else if readability_score >= 0.55 {
        "medium"
    } else {
        "low"
    };
    let summary_text = format!(
        "Evidence readability is {} (score {:.2}) across {} matches; top relations were compacted into a readable summary.",
        summary_quality,
        readability_score,
        matches.len()
    );

    json!({
        "enabled": true,
        "coverage": coverage,
        "mean_score": mean_score,
        "readability_score": readability_score,
        "summary_quality": summary_quality,
        "summary_text": summary_text,
    })
}

fn summarize_bank(bank: &Value) -> BankSummary {
    let mut crystal_count = 0usize;
    let mut edge_count = 0usize;
    let mut event_count = 0usize;

    let bank_id = bank
        .get("bank_id")
        .and_then(Value::as_str)
        .unwrap_or("<unknown_bank>")
        .to_string();

    if let Some(crystals) = bank.get("crystals").and_then(Value::as_array) {
        crystal_count = crystals.len();
        for crystal in crystals {
            if let Some(edges) = crystal.get("edges").and_then(Value::as_array) {
                edge_count += edges.len();
                for edge in edges {
                    if let Some(events) = edge.get("phase_trajectory").and_then(Value::as_array) {
                        event_count += events.len();
                    }
                }
            }
        }
    }

    BankSummary {
        bank_id,
        crystal_count,
        edge_count,
        event_count,
    }
}

fn normalize_token(token: &str) -> Option<String> {
    let cleaned = token
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect::<String>()
        .trim()
        .to_string();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

fn tokenize(text: &str) -> Vec<String> {
    text.split_whitespace().filter_map(normalize_token).collect()
}

fn rewrite_retrieval_query(query: &str) -> (String, Vec<String>) {
    let base_tokens = tokenize(query);
    let mut expanded = base_tokens.clone();
    let mut reasons = Vec::<String>::new();

    if base_tokens.iter().any(|t| t == "luz" || t == "lumiere" || t == "光") {
        expanded.push("light".to_string());
        reasons.push("cross_language_alias_to_canonical_light".to_string());
    }

    let query_lower = query.to_lowercase();
    if query_lower.contains("light speed")
        || query_lower.contains("velocidad de la luz")
        || query_lower.contains("vitesse de la lumi")
        || query.contains("光速")
    {
        expanded.push("light".to_string());
        expanded.push("speed".to_string());
        reasons.push("phrase_alias_expansion_light_speed".to_string());
    }

    expanded.sort();
    expanded.dedup();

    (expanded.join(" "), reasons)
}

fn classify_math_error(error_message: &str) -> (&'static str, &'static str) {
    let lower = error_message.to_lowercase();
    if lower.contains("unsupported function") {
        ("unsupported_function", "MATH_UNSUPPORTED_FUNCTION")
    } else {
        ("parse_error", "MATH_PARSE_ERROR")
    }
}

fn normalize_lexeme_token(token: &str) -> String {
    token.trim().to_lowercase()
}

fn normalize_frame_value(value: Option<&str>, fallback: &str) -> String {
    value
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| v.to_lowercase().replace(' ', "_"))
        .unwrap_or_else(|| fallback.to_string())
}

fn default_frame_context() -> FrameContext {
    FrameContext {
        observer_frame: "internal_observer".to_string(),
        ontology_frame: "general_ontology".to_string(),
        temporal_frame: "present".to_string(),
        modality_frame: "assertive".to_string(),
        epistemic_source_frame: "user_reported".to_string(),
    }
}

fn default_conservation_policy() -> ConservationPolicy {
    ConservationPolicy {
        required_invariants: vec![
            "identity_continuity".to_string(),
            "causal_continuity".to_string(),
            "topology_continuity".to_string(),
            "observer_consistency".to_string(),
            "modality_preservation".to_string(),
        ],
        allow_lossy: false,
        max_total_loss: 0.8,
    }
}

fn resolve_conservation_policy(policy: Option<&ConservationPolicyRequest>) -> ConservationPolicy {
    let default = default_conservation_policy();
    let required_invariants = policy
        .and_then(|p| p.required_invariants.clone())
        .unwrap_or_else(|| default.required_invariants.clone())
        .into_iter()
        .map(|inv| inv.trim().to_lowercase())
        .filter(|inv| !inv.is_empty())
        .collect::<Vec<_>>();
    ConservationPolicy {
        required_invariants: if required_invariants.is_empty() {
            default.required_invariants
        } else {
            required_invariants
        },
        allow_lossy: policy
            .and_then(|p| p.allow_lossy)
            .unwrap_or(default.allow_lossy),
        max_total_loss: policy
            .and_then(|p| p.max_total_loss)
            .unwrap_or(default.max_total_loss)
            .max(0.0),
    }
}

fn all_lexicon_pack_ids() -> Vec<&'static str> {
    vec![
        "csif_compact_lexicon_v1",
        "csif_compact_lexicon_legacy_v2",
        "csif_compact_lexicon_reasoning_v2",
        "csif_compact_lexicon_etymology_v3",
    ]
}

fn default_lexicon_pack_weights() -> HashMap<String, f64> {
    HashMap::from([
        ("csif_compact_lexicon_v1".to_string(), 1.0),
        ("csif_compact_lexicon_legacy_v2".to_string(), 0.9),
        ("csif_compact_lexicon_reasoning_v2".to_string(), 1.1),
        ("csif_compact_lexicon_etymology_v3".to_string(), 1.15),
    ])
}

fn resolve_lexicon_control(requested_packs: Option<&Vec<String>>) -> LexiconControl {
    let known_packs = all_lexicon_pack_ids()
        .into_iter()
        .map(|p| p.to_string())
        .collect::<HashSet<_>>();

    let requested = requested_packs
        .map(|packs| {
            packs
                .iter()
                .map(|p| p.trim().to_lowercase())
                .filter(|p| known_packs.contains(p))
                .collect::<HashSet<_>>()
        })
        .unwrap_or_default();

    let active_packs = if requested.is_empty() {
        known_packs
    } else {
        requested
    };

    let pack_weights = default_lexicon_pack_weights()
        .into_iter()
        .filter(|(pack, _)| active_packs.contains(pack))
        .collect::<HashMap<_, _>>();

    LexiconControl {
        active_packs,
        pack_weights,
    }
}

fn conservation_invariant_profile(name: &str, preserved: bool, delta: f64, reason: &str) -> Value {
    json!({
        "name": name,
        "preserved": preserved,
        "delta": if preserved { 0.0 } else { delta },
        "violation_reason": if preserved { Value::Null } else { json!(reason) },
    })
}

fn resolve_frame_context(frame: Option<&FrameContextRequest>) -> FrameContext {
    let default = default_frame_context();
    FrameContext {
        observer_frame: normalize_frame_value(
            frame.and_then(|f| f.observer_frame.as_deref()),
            &default.observer_frame,
        ),
        ontology_frame: normalize_frame_value(
            frame.and_then(|f| f.ontology_frame.as_deref()),
            &default.ontology_frame,
        ),
        temporal_frame: normalize_frame_value(
            frame.and_then(|f| f.temporal_frame.as_deref()),
            &default.temporal_frame,
        ),
        modality_frame: normalize_frame_value(
            frame.and_then(|f| f.modality_frame.as_deref()),
            &default.modality_frame,
        ),
        epistemic_source_frame: normalize_frame_value(
            frame.and_then(|f| f.epistemic_source_frame.as_deref()),
            &default.epistemic_source_frame,
        ),
    }
}

fn frame_signature(frame: &FrameContext) -> String {
    format!(
        "obs:{}|onto:{}|time:{}|mod:{}|epi:{}",
        frame.observer_frame,
        frame.ontology_frame,
        frame.temporal_frame,
        frame.modality_frame,
        frame.epistemic_source_frame
    )
}

fn frame_alignment_score(prior: &FrameContext, active: &FrameContext) -> f64 {
    let mut matches = 0usize;
    if prior.observer_frame == active.observer_frame {
        matches += 1;
    }
    if prior.ontology_frame == active.ontology_frame {
        matches += 1;
    }
    if prior.temporal_frame == active.temporal_frame {
        matches += 1;
    }
    if prior.modality_frame == active.modality_frame {
        matches += 1;
    }
    if prior.epistemic_source_frame == active.epistemic_source_frame {
        matches += 1;
    }
    matches as f64 / 5.0
}

fn frame_reconciliation_status(alignment_score: f64) -> &'static str {
    if alignment_score >= 1.0 {
        "stable"
    } else if alignment_score >= 0.8 {
        "compatible_shift"
    } else if alignment_score >= 0.5 {
        "partial_shift"
    } else {
        "frame_conflict"
    }
}

fn contradiction_type_for_frame(
    frame_status: &str,
    active: &FrameContext,
    context: &str,
) -> &'static str {
    let context_lower = context.to_lowercase();
    if frame_status == "frame_conflict" {
        "frame_mismatch"
    } else if active.ontology_frame.contains("quantum")
        && active.ontology_frame.contains("classical")
    {
        "ontology_collision"
    } else if context_lower.contains("scope") || context_lower.contains("local") {
        "scope_mismatch"
    } else if active.observer_frame.contains("third_person")
        && active.epistemic_source_frame.contains("first_hand")
    {
        "perspective_divergence"
    } else {
        "global_contradiction"
    }
}

fn frame_bonus_for_sense(frame: &FrameContext, sense_node_id: &str) -> f64 {
    if frame.ontology_frame.contains("electromagnetic") || frame.ontology_frame.contains("physics")
    {
        if sense_node_id == "sense_light_visible_em" {
            0.75
        } else {
            0.0
        }
    } else if frame.ontology_frame.contains("pedagogy") || frame.ontology_frame.contains("cognition")
    {
        if sense_node_id == "sense_light_insight" {
            0.75
        } else {
            0.0
        }
    } else {
        0.0
    }
}

fn sense_concept_node(sense_node_id: &str) -> &'static str {
    match sense_node_id {
        "sense_light_insight" => "concept_light_insight",
        "sense_light_visible_em" => "concept_light_visible",
        "sense_light_low_mass" => "concept_light_weight",
        "sense_light_ignition" => "concept_light_ignition",
        "sense_light_brightness_state" => "concept_light_brightness",
        _ => "concept_unknown",
    }
}

fn compact_lexicon_pack() -> Vec<LexiconEntry> {
    let mut entries = vec![
        LexiconEntry {
            pack: "csif_compact_lexicon_v1",
            language: "en",
            lemma: "light",
            concept_node: "concept_light_visible",
            relation: "maps_to_concept",
            related_terms: &["visible", "brightness", "radiation", "vision", "illumination"],
        },
        LexiconEntry {
            pack: "csif_compact_lexicon_v1",
            language: "en",
            lemma: "insight",
            concept_node: "concept_light_insight",
            relation: "maps_to_concept",
            related_terms: &["understanding", "clarity", "realization", "learn"],
        },
        LexiconEntry {
            pack: "csif_compact_lexicon_v1",
            language: "en",
            lemma: "lightweight",
            concept_node: "concept_light_weight",
            relation: "maps_to_concept",
            related_terms: &["light", "weight", "portable", "mass"],
        },
        LexiconEntry {
            pack: "csif_compact_lexicon_v1",
            language: "es",
            lemma: "luz",
            concept_node: "concept_light_visible",
            relation: "maps_to_concept",
            related_terms: &["visible", "luminosidad", "radiacion", "ojos", "ver"],
        },
        LexiconEntry {
            pack: "csif_compact_lexicon_v1",
            language: "es",
            lemma: "entendimiento",
            concept_node: "concept_light_insight",
            relation: "maps_to_concept",
            related_terms: &["entender", "comprension", "claridad", "aprender"],
        },
        LexiconEntry {
            pack: "csif_compact_lexicon_v1",
            language: "fr",
            lemma: "lumiere",
            concept_node: "concept_light_visible",
            relation: "maps_to_concept",
            related_terms: &["lumière", "visible", "luminosite", "vision", "voir"],
        },
        LexiconEntry {
            pack: "csif_compact_lexicon_v1",
            language: "fr",
            lemma: "clarte",
            concept_node: "concept_light_insight",
            relation: "maps_to_concept",
            related_terms: &["clarté", "comprehension", "realisation", "savoir"],
        },
        LexiconEntry {
            pack: "csif_compact_lexicon_v1",
            language: "zh",
            lemma: "光",
            concept_node: "concept_light_visible",
            relation: "maps_to_concept",
            related_terms: &["可见", "亮度", "辐射", "眼睛", "光速"],
        },
        LexiconEntry {
            pack: "csif_compact_lexicon_v1",
            language: "zh",
            lemma: "理解",
            concept_node: "concept_light_insight",
            relation: "maps_to_concept",
            related_terms: &["领悟", "明白", "认知", "洞察"],
        },
    ];

    entries.extend([
        LexiconEntry {
            pack: "csif_compact_lexicon_legacy_v2",
            language: "en",
            lemma: "effulgence",
            concept_node: "concept_light_visible",
            relation: "maps_to_concept",
            related_terms: &["refulgence", "luster", "lustre", "gleam", "radiance"],
        },
        LexiconEntry {
            pack: "csif_compact_lexicon_legacy_v2",
            language: "es",
            lemma: "lumbre",
            concept_node: "concept_light_visible",
            relation: "maps_to_concept",
            related_terms: &["claror", "fulgor", "resplandor", "candela"],
        },
        LexiconEntry {
            pack: "csif_compact_lexicon_legacy_v2",
            language: "fr",
            lemma: "resplendissement",
            concept_node: "concept_light_visible",
            relation: "maps_to_concept",
            related_terms: &["eclat", "éclat", "splendeur", "lueur"],
        },
        LexiconEntry {
            pack: "csif_compact_lexicon_legacy_v2",
            language: "zh",
            lemma: "烛照",
            concept_node: "concept_light_visible",
            relation: "maps_to_concept",
            related_terms: &["昭明", "辉映", "明耀", "光华"],
        },
    ]);

    entries.extend([
        LexiconEntry {
            pack: "csif_compact_lexicon_reasoning_v2",
            language: "en",
            lemma: "dividual",
            concept_node: "concept_light_insight",
            relation: "maps_to_concept",
            related_terms: &["individual", "partition", "analytic", "individuation"],
        },
        LexiconEntry {
            pack: "csif_compact_lexicon_reasoning_v2",
            language: "es",
            lemma: "inteleccion",
            concept_node: "concept_light_insight",
            relation: "maps_to_concept",
            related_terms: &["discernimiento", "entendimiento", "aprehension", "individuacion"],
        },
        LexiconEntry {
            pack: "csif_compact_lexicon_reasoning_v2",
            language: "fr",
            lemma: "intellection",
            concept_node: "concept_light_insight",
            relation: "maps_to_concept",
            related_terms: &["discernement", "comprehension", "individuation", "analyse"],
        },
        LexiconEntry {
            pack: "csif_compact_lexicon_reasoning_v2",
            language: "zh",
            lemma: "格物",
            concept_node: "concept_light_insight",
            relation: "maps_to_concept",
            related_terms: &["穷理", "明辨", "洞察", "致知"],
        },
    ]);

    entries.extend([
        LexiconEntry {
            pack: "csif_compact_lexicon_etymology_v3",
            language: "en",
            lemma: "lucidity",
            concept_node: "concept_light_insight",
            relation: "maps_to_concept",
            related_terms: &["lucid", "elucidate", "clarify", "clarity"],
        },
        LexiconEntry {
            pack: "csif_compact_lexicon_etymology_v3",
            language: "en",
            lemma: "illumination",
            concept_node: "concept_light_insight",
            relation: "maps_to_concept",
            related_terms: &["illumine", "illustrate", "clarification", "luminous"],
        },
        LexiconEntry {
            pack: "csif_compact_lexicon_etymology_v3",
            language: "es",
            lemma: "lucidez",
            concept_node: "concept_light_insight",
            relation: "maps_to_concept",
            related_terms: &["lucido", "elucidar", "clarificar", "claridad"],
        },
        LexiconEntry {
            pack: "csif_compact_lexicon_etymology_v3",
            language: "es",
            lemma: "ilustracion",
            concept_node: "concept_light_insight",
            relation: "maps_to_concept",
            related_terms: &["iluminar", "clarificacion", "luciente", "esclarecer"],
        },
        LexiconEntry {
            pack: "csif_compact_lexicon_etymology_v3",
            language: "fr",
            lemma: "lucidite",
            concept_node: "concept_light_insight",
            relation: "maps_to_concept",
            related_terms: &["lucide", "elucider", "clarifier", "clarte"],
        },
        LexiconEntry {
            pack: "csif_compact_lexicon_etymology_v3",
            language: "fr",
            lemma: "illustration",
            concept_node: "concept_light_insight",
            relation: "maps_to_concept",
            related_terms: &["illuminer", "clarification", "lumineux", "eclaircir"],
        },
        LexiconEntry {
            pack: "csif_compact_lexicon_etymology_v3",
            language: "zh",
            lemma: "明晰",
            concept_node: "concept_light_insight",
            relation: "maps_to_concept",
            related_terms: &["澄明", "明辨", "洞明", "昭晰"],
        },
        LexiconEntry {
            pack: "csif_compact_lexicon_etymology_v3",
            language: "zh",
            lemma: "阐明",
            concept_node: "concept_light_insight",
            relation: "maps_to_concept",
            related_terms: &["明示", "昭示", "澄清", "洞彻"],
        },
    ]);

    entries
}

fn resolve_lexicon_matches(
    language: &str,
    context_tokens: &[String],
    lexicon_control: &LexiconControl,
) -> Vec<MatchedLexiconEdge> {
    let language_norm = normalize_lexeme_token(language);
    let mut matches = Vec::<MatchedLexiconEdge>::new();
    for entry in compact_lexicon_pack() {
        if !lexicon_control.active_packs.contains(entry.pack) {
            continue;
        }
        if entry.language != language_norm {
            continue;
        }
        for token in context_tokens {
            if token == entry.lemma {
                matches.push(MatchedLexiconEdge {
                    entry,
                    matched_token: token.clone(),
                    via: "lemma",
                });
                continue;
            }
            if entry.related_terms.iter().any(|term| {
                normalize_token(term)
                    .map(|normalized| normalized == *token)
                    .unwrap_or(false)
            }) {
                matches.push(MatchedLexiconEdge {
                    entry,
                    matched_token: token.clone(),
                    via: "related_term",
                });
            }
        }
    }

    let mut seen = HashSet::<String>::new();
    matches
        .into_iter()
        .filter(|m| {
            let key = format!("{}|{}|{}", m.entry.lemma, m.entry.concept_node, m.matched_token);
            if seen.contains(&key) {
                false
            } else {
                seen.insert(key);
                true
            }
        })
        .collect::<Vec<_>>()
}

#[derive(Clone, Copy)]
struct FrameTranslationOperator {
    operator_id: &'static str,
    source_ontology_frame: &'static str,
    target_ontology_frame: &'static str,
    invertible: bool,
    preconditions: &'static [&'static str],
    transition_cost_delta: f64,
    coherence_delta: f64,
    preserves_topology: bool,
    lossy: bool,
}

fn frame_translation_operators() -> Vec<FrameTranslationOperator> {
    vec![
        FrameTranslationOperator {
            operator_id: "op_classical_optics_to_quantum_interaction",
            source_ontology_frame: "classical_optics",
            target_ontology_frame: "quantum_interaction",
            invertible: true,
            preconditions: &["wave", "particle", "photon", "quantum"],
            transition_cost_delta: -0.25,
            coherence_delta: 0.4,
            preserves_topology: true,
            lossy: false,
        },
        FrameTranslationOperator {
            operator_id: "op_legal_ownership_to_physical_possession",
            source_ontology_frame: "legal_ownership",
            target_ontology_frame: "physical_possession",
            invertible: true,
            preconditions: &["owner", "ownership", "possess", "custody", "title"],
            transition_cost_delta: -0.2,
            coherence_delta: 0.3,
            preserves_topology: true,
            lossy: false,
        },
        FrameTranslationOperator {
            operator_id: "op_colloquial_meaning_to_scientific_meaning",
            source_ontology_frame: "colloquial_meaning",
            target_ontology_frame: "scientific_meaning",
            invertible: false,
            preconditions: &["technical", "scientific", "formal", "measurement"],
            transition_cost_delta: -0.15,
            coherence_delta: 0.25,
            preserves_topology: true,
            lossy: false,
        },
        FrameTranslationOperator {
            operator_id: "op_present_identity_to_historical_identity",
            source_ontology_frame: "present_state_identity",
            target_ontology_frame: "historical_identity",
            invertible: true,
            preconditions: &["historical", "history", "formerly", "past"],
            transition_cost_delta: -0.1,
            coherence_delta: 0.2,
            preserves_topology: true,
            lossy: false,
        },
    ]
}

fn frame_operator_preconditions_met(context_lower: &str, preconditions: &[&str]) -> bool {
    preconditions.iter().any(|term| context_lower.contains(term))
}

fn resolve_frame_operator_direction(
    operator: &FrameTranslationOperator,
    prior: &FrameContext,
    active: &FrameContext,
) -> Option<&'static str> {
    let forward = prior.ontology_frame == operator.source_ontology_frame
        && active.ontology_frame == operator.target_ontology_frame;
    if forward {
        return Some("forward");
    }
    let reverse = operator.invertible
        && prior.ontology_frame == operator.target_ontology_frame
        && active.ontology_frame == operator.source_ontology_frame;
    if reverse {
        Some("reverse")
    } else {
        None
    }
}

fn build_frame_operator_payload(
    active_frame: &FrameContext,
    prior_frame: &FrameContext,
    context: &str,
    candidates: &[Value],
    frame_transition_cost: f64,
    contradiction_type: &str,
    conservation_policy: &ConservationPolicy,
) -> Value {
    let context_lower = context.to_lowercase();
    let eligible = frame_translation_operators()
        .into_iter()
        .filter_map(|operator| {
            let direction = resolve_frame_operator_direction(&operator, prior_frame, active_frame)?;
            if !frame_operator_preconditions_met(&context_lower, operator.preconditions) {
                return None;
            }
            Some((operator, direction))
        })
        .collect::<Vec<_>>();

    let eligible_operators = eligible
        .iter()
        .map(|(operator, direction)| {
            json!({
                "operator_id": operator.operator_id,
                "source_frame_signature": frame_signature(prior_frame),
                "target_frame_signature": frame_signature(active_frame),
                "direction": direction,
                "invertible": operator.invertible,
                "preconditions": operator.preconditions,
                "transition_cost_delta": operator.transition_cost_delta,
                "coherence_delta": operator.coherence_delta,
                "preserves_topology": operator.preserves_topology,
                "lossy": operator.lossy,
            })
        })
        .collect::<Vec<_>>();

    let projected_candidates = eligible
        .iter()
        .flat_map(|(operator, direction)| {
            candidates.iter().take(2).map(move |candidate| {
                let node_id = candidate
                    .get("sense_node")
                    .and_then(|v| v.get("node_id"))
                    .and_then(Value::as_str)
                    .unwrap_or("unknown_sense");
                let base_score = candidate.get("score").and_then(Value::as_f64).unwrap_or(0.0);
                let projected_score = base_score + operator.coherence_delta;
                let projected_transition_cost =
                    (frame_transition_cost + operator.transition_cost_delta).max(0.0);
                let identity_continuity = true;
                let causal_continuity = !operator.lossy;
                let topology_continuity = operator.preserves_topology;
                let observer_consistency = active_frame.observer_frame == prior_frame.observer_frame;
                let modality_preservation = active_frame.modality_frame == prior_frame.modality_frame;
                let invariant_profiles = vec![
                    conservation_invariant_profile(
                        "identity_continuity",
                        identity_continuity,
                        0.6,
                        "entity referent changed across transform",
                    ),
                    conservation_invariant_profile(
                        "causal_continuity",
                        causal_continuity,
                        0.4,
                        "causal direction altered by transform",
                    ),
                    conservation_invariant_profile(
                        "topology_continuity",
                        topology_continuity,
                        0.5,
                        "relation topology not preserved",
                    ),
                    conservation_invariant_profile(
                        "observer_consistency",
                        observer_consistency,
                        0.35,
                        "observer frame changed without explicit containment mapping",
                    ),
                    conservation_invariant_profile(
                        "modality_preservation",
                        modality_preservation,
                        0.3,
                        "modality state drifted across transform",
                    ),
                ];
                let total_loss = invariant_profiles
                    .iter()
                    .filter_map(|profile| {
                        if profile.get("preserved").and_then(Value::as_bool) == Some(false) {
                            profile.get("delta").and_then(Value::as_f64)
                        } else {
                            None
                        }
                    })
                    .sum::<f64>();
                let violated_invariants = invariant_profiles
                    .iter()
                    .filter_map(|profile| {
                        if profile.get("preserved").and_then(Value::as_bool) == Some(false) {
                            profile.get("name").and_then(Value::as_str).map(|s| s.to_string())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                let required_set = conservation_policy
                    .required_invariants
                    .iter()
                    .cloned()
                    .collect::<HashSet<_>>();
                let required_satisfied = invariant_profiles.iter().all(|profile| {
                    let name = profile.get("name").and_then(Value::as_str).unwrap_or("");
                    if required_set.contains(name) {
                        profile.get("preserved").and_then(Value::as_bool).unwrap_or(false)
                    } else {
                        true
                    }
                });
                let lossy_satisfied = conservation_policy.allow_lossy || !operator.lossy;
                let loss_satisfied = total_loss <= conservation_policy.max_total_loss;
                let conservation_admissible = required_satisfied && lossy_satisfied && loss_satisfied;
                let post_transform_contradiction =
                    if contradiction_type == "frame_mismatch" && projected_transition_cost > 0.35 {
                        "frame_mismatch"
                    } else {
                        "global_contradiction"
                    };
                let collapse_allowed = conservation_admissible
                    && (post_transform_contradiction != "frame_mismatch"
                        || projected_transition_cost <= 0.35);

                json!({
                    "projection_id": format!("{}::{}", operator.operator_id, node_id),
                    "operator_id": operator.operator_id,
                    "direction": direction,
                    "candidate_node_id": node_id,
                    "base_score": base_score,
                    "projected_score": projected_score,
                    "transition_cost_delta": operator.transition_cost_delta,
                    "coherence_delta": operator.coherence_delta,
                    "projected_transition_cost": projected_transition_cost,
                    "post_transform_contradiction": post_transform_contradiction,
                    "collapse_allowed": collapse_allowed,
                    "conservation_blocked": !conservation_admissible,
                    "preserves_topology": operator.preserves_topology,
                    "lossy": operator.lossy,
                    "conservation_profile": {
                        "required_invariants": conservation_policy.required_invariants,
                        "allow_lossy": conservation_policy.allow_lossy,
                        "max_total_loss": conservation_policy.max_total_loss,
                        "invariants": invariant_profiles,
                        "total_loss": total_loss,
                        "violated_invariants": violated_invariants,
                        "admissible": conservation_admissible,
                    },
                })
            })
        })
        .collect::<Vec<_>>();

    json!({
        "enabled": true,
        "strict_deterministic": true,
        "transform_policy": {
            "implicit_transforms": false,
            "allow_chaining": false,
            "single_step_only": true,
        },
        "eligible_operators": eligible_operators,
        "projected_candidates": projected_candidates,
        "conservation_policy": {
            "required_invariants": conservation_policy.required_invariants,
            "allow_lossy": conservation_policy.allow_lossy,
            "max_total_loss": conservation_policy.max_total_loss,
        },
    })
}

fn resolve_lexeme_identity(_language: &str, token: &str) -> Option<LexemeIdentity> {
    let normalized = normalize_lexeme_token(token);
    let normalized_no_diacritic = normalized.replace('è', "e");
    if ["light", "luz", "光", "lumière", "lumiere"]
        .contains(&normalized.as_str())
        || ["light", "luz", "光", "lumiere"].contains(&normalized_no_diacritic.as_str())
    {
        Some(LexemeIdentity {
            canonical_language: "semantic",
            canonical_token: "light",
            canonical_lexeme_node: "lexeme_semantic_light",
            aliases: &["light", "luz", "光", "lumière", "lumiere"],
        })
    } else {
        None
    }
}

fn lexeme_senses(canonical_token: &str) -> Vec<SenseSpec> {
    match canonical_token {
        "light" => vec![
            SenseSpec {
                node_id: "sense_light_insight",
                label: "understanding or realization",
                anchor_terms: &[
                    "understanding",
                    "insight",
                    "realization",
                    "lecture",
                    "learn",
                    "understand",
                    "clarity",
                    "entender",
                    "entendimiento",
                    "comprension",
                    "comprensión",
                    "insightful",
                    "clarté",
                    "clair",
                    "理解",
                    "领悟",
                ],
            },
            SenseSpec {
                node_id: "sense_light_visible_em",
                label: "visible electromagnetic radiation",
                anchor_terms: &[
                    "see",
                    "eyes",
                    "visible",
                    "brightness",
                    "radiation",
                    "vision",
                    "ver",
                    "ojos",
                    "visible",
                    "luminosidad",
                    "lumière",
                    "luminosite",
                    "lumiere",
                    "光",
                    "眼睛",
                    "可见",
                ],
            },
            SenseSpec {
                node_id: "sense_light_low_mass",
                label: "low mass or low weight",
                anchor_terms: &["weight", "mass", "carry", "portable", "lighter"],
            },
            SenseSpec {
                node_id: "sense_light_ignition",
                label: "ignition source or flame",
                anchor_terms: &["ignite", "flame", "candle", "spark", "burn"],
            },
            SenseSpec {
                node_id: "sense_light_brightness_state",
                label: "brightness state or illumination level",
                anchor_terms: &["bright", "dim", "illuminate", "room", "lamp"],
            },
        ],
        _ => Vec::new(),
    }
}

fn phrase_specs(canonical_token: &str) -> Vec<PhraseSpec> {
    match canonical_token {
        "light" => vec![
            PhraseSpec {
                node_id: "phrase_light_speed",
                label: "electromagnetic propagation limit",
                aliases: &[
                    PhraseAliasSpec {
                        language: "en",
                        surface: "light speed",
                        pattern: &["light", "speed"],
                    },
                    PhraseAliasSpec {
                        language: "es",
                        surface: "velocidad de la luz",
                        pattern: &["velocidad", "de", "la", "luz"],
                    },
                    PhraseAliasSpec {
                        language: "fr",
                        surface: "vitesse de la lumière",
                        pattern: &["vitesse", "de", "la", "lumière"],
                    },
                    PhraseAliasSpec {
                        language: "fr",
                        surface: "vitesse de la lumiere",
                        pattern: &["vitesse", "de", "la", "lumiere"],
                    },
                    PhraseAliasSpec {
                        language: "zh",
                        surface: "光速",
                        pattern: &["光速"],
                    },
                ],
                preferred_sense: "sense_light_visible_em",
                lobe: "semantic_lobe_radiance",
            },
            PhraseSpec {
                node_id: "phrase_light_reading",
                label: "easy or non-intensive reading",
                aliases: &[PhraseAliasSpec {
                    language: "en",
                    surface: "light reading",
                    pattern: &["light", "reading"],
                }],
                preferred_sense: "sense_light_low_mass",
                lobe: "semantic_lobe_materiality",
            },
            PhraseSpec {
                node_id: "phrase_light_touch",
                label: "gentle pressure or subtle interaction",
                aliases: &[PhraseAliasSpec {
                    language: "en",
                    surface: "light touch",
                    pattern: &["light", "touch"],
                }],
                preferred_sense: "sense_light_low_mass",
                lobe: "semantic_lobe_materiality",
            },
            PhraseSpec {
                node_id: "phrase_light_rail",
                label: "urban rail transit class",
                aliases: &[PhraseAliasSpec {
                    language: "en",
                    surface: "light rail",
                    pattern: &["light", "rail"],
                }],
                preferred_sense: "sense_light_low_mass",
                lobe: "semantic_lobe_materiality",
            },
            PhraseSpec {
                node_id: "phrase_light_industry",
                label: "manufacturing with lower capital intensity",
                aliases: &[PhraseAliasSpec {
                    language: "en",
                    surface: "light industry",
                    pattern: &["light", "industry"],
                }],
                preferred_sense: "sense_light_low_mass",
                lobe: "semantic_lobe_materiality",
            },
            PhraseSpec {
                node_id: "phrase_speed_engine",
                label: "engine as a speed-centric subsystem",
                aliases: &[PhraseAliasSpec {
                    language: "en",
                    surface: "speed engine",
                    pattern: &["speed", "engine"],
                }],
                preferred_sense: "sense_light_low_mass",
                lobe: "semantic_lobe_materiality",
            },
            PhraseSpec {
                node_id: "phrase_light_speed_engine",
                label: "compound engine concept constrained by light speed framing",
                aliases: &[PhraseAliasSpec {
                    language: "en",
                    surface: "light speed engine",
                    pattern: &["light", "speed", "engine"],
                }],
                preferred_sense: "sense_light_visible_em",
                lobe: "semantic_lobe_radiance",
            },
        ],
        _ => Vec::new(),
    }
}

fn find_contiguous_pattern_span(tokens: &[String], pattern: &[&str]) -> Option<(usize, usize)> {
    if pattern.is_empty() || tokens.len() < pattern.len() {
        return None;
    }
    tokens
        .windows(pattern.len())
        .position(|window| {
            window
            .iter()
            .zip(pattern.iter())
            .all(|(token, wanted)| token == wanted)
        })
        .map(|start| (start, start + pattern.len()))
}

fn spans_overlap(
    left_start: Option<usize>,
    left_end_exclusive: Option<usize>,
    right_start: Option<usize>,
    right_end_exclusive: Option<usize>,
) -> bool {
    match (
        left_start,
        left_end_exclusive,
        right_start,
        right_end_exclusive,
    ) {
        (Some(ls), Some(le), Some(rs), Some(re)) => ls < re && rs < le,
        _ => false,
    }
}

fn resolve_phrase_matches(
    canonical_token: &str,
    language: &str,
    context_tokens: &[String],
) -> Vec<PhraseMatch> {
    let mut matches = phrase_specs(canonical_token)
        .into_iter()
        .filter_map(|spec| {
            let best_alias = spec
                .aliases
                .iter()
                .filter_map(|alias| {
                    let contiguous_span = find_contiguous_pattern_span(context_tokens, alias.pattern);
                    let all_present = alias
                        .pattern
                        .iter()
                        .all(|token| context_tokens.iter().any(|ctx| ctx == token));

                    let alias_score = if contiguous_span.is_some() {
                        if alias.language == language { 1.0 } else { 0.85 }
                    } else if all_present {
                        if alias.language == language { 0.65 } else { 0.5 }
                    } else {
                        0.0
                    };
                    if alias_score <= 0.0 {
                        return None;
                    }

                    let composition_depth = alias.pattern.len().max(1);
                    let depth_bonus = 0.05 * (composition_depth.saturating_sub(2) as f64);
                    let phrase_coherence_score = alias_score + depth_bonus;
                    let token_composition_score = if contiguous_span.is_some() {
                        phrase_coherence_score * 0.9
                    } else {
                        phrase_coherence_score * 0.6
                    };

                    Some(PhraseMatch {
                        spec,
                        matched_language: alias.language,
                        matched_surface: alias.surface,
                        matched_pattern: alias.pattern,
                        phrase_text: alias.pattern.join(" "),
                        phrase_coherence_score,
                        token_composition_score,
                        composition_depth,
                        span_start: contiguous_span.map(|(start, _)| start),
                        span_end_exclusive: contiguous_span.map(|(_, end)| end),
                        overlap_group: 0,
                        overlaps_with: Vec::new(),
                    })
                })
                .max_by(|a, b| {
                    a.phrase_coherence_score
                        .partial_cmp(&b.phrase_coherence_score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| a.matched_surface.cmp(b.matched_surface))
                });
            best_alias
        })
        .collect::<Vec<_>>();

    matches.sort_by(|a, b| a.spec.node_id.cmp(b.spec.node_id));

    for left_index in 0..matches.len() {
        for right_index in (left_index + 1)..matches.len() {
            if spans_overlap(
                matches[left_index].span_start,
                matches[left_index].span_end_exclusive,
                matches[right_index].span_start,
                matches[right_index].span_end_exclusive,
            ) {
                let left_id = matches[left_index].spec.node_id.to_string();
                let right_id = matches[right_index].spec.node_id.to_string();
                matches[left_index].overlaps_with.push(right_id.clone());
                matches[right_index].overlaps_with.push(left_id);
            }
        }
    }

    let mut visited = vec![false; matches.len()];
    let mut group_id = 0usize;
    for idx in 0..matches.len() {
        if visited[idx] {
            continue;
        }
        group_id += 1;
        let mut stack = vec![idx];
        while let Some(current) = stack.pop() {
            if visited[current] {
                continue;
            }
            visited[current] = true;
            matches[current].overlap_group = group_id;
            for next in 0..matches.len() {
                if !visited[next]
                    && spans_overlap(
                        matches[current].span_start,
                        matches[current].span_end_exclusive,
                        matches[next].span_start,
                        matches[next].span_end_exclusive,
                    )
                {
                    stack.push(next);
                }
            }
        }
    }

    for phrase_match in matches.iter_mut() {
        phrase_match.overlaps_with.sort();
        phrase_match.overlaps_with.dedup();
    }

    matches
}

fn context_anchor_hit(context_text_lower: &str, context_set: &HashSet<String>, anchor: &str) -> bool {
    let anchor_lower = anchor.to_lowercase();
    if context_text_lower.contains(&anchor_lower) {
        return true;
    }
    if let Some(anchor_norm) = normalize_token(anchor) {
        context_set.contains(&anchor_norm)
    } else {
        false
    }
}

fn sense_lobe(node_id: &str) -> &'static str {
    if node_id.contains("insight") {
        "semantic_lobe_cognition"
    } else if node_id.contains("visible") || node_id.contains("brightness") {
        "semantic_lobe_radiance"
    } else if node_id.contains("low_mass") {
        "semantic_lobe_materiality"
    } else if node_id.contains("ignition") {
        "semantic_lobe_combustion"
    } else {
        "semantic_lobe_unknown"
    }
}

fn disambiguate_payload_with_inertia_and_frame_and_policy(
    state: &AppState,
    language: &str,
    token: &str,
    context: &str,
    top_k: usize,
    margin_threshold: f64,
    inertia_profile: Option<&LexemeInertiaProfile>,
    inertia_coefficient: f64,
    sandbox_on_inertia_block: bool,
    active_frame: &FrameContext,
    prior_frame: &FrameContext,
    conservation_policy: &ConservationPolicy,
    lexicon_control: &LexiconControl,
) -> Value {
    let normalized_token = normalize_lexeme_token(token);
    let full_context_tokens = tokenize(context);
    let identity = resolve_lexeme_identity(language, &normalized_token);
    let (canonical_language, canonical_token, lexeme_node_id, aliases) = if let Some(id) = identity {
        (
            id.canonical_language,
            id.canonical_token.to_string(),
            id.canonical_lexeme_node.to_string(),
            id.aliases.to_vec(),
        )
    } else {
        (
            language,
            normalized_token.clone(),
            format!("lexeme_{}_{}", language, normalized_token),
            vec![],
        )
    };
    let senses = lexeme_senses(&canonical_token);
    let lexicon_matches = resolve_lexicon_matches(language, &full_context_tokens, lexicon_control);
    let mut active_packs_sorted = lexicon_control
        .active_packs
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    active_packs_sorted.sort();
    let lexicon_matched_tokens = lexicon_matches
        .iter()
        .map(|m| m.matched_token.clone())
        .collect::<HashSet<_>>();
    let lexicon_coverage_ratio = if full_context_tokens.is_empty() {
        0.0
    } else {
        lexicon_matched_tokens.len() as f64 / full_context_tokens.len() as f64
    };
    let matched_lexicon_edges = lexicon_matches
        .iter()
        .map(|m| {
            json!({
                "pack": m.entry.pack,
                "language": m.entry.language,
                "lemma": m.entry.lemma,
                "concept_node": m.entry.concept_node,
                "relation": m.entry.relation,
                "matched_token": m.matched_token,
                "via": m.via,
            })
        })
        .collect::<Vec<_>>();
    let prior_frame_signature = frame_signature(prior_frame);
    let candidate_frame_signature = frame_signature(active_frame);
    let frame_alignment = frame_alignment_score(prior_frame, active_frame);
    let frame_transition_cost = 1.0 - frame_alignment;
    let frame_status = frame_reconciliation_status(frame_alignment);
    let contradiction_type = contradiction_type_for_frame(frame_status, active_frame, context);

    if senses.is_empty() {
        let frame_operators = build_frame_operator_payload(
            active_frame,
            prior_frame,
            context,
            &[],
            frame_transition_cost,
            contradiction_type,
            conservation_policy,
        );
        return json!({
            "object": "csif.disambiguation.result",
            "schema_version": "csif_disambiguation_v1",
            "deterministic": true,
            "status": "unknown_lexeme",
            "input": {
                "language": language,
                "token": token,
                "context": context,
            },
            "lexeme_node": {
                "node_id": lexeme_node_id,
                "node_type": "lexeme",
                "label": normalized_token,
                "language": language,
            },
            "semantic_identity": Value::Null,
            "sense_nodes": [],
            "mapping_edges": [],
            "retrieval_evidence": [],
            "lexicon": {
                "pack": "csif_compact_lexicon_v1",
                "packs": active_packs_sorted,
                "pack_weights": lexicon_control.pack_weights,
                "languages": ["en", "es", "fr", "zh"],
                "coverage": {
                    "context_token_count": full_context_tokens.len(),
                    "matched_token_count": lexicon_matched_tokens.len(),
                    "coverage_ratio": lexicon_coverage_ratio,
                },
                "matched_lexicon_edges": matched_lexicon_edges,
                "unknown_due_to_lexicon_gap": lexicon_matches.is_empty(),
            },
            "resolver": {
                "margin_threshold": margin_threshold,
                "ambiguity_margin": 0.0,
                "weights": {
                    "context_overlap": 2.0,
                    "retrieval_overlap": 1.0,
                    "phase_resonance": 0.5,
                    "conflict_penalty": 1.5
                },
                "inertia": {
                    "inertia_coefficient": inertia_coefficient,
                    "crystallization_depth": inertia_profile.map(|p| p.crystallization_depth).unwrap_or(0.0),
                    "effective_margin_threshold": margin_threshold,
                    "prior_selected_sense": inertia_profile.and_then(|p| p.last_selected_sense.as_ref()),
                    "reassignment_pressure": false,
                },
            },
            "inertia_decision": {
                "blocked": false,
                "reason": "unknown lexeme",
                "recommended_action": "none",
            },
            "frame_semantics": {
                "active_frame": {
                    "observer_frame": active_frame.observer_frame.as_str(),
                    "ontology_frame": active_frame.ontology_frame.as_str(),
                    "temporal_frame": active_frame.temporal_frame.as_str(),
                    "modality_frame": active_frame.modality_frame.as_str(),
                    "epistemic_source_frame": active_frame.epistemic_source_frame.as_str(),
                },
                "transition": {
                    "prior_frame_signature": prior_frame_signature,
                    "candidate_frame_signature": candidate_frame_signature,
                    "frame_alignment_score": frame_alignment,
                    "transition_cost": frame_transition_cost,
                    "frame_reconciliation_status": frame_status,
                },
                "contradiction_typing": {
                    "type": contradiction_type,
                    "typed": true,
                },
                "unresolved_torsion": {
                    "active": true,
                    "kind": "unknown_lexeme",
                    "collapse_allowed": false,
                    "recommended_action": "request_more_context",
                }
            },
            "frame_operators": frame_operators,
            "selected_sense": Value::Null,
            "candidates": []
        });
    }

    let context_tokens = full_context_tokens
        .clone()
        .into_iter()
        .filter(|t| t != &normalized_token)
        .collect::<Vec<_>>();
    let context_set = context_tokens.iter().cloned().collect::<HashSet<_>>();
    let context_text_lower = context.to_lowercase();
    let lexicon_support_by_sense = senses
        .iter()
        .map(|sense| {
            let sense_concept = sense_concept_node(sense.node_id);
            let support = lexicon_matches
                .iter()
                .filter(|m| m.entry.concept_node == sense_concept)
                .map(|m| {
                    lexicon_control
                        .pack_weights
                        .get(m.entry.pack)
                        .copied()
                        .unwrap_or(1.0)
                })
                .sum::<f64>();
            (sense.node_id.to_string(), support)
        })
        .collect::<HashMap<_, _>>();

    let phrase_matches = resolve_phrase_matches(&canonical_token, language, &full_context_tokens);
    let mut phrase_boost_by_sense = HashMap::<String, f64>::new();
    for phrase_match in &phrase_matches {
        *phrase_boost_by_sense
            .entry(phrase_match.spec.preferred_sense.to_string())
            .or_insert(0.0) += 2.0 * phrase_match.phrase_coherence_score;
    }

    let mut retrieval_evidence = Vec::<Value>::new();
    let mut retrieval_terms = HashSet::<String>::new();
    if let Some(index) = state.bank_index.as_ref() {
        let query = format!("{} {}", normalized_token, context);
        let (rewritten_query, rewrite_reasons) = rewrite_retrieval_query(&query);
        for (entry_id, score) in retrieve_from_index(index, &rewritten_query, top_k) {
            let entry = &index.entries[entry_id];
            for tok in tokenize(&entry.searchable_text) {
                retrieval_terms.insert(tok);
            }
            retrieval_evidence.push(json!({
                "score": score,
                "rewritten_query": rewritten_query,
                "rewrite_reasons": rewrite_reasons,
                "crystal_id": entry.crystal_id,
                "edge_id": entry.edge_id,
                "searchable_text": entry.searchable_text,
            }));
        }
    }

    let all_anchor_hits = senses
        .iter()
        .map(|sense| {
            sense
                .anchor_terms
                .iter()
                .filter(|term| context_anchor_hit(&context_text_lower, &context_set, term))
                .count() as f64
        })
        .sum::<f64>();

    let mut candidates = senses
        .iter()
        .map(|sense| {
            let context_overlap = sense
                .anchor_terms
                .iter()
                .filter(|term| context_anchor_hit(&context_text_lower, &context_set, term))
                .count() as f64;
            let retrieval_overlap = sense
                .anchor_terms
                .iter()
                .filter_map(|term| normalize_token(term))
                .filter(|term| retrieval_terms.contains(term))
                .count() as f64;
            let conflict = (all_anchor_hits - context_overlap).max(0.0);
            let phase_resonance = if context_overlap + retrieval_overlap > 0.0 {
                1.0 - (1.0 / (1.0 + context_overlap + retrieval_overlap))
            } else {
                0.0
            };

            let score = (2.0 * context_overlap)
                + (1.0 * retrieval_overlap)
                + (0.5 * phase_resonance)
                - (1.5 * conflict)
                + frame_bonus_for_sense(active_frame, sense.node_id)
                + (0.4 * lexicon_support_by_sense.get(sense.node_id).copied().unwrap_or(0.0))
                + phrase_boost_by_sense
                    .get(sense.node_id)
                    .copied()
                    .unwrap_or(0.0);

            json!({
                "sense_node": {
                    "node_id": sense.node_id,
                    "node_type": "semantic_sense",
                    "label": sense.label
                },
                "score": score,
                "features": {
                    "context_overlap": context_overlap,
                    "retrieval_overlap": retrieval_overlap,
                    "phase_resonance": phase_resonance,
                    "conflict": conflict,
                    "frame_bonus": frame_bonus_for_sense(active_frame, sense.node_id),
                    "lexicon_support": lexicon_support_by_sense
                        .get(sense.node_id)
                        .copied()
                        .unwrap_or(0.0),
                    "phrase_boost": phrase_boost_by_sense
                        .get(sense.node_id)
                        .copied()
                        .unwrap_or(0.0),
                    "anchor_terms": sense.anchor_terms
                }
            })
        })
        .collect::<Vec<_>>();

    candidates.sort_by(|a, b| {
        let score_a = a.get("score").and_then(Value::as_f64).unwrap_or(f64::NEG_INFINITY);
        let score_b = b.get("score").and_then(Value::as_f64).unwrap_or(f64::NEG_INFINITY);
        score_b
            .partial_cmp(&score_a)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                let id_a = a
                    .get("sense_node")
                    .and_then(|v| v.get("node_id"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let id_b = b
                    .get("sense_node")
                    .and_then(|v| v.get("node_id"))
                    .and_then(Value::as_str)
                    .unwrap_or("");
                id_a.cmp(id_b)
            })
    });

    let frame_operators = build_frame_operator_payload(
        active_frame,
        prior_frame,
        context,
        &candidates,
        frame_transition_cost,
        contradiction_type,
        conservation_policy,
    );
    let has_eligible_operator = frame_operators
        .get("eligible_operators")
        .and_then(Value::as_array)
        .map(|ops| !ops.is_empty())
        .unwrap_or(false);

    let best_score = candidates
        .first()
        .and_then(|v| v.get("score"))
        .and_then(Value::as_f64)
        .unwrap_or(f64::NEG_INFINITY);
    let second_score = candidates
        .get(1)
        .and_then(|v| v.get("score"))
        .and_then(Value::as_f64)
        .unwrap_or(f64::NEG_INFINITY);

    let ambiguity_margin = if second_score.is_finite() {
        best_score - second_score
    } else if best_score.is_finite() {
        best_score
    } else {
        0.0
    };

    let top_candidate_node_id = candidates
        .first()
        .and_then(|v| v.get("sense_node"))
        .and_then(|v| v.get("node_id"))
        .and_then(Value::as_str)
        .map(|v| v.to_string());

    let prior_selected_sense = inertia_profile.and_then(|p| p.last_selected_sense.clone());
    let crystallization_depth = inertia_profile
        .map(|p| p.crystallization_depth)
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);
    let safe_inertia_coefficient = inertia_coefficient.max(0.0);
    let reassignment_pressure = matches!(
        (prior_selected_sense.as_deref(), top_candidate_node_id.as_deref()),
        (Some(prev), Some(top)) if prev != top
    );
    let effective_margin_threshold = if reassignment_pressure {
        margin_threshold + (crystallization_depth * safe_inertia_coefficient)
    } else {
        margin_threshold
    };

    let selected_sense_pre_frame = if best_score > 0.0 && ambiguity_margin >= effective_margin_threshold {
        candidates
            .first()
            .and_then(|v| v.get("sense_node"))
            .cloned()
            .unwrap_or(Value::Null)
    } else {
        Value::Null
    };

    let frame_forces_unresolved = frame_status == "frame_conflict";
    let selected_sense = if frame_forces_unresolved {
        Value::Null
    } else {
        selected_sense_pre_frame
    };

    let inertia_blocked = reassignment_pressure
        && best_score > 0.0
        && ambiguity_margin >= margin_threshold
        && ambiguity_margin < effective_margin_threshold;

    let unresolved_torsion_active = inertia_blocked || frame_forces_unresolved || selected_sense.is_null();

    let status = if selected_sense.is_null() {
        "ambiguous"
    } else {
        "resolved"
    };

    let sense_nodes = senses
        .iter()
        .map(|sense| {
            json!({
                "node_id": sense.node_id,
                "node_type": "semantic_sense",
                "label": sense.label
            })
        })
        .collect::<Vec<_>>();

    let mapping_edges = senses
        .iter()
        .map(|sense| {
            json!({
                "relation": "maps_to_sense",
                "source_node": lexeme_node_id,
                "target_node": sense.node_id,
            })
        })
        .collect::<Vec<_>>();

    let selected_phrase = phrase_matches
        .iter()
        .max_by(|a, b| {
            a.phrase_coherence_score
                .partial_cmp(&b.phrase_coherence_score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.composition_depth.cmp(&b.composition_depth))
                .then_with(|| a.spec.node_id.cmp(b.spec.node_id))
        })
        .filter(|m| m.phrase_coherence_score >= 0.75)
        .map(|m| {
            json!({
                "node_id": m.spec.node_id,
                "node_type": "semantic_phrase",
                "label": m.spec.label,
                "phrase_text": m.phrase_text,
                "matched_alias": {
                    "language": m.matched_language,
                    "surface": m.matched_surface,
                    "pattern_tokens": m.matched_pattern,
                },
                "preferred_sense": m.spec.preferred_sense,
                "lobe": m.spec.lobe,
                "phrase_coherence_score": m.phrase_coherence_score,
                "token_composition_score": m.token_composition_score,
                "composition_depth": m.composition_depth,
                "span": {
                    "start": m.span_start,
                    "end_exclusive": m.span_end_exclusive,
                },
                "chunk_topology": {
                    "overlap_group": m.overlap_group,
                    "overlap_count": m.overlaps_with.len(),
                    "overlaps_with": m.overlaps_with,
                },
            })
        })
        .unwrap_or(Value::Null);

    let phrase_candidates = phrase_matches
        .iter()
        .map(|m| {
            json!({
                "node_id": m.spec.node_id,
                "node_type": "semantic_phrase",
                "label": m.spec.label,
                "phrase_text": m.phrase_text,
                "matched_alias": {
                    "language": m.matched_language,
                    "surface": m.matched_surface,
                    "pattern_tokens": m.matched_pattern,
                },
                "preferred_sense": m.spec.preferred_sense,
                "lobe": m.spec.lobe,
                "phrase_coherence_score": m.phrase_coherence_score,
                "token_composition_score": m.token_composition_score,
                "composition_depth": m.composition_depth,
                "span": {
                    "start": m.span_start,
                    "end_exclusive": m.span_end_exclusive,
                },
                "chunk_topology": {
                    "overlap_group": m.overlap_group,
                    "overlap_count": m.overlaps_with.len(),
                    "overlaps_with": m.overlaps_with,
                },
            })
        })
        .collect::<Vec<_>>();

    let lattice_groups = {
        let mut groups = BTreeMap::<usize, Vec<String>>::new();
        for m in &phrase_matches {
            groups
                .entry(m.overlap_group)
                .or_default()
                .push(m.spec.node_id.to_string());
        }
        groups
            .into_iter()
            .map(|(group, mut node_ids)| {
                node_ids.sort();
                json!({
                    "group": group,
                    "phrase_nodes": node_ids,
                })
            })
            .collect::<Vec<_>>()
    };

    json!({
        "object": "csif.disambiguation.result",
        "schema_version": "csif_disambiguation_v1",
        "deterministic": true,
        "status": status,
        "input": {
            "language": language,
            "token": token,
            "context": context,
            "context_tokens": context_tokens,
            "top_k": top_k,
            "margin_threshold": margin_threshold,
        },
        "lexeme_node": {
            "node_id": lexeme_node_id,
            "node_type": "lexeme",
            "label": normalized_token,
            "language": language,
        },
        "semantic_identity": {
            "canonical_language": canonical_language,
            "canonical_token": canonical_token,
            "canonical_lexeme_node": lexeme_node_id,
            "alias_matched": normalized_token,
            "known_aliases": aliases,
        },
        "phrase_layer": {
            "enabled": true,
            "mode": "deterministic_multilingual_lattice_v2",
            "lattice": {
                "overlap_policy": "non_recursive_competing_chunks",
                "groups": lattice_groups,
            },
            "selected_phrase": selected_phrase,
            "phrase_candidates": phrase_candidates,
        },
        "sense_nodes": sense_nodes,
        "mapping_edges": mapping_edges,
        "retrieval_evidence": retrieval_evidence,
        "lexicon": {
            "pack": "csif_compact_lexicon_v1",
            "packs": active_packs_sorted,
            "pack_weights": lexicon_control.pack_weights,
            "languages": ["en", "es", "fr", "zh"],
            "coverage": {
                "context_token_count": full_context_tokens.len(),
                "matched_token_count": lexicon_matched_tokens.len(),
                "coverage_ratio": lexicon_coverage_ratio,
            },
            "matched_lexicon_edges": matched_lexicon_edges,
            "unknown_due_to_lexicon_gap": false,
        },
        "resolver": {
            "weights": {
                "context_overlap": 2.0,
                "retrieval_overlap": 1.0,
                "phase_resonance": 0.5,
                "conflict_penalty": 1.5,
                "frame_bonus": 1.0,
                "lexicon_support": 0.4
            },
            "margin_threshold": margin_threshold,
            "ambiguity_margin": ambiguity_margin,
            "inertia": {
                "inertia_coefficient": safe_inertia_coefficient,
                "crystallization_depth": crystallization_depth,
                "effective_margin_threshold": effective_margin_threshold,
                "prior_selected_sense": prior_selected_sense,
                "reassignment_pressure": reassignment_pressure,
                "current_streak": inertia_profile.map(|p| p.current_streak).unwrap_or(0),
                "resolved_count": inertia_profile.map(|p| p.resolved_count).unwrap_or(0),
            }
        },
        "inertia_decision": {
            "blocked": inertia_blocked,
            "reason": if inertia_blocked {
                "reassignment pressure exists but effective inertia threshold not met"
            } else {
                "none"
            },
            "recommended_action": if inertia_blocked && sandbox_on_inertia_block {
                "sandbox_review"
            } else if inertia_blocked {
                "request_more_context"
            } else if frame_forces_unresolved {
                if has_eligible_operator { "operator_projection_review" } else { "frame_reconcile" }
            } else {
                "none"
            },
            "candidate_under_pressure": top_candidate_node_id,
        },
        "frame_semantics": {
            "active_frame": {
                "observer_frame": active_frame.observer_frame.as_str(),
                "ontology_frame": active_frame.ontology_frame.as_str(),
                "temporal_frame": active_frame.temporal_frame.as_str(),
                "modality_frame": active_frame.modality_frame.as_str(),
                "epistemic_source_frame": active_frame.epistemic_source_frame.as_str(),
            },
            "transition": {
                "prior_frame_signature": prior_frame_signature,
                "candidate_frame_signature": candidate_frame_signature,
                "frame_alignment_score": frame_alignment,
                "transition_cost": frame_transition_cost,
                "frame_reconciliation_status": frame_status,
            },
            "contradiction_typing": {
                "type": contradiction_type,
                "typed": true,
            },
            "unresolved_torsion": {
                "active": unresolved_torsion_active,
                "kind": if frame_forces_unresolved {
                    "frame_conflict"
                } else if inertia_blocked {
                    "inertial_resistance"
                } else if selected_sense.is_null() {
                    "ambiguity"
                } else {
                    "none"
                },
                "collapse_allowed": !frame_forces_unresolved,
                "recommended_action": if frame_forces_unresolved {
                    if has_eligible_operator {
                        "operator_projection_review"
                    } else {
                        "frame_reconcile"
                    }
                } else if inertia_blocked {
                    "sandbox_review"
                } else if selected_sense.is_null() {
                    "request_more_context"
                } else {
                    "none"
                }
            }
        },
        "frame_operators": frame_operators,
        "selected_sense": selected_sense,
        "candidates": candidates,
    })
}

fn disambiguate_payload_with_inertia(
    state: &AppState,
    language: &str,
    token: &str,
    context: &str,
    top_k: usize,
    margin_threshold: f64,
    inertia_profile: Option<&LexemeInertiaProfile>,
    inertia_coefficient: f64,
    sandbox_on_inertia_block: bool,
) -> Value {
    let default_active = default_frame_context();
    let default_prior = default_frame_context();
    let default_policy = default_conservation_policy();
    let default_lexicon_control = resolve_lexicon_control(None);
    disambiguate_payload_with_inertia_and_frame_and_policy(
        state,
        language,
        token,
        context,
        top_k,
        margin_threshold,
        inertia_profile,
        inertia_coefficient,
        sandbox_on_inertia_block,
        &default_active,
        &default_prior,
        &default_policy,
        &default_lexicon_control,
    )
}

#[allow(dead_code)]
fn disambiguate_payload_with_inertia_and_frame(
    state: &AppState,
    language: &str,
    token: &str,
    context: &str,
    top_k: usize,
    margin_threshold: f64,
    inertia_profile: Option<&LexemeInertiaProfile>,
    inertia_coefficient: f64,
    sandbox_on_inertia_block: bool,
    active_frame: &FrameContext,
    prior_frame: &FrameContext,
) -> Value {
    let default_policy = default_conservation_policy();
    let default_lexicon_control = resolve_lexicon_control(None);
    disambiguate_payload_with_inertia_and_frame_and_policy(
        state,
        language,
        token,
        context,
        top_k,
        margin_threshold,
        inertia_profile,
        inertia_coefficient,
        sandbox_on_inertia_block,
        active_frame,
        prior_frame,
        &default_policy,
        &default_lexicon_control,
    )
}

fn modal_label_for_branch(is_top_candidate: bool, would_commit: bool, forced: bool) -> Value {
    json!({
        "possibility": if would_commit { "possible" } else { "possible" },
        "necessity": if is_top_candidate && would_commit { "high" } else { "low" },
        "belief_source": if forced {
            "counterfactual"
        } else if is_top_candidate {
            "observed+retrieved"
        } else {
            "hypothetical"
        },
        "certainty_mode": if would_commit { "commit-ready" } else { "sandbox" },
        "counterfactual": forced,
    })
}

fn branch_rejection_causes(
    branch_score: f64,
    winner_score: Option<f64>,
    selected_match: bool,
    forced: bool,
    contradiction_pressure: f64,
    inertia_break_cost: f64,
    lobe_drift: f64,
    identity_persistence: f64,
    causal_consistency: f64,
    context_overlap: f64,
    frame_transition_cost: f64,
    contradiction_type: &str,
    conservation_loss: f64,
    conservation_blocked: bool,
) -> Vec<String> {
    let mut causes = Vec::<String>::new();
    if let Some(winner_score) = winner_score {
        if branch_score < winner_score {
            causes.push("lower_trajectory_coherence".to_string());
        }
    }
    if forced {
        causes.push("counterfactual_forced_branch".to_string());
    }
    if contradiction_pressure > 0.0 {
        causes.push("contradiction_pressure".to_string());
    }
    if inertia_break_cost > 0.0 {
        causes.push("high_inertia_break_cost".to_string());
    }
    if lobe_drift > 0.0 {
        causes.push("lobe_drift".to_string());
    }
    if identity_persistence < 1.0 {
        causes.push("identity_instability".to_string());
    }
    if causal_consistency <= 0.0 {
        causes.push("causal_conflict".to_string());
    }
    if context_overlap <= 0.0 {
        causes.push("weak_context_overlap".to_string());
    }
    if frame_transition_cost > 0.0 {
        causes.push("frame_transition_cost".to_string());
    }
    if contradiction_type != "global_contradiction" {
        causes.push(format!("typed_{}", contradiction_type));
    }
    if conservation_loss > 0.0 {
        causes.push("conservation_loss".to_string());
    }
    if conservation_blocked {
        causes.push("conservation_violation".to_string());
    }
    if selected_match && causes.is_empty() {
        causes.push("accepted_branch".to_string());
    }
    causes
}

fn build_sandbox_simulation(
    payload: &Value,
    language: &str,
    token: &str,
    context: &str,
    top_k: usize,
    margin_threshold: f64,
    inertia_coefficient: f64,
    branch_limit: usize,
    forced_sense_node: Option<&str>,
) -> Value {
    let frame_semantics = payload
        .get("frame_semantics")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let frame_transition = frame_semantics
        .get("transition")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let frame_transition_cost = frame_transition
        .get("transition_cost")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let frame_alignment_score = frame_transition
        .get("frame_alignment_score")
        .and_then(Value::as_f64)
        .unwrap_or(1.0);
    let frame_reconciliation_status = frame_transition
        .get("frame_reconciliation_status")
        .and_then(Value::as_str)
        .unwrap_or("stable");
    let contradiction_type = frame_semantics
        .get("contradiction_typing")
        .and_then(|v| v.get("type"))
        .and_then(Value::as_str)
        .unwrap_or("global_contradiction");
    let frame_operators = payload
        .get("frame_operators")
        .cloned()
        .unwrap_or_else(|| json!({}));

    let candidates = payload
        .get("candidates")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let selected = payload
        .get("selected_sense")
        .and_then(|v| v.get("node_id"))
        .and_then(Value::as_str)
        .map(|s| s.to_string());

    let effective_threshold = payload
        .get("resolver")
        .and_then(|v| v.get("inertia"))
        .and_then(|v| v.get("effective_margin_threshold"))
        .and_then(Value::as_f64)
        .unwrap_or(margin_threshold);

    let mut branches = candidates
        .iter()
        .take(branch_limit)
        .enumerate()
        .map(|(idx, candidate)| {
            let node_id = candidate
                .get("sense_node")
                .and_then(|v| v.get("node_id"))
                .and_then(Value::as_str)
                .unwrap_or("unknown_sense");
            let score = candidate.get("score").and_then(Value::as_f64).unwrap_or(0.0);
            let selected_match = selected.as_deref() == Some(node_id);
            let forced = forced_sense_node == Some(node_id);
            let would_commit = !forced && selected_match && score > 0.0 && score >= effective_threshold;
            let torsion = if forced {
                "counterfactual"
            } else if !selected_match {
                "alternative"
            } else {
                "stable"
            };

            json!({
                "branch_id": format!("sandbox-{}-{}", idx, node_id),
                "rank": idx + 1,
                "forced": forced,
                "commit_state": {
                    "would_commit": would_commit,
                    "committed": false,
                    "blocked": !would_commit,
                },
                "modal_semantics": modal_label_for_branch(selected_match, would_commit, forced),
                "trajectory": {
                    "status": if would_commit { "commit_ready" } else { "hypothetical" },
                    "selected_sense": node_id,
                    "score": score,
                    "effective_threshold": effective_threshold,
                    "margin_threshold": margin_threshold,
                    "inertia_coefficient": inertia_coefficient,
                    "torsion": torsion,
                },
                "candidate": candidate,
            })
        })
        .collect::<Vec<_>>();

    if let Some(forced_node) = forced_sense_node {
        let already_included = branches.iter().any(|branch| {
            branch
                .get("candidate")
                .and_then(|v| v.get("sense_node"))
                .and_then(|v| v.get("node_id"))
                .and_then(Value::as_str)
                == Some(forced_node)
        });
        if !already_included {
            let forced_candidate = candidates.iter().find(|candidate| {
                candidate
                    .get("sense_node")
                    .and_then(|v| v.get("node_id"))
                    .and_then(Value::as_str)
                    == Some(forced_node)
            });
            let forced_score = forced_candidate
                .and_then(|candidate| candidate.get("score"))
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            let forced_modal = modal_label_for_branch(false, false, true);
            branches.push(json!({
                "branch_id": format!("sandbox-forced-{}", forced_node),
                "rank": branches.len() + 1,
                "forced": true,
                "commit_state": {
                    "would_commit": false,
                    "committed": false,
                    "blocked": true,
                },
                "modal_semantics": forced_modal,
                "trajectory": {
                    "status": "hypothetical",
                    "selected_sense": forced_node,
                    "score": forced_score,
                    "effective_threshold": effective_threshold,
                    "margin_threshold": margin_threshold,
                    "inertia_coefficient": inertia_coefficient,
                    "torsion": "counterfactual",
                },
                "candidate": forced_candidate.cloned().unwrap_or_else(|| json!({
                    "sense_node": {"node_id": forced_node, "node_type": "semantic_sense", "label": forced_node},
                    "score": forced_score,
                    "features": {}
                })),
            }));
        }
    }

    let phrase_candidates = payload
        .get("phrase_layer")
        .and_then(|v| v.get("phrase_candidates"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    for phrase_candidate in phrase_candidates {
        let phrase_node_id = phrase_candidate
            .get("node_id")
            .and_then(Value::as_str)
            .unwrap_or("unknown_phrase");
        let phrase_label = phrase_candidate
            .get("label")
            .and_then(Value::as_str)
            .unwrap_or("unknown phrase");
        let preferred_sense = phrase_candidate
            .get("preferred_sense")
            .and_then(Value::as_str)
            .unwrap_or("unknown_sense");
        let phrase_coherence_score = phrase_candidate
            .get("phrase_coherence_score")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        if phrase_coherence_score <= 0.0 {
            continue;
        }
        let token_composition_score = phrase_candidate
            .get("token_composition_score")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let overlap_count = phrase_candidate
            .get("chunk_topology")
            .and_then(|v| v.get("overlap_count"))
            .and_then(Value::as_u64)
            .unwrap_or(0) as f64;
        let overlap_penalty = 0.25 * overlap_count;
        let branch_score = (3.0 * phrase_coherence_score) + token_composition_score - overlap_penalty;
        let branch_id = format!("sandbox-phrase-{}", phrase_node_id);
        let already_included = branches.iter().any(|branch| {
            branch.get("branch_id").and_then(Value::as_str) == Some(branch_id.as_str())
        });
        if already_included {
            continue;
        }
        branches.push(json!({
            "branch_id": branch_id,
            "rank": branches.len() + 1,
            "forced": false,
            "commit_state": {
                "would_commit": false,
                "committed": false,
                "blocked": true,
            },
            "modal_semantics": "possible_phrase",
            "trajectory": {
                "status": "hypothetical",
                "selected_sense": preferred_sense,
                "phrase_node_id": phrase_node_id,
                "score": branch_score,
                "effective_threshold": effective_threshold,
                "margin_threshold": margin_threshold,
                "inertia_coefficient": inertia_coefficient,
                "torsion": "compositional",
            },
            "candidate": {
                "sense_node": {
                    "node_id": phrase_node_id,
                    "node_type": "semantic_phrase",
                    "label": phrase_label,
                },
                "score": branch_score,
                "features": {
                    "context_overlap": phrase_coherence_score,
                    "retrieval_overlap": token_composition_score,
                    "phase_resonance": phrase_coherence_score,
                    "conflict": overlap_penalty,
                },
                "phrase": phrase_candidate,
            },
        }));
    }

    let projected_candidates = payload
        .get("frame_operators")
        .and_then(|v| v.get("projected_candidates"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    for projected in projected_candidates {
        let operator_id = projected
            .get("operator_id")
            .and_then(Value::as_str)
            .unwrap_or("unknown_operator");
        let candidate_node_id = projected
            .get("candidate_node_id")
            .and_then(Value::as_str)
            .unwrap_or("unknown_sense");
        let projected_score = projected
            .get("projected_score")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let transition_cost_delta = projected
            .get("transition_cost_delta")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let collapse_allowed = projected
            .get("collapse_allowed")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let branch_id = format!("sandbox-transform-{}-{}", operator_id, candidate_node_id);
        let already_included = branches
            .iter()
            .any(|branch| branch.get("branch_id").and_then(Value::as_str) == Some(branch_id.as_str()));
        if already_included {
            continue;
        }

        let source_candidate = candidates.iter().find(|candidate| {
            candidate
                .get("sense_node")
                .and_then(|v| v.get("node_id"))
                .and_then(Value::as_str)
                == Some(candidate_node_id)
        });
        let source_label = source_candidate
            .and_then(|candidate| candidate.get("sense_node"))
            .and_then(|v| v.get("label"))
            .and_then(Value::as_str)
            .unwrap_or(candidate_node_id);
        let base_context_overlap = source_candidate
            .and_then(|candidate| candidate.get("features"))
            .and_then(|v| v.get("context_overlap"))
            .and_then(Value::as_f64)
            .unwrap_or(0.0);

        branches.push(json!({
            "branch_id": branch_id,
            "rank": branches.len() + 1,
            "forced": false,
            "commit_state": {
                "would_commit": collapse_allowed && projected_score >= effective_threshold,
                "committed": false,
                "blocked": !collapse_allowed,
            },
            "modal_semantics": {
                "possibility": "possible_transform",
                "necessity": if collapse_allowed { "conditional" } else { "low" },
                "belief_source": "frame_operator_projection",
                "certainty_mode": "sandbox",
                "counterfactual": false,
            },
            "trajectory": {
                "status": "hypothetical",
                "selected_sense": candidate_node_id,
                "score": projected_score,
                "effective_threshold": effective_threshold,
                "margin_threshold": margin_threshold,
                "inertia_coefficient": inertia_coefficient,
                "torsion": "frame_transform",
                "transform_operator_id": operator_id,
            },
            "candidate": {
                "sense_node": {
                    "node_id": candidate_node_id,
                    "node_type": "semantic_transform",
                    "label": source_label,
                },
                "score": projected_score,
                "features": {
                    "context_overlap": base_context_overlap,
                    "retrieval_overlap": projected
                        .get("coherence_delta")
                        .and_then(Value::as_f64)
                        .unwrap_or(0.0),
                    "phase_resonance": projected
                        .get("coherence_delta")
                        .and_then(Value::as_f64)
                        .unwrap_or(0.0),
                    "conflict": transition_cost_delta.abs(),
                },
                "operator_projection": projected,
            },
        }));
    }

    let prior_selected_sense = payload
        .get("resolver")
        .and_then(|v| v.get("inertia"))
        .and_then(|v| v.get("prior_selected_sense"))
        .and_then(Value::as_str)
        .map(|s| s.to_string());
    let prior_selected_lobe = prior_selected_sense.as_deref().map(sense_lobe);

    for branch in branches.iter_mut() {
        let node_id = branch
            .get("trajectory")
            .and_then(|v| v.get("selected_sense"))
            .and_then(Value::as_str)
            .unwrap_or("unknown_sense");
        let score = branch
            .get("trajectory")
            .and_then(|v| v.get("score"))
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let features = branch
            .get("candidate")
            .and_then(|v| v.get("features"))
            .cloned()
            .unwrap_or_else(|| json!({}));
        let context_overlap = features
            .get("context_overlap")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let retrieval_overlap = features
            .get("retrieval_overlap")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let phase_resonance = features
            .get("phase_resonance")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let conflict = features
            .get("conflict")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let forced = branch.get("forced").and_then(Value::as_bool).unwrap_or(false);
        let resonance_alignment = context_overlap + retrieval_overlap + phase_resonance;
        let contradiction_pressure = conflict + if forced { 1.0 } else { 0.0 };
        let inertia_break_cost = if prior_selected_sense.as_deref() == Some(node_id) {
            0.0
        } else {
            inertia_coefficient
                * payload
                    .get("resolver")
                    .and_then(|v| v.get("inertia"))
                    .and_then(|v| v.get("crystallization_depth"))
                    .and_then(Value::as_f64)
                    .unwrap_or(0.0)
        };
        let candidate_lobe = sense_lobe(node_id);
        let lobe_drift = match prior_selected_lobe {
            Some(prev) if prev != candidate_lobe => 1.0,
            Some(_) => 0.0,
            None => 0.0,
        };
        let identity_persistence = if prior_selected_sense.as_deref() == Some(node_id) {
            1.0
        } else if prior_selected_lobe == Some(candidate_lobe) {
            0.65
        } else {
            0.25
        };
        let causal_consistency = if score >= effective_threshold { 1.0 } else { 0.0 };
        let conservation_loss = branch
            .get("candidate")
            .and_then(|v| v.get("operator_projection"))
            .and_then(|v| v.get("conservation_profile"))
            .and_then(|v| v.get("total_loss"))
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let conservation_blocked = branch
            .get("candidate")
            .and_then(|v| v.get("operator_projection"))
            .and_then(|v| v.get("conservation_profile"))
            .and_then(|v| v.get("admissible"))
            .and_then(Value::as_bool)
            .map(|admissible| !admissible)
            .unwrap_or(false);
        let frame_alignment_bonus = frame_alignment_score;
        let frame_transition_penalty = frame_transition_cost;
        let trajectory_coherence_score = resonance_alignment
            - contradiction_pressure
            - inertia_break_cost
            - lobe_drift
            - frame_transition_penalty
            - conservation_loss
            + identity_persistence
            + causal_consistency
            + frame_alignment_bonus;

        if let Some(obj) = branch.as_object_mut() {
            obj.insert(
                "coherence".to_string(),
                json!({
                    "trajectory_coherence_score": trajectory_coherence_score,
                    "resonance_alignment": resonance_alignment,
                    "contradiction_pressure": contradiction_pressure,
                    "inertia_break_cost": inertia_break_cost,
                    "lobe_drift": lobe_drift,
                    "identity_persistence": identity_persistence,
                    "causal_consistency": causal_consistency,
                    "frame_transition_cost": frame_transition_penalty,
                    "frame_alignment_bonus": frame_alignment_bonus,
                    "frame_reconciliation_status": frame_reconciliation_status,
                    "typed_contradiction": contradiction_type,
                    "conservation_loss": conservation_loss,
                    "conservation_blocked": conservation_blocked,
                }),
            );
        }
    }

    let mut ranked = branches.clone();
    ranked.sort_by(|a, b| {
        let score_a = a
            .get("coherence")
            .and_then(|v| v.get("trajectory_coherence_score"))
            .and_then(Value::as_f64)
            .unwrap_or(f64::NEG_INFINITY);
        let score_b = b
            .get("coherence")
            .and_then(|v| v.get("trajectory_coherence_score"))
            .and_then(Value::as_f64)
            .unwrap_or(f64::NEG_INFINITY);
        score_b
            .partial_cmp(&score_a)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                let id_a = a.get("branch_id").and_then(Value::as_str).unwrap_or("");
                let id_b = b.get("branch_id").and_then(Value::as_str).unwrap_or("");
                id_a.cmp(id_b)
            })
    });

    let winner = ranked.first().cloned();
    let winner_branch_id = winner
        .as_ref()
        .and_then(|branch| branch.get("branch_id"))
        .and_then(Value::as_str)
        .map(|s| s.to_string());
    let winner_selected_sense = winner
        .as_ref()
        .and_then(|branch| branch.get("trajectory"))
        .and_then(|v| v.get("selected_sense"))
        .and_then(Value::as_str)
        .map(|s| s.to_string());
    let winner_score = winner
        .as_ref()
        .and_then(|branch| branch.get("coherence"))
        .and_then(|v| v.get("trajectory_coherence_score"))
        .and_then(Value::as_f64)
        .unwrap_or(0.0);

    for branch in branches.iter_mut() {
        let branch_id = branch.get("branch_id").and_then(Value::as_str).unwrap_or("");
        let is_winner = winner_branch_id.as_deref() == Some(branch_id);
        let branch_score = branch
            .get("coherence")
            .and_then(|v| v.get("trajectory_coherence_score"))
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let selected_match = branch
            .get("trajectory")
            .and_then(|v| v.get("selected_sense"))
            .and_then(Value::as_str)
            == selected.as_deref();
        let forced = branch.get("forced").and_then(Value::as_bool).unwrap_or(false);
        let contradiction_pressure = branch
            .get("coherence")
            .and_then(|v| v.get("contradiction_pressure"))
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let inertia_break_cost = branch
            .get("coherence")
            .and_then(|v| v.get("inertia_break_cost"))
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let lobe_drift = branch
            .get("coherence")
            .and_then(|v| v.get("lobe_drift"))
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let identity_persistence = branch
            .get("coherence")
            .and_then(|v| v.get("identity_persistence"))
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let causal_consistency = branch
            .get("coherence")
            .and_then(|v| v.get("causal_consistency"))
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let resonance_alignment = branch
            .get("coherence")
            .and_then(|v| v.get("resonance_alignment"))
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let frame_transition_cost_branch = branch
            .get("coherence")
            .and_then(|v| v.get("frame_transition_cost"))
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let typed_contradiction = branch
            .get("coherence")
            .and_then(|v| v.get("typed_contradiction"))
            .and_then(Value::as_str)
            .unwrap_or("global_contradiction")
            .to_string();
        let conservation_loss = branch
            .get("coherence")
            .and_then(|v| v.get("conservation_loss"))
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let conservation_blocked = branch
            .get("coherence")
            .and_then(|v| v.get("conservation_blocked"))
            .and_then(Value::as_bool)
            .unwrap_or(false);

        if let Some(obj) = branch.as_object_mut() {
            obj.insert(
                "rejection".to_string(),
                json!({
                    "rejected_branch": !is_winner,
                    "winner_branch": winner_branch_id,
                    "winning_selected_sense": winner_selected_sense,
                    "rejection_causes": if is_winner {
                        vec!["accepted_branch".to_string()]
                    } else {
                        branch_rejection_causes(
                            branch_score,
                            Some(winner_score),
                            selected_match,
                            forced,
                            contradiction_pressure,
                            inertia_break_cost,
                            lobe_drift,
                            identity_persistence,
                            causal_consistency,
                            resonance_alignment,
                            frame_transition_cost_branch,
                            typed_contradiction.as_str(),
                            conservation_loss,
                            conservation_blocked,
                        )
                    },
                }),
            );
        }
    }

    json!({
        "object": "csif.simulation.result",
        "schema_version": "csif_simulation_v1",
        "deterministic": true,
        "sandbox": true,
        "committed": false,
        "input": {
            "language": language,
            "token": token,
            "context": context,
            "top_k": top_k,
            "margin_threshold": margin_threshold,
            "inertia_coefficient": inertia_coefficient,
            "branch_limit": branch_limit,
            "forced_sense_node": forced_sense_node,
        },
        "modal_semantics": {
            "layer": 1,
            "possibility": "explicit",
            "necessity": "conditional",
            "belief_source": "sandbox",
            "certainty_mode": "hypothetical",
            "perspective_frame": "internal"
        },
        "frame_semantics": frame_semantics,
        "frame_operators": frame_operators,
        "sandbox_decision": {
            "commit_ready_count": branches
                .iter()
                .filter(|branch| branch.get("commit_state").and_then(|v| v.get("would_commit")).and_then(Value::as_bool) == Some(true))
                .count(),
            "alternate_branch_count": branches.len(),
            "reconciliation_required": branches.iter().any(|branch| branch.get("commit_state").and_then(|v| v.get("blocked")).and_then(Value::as_bool) == Some(true)),
            "competitive_selection": true,
            "winner_branch_id": winner_branch_id,
            "winning_selected_sense": winner_selected_sense,
            "winner_score": winner_score,
            "ranked_branch_ids": ranked
                .iter()
                .filter_map(|branch| branch.get("branch_id").and_then(Value::as_str))
                .map(|s| s.to_string())
                .collect::<Vec<_>>(),
            "trajectory_coherence_score": winner_score,
            "frame_alignment_score": frame_alignment_score,
            "frame_transition_cost": frame_transition_cost,
            "frame_reconciliation_status": frame_reconciliation_status,
            "typed_contradiction": contradiction_type,
            "transform_branch_count": branches
                .iter()
                .filter(|branch| {
                    branch
                        .get("trajectory")
                        .and_then(|v| v.get("torsion"))
                        .and_then(Value::as_str)
                        == Some("frame_transform")
                })
                .count(),
        },
        "branches": branches,
    })
}

fn build_reconciliation_payload(
    simulation: &Value,
    losing_branch_id: Option<&str>,
) -> Result<Value, String> {
    let winner_branch_id = simulation
        .get("sandbox_decision")
        .and_then(|v| v.get("winner_branch_id"))
        .and_then(Value::as_str)
        .ok_or_else(|| "simulation did not produce winner_branch_id".to_string())?;
    let branches = simulation
        .get("branches")
        .and_then(Value::as_array)
        .ok_or_else(|| "simulation did not produce branches".to_string())?;

    let winner = branches
        .iter()
        .find(|branch| branch.get("branch_id").and_then(Value::as_str) == Some(winner_branch_id))
        .ok_or_else(|| format!("winner branch {} missing from branches", winner_branch_id))?;

    let loser = if let Some(requested_loser) = losing_branch_id {
        branches
            .iter()
            .find(|branch| branch.get("branch_id").and_then(Value::as_str) == Some(requested_loser))
            .ok_or_else(|| format!("losing branch {} not found", requested_loser))?
    } else {
        branches
            .iter()
            .find(|branch| branch.get("branch_id").and_then(Value::as_str) != Some(winner_branch_id))
            .ok_or_else(|| "simulation did not produce a losing branch to reconcile".to_string())?
    };

    let winner_coherence = winner
        .get("coherence")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let loser_coherence = loser
        .get("coherence")
        .cloned()
        .unwrap_or_else(|| json!({}));

    let winner_selected_sense = winner
        .get("trajectory")
        .and_then(|v| v.get("selected_sense"))
        .and_then(Value::as_str)
        .unwrap_or("unknown_sense");
    let loser_selected_sense = loser
        .get("trajectory")
        .and_then(|v| v.get("selected_sense"))
        .and_then(Value::as_str)
        .unwrap_or("unknown_sense");

    let winner_lobe = sense_lobe(winner_selected_sense);
    let loser_lobe = sense_lobe(loser_selected_sense);

    let winner_node_type = winner
        .get("candidate")
        .and_then(|v| v.get("sense_node"))
        .and_then(|v| v.get("node_type"))
        .and_then(Value::as_str)
        .unwrap_or("semantic_sense");
    let loser_node_type = loser
        .get("candidate")
        .and_then(|v| v.get("sense_node"))
        .and_then(|v| v.get("node_type"))
        .and_then(Value::as_str)
        .unwrap_or("semantic_sense");
    let phrase_reconciliation_mode = if winner_node_type == "semantic_phrase"
        || loser_node_type == "semantic_phrase"
    {
        "phrase_vs_token_or_phrase"
    } else {
        "token_sense_only"
    };

    let frame_semantics = simulation
        .get("frame_semantics")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let frame_transition = frame_semantics
        .get("transition")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let frame_alignment_score = frame_transition
        .get("frame_alignment_score")
        .and_then(Value::as_f64)
        .unwrap_or(1.0);
    let frame_transition_cost = frame_transition
        .get("transition_cost")
        .and_then(Value::as_f64)
        .unwrap_or(0.0);
    let frame_reconciliation_status = frame_transition
        .get("frame_reconciliation_status")
        .and_then(Value::as_str)
        .unwrap_or("stable");
    let contradiction_type = frame_semantics
        .get("contradiction_typing")
        .and_then(|v| v.get("type"))
        .and_then(Value::as_str)
        .unwrap_or("global_contradiction");
    let winner_operator_audit = winner
        .get("candidate")
        .and_then(|v| v.get("operator_projection"))
        .cloned()
        .unwrap_or(Value::Null);
    let loser_operator_audit = loser
        .get("candidate")
        .and_then(|v| v.get("operator_projection"))
        .cloned()
        .unwrap_or(Value::Null);
    let winner_conservation_profile = winner
        .get("candidate")
        .and_then(|v| v.get("operator_projection"))
        .and_then(|v| v.get("conservation_profile"))
        .cloned()
        .unwrap_or(Value::Null);
    let loser_conservation_profile = loser
        .get("candidate")
        .and_then(|v| v.get("operator_projection"))
        .and_then(|v| v.get("conservation_profile"))
        .cloned()
        .unwrap_or(Value::Null);

    Ok(json!({
        "object": "csif.reconciliation.result",
        "schema_version": "csif_reconciliation_v1",
        "deterministic": true,
        "winner_branch_id": winner_branch_id,
        "losing_branch_id": loser.get("branch_id").and_then(Value::as_str),
        "winner_selected_sense": winner_selected_sense,
        "losing_selected_sense": loser_selected_sense,
        "topology_explanation": {
            "identity_persistence": winner_coherence.get("identity_persistence").and_then(Value::as_f64).unwrap_or(0.0),
            "causal_alignment": winner_coherence.get("causal_consistency").and_then(Value::as_f64).unwrap_or(0.0),
            "historical_resonance": winner_coherence.get("resonance_alignment").and_then(Value::as_f64).unwrap_or(0.0),
            "inertia_break_cost": winner_coherence.get("inertia_break_cost").and_then(Value::as_f64).unwrap_or(0.0),
            "lobe_stability": if winner_lobe == loser_lobe { 1.0 } else { 0.5 },
            "winner_score": winner_coherence.get("trajectory_coherence_score").and_then(Value::as_f64).unwrap_or(0.0),
            "winner_node_type": winner_node_type,
            "loser_node_type": loser_node_type,
            "phrase_reconciliation_mode": phrase_reconciliation_mode,
            "winner_phrase_node_id": winner
                .get("trajectory")
                .and_then(|v| v.get("phrase_node_id"))
                .and_then(Value::as_str),
            "loser_phrase_node_id": loser
                .get("trajectory")
                .and_then(|v| v.get("phrase_node_id"))
                .and_then(Value::as_str),
            "winner_chunk_topology": winner
                .get("candidate")
                .and_then(|v| v.get("phrase"))
                .and_then(|v| v.get("chunk_topology"))
                .cloned()
                .unwrap_or(Value::Null),
            "loser_chunk_topology": loser
                .get("candidate")
                .and_then(|v| v.get("phrase"))
                .and_then(|v| v.get("chunk_topology"))
                .cloned()
                .unwrap_or(Value::Null),
            "prior_frame_signature": frame_transition
                .get("prior_frame_signature")
                .and_then(Value::as_str),
            "candidate_frame_signature": frame_transition
                .get("candidate_frame_signature")
                .and_then(Value::as_str),
            "frame_alignment_score": frame_alignment_score,
            "frame_transition_cost": frame_transition_cost,
            "frame_reconciliation_status": frame_reconciliation_status,
            "typed_contradiction": contradiction_type,
            "winner_operator_audit": winner_operator_audit,
            "loser_operator_audit": loser_operator_audit,
            "winner_conservation_profile": winner_conservation_profile,
            "loser_conservation_profile": loser_conservation_profile,
            "violated_invariants": loser
                .get("candidate")
                .and_then(|v| v.get("operator_projection"))
                .and_then(|v| v.get("conservation_profile"))
                .and_then(|v| v.get("violated_invariants"))
                .cloned()
                .unwrap_or_else(|| json!([])),
        },
        "rejected_topology": {
            "contradiction_pressure": loser_coherence.get("contradiction_pressure").and_then(Value::as_f64).unwrap_or(0.0),
            "semantic_drift": loser_coherence.get("lobe_drift").and_then(Value::as_f64).unwrap_or(0.0),
            "identity_fragmentation": 1.0 - loser_coherence.get("identity_persistence").and_then(Value::as_f64).unwrap_or(0.0),
            "loser_score": loser_coherence.get("trajectory_coherence_score").and_then(Value::as_f64).unwrap_or(0.0),
            "frame_transition_cost": loser_coherence.get("frame_transition_cost").and_then(Value::as_f64).unwrap_or(0.0),
            "typed_contradiction": loser_coherence.get("typed_contradiction").and_then(Value::as_str).unwrap_or("global_contradiction"),
            "loser_operator_audit": loser
                .get("candidate")
                .and_then(|v| v.get("operator_projection"))
                .cloned()
                .unwrap_or(Value::Null),
            "loser_conservation_profile": loser
                .get("candidate")
                .and_then(|v| v.get("operator_projection"))
                .and_then(|v| v.get("conservation_profile"))
                .cloned()
                .unwrap_or(Value::Null),
        },
        "frame_semantics": frame_semantics,
        "rejection_causes": loser
            .get("rejection")
            .and_then(|v| v.get("rejection_causes"))
            .cloned()
            .unwrap_or_else(|| json!([])),
        "winner_branch": winner,
        "losing_branch": loser,
    }))
}

#[allow(dead_code)]
fn disambiguate_payload(
    state: &AppState,
    language: &str,
    token: &str,
    context: &str,
    top_k: usize,
    margin_threshold: f64,
) -> Value {
    disambiguate_payload_with_inertia(
        state,
        language,
        token,
        context,
        top_k,
        margin_threshold,
        None,
        0.0,
        false,
    )
}

fn build_disambiguation_event(
    payload: &Value,
    language: &str,
    token: &str,
    context: &str,
    top_k: usize,
    margin_threshold: f64,
    previous_event: Option<&Value>,
) -> Value {
    let now = unix_time_secs();
    let status = payload
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let selected_node = payload
        .get("selected_sense")
        .and_then(|v| v.get("node_id"))
        .and_then(Value::as_str)
        .map(|v| v.to_string());
    let ambiguity_margin = payload
        .get("resolver")
        .and_then(|v| v.get("ambiguity_margin"))
        .and_then(Value::as_f64)
        .unwrap_or(0.0);

    let candidates = payload
        .get("candidates")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let candidate_rankings = candidates
        .iter()
        .map(|c| {
            json!({
                "node_id": c
                    .get("sense_node")
                    .and_then(|v| v.get("node_id"))
                    .and_then(Value::as_str)
                    .unwrap_or(""),
                "score": c.get("score").and_then(Value::as_f64).unwrap_or(0.0),
                "features": c.get("features").cloned().unwrap_or_else(|| json!({})),
            })
        })
        .collect::<Vec<_>>();

    let rejected_senses = candidate_rankings
        .iter()
        .filter_map(|entry| {
            let node_id = entry.get("node_id").and_then(Value::as_str)?;
            if selected_node.as_deref() == Some(node_id) {
                None
            } else {
                Some(node_id.to_string())
            }
        })
        .collect::<Vec<_>>();

    let retrieval_match_count = payload
        .get("retrieval_evidence")
        .and_then(Value::as_array)
        .map(|v| v.len())
        .unwrap_or(0);

    let previous_selected = previous_event
        .and_then(|evt| evt.get("selected_sense"))
        .and_then(Value::as_str)
        .map(|v| v.to_string());
    let previous_status = previous_event
        .and_then(|evt| evt.get("status"))
        .and_then(Value::as_str)
        .map(|v| v.to_string());

    let inertia_blocked = payload
        .get("inertia_decision")
        .and_then(|v| v.get("blocked"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let inertia_reason = payload
        .get("inertia_decision")
        .and_then(|v| v.get("reason"))
        .and_then(Value::as_str)
        .unwrap_or("none");

    let changed = match (&previous_selected, &selected_node) {
        (Some(prev), Some(curr)) => prev != curr,
        _ => false,
    };
    let contradiction_encountered = changed || inertia_blocked;

    json!({
        "object": "csif.disambiguation.event",
        "schema_version": "csif_disambiguation_event_v1",
        "event_id": format!("sde-{}-{}", now, token.to_ascii_lowercase()),
        "created_at": now,
        "input": {
            "language": language,
            "token": token,
            "context": context,
            "top_k": top_k,
            "margin_threshold": margin_threshold,
        },
        "status": status,
        "selected_sense": selected_node,
        "candidate_rankings": candidate_rankings,
        "rejected_senses": rejected_senses,
        "confidence": {
            "ambiguity_margin": ambiguity_margin,
            "margin_threshold": margin_threshold,
            "resolved": payload.get("status") == Some(&json!("resolved")),
        },
        "retrieval_match_count": retrieval_match_count,
        "contradiction": {
            "encountered": contradiction_encountered,
            "reason": if contradiction_encountered {
                if changed {
                    "selected sense changed from prior resolved event"
                } else {
                    inertia_reason
                }
            } else {
                "none"
            }
        },
        "torsion": {
            "reassignment_pressure": payload
                .get("resolver")
                .and_then(|v| v.get("inertia"))
                .and_then(|v| v.get("reassignment_pressure"))
                .and_then(Value::as_bool)
                .unwrap_or(false),
            "inertia_blocked": inertia_blocked,
            "effective_margin_threshold": payload
                .get("resolver")
                .and_then(|v| v.get("inertia"))
                .and_then(|v| v.get("effective_margin_threshold"))
                .and_then(Value::as_f64),
            "ambiguity_margin": ambiguity_margin,
        },
        "correction_history": {
            "previous_selected_sense": previous_selected,
            "previous_status": previous_status,
            "changed": changed
        }
    })
}

fn read_sense_trajectory_events(path: &str) -> Result<Vec<Value>, String> {
    if !Path::new(path).exists() {
        return Ok(Vec::new());
    }

    let file = fs::File::open(path).map_err(|e| format!("failed reading {}: {}", path, e))?;
    let reader = BufReader::new(file);
    let mut events = Vec::new();

    for (line_no, line) in reader.lines().enumerate() {
        let line = line.map_err(|e| format!("failed reading {} line {}: {}", path, line_no + 1, e))?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match serde_json::from_str::<Value>(trimmed) {
            Ok(v) => events.push(v),
            Err(e) => {
                return Err(format!(
                    "failed parsing {} line {} as json: {}",
                    path,
                    line_no + 1,
                    e
                ));
            }
        }
    }

    Ok(events)
}

fn append_sense_trajectory_event(path: &str, event: &Value) -> Result<(), String> {
    let parent = Path::new(path).parent();
    if let Some(parent_dir) = parent {
        if !parent_dir.as_os_str().is_empty() {
            fs::create_dir_all(parent_dir)
                .map_err(|e| format!("failed creating {}: {}", parent_dir.display(), e))?;
        }
    }

    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| format!("failed opening {} for append: {}", path, e))?;
    let line = serde_json::to_string(event).map_err(|e| format!("failed serializing event: {}", e))?;
    writeln!(file, "{}", line).map_err(|e| format!("failed appending {}: {}", path, e))?;
    Ok(())
}

fn latest_event_for_lexeme(events: &[Value], language: &str, token: &str) -> Option<Value> {
    events
        .iter()
        .rev()
        .find(|evt| {
            evt.get("input")
                .and_then(|v| v.get("language"))
                .and_then(Value::as_str)
                == Some(language)
                && evt
                    .get("input")
                    .and_then(|v| v.get("token"))
                    .and_then(Value::as_str)
                    == Some(token)
        })
        .cloned()
}

fn build_lexeme_inertia_profile(events: &[Value], language: &str, token: &str) -> LexemeInertiaProfile {
    let mut resolved = Vec::<String>::new();
    for evt in events {
        if evt
            .get("input")
            .and_then(|v| v.get("language"))
            .and_then(Value::as_str)
            != Some(language)
        {
            continue;
        }
        if evt
            .get("input")
            .and_then(|v| v.get("token"))
            .and_then(Value::as_str)
            != Some(token)
        {
            continue;
        }
        if evt.get("status").and_then(Value::as_str) != Some("resolved") {
            continue;
        }
        if let Some(sel) = evt.get("selected_sense").and_then(Value::as_str) {
            resolved.push(sel.to_string());
        }
    }

    let resolved_count = resolved.len();
    let last_selected_sense = resolved.last().cloned();
    let current_streak = if let Some(last) = last_selected_sense.as_ref() {
        resolved
            .iter()
            .rev()
            .take_while(|sense| *sense == last)
            .count()
    } else {
        0
    };

    let crystallization_depth = if resolved_count == 0 {
        0.0
    } else {
        current_streak as f64 / resolved_count as f64
    };

    LexemeInertiaProfile {
        crystallization_depth,
        last_selected_sense,
        current_streak,
        resolved_count,
    }
}

fn filtered_trajectory_events(
    state: &AppState,
    language: Option<&str>,
    token: Option<&str>,
    limit: usize,
) -> Result<Vec<Value>, String> {
    let Some(path) = state.sense_trajectory_log_path.as_ref() else {
        return Ok(Vec::new());
    };

    let mut events = read_sense_trajectory_events(path)?;
    if let Some(lang) = language {
        events.retain(|evt| {
            evt.get("input")
                .and_then(|v| v.get("language"))
                .and_then(Value::as_str)
                == Some(lang)
        });
    }
    if let Some(tok) = token {
        events.retain(|evt| {
            evt.get("input")
                .and_then(|v| v.get("token"))
                .and_then(Value::as_str)
                == Some(tok)
        });
    }

    let count = events.len();
    if count > limit {
        events = events.split_off(count - limit);
    }
    Ok(events)
}

fn trajectory_events_payload(
    state: &AppState,
    language: Option<&str>,
    token: Option<&str>,
    limit: usize,
) -> Result<Value, String> {
    let Some(path) = state.sense_trajectory_log_path.as_ref() else {
        return Ok(json!({
            "object": "csif.disambiguation.trajectory.list",
            "schema_version": "csif_disambiguation_event_v1",
            "log_enabled": false,
            "count": 0,
            "events": []
        }));
    };

    let events = filtered_trajectory_events(state, language, token, limit)?;

    Ok(json!({
        "object": "csif.disambiguation.trajectory.list",
        "schema_version": "csif_disambiguation_event_v1",
        "log_enabled": true,
        "log_path": path,
        "count": events.len(),
        "filters": {
            "language": language,
            "token": token,
            "limit": limit,
        },
        "events": events,
    }))
}

fn summarize_trajectory_events(events: &[Value]) -> Value {
    if events.is_empty() {
        return json!({
            "event_count": 0,
            "resolved_count": 0,
            "ambiguous_count": 0,
            "unknown_count": 0,
            "stability_score": 1.0,
            "contradiction_rate": 0.0,
            "crystallization_depth": 0.0,
            "ambiguity_entropy": 0.0,
            "lobe_drift": 0.0,
            "resonance_persistence": 0.0,
            "sense_histogram": {},
            "lobe_histogram": {},
            "last_selected_sense": Value::Null,
        });
    }

    let event_count = events.len();
    let mut resolved_count = 0usize;
    let mut ambiguous_count = 0usize;
    let mut unknown_count = 0usize;
    let mut contradiction_count = 0usize;
    let mut transition_count = 0usize;
    let mut lobe_transition_count = 0usize;
    let mut max_run = 0usize;
    let mut current_run = 0usize;
    let mut last_sense: Option<String> = None;
    let mut last_lobe: Option<&str> = None;
    let mut resonance_sum = 0.0f64;
    let mut resonance_obs = 0usize;
    let mut sense_hist = HashMap::<String, usize>::new();
    let mut lobe_hist = HashMap::<String, usize>::new();

    for event in events {
        let status = event.get("status").and_then(Value::as_str).unwrap_or("unknown");
        match status {
            "resolved" => resolved_count += 1,
            "ambiguous" => ambiguous_count += 1,
            _ => unknown_count += 1,
        }

        if event
            .get("contradiction")
            .and_then(|v| v.get("encountered"))
            .and_then(Value::as_bool)
            == Some(true)
        {
            contradiction_count += 1;
        }

        if let Some(margin) = event
            .get("confidence")
            .and_then(|v| v.get("ambiguity_margin"))
            .and_then(Value::as_f64)
        {
            resonance_sum += margin;
            resonance_obs += 1;
        }

        let selected = event
            .get("selected_sense")
            .and_then(Value::as_str)
            .map(|s| s.to_string());
        if let Some(sel) = selected.as_ref() {
            *sense_hist.entry(sel.clone()).or_insert(0) += 1;
            let lobe = sense_lobe(sel);
            *lobe_hist.entry(lobe.to_string()).or_insert(0) += 1;

            if let Some(prev) = last_sense.as_ref() {
                if prev != sel {
                    transition_count += 1;
                    current_run = 1;
                } else {
                    current_run += 1;
                }
            } else {
                current_run = 1;
            }
            if current_run > max_run {
                max_run = current_run;
            }

            if let Some(prev_lobe) = last_lobe {
                if prev_lobe != lobe {
                    lobe_transition_count += 1;
                }
            }

            last_sense = Some(sel.clone());
            last_lobe = Some(lobe);
        }
    }

    let stability_score = if resolved_count <= 1 {
        1.0
    } else {
        1.0 - (transition_count as f64 / (resolved_count - 1) as f64)
    };
    let contradiction_rate = contradiction_count as f64 / event_count as f64;
    let crystallization_depth = if resolved_count == 0 {
        0.0
    } else {
        max_run as f64 / resolved_count as f64
    };

    let categories = sense_hist.len();
    let ambiguity_entropy = if categories <= 1 {
        0.0
    } else {
        let total = resolved_count as f64;
        let entropy = sense_hist
            .values()
            .map(|count| {
                let p = *count as f64 / total;
                -(p * (p.ln() / std::f64::consts::LN_2))
            })
            .sum::<f64>();
        entropy / ((categories as f64).ln() / std::f64::consts::LN_2)
    };

    let lobe_drift = if resolved_count <= 1 {
        0.0
    } else {
        lobe_transition_count as f64 / (resolved_count - 1) as f64
    };

    let resonance_persistence = if resonance_obs == 0 {
        0.0
    } else {
        resonance_sum / resonance_obs as f64
    };

    json!({
        "event_count": event_count,
        "resolved_count": resolved_count,
        "ambiguous_count": ambiguous_count,
        "unknown_count": unknown_count,
        "stability_score": stability_score,
        "contradiction_rate": contradiction_rate,
        "crystallization_depth": crystallization_depth,
        "ambiguity_entropy": ambiguity_entropy,
        "lobe_drift": lobe_drift,
        "resonance_persistence": resonance_persistence,
        "sense_histogram": sense_hist,
        "lobe_histogram": lobe_hist,
        "last_selected_sense": last_sense,
    })
}

fn build_bank_index(bank: &Value) -> BankIndex {
    let mut entries = Vec::<IndexEntry>::new();

    if let Some(crystals) = bank.get("crystals").and_then(Value::as_array) {
        for crystal in crystals {
            let crystal_id = crystal
                .get("crystal_id")
                .and_then(Value::as_str)
                .unwrap_or("<unknown_crystal>")
                .to_string();

            if let Some(edges) = crystal.get("edges").and_then(Value::as_array) {
                for edge in edges {
                    let edge_id = edge
                        .get("edge_id")
                        .and_then(Value::as_str)
                        .unwrap_or("<unknown_edge>")
                        .to_string();
                    let source_node = edge
                        .get("source_node")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    let relation = edge
                        .get("relation")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    let target_node = edge
                        .get("target_node")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();

                    let searchable_text = format!("{} {} {}", source_node, relation, target_node)
                        .trim()
                        .to_string();

                    entries.push(IndexEntry {
                        crystal_id: crystal_id.clone(),
                        edge_id,
                        source_node,
                        relation,
                        target_node,
                        searchable_text,
                    });
                }
            }
        }
    }

    let mut postings = HashMap::<String, Vec<usize>>::new();
    for (idx, entry) in entries.iter().enumerate() {
        let mut seen = HashSet::<String>::new();
        for token in tokenize(&entry.searchable_text) {
            if seen.insert(token.clone()) {
                postings.entry(token).or_default().push(idx);
            }
        }
    }

    BankIndex { entries, postings }
}

fn retrieve_from_index(index: &BankIndex, query: &str, top_k: usize) -> Vec<(usize, usize)> {
    let mut scores = HashMap::<usize, usize>::new();
    for token in tokenize(query) {
        if let Some(ids) = index.postings.get(&token) {
            for id in ids {
                *scores.entry(*id).or_insert(0) += 1;
            }
        }
    }

    let mut ranked = scores.into_iter().collect::<Vec<_>>();
    ranked.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then_with(|| {
                index.entries[a.0]
                    .searchable_text
                    .cmp(&index.entries[b.0].searchable_text)
            })
    });
    ranked.truncate(top_k);
    ranked
}

fn index_summary_payload(state: &AppState) -> Value {
    if let (Some(summary), Some(index)) = (state.bank_summary.as_ref(), state.bank_index.as_ref()) {
        let mut top_terms = index
            .postings
            .iter()
            .map(|(term, ids)| (term.clone(), ids.len()))
            .collect::<Vec<_>>();
        top_terms.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        top_terms.truncate(20);

        return json!({
            "loaded": true,
            "bank_id": summary.bank_id,
            "crystal_count": summary.crystal_count,
            "edge_count": summary.edge_count,
            "event_count": summary.event_count,
            "index_entries": index.entries.len(),
            "vocab_size": index.postings.len(),
            "top_terms": top_terms
                .into_iter()
                .map(|(term, freq)| json!({"term": term, "entry_hits": freq}))
                .collect::<Vec<_>>()
        });
    }

    json!({
        "loaded": false,
        "message": "No RWIF bank index loaded"
    })
}

fn build_index_output(bank: &Value) -> Value {
    let summary = summarize_bank(bank);
    let index = build_bank_index(bank);
    let mut top_terms = index
        .postings
        .iter()
        .map(|(term, ids)| (term.clone(), ids.len()))
        .collect::<Vec<_>>();
    top_terms.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    top_terms.truncate(50);

    json!({
        "suite": "rwif_v2_bank_index",
        "bank_id": summary.bank_id,
        "crystal_count": summary.crystal_count,
        "edge_count": summary.edge_count,
        "event_count": summary.event_count,
        "index_entries": index.entries.len(),
        "vocab_size": index.postings.len(),
        "top_terms": top_terms
            .into_iter()
            .map(|(term, freq)| json!({"term": term, "entry_hits": freq}))
            .collect::<Vec<_>>(),
        "entries": index
            .entries
            .iter()
            .map(|e| {
                json!({
                    "crystal_id": e.crystal_id,
                    "edge_id": e.edge_id,
                    "source_node": e.source_node,
                    "relation": e.relation,
                    "target_node": e.target_node,
                    "searchable_text": e.searchable_text
                })
            })
            .collect::<Vec<_>>()
    })
}

fn format_number(n: f64) -> String {
    if (n.fract()).abs() < 1e-12 {
        format!("{:.0}", n)
    } else {
        format!("{:.12}", n)
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    }
}

fn normalize_two_arg_function_whitespace(expr: &str, function_name: &str) -> String {
    let bytes = expr.as_bytes();
    let fn_lower = function_name.to_ascii_lowercase();
    let mut i = 0usize;
    let mut out = String::with_capacity(expr.len());

    while i < bytes.len() {
        if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
            let start = i;
            i += 1;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            let ident = &expr[start..i];
            let ident_lower = ident.to_ascii_lowercase();

            let mut j = i;
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }

            if ident_lower == fn_lower && j < bytes.len() && bytes[j] == b'(' {
                out.push_str(&expr[start..=j]);
                let mut k = j + 1;
                let mut depth = 1i32;
                while k < bytes.len() && depth > 0 {
                    match bytes[k] {
                        b'(' => depth += 1,
                        b')' => depth -= 1,
                        _ => {}
                    }
                    k += 1;
                }
                if depth != 0 {
                    out.push_str(&expr[j + 1..]);
                    return out;
                }

                let inner = &expr[j + 1..k - 1];
                if inner.contains(',') {
                    out.push_str(inner);
                } else {
                    let inner_bytes = inner.as_bytes();
                    let mut split_idx: Option<usize> = None;
                    let mut inner_depth = 0i32;
                    for idx in 0..inner_bytes.len() {
                        match inner_bytes[idx] {
                            b'(' => inner_depth += 1,
                            b')' => inner_depth -= 1,
                            b' ' | b'\t' | b'\n' | b'\r' if inner_depth == 0 => {
                                let prev = inner[..idx].trim_end();
                                let next = inner[idx + 1..].trim_start();
                                if !prev.is_empty() && !next.is_empty() {
                                    split_idx = Some(idx);
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }

                    if let Some(idx) = split_idx {
                        let lhs = inner[..idx].trim();
                        let rhs = inner[idx + 1..].trim();
                        out.push_str(lhs);
                        out.push(',');
                        out.push_str(rhs);
                    } else {
                        out.push_str(inner);
                    }
                }
                out.push(')');
                i = k;
            } else {
                out.push_str(ident);
            }
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }

    out
}

fn normalize_expression(expr: &str) -> String {
    // Map Greek letters to function names
    let expr = expr
        .replace("Γ", "gamma")
        .replace("γ", "gamma")
        .replace("Ζ", "zeta")
        .replace("ζ", "zeta")
        .replace("∫", "integral")
        .replace("π", "pi")
        .replace("⇒", "=>")
        .replace("→", "=>")
        .replace("↔", "<->")
        .replace("⇔", "<->")
        .replace("⊕", "^^");
    
    let e1 = normalize_two_arg_function_whitespace(&expr, "c");
    let e2 = normalize_two_arg_function_whitespace(&e1, "comb");
    let e3 = normalize_two_arg_function_whitespace(&e2, "atan2");
    let e4 = normalize_two_arg_function_whitespace(&e3, "polylog");
    let e5 = normalize_two_arg_function_whitespace(&e4, "gammainc");
    let e6 = normalize_two_arg_function_whitespace(&e5, "j_sph");
    let e7 = normalize_two_arg_function_whitespace(&e6, "beta");
    normalize_two_arg_function_whitespace(&e7, "theta4")
}

fn tokenize_expression(expr: &str) -> Result<Vec<Token>, String> {
    let normalized = normalize_expression(expr);
    let mut tokens = Vec::<Token>::new();
    let chars = normalized.chars().collect::<Vec<_>>();
    let mut i = 0usize;

    let push_token = |tokens: &mut Vec<Token>, token: Token| {
        let is_logic_keyword = matches!(&token, Token::Identifier(name) if name == "and" || name == "or" || name == "not" || name == "xor" || name == "implies" || name == "equiv" || name == "iff" || name == "true" || name == "false")
            || matches!(token, Token::AndAnd | Token::OrOr | Token::XorCaret | Token::ImpliesArrow | Token::EquivArrow);
        let needs_implicit_mul = matches!(tokens.last(), Some(Token::Number(_) | Token::Imaginary | Token::RParen))
            && matches!(token, Token::Number(_) | Token::Identifier(_) | Token::Imaginary | Token::LParen)
            && !is_logic_keyword;

        if needs_implicit_mul {
            tokens.push(Token::Star);
        }

        if matches!(tokens.last(), Some(Token::Identifier(_))) && matches!(token, Token::LParen) {
            // Function call: leave as-is.
        }

        tokens.push(token);
    };

    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }

        match c {
            '+' => {
                push_token(&mut tokens, Token::Plus);
                i += 1;
            }
            '&' => {
                if i + 1 < chars.len() && chars[i + 1] == '&' {
                    push_token(&mut tokens, Token::AndAnd);
                    i += 2;
                } else {
                    return Err(format!("unexpected character '{}' at position {}", c, i + 1));
                }
            }
            '|' => {
                if i + 1 < chars.len() && chars[i + 1] == '|' {
                    push_token(&mut tokens, Token::OrOr);
                    i += 2;
                } else {
                    return Err(format!("unexpected character '{}' at position {}", c, i + 1));
                }
            }
            '-' => {
                if i + 1 < chars.len() && chars[i + 1] == '>' {
                    push_token(&mut tokens, Token::ImpliesArrow);
                    i += 2;
                } else {
                    push_token(&mut tokens, Token::Minus);
                    i += 1;
                }
            }
            '*' => {
                push_token(&mut tokens, Token::Star);
                i += 1;
            }
            '/' => {
                push_token(&mut tokens, Token::Slash);
                i += 1;
            }
            '=' => {
                if i + 1 < chars.len() && chars[i + 1] == '>' {
                    push_token(&mut tokens, Token::ImpliesArrow);
                    i += 2;
                } else if i + 1 < chars.len() && chars[i + 1] == '=' {
                    push_token(&mut tokens, Token::EqEq);
                    i += 2;
                } else {
                    return Err(format!("unexpected character '{}' at position {}", c, i + 1));
                }
            }
            '<' => {
                if i + 2 < chars.len() && chars[i + 1] == '-' && chars[i + 2] == '>' {
                    push_token(&mut tokens, Token::EquivArrow);
                    i += 3;
                } else if i + 1 < chars.len() && chars[i + 1] == '=' {
                    push_token(&mut tokens, Token::Le);
                    i += 2;
                } else {
                    push_token(&mut tokens, Token::Lt);
                    i += 1;
                }
            }
            '>' => {
                if i + 1 < chars.len() && chars[i + 1] == '=' {
                    push_token(&mut tokens, Token::Ge);
                    i += 2;
                } else {
                    push_token(&mut tokens, Token::Gt);
                    i += 1;
                }
            }
            '^' => {
                if i + 1 < chars.len() && chars[i + 1] == '^' {
                    push_token(&mut tokens, Token::XorCaret);
                    i += 2;
                } else {
                    push_token(&mut tokens, Token::Caret);
                    i += 1;
                }
            }
            '(' => {
                push_token(&mut tokens, Token::LParen);
                i += 1;
            }
            ')' => {
                push_token(&mut tokens, Token::RParen);
                i += 1;
            }
            '[' => {
                push_token(&mut tokens, Token::LBracket);
                i += 1;
            }
            ']' => {
                push_token(&mut tokens, Token::RBracket);
                i += 1;
            }
            ',' => {
                push_token(&mut tokens, Token::Comma);
                i += 1;
            }
            '!' => {
                if i + 1 < chars.len() && chars[i + 1] == '=' {
                    push_token(&mut tokens, Token::NotEq);
                    i += 2;
                } else {
                    push_token(&mut tokens, Token::Bang);
                    i += 1;
                }
            }
            _ => {
                if c.is_ascii_digit() || c == '.' {
                    let start = i;
                    let mut dot_count = 0usize;
                    while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                        if chars[i] == '.' {
                            dot_count += 1;
                            if dot_count > 1 {
                                return Err(format!("invalid number near position {}", i + 1));
                            }
                        }
                        i += 1;
                    }
                    let slice = chars[start..i].iter().collect::<String>();
                    let num = slice
                        .parse::<f64>()
                        .map_err(|_| format!("invalid number '{}'", slice))?;
                    push_token(&mut tokens, Token::Number(num));
                } else if c.is_ascii_alphabetic() || c == '_' {
                    let start = i;
                    while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                        i += 1;
                    }
                    let ident = chars[start..i].iter().collect::<String>().to_ascii_lowercase();
                    if ident == "i" {
                        push_token(&mut tokens, Token::Imaginary);
                    } else {
                        push_token(&mut tokens, Token::Identifier(ident));
                    }
                } else {
                    return Err(format!("unexpected character '{}' at position {}", c, i + 1));
                }
            }
        }
    }

    if tokens.is_empty() {
        return Err("expression must not be empty".to_string());
    }
    Ok(tokens)
}

fn parse_math_or_logic_tokens(tokens: &[Token]) -> Result<ParsedMathInput, String> {
    fn parse_expr(tokens: &[Token], pos: &mut usize) -> Result<AstNode, String> {
        let mut node = parse_term(tokens, pos)?;
        loop {
            let op = match tokens.get(*pos) {
                Some(Token::Plus) => '+',
                Some(Token::Minus) => '-',
                _ => break,
            };
            *pos += 1;
            let right = parse_term(tokens, pos)?;
            node = AstNode::Binary {
                op,
                left: Box::new(node),
                right: Box::new(right),
            };
        }
        Ok(node)
    }

    fn parse_term(tokens: &[Token], pos: &mut usize) -> Result<AstNode, String> {
        let mut node = parse_unary(tokens, pos)?;
        loop {
            let op = match tokens.get(*pos) {
                Some(Token::Star) => '*',
                Some(Token::Slash) => '/',
                _ => break,
            };
            *pos += 1;
            let right = parse_unary(tokens, pos)?;
            node = AstNode::Binary {
                op,
                left: Box::new(node),
                right: Box::new(right),
            };
        }
        Ok(node)
    }

    fn parse_power(tokens: &[Token], pos: &mut usize) -> Result<AstNode, String> {
        let mut node = parse_primary(tokens, pos)?;
        if let Some(Token::Caret) = tokens.get(*pos) {
            *pos += 1;
            let right = parse_power(tokens, pos)?;
            node = AstNode::Binary {
                op: '^',
                left: Box::new(node),
                right: Box::new(right),
            };
        }
        Ok(node)
    }

    fn parse_unary(tokens: &[Token], pos: &mut usize) -> Result<AstNode, String> {
        if let Some(Token::Minus) = tokens.get(*pos) {
            *pos += 1;
            let inner = parse_unary(tokens, pos)?;
            Ok(AstNode::UnaryNeg(Box::new(inner)))
        } else if let Some(Token::Plus) = tokens.get(*pos) {
            *pos += 1;
            parse_unary(tokens, pos)
        } else {
            parse_power(tokens, pos)
        }
    }

    fn parse_primary(tokens: &[Token], pos: &mut usize) -> Result<AstNode, String> {
        let mut node = match tokens.get(*pos) {
            Some(Token::Number(n)) => {
                *pos += 1;
                AstNode::Number(*n)
            }
            Some(Token::Imaginary) => {
                *pos += 1;
                AstNode::Function {
                    name: "imaginary".to_string(),
                    args: vec![AstNode::Number(1.0)],
                }
            }
            Some(Token::LBracket) => {
                // Matrix literal: [[expr, ...], [expr, ...], ...]
                *pos += 1;
                let mut rows: Vec<Vec<AstNode>> = Vec::new();
                loop {
                    match tokens.get(*pos) {
                        Some(Token::LBracket) => {
                            *pos += 1;
                            let mut row: Vec<AstNode> = Vec::new();
                            loop {
                                row.push(parse_expr(tokens, pos)?);
                                match tokens.get(*pos) {
                                    Some(Token::Comma) => { *pos += 1; }
                                    Some(Token::RBracket) => { *pos += 1; break; }
                                    _ => return Err("expected ',' or ']' in matrix row".to_string()),
                                }
                            }
                            rows.push(row);
                        }
                        Some(Token::RBracket) => { *pos += 1; break; }
                        Some(Token::Comma) => { *pos += 1; }
                        _ => return Err("expected '[' for matrix row or ']' to end matrix".to_string()),
                    }
                }
                AstNode::Matrix(rows)
            }
            Some(Token::LParen) => {
                *pos += 1;
                let expr = parse_expr(tokens, pos)?;
                match tokens.get(*pos) {
                    Some(Token::RParen) => {
                        *pos += 1;
                        expr
                    }
                    _ => return Err("missing closing ')'".to_string()),
                }
            }
            Some(Token::Identifier(name)) => {
                let ident = name.clone();
                *pos += 1;
                if let Some(Token::LParen) = tokens.get(*pos) {
                    *pos += 1;
                    let mut args = Vec::<AstNode>::new();
                    if !matches!(tokens.get(*pos), Some(Token::RParen)) {
                        loop {
                            args.push(parse_expr(tokens, pos)?);
                            match tokens.get(*pos) {
                                Some(Token::Comma) => {
                                    *pos += 1;
                                }
                                Some(Token::RParen) => break,
                                Some(Token::Number(_)
                                | Token::Imaginary
                                | Token::Identifier(_)
                                | Token::LParen
                                | Token::Plus
                                | Token::Minus) => {
                                    // Support space-separated arguments for selected multi-arg functions.
                                }
                                _ => {
                                    return Err(
                                        "missing closing ')' after function argument".to_string(),
                                    )
                                }
                            }
                        }
                    }
                    match tokens.get(*pos) {
                        Some(Token::RParen) => {
                            *pos += 1;
                            AstNode::Function { name: ident, args }
                        }
                        _ => return Err("missing closing ')' after function argument".to_string()),
                    }
                } else {
                    match ident.as_str() {
                        "pi" => AstNode::Number(std::f64::consts::PI),
                        "tau" => AstNode::Number(std::f64::consts::TAU),
                        "e" => AstNode::Number(std::f64::consts::E),
                        _ => AstNode::Variable(ident),
                    }
                }
            }
            _ => return Err("expected number, identifier, or '('".to_string()),
        };

        while let Some(Token::Bang) = tokens.get(*pos) {
            *pos += 1;
            node = AstNode::Function {
                name: "factorial".to_string(),
                args: vec![node],
            };
        }

        Ok(node)
    }

    fn parse_logic_atom(tokens: &[Token], pos: &mut usize) -> Result<LogicExprNode, String> {
        match tokens.get(*pos) {
            Some(Token::Bang) => {
                *pos += 1;
                return Ok(LogicExprNode::Not(Box::new(parse_logic_atom(tokens, pos)?)));
            }
            Some(Token::LParen) => {
                *pos += 1;
                let expr = parse_logic_equivalent(tokens, pos)?;
                match tokens.get(*pos) {
                    Some(Token::RParen) => {
                        *pos += 1;
                        return Ok(expr);
                    }
                    _ => return Err("missing closing ')' in logic expression".to_string()),
                }
            }
            Some(Token::Identifier(name)) => match name.as_str() {
                "not" => {
                    *pos += 1;
                    return Ok(LogicExprNode::Not(Box::new(parse_logic_atom(tokens, pos)?)));
                }
                "true" => {
                    *pos += 1;
                    return Ok(LogicExprNode::BoolLiteral(true));
                }
                "false" => {
                    *pos += 1;
                    return Ok(LogicExprNode::BoolLiteral(false));
                }
                _ => {}
            },
            _ => {}
        }

        let left = parse_expr(tokens, pos)?;
        let mut comparisons = Vec::<LogicExprNode>::new();
        let mut current_left = left.clone();

        loop {
            let compare_op = match tokens.get(*pos) {
                Some(Token::EqEq) => Some(ComparisonOp::Eq),
                Some(Token::NotEq) => Some(ComparisonOp::Ne),
                Some(Token::Lt) => Some(ComparisonOp::Lt),
                Some(Token::Le) => Some(ComparisonOp::Le),
                Some(Token::Gt) => Some(ComparisonOp::Gt),
                Some(Token::Ge) => Some(ComparisonOp::Ge),
                _ => None,
            };
            let Some(op) = compare_op else {
                break;
            };
            *pos += 1;
            let right = parse_expr(tokens, pos)?;
            comparisons.push(LogicExprNode::Comparison {
                op,
                left: current_left.clone(),
                right: right.clone(),
            });
            current_left = right;
        }

        if comparisons.len() == 1 {
            return Ok(comparisons.remove(0));
        }
        if comparisons.len() > 1 {
            return Ok(LogicExprNode::And(comparisons));
        }

        match left {
            AstNode::Function { name, args } => Ok(LogicExprNode::Predicate { name, args }),
            AstNode::Variable(name) if name == "true" => Ok(LogicExprNode::BoolLiteral(true)),
            AstNode::Variable(name) if name == "false" => Ok(LogicExprNode::BoolLiteral(false)),
            _ => Err("expected comparison or predicate in logic expression".to_string()),
        }
    }

    fn parse_logic_and(tokens: &[Token], pos: &mut usize) -> Result<LogicExprNode, String> {
        let mut nodes = vec![parse_logic_atom(tokens, pos)?];
        loop {
            let is_and = matches!(tokens.get(*pos), Some(Token::Identifier(name)) if name == "and")
                || matches!(tokens.get(*pos), Some(Token::AndAnd));
            if !is_and {
                break;
            }
            *pos += 1;
            nodes.push(parse_logic_atom(tokens, pos)?);
        }
        if nodes.len() == 1 {
            Ok(nodes.remove(0))
        } else {
            Ok(LogicExprNode::And(nodes))
        }
    }

    fn parse_logic_xor(tokens: &[Token], pos: &mut usize) -> Result<LogicExprNode, String> {
        let mut nodes = vec![parse_logic_and(tokens, pos)?];
        loop {
            let is_xor = matches!(tokens.get(*pos), Some(Token::Identifier(name)) if name == "xor")
                || matches!(tokens.get(*pos), Some(Token::XorCaret));
            if !is_xor {
                break;
            }
            *pos += 1;
            nodes.push(parse_logic_and(tokens, pos)?);
        }
        if nodes.len() == 1 {
            Ok(nodes.remove(0))
        } else {
            Ok(LogicExprNode::Xor(nodes))
        }
    }

    fn parse_logic_or(tokens: &[Token], pos: &mut usize) -> Result<LogicExprNode, String> {
        let mut nodes = vec![parse_logic_xor(tokens, pos)?];
        loop {
            let is_or = matches!(tokens.get(*pos), Some(Token::Identifier(name)) if name == "or")
                || matches!(tokens.get(*pos), Some(Token::OrOr));
            if !is_or {
                break;
            }
            *pos += 1;
            nodes.push(parse_logic_xor(tokens, pos)?);
        }
        if nodes.len() == 1 {
            Ok(nodes.remove(0))
        } else {
            Ok(LogicExprNode::Or(nodes))
        }
    }

    fn parse_logic_implies(tokens: &[Token], pos: &mut usize) -> Result<LogicExprNode, String> {
        let lhs = parse_logic_or(tokens, pos)?;
        let is_implies = matches!(tokens.get(*pos), Some(Token::Identifier(name)) if name == "implies")
            || matches!(tokens.get(*pos), Some(Token::ImpliesArrow));
        if !is_implies {
            return Ok(lhs);
        }
        *pos += 1;
        let rhs = parse_logic_implies(tokens, pos)?;
        Ok(LogicExprNode::Implies(Box::new(lhs), Box::new(rhs)))
    }

    fn parse_logic_equivalent(tokens: &[Token], pos: &mut usize) -> Result<LogicExprNode, String> {
        let mut node = parse_logic_implies(tokens, pos)?;
        loop {
            let is_equiv = matches!(tokens.get(*pos), Some(Token::Identifier(name)) if name == "equiv" || name == "iff")
                || matches!(tokens.get(*pos), Some(Token::EquivArrow));
            if !is_equiv {
                break;
            }
            *pos += 1;
            let rhs = parse_logic_implies(tokens, pos)?;
            node = LogicExprNode::Equivalent(Box::new(node), Box::new(rhs));
        }
        Ok(node)
    }

    let mut pos = 0usize;
    if tokens.iter().enumerate().any(|(idx, token)| {
        matches!(token, Token::EqEq | Token::NotEq | Token::Lt | Token::Le | Token::Gt | Token::Ge | Token::AndAnd | Token::OrOr | Token::XorCaret | Token::ImpliesArrow | Token::EquivArrow)
            || matches!(token, Token::Identifier(name) if name == "and" || name == "or" || name == "not" || name == "xor" || name == "implies" || name == "equiv" || name == "iff" || name == "true" || name == "false")
            || matches!(token, Token::Bang)
                && matches!(
                    idx.checked_sub(1).and_then(|prev| tokens.get(prev)),
                    None
                        | Some(Token::LParen)
                        | Some(Token::Comma)
                        | Some(Token::AndAnd)
                        | Some(Token::OrOr)
                        | Some(Token::XorCaret)
                        | Some(Token::ImpliesArrow)
                        | Some(Token::EquivArrow)
                        | Some(Token::EqEq)
                        | Some(Token::NotEq)
                        | Some(Token::Lt)
                        | Some(Token::Le)
                        | Some(Token::Gt)
                        | Some(Token::Ge)
                )
    }) {
        let logic = parse_logic_equivalent(tokens, &mut pos)?;
        if pos != tokens.len() {
            return Err("unexpected trailing tokens".to_string());
        }
        return Ok(ParsedMathInput::Logic(logic));
    }

    let left = parse_expr(tokens, &mut pos)?;
    if pos != tokens.len() {
        return Err("unexpected trailing tokens".to_string());
    }
    Ok(ParsedMathInput::Scalar(left))
}

#[allow(dead_code)]
fn parse_expression_tokens(tokens: &[Token]) -> Result<AstNode, String> {
    match parse_math_or_logic_tokens(tokens)? {
        ParsedMathInput::Scalar(ast) => Ok(ast),
        ParsedMathInput::Logic(_) => Err("logic expression passed to scalar parser".to_string()),
    }
}

fn ast_to_infix(ast: &AstNode) -> String {
    match ast {
        AstNode::Number(n) => format_number(*n),
        AstNode::Variable(name) => name.clone(),
        AstNode::Matrix(rows) => {
            let inner = rows.iter()
                .map(|row| format!("[{}]", row.iter().map(ast_to_infix).collect::<Vec<_>>().join(", ")))
                .collect::<Vec<_>>().join(", ");
            format!("[{}]", inner)
        }
        AstNode::UnaryNeg(inner) => format!("-({})", ast_to_infix(inner)),
        AstNode::Function { name, args } if name == "imaginary" => {
            match args.first() {
                Some(AstNode::Number(n)) if (*n - 1.0).abs() < 1e-12 => "i".to_string(),
                Some(arg) => format!("{}*i", ast_to_infix(arg)),
                None => "i".to_string(),
            }
        }
        AstNode::Function { name, args } if name == "factorial" => {
            format!("{}!", args.first().map(ast_to_infix).unwrap_or_default())
        }
        AstNode::Function { name, args } => {
            let rendered = args.iter().map(ast_to_infix).collect::<Vec<_>>().join(", ");
            format!("{}({})", name, rendered)
        }
        AstNode::Binary { op, left, right } => {
            format!("({} {} {})", ast_to_infix(left), op, ast_to_infix(right))
        }
    }
}

fn ast_to_latex(ast: &AstNode) -> String {
    match ast {
        AstNode::Number(n) => format_number(*n),
        AstNode::Variable(name) => name.clone(),
        AstNode::Matrix(rows) => {
            let body = rows.iter()
                .map(|row| row.iter().map(ast_to_latex).collect::<Vec<_>>().join(" & "))
                .collect::<Vec<_>>().join(" \\\\ ");
            format!("\\begin{{pmatrix}}{}\\end{{pmatrix}}", body)
        }
        AstNode::UnaryNeg(inner) => format!("-\\left({}\\right)", ast_to_latex(inner)),
        AstNode::Function { name, args } if name == "imaginary" => {
            match args.first() {
                Some(AstNode::Number(n)) if (*n - 1.0).abs() < 1e-12 => "i".to_string(),
                Some(arg) => format!("{}i", ast_to_latex(arg)),
                None => "i".to_string(),
            }
        }
        AstNode::Function { name, args } if name == "factorial" => {
            format!("{}!", args.first().map(ast_to_latex).unwrap_or_default())
        }
        AstNode::Function { name, args } => {
            let joined = args.iter().map(ast_to_latex).collect::<Vec<_>>().join(", ");
            match name.as_str() {
                "sqrt" if args.len() == 1 => format!("\\sqrt{{{}}}", joined),
                "ln" if args.len() == 1 => format!("\\ln\\left({}\\right)", joined),
                "log10" if args.len() == 1 => format!("\\log_{{10}}\\left({}\\right)", joined),
                "sin" | "cos" | "tan" | "asin" | "acos" | "atan" | "sinh" | "cosh"
                | "tanh" | "asinh" | "acosh" | "atanh" | "exp" | "abs" | "gamma"
                | "zeta" | "lambertw" | "w" | "polylog" | "gammainc" | "j_sph" | "det"
                | "permanent" | "erf" | "erfc" | "si" | "ci" | "fresnelc" | "fresnels"
                | "ei" | "li" | "sinc" | "ai" | "bi" | "beta" | "theta4" | "hafnian" => {
                    format!("\\{}\\left({}\\right)", name, joined)
                }
                _ => format!("\\operatorname{{{}}}\\left({}\\right)", name, joined),
            }
        }
        AstNode::Binary { op, left, right } => match op {
            '+' => format!("\\left({} + {}\\right)", ast_to_latex(left), ast_to_latex(right)),
            '-' => format!("\\left({} - {}\\right)", ast_to_latex(left), ast_to_latex(right)),
            '*' => format!("\\left({} \\cdot {}\\right)", ast_to_latex(left), ast_to_latex(right)),
            '/' => format!("\\frac{{{}}}{{{}}}", ast_to_latex(left), ast_to_latex(right)),
            '^' => format!("\\left({}^{{{}}}\\right)", ast_to_latex(left), ast_to_latex(right)),
            _ => format!("\\left({} ? {}\\right)", ast_to_latex(left), ast_to_latex(right)),
        },
    }
}

fn comparison_op_to_text(op: &ComparisonOp) -> &'static str {
    match op {
        ComparisonOp::Eq => "==",
        ComparisonOp::Ne => "!=",
        ComparisonOp::Lt => "<",
        ComparisonOp::Le => "<=",
        ComparisonOp::Gt => ">",
        ComparisonOp::Ge => ">=",
    }
}

fn comparison_op_to_latex(op: &ComparisonOp) -> &'static str {
    match op {
        ComparisonOp::Eq => "=",
        ComparisonOp::Ne => "\\neq",
        ComparisonOp::Lt => "<",
        ComparisonOp::Le => "\\le",
        ComparisonOp::Gt => ">",
        ComparisonOp::Ge => "\\ge",
    }
}

fn logic_expr_to_text(expr: &LogicExprNode) -> String {
    match expr {
        LogicExprNode::BoolLiteral(v) => v.to_string(),
        LogicExprNode::Predicate { name, args } => format!(
            "{}({})",
            name,
            args.iter().map(ast_to_infix).collect::<Vec<_>>().join(", ")
        ),
        LogicExprNode::Comparison { op, left, right } => format!(
            "{} {} {}",
            ast_to_infix(left),
            comparison_op_to_text(op),
            ast_to_infix(right)
        ),
        LogicExprNode::Not(inner) => format!("not ({})", logic_expr_to_text(inner)),
        LogicExprNode::And(parts) => parts.iter().map(logic_expr_to_text).collect::<Vec<_>>().join(" and "),
        LogicExprNode::Or(parts) => parts.iter().map(logic_expr_to_text).collect::<Vec<_>>().join(" or "),
        LogicExprNode::Xor(parts) => parts.iter().map(logic_expr_to_text).collect::<Vec<_>>().join(" xor "),
        LogicExprNode::Implies(lhs, rhs) => format!("({}) -> ({})", logic_expr_to_text(lhs), logic_expr_to_text(rhs)),
        LogicExprNode::Equivalent(lhs, rhs) => format!("({}) <-> ({})", logic_expr_to_text(lhs), logic_expr_to_text(rhs)),
    }
}

fn logic_expr_to_latex(expr: &LogicExprNode) -> String {
    match expr {
        LogicExprNode::BoolLiteral(true) => "\\mathrm{true}".to_string(),
        LogicExprNode::BoolLiteral(false) => "\\mathrm{false}".to_string(),
        LogicExprNode::Predicate { name, args } => format!(
            "\\operatorname{{{}}}\\left({}\\right)",
            name,
            args.iter().map(ast_to_latex).collect::<Vec<_>>().join(", ")
        ),
        LogicExprNode::Comparison { op, left, right } => format!(
            "{} {} {}",
            ast_to_latex(left),
            comparison_op_to_latex(op),
            ast_to_latex(right)
        ),
        LogicExprNode::Not(inner) => format!("\\neg\\left({}\\right)", logic_expr_to_latex(inner)),
        LogicExprNode::And(parts) => parts.iter().map(logic_expr_to_latex).collect::<Vec<_>>().join(" \\land "),
        LogicExprNode::Or(parts) => parts.iter().map(logic_expr_to_latex).collect::<Vec<_>>().join(" \\lor "),
        LogicExprNode::Xor(parts) => parts.iter().map(logic_expr_to_latex).collect::<Vec<_>>().join(" \\oplus "),
        LogicExprNode::Implies(lhs, rhs) => format!("\\left({}\\right) \\to \\left({}\\right)", logic_expr_to_latex(lhs), logic_expr_to_latex(rhs)),
        LogicExprNode::Equivalent(lhs, rhs) => format!("\\left({}\\right) \\leftrightarrow \\left({}\\right)", logic_expr_to_latex(lhs), logic_expr_to_latex(rhs)),
    }
}

fn compare_complex_values(op: &ComparisonOp, left: ComplexValue, right: ComplexValue) -> Result<bool, String> {
    let approx_eq = (left.re - right.re).abs() < 1e-12 && (left.im - right.im).abs() < 1e-12;
    match op {
        ComparisonOp::Eq => Ok(approx_eq),
        ComparisonOp::Ne => Ok(!approx_eq),
        ComparisonOp::Lt | ComparisonOp::Le | ComparisonOp::Gt | ComparisonOp::Ge => {
            if !left.is_real() || !right.is_real() {
                return Err("ordering comparisons require real-valued operands".to_string());
            }
            match op {
                ComparisonOp::Lt => Ok(left.re < right.re),
                ComparisonOp::Le => Ok(left.re <= right.re),
                ComparisonOp::Gt => Ok(left.re > right.re),
                ComparisonOp::Ge => Ok(left.re >= right.re),
                _ => unreachable!(),
            }
        }
    }
}

fn evaluate_logic_expression(
    expr: &LogicExprNode,
    state: &AppState,
    options: MathOptions,
    steps: &mut Vec<MathStep>,
) -> Result<LogicResult, String> {
    fn truth_value_from_bool(value: bool) -> TruthValue {
        if value { TruthValue::True } else { TruthValue::False }
    }

    fn modality_from_bool(value: bool) -> Modality {
        if value { Modality::Must } else { Modality::Impossible }
    }

    fn truth_to_known_bool(value: &TruthValue) -> Option<bool> {
        match value {
            TruthValue::True => Some(true),
            TruthValue::False => Some(false),
            TruthValue::Unknown | TruthValue::NeedsInput => None,
        }
    }

    fn negate_truth(value: &TruthValue) -> TruthValue {
        match value {
            TruthValue::True => TruthValue::False,
            TruthValue::False => TruthValue::True,
            TruthValue::Unknown => TruthValue::Unknown,
            TruthValue::NeedsInput => TruthValue::NeedsInput,
        }
    }

    match expr {
        LogicExprNode::BoolLiteral(v) => Ok(LogicResult {
            status: LogicStatus::Success,
            truth: if *v { TruthValue::True } else { TruthValue::False },
            modality: if *v { Modality::Must } else { Modality::Impossible },
            satisfiable: Some(*v),
            models_found: None,
            blocking_conditions: Vec::new(),
            error: None,
        }),
        LogicExprNode::Comparison { op, left, right } => {
            let left_value = evaluate_ast_complex(left, state, options, steps)?;
            let right_value = evaluate_ast_complex(right, state, options, steps)?;
            let comparison = compare_complex_values(op, left_value, right_value)?;
            Ok(LogicResult {
                status: LogicStatus::Success,
                truth: if comparison { TruthValue::True } else { TruthValue::False },
                modality: if comparison { Modality::Must } else { Modality::Impossible },
                satisfiable: Some(comparison),
                models_found: None,
                blocking_conditions: Vec::new(),
                error: None,
            })
        }
        LogicExprNode::Predicate { name, args } => {
            let mut values = Vec::with_capacity(args.len());
            for arg in args {
                values.push(evaluate_ast_complex(arg, state, options, steps)?);
            }
            let truth = match name.as_str() {
                "isreal" if values.len() == 1 => values[0].is_real(),
                "iszero" if values.len() == 1 => values[0].is_zero(),
                "isfinite" if values.len() == 1 => values[0].is_finite(),
                "ispositive" if values.len() == 1 => values[0].is_real() && values[0].re > 0.0,
                "isnegative" if values.len() == 1 => values[0].is_real() && values[0].re < 0.0,
                "isnonnegative" if values.len() == 1 => values[0].is_real() && values[0].re >= 0.0,
                "isnonzero" if values.len() == 1 => !values[0].is_zero(),
                "isinteger" if values.len() == 1 => values[0].is_real() && (values[0].re - values[0].re.round()).abs() < 1e-12,
                "isimaginary" if values.len() == 1 => values[0].re.abs() < 1e-12 && values[0].im.abs() >= 1e-12,
                _ => return Err(format!("unsupported predicate '{}'", name)),
            };
            Ok(LogicResult {
                status: LogicStatus::Success,
                truth: truth_value_from_bool(truth),
                modality: modality_from_bool(truth),
                satisfiable: Some(truth),
                models_found: None,
                blocking_conditions: Vec::new(),
                error: None,
            })
        }
        LogicExprNode::Not(inner) => {
            let inner_result = evaluate_logic_expression(inner, state, options, steps)?;
            let truth = negate_truth(&inner_result.truth);
            Ok(LogicResult {
                status: inner_result.status,
                truth,
                modality: inner_result.modality,
                satisfiable: inner_result.satisfiable.map(|v| !v),
                models_found: inner_result.models_found,
                blocking_conditions: inner_result.blocking_conditions,
                error: inner_result.error,
            })
        }
        LogicExprNode::And(parts) => {
            let mut saw_unknown = false;
            for part in parts {
                let part_result = evaluate_logic_expression(part, state, options, steps)?;
                match part_result.truth {
                    TruthValue::False => {
                        return Ok(LogicResult {
                            status: LogicStatus::Success,
                            truth: TruthValue::False,
                            modality: Modality::Impossible,
                            satisfiable: Some(false),
                            models_found: None,
                            blocking_conditions: Vec::new(),
                            error: None,
                        })
                    }
                    TruthValue::Unknown | TruthValue::NeedsInput => saw_unknown = true,
                    TruthValue::True => {}
                }
            }
            Ok(LogicResult {
                status: LogicStatus::Success,
                truth: if saw_unknown { TruthValue::Unknown } else { TruthValue::True },
                modality: if saw_unknown { Modality::Unknown } else { Modality::Must },
                satisfiable: if saw_unknown { None } else { Some(true) },
                models_found: None,
                blocking_conditions: Vec::new(),
                error: None,
            })
        }
        LogicExprNode::Or(parts) => {
            let mut saw_unknown = false;
            for part in parts {
                let part_result = evaluate_logic_expression(part, state, options, steps)?;
                match part_result.truth {
                    TruthValue::True => {
                        return Ok(LogicResult {
                            status: LogicStatus::Success,
                            truth: TruthValue::True,
                            modality: Modality::Must,
                            satisfiable: Some(true),
                            models_found: None,
                            blocking_conditions: Vec::new(),
                            error: None,
                        })
                    }
                    TruthValue::Unknown | TruthValue::NeedsInput => saw_unknown = true,
                    TruthValue::False => {}
                }
            }
            Ok(LogicResult {
                status: LogicStatus::Success,
                truth: if saw_unknown { TruthValue::Unknown } else { TruthValue::False },
                modality: if saw_unknown { Modality::Unknown } else { Modality::Impossible },
                satisfiable: if saw_unknown { None } else { Some(false) },
                models_found: None,
                blocking_conditions: Vec::new(),
                error: None,
            })
        }
        LogicExprNode::Xor(parts) => {
            let mut true_count = 0usize;
            for part in parts {
                let part_result = evaluate_logic_expression(part, state, options, steps)?;
                match part_result.truth {
                    TruthValue::True => true_count += 1,
                    TruthValue::False => {}
                    _ => {
                        return Ok(LogicResult {
                            status: LogicStatus::Unknown,
                            truth: TruthValue::Unknown,
                            modality: Modality::Unknown,
                            satisfiable: None,
                            models_found: None,
                            blocking_conditions: Vec::new(),
                            error: None,
                        })
                    }
                }
            }
            let truth = true_count % 2 == 1;
            Ok(LogicResult {
                status: LogicStatus::Success,
                truth: if truth { TruthValue::True } else { TruthValue::False },
                modality: if truth { Modality::Must } else { Modality::Impossible },
                satisfiable: Some(truth),
                models_found: None,
                blocking_conditions: Vec::new(),
                error: None,
            })
        }
        LogicExprNode::Implies(lhs, rhs) => {
            let lhs_result = evaluate_logic_expression(lhs, state, options, steps)?;
            let rhs_result = evaluate_logic_expression(rhs, state, options, steps)?;
            let truth = match (truth_to_known_bool(&lhs_result.truth), truth_to_known_bool(&rhs_result.truth)) {
                (Some(false), _) => TruthValue::True,
                (Some(true), Some(value)) => truth_value_from_bool(value),
                (None, Some(true)) => TruthValue::True,
                (Some(true), None) | (None, Some(false)) | (None, None) => TruthValue::Unknown,
            };
            let satisfiable = truth_to_known_bool(&truth);
            Ok(LogicResult {
                status: if matches!(truth, TruthValue::Unknown) { LogicStatus::Unknown } else { LogicStatus::Success },
                truth: truth.clone(),
                modality: match truth {
                    TruthValue::True => Modality::Must,
                    TruthValue::False => Modality::Impossible,
                    TruthValue::Unknown | TruthValue::NeedsInput => Modality::Unknown,
                },
                satisfiable,
                models_found: None,
                blocking_conditions: Vec::new(),
                error: None,
            })
        }
        LogicExprNode::Equivalent(lhs, rhs) => {
            let lhs_result = evaluate_logic_expression(lhs, state, options, steps)?;
            let rhs_result = evaluate_logic_expression(rhs, state, options, steps)?;
            let truth = match (truth_to_known_bool(&lhs_result.truth), truth_to_known_bool(&rhs_result.truth)) {
                (Some(lhs_value), Some(rhs_value)) => truth_value_from_bool(lhs_value == rhs_value),
                _ => TruthValue::Unknown,
            };
            let satisfiable = truth_to_known_bool(&truth);
            Ok(LogicResult {
                status: if matches!(truth, TruthValue::Unknown) { LogicStatus::Unknown } else { LogicStatus::Success },
                truth: truth.clone(),
                modality: match truth {
                    TruthValue::True => Modality::Must,
                    TruthValue::False => Modality::Impossible,
                    TruthValue::Unknown | TruthValue::NeedsInput => Modality::Unknown,
                },
                satisfiable,
                models_found: None,
                blocking_conditions: Vec::new(),
                error: None,
            })
        }
    }
}

fn truth_phase_theta(truth: &TruthValue) -> f64 {
    match truth {
        TruthValue::True => 0.0,
        TruthValue::False => std::f64::consts::PI,
        TruthValue::Unknown => std::f64::consts::PI / 2.0,
        TruthValue::NeedsInput => -std::f64::consts::PI / 2.0,
    }
}

fn truth_resonance(truth: &TruthValue) -> f64 {
    match truth {
        TruthValue::True | TruthValue::False => 1.0,
        TruthValue::Unknown => 0.5,
        TruthValue::NeedsInput => 0.35,
    }
}

fn logic_prop_id(expr: &LogicExprNode) -> String {
    let label = logic_expr_to_text(expr);
    format!("prop.{}", stable_bridge_id("logic_prop", &label))
}

fn build_logic_crystal_payload(
    expr: &LogicExprNode,
    result: &LogicResult,
    final_theta: f64,
    torsion_norm: f64,
) -> Value {
    let theta = wrap_to_pi((truth_phase_theta(&result.truth) + final_theta) / 2.0);
    let resonance_score = (truth_resonance(&result.truth) * (1.0 - torsion_norm).clamp(0.0, 1.0))
        .clamp(0.0, 1.0);
    let label = logic_expr_to_text(expr);
    let prop_id = logic_prop_id(expr);
    json!({
        "logic_prop_id": prop_id,
        "node_type": "logic_prop",
        "label": label,
        "formula": logic_expr_to_latex(expr),
        "phase_signature": {
            "phase_theta": theta,
            "resonance": resonance_score,
            "torsion_norm": torsion_norm,
            "context_frame_id": "frame.default"
        },
        "trajectory": [
            {
                "monotonic_index": 1,
                "op": "logic_eval_commit",
                "inputs": [logic_expr_to_text(expr)],
                "output": prop_id,
                "phase_theta": theta
            }
        ],
        "provenance": {
            "source": "csif_logic_profile_v1"
        }
    })
}

fn append_logic_connectives(expr: &LogicExprNode, out: &mut Vec<Value>) {
    let phase_update = |op: &str| match op {
        "not" => std::f64::consts::PI,
        "and" => std::f64::consts::PI / 8.0,
        "or" => std::f64::consts::PI / 12.0,
        "implies" => std::f64::consts::PI / 16.0,
        "equiv" => 0.0,
        "xor" => std::f64::consts::PI / 10.0,
        _ => 0.0,
    };

    match expr {
        LogicExprNode::Not(inner) => {
            out.push(json!({
                "operator": "not",
                "inputs": [logic_expr_to_text(inner)],
                "output": logic_expr_to_text(expr),
                "phase_update": phase_update("not"),
                "residual": 0.0
            }));
            append_logic_connectives(inner, out);
        }
        LogicExprNode::And(parts) => {
            out.push(json!({
                "operator": "and",
                "inputs": parts.iter().map(logic_expr_to_text).collect::<Vec<_>>(),
                "output": logic_expr_to_text(expr),
                "phase_update": phase_update("and"),
                "residual": 0.0
            }));
            for p in parts {
                append_logic_connectives(p, out);
            }
        }
        LogicExprNode::Or(parts) => {
            out.push(json!({
                "operator": "or",
                "inputs": parts.iter().map(logic_expr_to_text).collect::<Vec<_>>(),
                "output": logic_expr_to_text(expr),
                "phase_update": phase_update("or"),
                "residual": 0.0
            }));
            for p in parts {
                append_logic_connectives(p, out);
            }
        }
        LogicExprNode::Xor(parts) => {
            out.push(json!({
                "operator": "xor",
                "inputs": parts.iter().map(logic_expr_to_text).collect::<Vec<_>>(),
                "output": logic_expr_to_text(expr),
                "phase_update": phase_update("xor"),
                "residual": 0.0
            }));
            for p in parts {
                append_logic_connectives(p, out);
            }
        }
        LogicExprNode::Implies(lhs, rhs) => {
            out.push(json!({
                "operator": "implies",
                "inputs": [logic_expr_to_text(lhs), logic_expr_to_text(rhs)],
                "output": logic_expr_to_text(expr),
                "phase_update": phase_update("implies"),
                "residual": 0.0
            }));
            append_logic_connectives(lhs, out);
            append_logic_connectives(rhs, out);
        }
        LogicExprNode::Equivalent(lhs, rhs) => {
            out.push(json!({
                "operator": "equiv",
                "inputs": [logic_expr_to_text(lhs), logic_expr_to_text(rhs)],
                "output": logic_expr_to_text(expr),
                "phase_update": phase_update("equiv"),
                "residual": 0.0
            }));
            append_logic_connectives(lhs, out);
            append_logic_connectives(rhs, out);
        }
        _ => {}
    }
}

fn build_logic_connective_payload(expr: &LogicExprNode) -> Vec<Value> {
    let mut out = Vec::new();
    append_logic_connectives(expr, &mut out);
    out
}

fn build_modus_ponens_morphisms(
    expr: &LogicExprNode,
    final_theta: f64,
    torsion_norm: f64,
) -> Vec<Value> {
    let LogicExprNode::Implies(lhs, rhs) = expr else {
        return Vec::new();
    };

    let p_id = logic_prop_id(lhs);
    let implies_id = logic_prop_id(expr);
    let q_id = logic_prop_id(rhs);

    let p_phase = wrap_to_pi(final_theta - std::f64::consts::PI / 16.0);
    let implies_phase = wrap_to_pi(final_theta);
    let q_phase = wrap_to_pi(final_theta + std::f64::consts::PI / 16.0);

    let hop1_drift = phase_drift(p_phase, implies_phase);
    let hop2_drift = phase_drift(implies_phase, q_phase);
    let (loop_torsion_threshold, threshold_source) = logic_inference_torsion_threshold_policy();
    let loop_torsion =
        (hop1_drift + hop2_drift).abs() + torsion_norm * std::f64::consts::PI * 0.25;
    let loop_torsion_norm = loop_torsion / std::f64::consts::PI;
    let loop_resonance = (1.0 - loop_torsion_norm).clamp(0.0, 1.0);
    let exceeds_threshold = loop_torsion_norm > loop_torsion_threshold;

    vec![json!({
        "chain_id": stable_bridge_id("logic_mp_chain", &logic_expr_to_text(expr)),
        "source_unit": p_id,
        "target_unit": q_id,
        "morphism_type": "inference_modus_ponens",
        "hops": [
            {
                "hop_index": 1,
                "source_unit": p_id,
                "target_unit": implies_id,
                "transform": "implication_constraint_bind",
                "phase_from": p_phase,
                "phase_to": implies_phase,
                "phase_drift": hop1_drift,
                "torsion_norm": torsion_norm,
                "confidence_band": resonance(implies_phase)
            },
            {
                "hop_index": 2,
                "source_unit": implies_id,
                "target_unit": q_id,
                "transform": "modus_ponens_fire",
                "phase_from": implies_phase,
                "phase_to": q_phase,
                "phase_drift": hop2_drift,
                "torsion_norm": torsion_norm,
                "confidence_band": resonance(q_phase)
            }
        ],
        "loop_metrics": {
            "is_closed": false,
            "closure_target": Value::Null,
            "hop_count": 2,
            "loop_torsion": loop_torsion,
            "loop_torsion_norm": loop_torsion_norm,
            "loop_resonance": loop_resonance,
            "loop_torsion_norm_threshold": loop_torsion_threshold,
            "threshold_source": threshold_source,
            "exceeds_threshold": exceeds_threshold
        }
    })]
}

fn build_logic_contradiction_signal(inference_morphisms: &[Value]) -> Value {
    let (threshold, threshold_source) = logic_inference_torsion_threshold_policy();
    let mut contradictions = Vec::<Value>::new();
    let mut max_loop_torsion_norm = 0.0_f64;

    for chain in inference_morphisms {
        let chain_id = chain
            .get("chain_id")
            .and_then(Value::as_str)
            .unwrap_or("unknown_chain");
        let loop_metrics = chain.get("loop_metrics").and_then(Value::as_object);
        let loop_torsion_norm = loop_metrics
            .and_then(|lm| lm.get("loop_torsion_norm"))
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let exceeds_threshold = loop_metrics
            .and_then(|lm| lm.get("exceeds_threshold"))
            .and_then(Value::as_bool)
            .unwrap_or(false);

        max_loop_torsion_norm = max_loop_torsion_norm.max(loop_torsion_norm);

        if exceeds_threshold {
            contradictions.push(json!({
                "code": "logic_inference_torsion_exceeded",
                "chain_id": chain_id,
                "loop_torsion_norm": loop_torsion_norm,
                "threshold": threshold,
                "severity": "Error"
            }));
        }
    }

    let triggered = !contradictions.is_empty();
    json!({
        "triggered": triggered,
        "stop_reason": if triggered {
            Some("logic_inference_torsion_exceeded")
        } else {
            None
        },
        "max_loop_torsion_norm": max_loop_torsion_norm,
        "threshold": threshold,
        "threshold_source": threshold_source,
        "contradictions": contradictions
    })
}

fn parse_math_mode(mode: Option<&str>) -> Result<MathMode, String> {
    match mode.map(|m| m.trim().to_ascii_lowercase()) {
        None => Ok(MathMode::Algebraic),
        Some(m) if m == "algebraic" => Ok(MathMode::Algebraic),
        Some(m) if m == "geometric" => Ok(MathMode::Geometric),
        Some(other) => Err(format!("invalid mode '{}'; expected algebraic or geometric", other)),
    }
}

fn parse_angle_unit(unit: Option<&str>) -> Result<AngleUnit, String> {
    match unit.map(|u| u.trim().to_ascii_lowercase()) {
        None => Ok(AngleUnit::Radians),
        Some(u) if u == "rad" || u == "radian" || u == "radians" => Ok(AngleUnit::Radians),
        Some(u) if u == "deg" || u == "degree" || u == "degrees" => Ok(AngleUnit::Degrees),
        Some(other) => Err(format!("invalid angle_unit '{}'; expected radians or degrees", other)),
    }
}

fn input_to_radians(v: f64, angle_unit: AngleUnit) -> f64 {
    match angle_unit {
        AngleUnit::Radians => v,
        AngleUnit::Degrees => v.to_radians(),
    }
}

fn trig_geometry_payload(fn_name: &str, theta_input: f64, theta_radians: f64) -> Option<Value> {
    if !matches!(fn_name, "sin" | "cos" | "tan") {
        return None;
    }

    let x = theta_radians.cos();
    let y = theta_radians.sin();
    let projection = match fn_name {
        "sin" => "y_axis",
        "cos" => "x_axis",
        "tan" => "y_over_x",
        _ => "none",
    };

    Some(json!({
        "model": "unit_circle",
        "theta_input": theta_input,
        "theta_radians": theta_radians,
        "point": {"x": x, "y": y},
        "projection": projection
    }))
}

fn parse_function_result(
    name: &str,
    args: &[ComplexValue],
    options: MathOptions,
) -> Result<(ComplexValue, Option<Value>, String), String> {
    let lowered = name.to_ascii_lowercase();

    if matches!(
        lowered.as_str(),
        "comb" | "c" | "atan2" | "polylog" | "gammainc" | "j_sph" | "beta" | "b" | "theta4"
    ) {
        if args.len() != 2 {
            return Err(format!("function '{}' expects 2 arguments", name));
        }
    } else if args.len() != 1 {
        return Err(format!("function '{}' expects 1 argument", name));
    }

    let arg = args
        .first()
        .copied()
        .unwrap_or_else(|| ComplexValue::new(0.0, 0.0));

    match lowered.as_str() {
        "comb" | "c" => Ok((c_comb(args[0], args[1])?, None, "combination".to_string())),
        "polylog" => Ok((c_polylog(args[0], args[1])?, None, "polylogarithm".to_string())),
        "gammainc" => Ok((c_gammainc(args[0], args[1])?, None, "incomplete_gamma".to_string())),
        "j_sph" => {
            if !args[0].is_real() || args[0].re < 0.0 || (args[0].re.round() - args[0].re).abs() > 1e-12 {
                return Err("j_sph requires a non-negative integer order".to_string());
            }
            Ok((c_spherical_bessel_j(args[0].re.round() as usize, args[1])?, None, "spherical_bessel_j".to_string()))
        }
        "beta" | "b" => Ok((c_beta(args[0], args[1])?, None, "beta_function".to_string())),
        "theta4" => Ok((c_theta4(args[0], args[1])?, None, "jacobi_theta4".to_string())),
        "atan2" => {
            if !args[0].is_real() || !args[1].is_real() {
                return Err("atan2 currently supports real inputs only".to_string());
            }
            Ok((
                ComplexValue::new(args[0].re.atan2(args[1].re), 0.0),
                None,
                "arctangent2".to_string(),
            ))
        }
        "j0" => Ok((c_bessel_j(0, arg)?, None, "bessel_j0".to_string())),
        "j1" => Ok((c_bessel_j(1, arg)?, None, "bessel_j1".to_string())),
        "j2" => Ok((c_bessel_j(2, arg)?, None, "bessel_j2".to_string())),
        "j3" => Ok((c_bessel_j(3, arg)?, None, "bessel_j3".to_string())),
        "det" => Ok((arg, None, "determinant_scalar".to_string())),
        "permanent" => Ok((arg, None, "permanent_scalar".to_string())),
        "hafnian" => Ok((arg, None, "hafnian_scalar".to_string())),
        "gamma" => Ok((c_gamma(arg)?, None, "gamma_function".to_string())),
        "lambertw" | "w" => Ok((c_lambertw(arg)?, None, "lambert_w".to_string())),
        "zeta" => Ok((c_zeta(arg)?, None, "riemann_zeta".to_string())),
        "erf" => Ok((c_erf(arg), None, "error_function".to_string())),
        "erfc" => Ok((c_erfc(arg), None, "complementary_error_function".to_string())),
        "si" => Ok((c_si(arg)?, None, "sine_integral".to_string())),
        "ci" => Ok((c_ci(arg)?, None, "cosine_integral".to_string())),
        "fresnelc" => Ok((c_fresnel_c(arg)?, None, "fresnel_cosine".to_string())),
        "fresnels" => Ok((c_fresnel_s(arg)?, None, "fresnel_sine".to_string())),
        "ei" => Ok((c_ei(arg)?, None, "exponential_integral".to_string())),
        "li" => Ok((c_li(arg)?, None, "logarithmic_integral".to_string())),
        "sinc" => Ok((c_sinc(arg), None, "sinc_function".to_string())),
        "ai" => Ok((c_ai(arg)?, None, "airy_ai".to_string())),
        "bi" => Ok((c_bi(arg)?, None, "airy_bi".to_string())),
        "sin" => {
            if arg.is_real() && arg.re.abs() < 1e-12 {
                return Ok((ComplexValue::new(0.0, 0.0), trig_geometry_payload("sin", arg.re, 0.0), "sine_unit_circle".to_string()));
            }
            let theta = input_to_radians(arg.re, options.angle_unit);
            let v = ComplexValue::new(theta.sin(), 0.0);
            Ok((v, trig_geometry_payload("sin", arg.re, theta), "sine_unit_circle".to_string()))
        }
        "cos" => {
            if arg.is_real() && arg.re.abs() < 1e-12 {
                return Ok((ComplexValue::new(1.0, 0.0), trig_geometry_payload("cos", arg.re, 0.0), "cosine_unit_circle".to_string()));
            }
            let theta = input_to_radians(arg.re, options.angle_unit);
            let v = ComplexValue::new(theta.cos(), 0.0);
            Ok((v, trig_geometry_payload("cos", arg.re, theta), "cosine_unit_circle".to_string()))
        }
        "tan" => {
            if arg.is_real() && arg.re.abs() < 1e-12 {
                return Ok((ComplexValue::new(0.0, 0.0), trig_geometry_payload("tan", arg.re, 0.0), "tangent_unit_circle".to_string()));
            }
            let theta = input_to_radians(arg.re, options.angle_unit);
            let c = theta.cos();
            if c.abs() < 1e-15 {
                return Err("tan undefined at odd pi/2".to_string());
            }
            let v = ComplexValue::new(theta.tan(), 0.0);
            Ok((v, trig_geometry_payload("tan", arg.re, theta), "tangent_unit_circle".to_string()))
        }
        "asin" => {
            if !arg.is_real() || !(-1.0..=1.0).contains(&arg.re) {
                return Err("asin domain error: input must be real in [-1, 1]".to_string());
            }
            Ok((ComplexValue::new(arg.re.asin(), 0.0), None, "arcsine".to_string()))
        }
        "acos" => {
            if !arg.is_real() || !(-1.0..=1.0).contains(&arg.re) {
                return Err("acos domain error: input must be real in [-1, 1]".to_string());
            }
            Ok((ComplexValue::new(arg.re.acos(), 0.0), None, "arccosine".to_string()))
        }
        "atan" => Ok((ComplexValue::new(arg.re.atan(), 0.0), None, "arctangent".to_string())),
        "sinh" => {
            if !arg.is_real() {
                return Err("sinh currently supports real inputs only".to_string());
            }
            let v = if arg.re.abs() < 1e-12 { 0.0 } else { arg.re.sinh() };
            Ok((ComplexValue::new(v, 0.0), None, "hyperbolic_sine".to_string()))
        }
        "cosh" => {
            if !arg.is_real() {
                return Err("cosh currently supports real inputs only".to_string());
            }
            let v = if arg.re.abs() < 1e-12 { 1.0 } else { arg.re.cosh() };
            Ok((ComplexValue::new(v, 0.0), None, "hyperbolic_cosine".to_string()))
        }
        "tanh" => {
            if !arg.is_real() {
                return Err("tanh currently supports real inputs only".to_string());
            }
            let v = if arg.re.abs() < 1e-12 { 0.0 } else { arg.re.tanh() };
            Ok((ComplexValue::new(v, 0.0), None, "hyperbolic_tangent".to_string()))
        }
        "asinh" => {
            if !arg.is_real() {
                return Err("asinh currently supports real inputs only".to_string());
            }
            let v = if arg.re.abs() < 1e-12 {
                0.0
            } else {
                (arg.re + (arg.re * arg.re + 1.0).sqrt()).ln()
            };
            Ok((ComplexValue::new(v, 0.0), None, "inverse_hyperbolic_sine".to_string()))
        }
        "acosh" => {
            if !arg.is_real() || arg.re < 1.0 {
                return Err("acosh domain error: input must be real and >= 1".to_string());
            }
            let v = if (arg.re - 1.0).abs() < 1e-12 {
                0.0
            } else {
                (arg.re + ((arg.re - 1.0) * (arg.re + 1.0)).sqrt()).ln()
            };
            Ok((ComplexValue::new(v, 0.0), None, "inverse_hyperbolic_cosine".to_string()))
        }
        "atanh" => {
            if !arg.is_real() || arg.re.abs() >= 1.0 {
                return Err("atanh domain error: input must be real in (-1, 1)".to_string());
            }
            let v = if arg.re.abs() < 1e-12 {
                0.0
            } else {
                0.5 * ((1.0 + arg.re) / (1.0 - arg.re)).ln()
            };
            Ok((ComplexValue::new(v, 0.0), None, "inverse_hyperbolic_tangent".to_string()))
        }
        "sqrt" => Ok((c_sqrt(arg), None, "square_root".to_string())),
        "ln" => Ok((c_log(arg)?, None, "natural_logarithm".to_string())),
        "log" | "log10" => {
            let v = c_log(arg)? / ComplexValue::new(10.0_f64.ln(), 0.0);
            Ok((v, None, "common_logarithm".to_string()))
        }
        "exp" => Ok((c_exp(arg), None, "exponential".to_string())),
        "abs" => Ok((c_abs(arg), None, "absolute_value".to_string())),
        "arg" => Ok((c_arg(arg), None, "argument".to_string())),
        "conj" => Ok((c_conj(arg), None, "conjugate".to_string())),
        "factorial" => Ok((c_fact(arg)?, None, "factorial".to_string())),
        _ => Err(format!("unsupported function '{}'", name)),
    }
}

/// Lightweight evaluator for integrand sub-expressions with one bound variable.
/// Does not produce derivation trace steps.
fn eval_node_with_var(ast: &AstNode, var: &str, val: ComplexValue) -> Result<ComplexValue, String> {
    match ast {
        AstNode::Number(n) => Ok(ComplexValue::new(*n, 0.0)),
        AstNode::Variable(name) => {
            if name == var {
                Ok(val)
            } else {
                Err(format!("unbound variable '{}' in integrand", name))
            }
        }
        AstNode::UnaryNeg(inner) => Ok(-eval_node_with_var(inner, var, val)?),
        AstNode::Function { name, args } if name == "imaginary" => {
            let v = match args.first() {
                Some(a) => eval_node_with_var(a, var, val)?,
                None => ComplexValue::new(1.0, 0.0),
            };
            Ok(ComplexValue::new(0.0, v.re))
        }
        AstNode::Function { name, args } => {
            let evaluated: Result<Vec<ComplexValue>, String> = args
                .iter()
                .map(|a| eval_node_with_var(a, var, val))
                .collect();
            let (result, _, _) = parse_function_result(name, &evaluated?, MathOptions::default())?;
            Ok(result)
        }
        AstNode::Binary { op, left, right } => {
            let l = eval_node_with_var(left, var, val)?;
            let r = eval_node_with_var(right, var, val)?;
            match op {
                '+' => Ok(c_add(l, r)),
                '-' => Ok(c_sub(l, r)),
                '*' => Ok(c_mul(l, r)),
                '/' => c_div(l, r),
                '^' => c_pow(l, r),
                _ => Err(format!("unsupported operator '{}'", op)),
            }
        }
        AstNode::Matrix(_) => Err("matrix literal not allowed inside integrand expression".to_string()),
    }
}

/// Adaptive Simpson's rule numerical integration of a real-valued integrand.
/// Uses composite Simpson's rule with n_steps panels (must be even).
fn numerical_integrate(
    integrand: &AstNode,
    var: &str,
    a: f64,
    b: f64,
    n_steps: usize,
) -> Result<ComplexValue, String> {
    let n = if n_steps % 2 == 0 { n_steps } else { n_steps + 1 };
    let h = (b - a) / n as f64;
    let mut sum = ComplexValue::new(0.0, 0.0);
    for k in 0..=n {
        let x = a + k as f64 * h;
        let fx = eval_node_with_var(integrand, var, ComplexValue::new(x, 0.0))?;
        let weight = if k == 0 || k == n {
            1.0
        } else if k % 2 == 1 {
            4.0
        } else {
            2.0
        };
        sum = sum + ComplexValue::new(weight, 0.0) * fx;
    }
    Ok(sum * ComplexValue::new(h / 3.0, 0.0))
}

/// Compute the hafnian of a 2n×2n symmetric complex matrix via
/// recursive perfect-matching enumeration.
/// haf([[a]]) = a for 1×1 (edge case), haf(2×2) = A[0][1], etc.
fn c_hafnian_matrix(m: &[Vec<ComplexValue>], indices: &[usize]) -> Result<ComplexValue, String> {
    if indices.is_empty() {
        return Ok(ComplexValue::new(1.0, 0.0));
    }
    if indices.len() == 2 {
        return Ok(m[indices[0]][indices[1]]);
    }
    let first = indices[0];
    let mut sum = ComplexValue::new(0.0, 0.0);
    for k in 1..indices.len() {
        let pair_val = m[first][indices[k]];
        let remaining: Vec<usize> = indices[1..]
            .iter()
            .cloned()
            .filter(|&i| i != indices[k])
            .collect();
        sum = sum + c_mul(pair_val, c_hafnian_matrix(m, &remaining)?);
    }
    Ok(sum)
}

fn c_hafnian(m: Vec<Vec<ComplexValue>>) -> Result<ComplexValue, String> {
    let n = m.len();
    if n == 0 {
        return Ok(ComplexValue::new(1.0, 0.0));
    }
    // Allow odd-size: treat as if last row/col is a 1×1 identity pair
    if n % 2 != 0 {
        return Err(format!(
            "hafnian requires an even-dimensional square matrix (got {}×{})",
            n, n
        ));
    }
    for (i, row) in m.iter().enumerate() {
        if row.len() != n {
            return Err(format!(
                "hafnian matrix must be square (row {} has {} columns, expected {})",
                i, row.len(), n
            ));
        }
    }
    let indices: Vec<usize> = (0..n).collect();
    c_hafnian_matrix(&m, &indices)
}

fn evaluate_ast_complex(
    ast: &AstNode,
    state: &AppState,
    options: MathOptions,
    steps: &mut Vec<MathStep>,
) -> Result<ComplexValue, String> {
    match ast {
        AstNode::Number(n) => Ok(ComplexValue::new(*n, 0.0)),
        AstNode::Variable(name) => Err(format!("unbound variable '{}'", name)),
        AstNode::Matrix(_) => Err("matrix literal used outside of a matrix function (hafnian)".to_string()),
        AstNode::UnaryNeg(inner) => {
            let v = evaluate_ast_complex(inner, state, options, steps)?;
            let result = -v;
            steps.push(MathStep {
                rule: "unary_negation".to_string(),
                expression: format!("-({})", complex_to_text(v)),
                latex: format!("-\\left({}\\right)", complex_to_text(v)),
                result: result.re,
                complex_result: result,
                crystal_traces: crystal_rule_traces(state, "negation"),
                geometry: Some(base_phase_payload("-")),
            });
            Ok(result)
        }
        AstNode::Function { name, args } if name == "imaginary" => {
            let v = match args.first() {
                Some(a) => evaluate_ast_complex(a, state, options, steps)?,
                None => ComplexValue::new(1.0, 0.0),
            };
            Ok(ComplexValue::new(0.0, v.re))
        }
        AstNode::Function { name, args }
            if name.to_ascii_lowercase() == "hafnian"
                && args.len() == 1
                && matches!(args.first(), Some(AstNode::Matrix(_))) =>
        {
            let rows_ast = match args.first().unwrap() {
                AstNode::Matrix(r) => r,
                _ => unreachable!(),
            };
            let n = rows_ast.len();
            let latex_mat = ast_to_latex(args.first().unwrap());
            let mut matrix: Vec<Vec<ComplexValue>> = Vec::with_capacity(n);
            for row_ast in rows_ast {
                let mut row: Vec<ComplexValue> = Vec::with_capacity(row_ast.len());
                for cell_ast in row_ast {
                    row.push(evaluate_ast_complex(cell_ast, state, options, steps)?);
                }
                matrix.push(row);
            }
            let result = c_hafnian(matrix)?;
            if !result.is_finite() {
                return Err("hafnian produced non-finite result".to_string());
            }
            steps.push(MathStep {
                rule: "hafnian_matrix".to_string(),
                expression: format!("hafnian({}x{} matrix)", n, n),
                latex: format!("\\operatorname{{haf}}{}", latex_mat),
                result: result.re,
                complex_result: result,
                crystal_traces: crystal_rule_traces(state, "hafnian"),
                geometry: Some(base_phase_payload("hafnian")),
            });
            Ok(result)
        }
        AstNode::Function { name, args } if name.to_ascii_lowercase() == "integral" => {
            // integral(a, b, expr, var)
            if args.len() != 4 {
                return Err("integral(a, b, expr, var) requires exactly 4 arguments".to_string());
            }
            let a_val = evaluate_ast_complex(&args[0], state, options, steps)?;
            let b_val = evaluate_ast_complex(&args[1], state, options, steps)?;
            if !a_val.is_real() || !b_val.is_real() {
                return Err("integral bounds must be real".to_string());
            }
            let var_name = match &args[3] {
                AstNode::Variable(v) => v.clone(),
                AstNode::Number(n) => return Err(format!("integral 4th argument must be a variable name, got {}", n)),
                _ => return Err("integral 4th argument must be a variable name".to_string()),
            };
            let integrand = &args[2];
            let latex_expr = ast_to_latex(integrand);
            let result = numerical_integrate(integrand, &var_name, a_val.re, b_val.re, 1000)?;
            if !result.is_finite() {
                return Err("integral produced non-finite result".to_string());
            }
            steps.push(MathStep {
                rule: "numerical_integration".to_string(),
                expression: format!(
                    "integral({}, {}, {}, {})",
                    a_val.re, b_val.re, latex_expr, var_name
                ),
                latex: format!(
                    "\\int_{{{}}}^{{{}}} {} \\, d{}",
                    format_number(a_val.re), format_number(b_val.re), latex_expr, var_name
                ),
                result: result.re,
                complex_result: result,
                crystal_traces: crystal_rule_traces(state, "integral"),
                geometry: Some(base_phase_payload("integral")),
            });
            Ok(result)
        }
        AstNode::Function { name, args } => {
            let mut arg_values = Vec::<ComplexValue>::with_capacity(args.len());
            for arg in args {
                arg_values.push(evaluate_ast_complex(arg, state, options, steps)?);
            }
            let (result, geometry, rule) = parse_function_result(name, &arg_values, options)?;
            if !result.is_finite() {
                return Err("non-finite result encountered".to_string());
            }

            let arg_text = arg_values
                .iter()
                .copied()
                .map(complex_to_text)
                .collect::<Vec<_>>()
                .join(", ");

            let latex = match name.as_str() {
                "sqrt" if arg_values.len() == 1 => {
                    format!("\\sqrt{{{}}}", complex_to_text(arg_values[0]))
                }
                "ln" if arg_values.len() == 1 => {
                    format!("\\ln\\left({}\\right)", complex_to_text(arg_values[0]))
                }
                "log" | "log10" if arg_values.len() == 1 => {
                    format!("\\log\\left({}\\right)", complex_to_text(arg_values[0]))
                }
                "sin" | "cos" | "tan" | "asin" | "acos" | "atan" | "sinh" | "cosh"
                | "tanh" | "asinh" | "acosh" | "atanh" | "exp" | "abs" | "arg" | "conj"
                | "gamma" | "zeta" | "lambertw" | "polylog" | "gammainc" | "j_sph" | "det"
                | "permanent" => {
                    format!("\\{}\\left({}\\right)", name, arg_text)
                }
                _ => format!("\\operatorname{{{}}}\\left({}\\right)", name, arg_text),
            };

            let geometry_trace = if options.mode == MathMode::Geometric {
                geometry
            } else {
                None
            };

            steps.push(MathStep {
                rule,
                expression: format!("{}({})", name, arg_text),
                latex,
                result: result.re,
                complex_result: result,
                crystal_traces: crystal_rule_traces(state, name),
                geometry: geometry_trace.or_else(|| Some(base_phase_payload(name))),
            });
            Ok(result)
        }
        AstNode::Binary { op, left, right } => {
            let l = evaluate_ast_complex(left, state, options, steps)?;
            let r = evaluate_ast_complex(right, state, options, steps)?;
            let result = match op {
                '+' if options.mode == MathMode::Geometric => geometric_decimal_complex_binary('+', l, r),
                '-' if options.mode == MathMode::Geometric => geometric_decimal_complex_binary('-', l, r),
                '+' => c_add(l, r),
                '-' => c_sub(l, r),
                '*' => c_mul(l, r),
                '/' => c_div(l, r)?,
                '^' => c_pow(l, r)?,
                _ => return Err(format!("unsupported operator '{}'", op)),
            };

            if !result.is_finite() {
                return Err("non-finite result encountered".to_string());
            }

            let op_txt = match op {
                '+' => "+",
                '-' => "-",
                '*' => "*",
                '/' => "/",
                '^' => "^",
                _ => "?",
            };
            let latex = match op {
                '+' => format!("{} + {}", complex_to_text(l), complex_to_text(r)),
                '-' => format!("{} - {}", complex_to_text(l), complex_to_text(r)),
                '*' => format!("{} \\cdot {}", complex_to_text(l), complex_to_text(r)),
                '/' => format!("\\frac{{{}}}{{{}}}", complex_to_text(l), complex_to_text(r)),
                '^' => format!("{}^{{{}}}", complex_to_text(l), complex_to_text(r)),
                _ => format!("{} ? {}", complex_to_text(l), complex_to_text(r)),
            };

            steps.push(MathStep {
                rule: rule_name_for_op(*op).to_string(),
                expression: format!("{} {} {}", complex_to_text(l), op_txt, complex_to_text(r)),
                latex,
                result: result.re,
                complex_result: result,
                crystal_traces: crystal_rule_traces(state, rule_name_for_op(*op)),
                geometry: Some(base_phase_payload(op_txt)),
            });
            Ok(result)
        }
    }
}

fn rule_name_for_op(op: char) -> &'static str {
    match op {
        '+' => "addition",
        '-' => "subtraction",
        '*' => "multiplication",
        '/' => "division",
        '^' => "exponentiation",
        _ => "operation",
    }
}

fn phase_for_op(op: &str) -> f64 {
    match op {
        "+" => 0.0,
        "-" => std::f64::consts::FRAC_PI_4,
        "*" => std::f64::consts::PI / 6.0,
        "/" => std::f64::consts::FRAC_PI_2,
        "^" => std::f64::consts::PI / 3.0,
        "sqrt" => -std::f64::consts::PI / 6.0,
        "log" => -std::f64::consts::FRAC_PI_4,
        "ln" => -std::f64::consts::FRAC_PI_4,
        "exp" => std::f64::consts::FRAC_PI_4,
        "sinh" => std::f64::consts::PI / 8.0,
        "cosh" => std::f64::consts::PI / 9.0,
        "tanh" => std::f64::consts::PI / 10.0,
        "asinh" => -std::f64::consts::PI / 8.0,
        "acosh" => -std::f64::consts::PI / 9.0,
        "atanh" => -std::f64::consts::PI / 10.0,
        "gamma" => std::f64::consts::PI / 12.0,
        "zeta" => std::f64::consts::PI / 14.0,
        "lambertw" | "w" => std::f64::consts::PI / 13.0,
        "polylog" => std::f64::consts::PI / 15.0,
        "gammainc" => std::f64::consts::PI / 16.0,
        "j_sph" => std::f64::consts::PI / 17.0,
        "det" => std::f64::consts::PI / 19.0,
        "permanent" => std::f64::consts::PI / 20.0,
        "hafnian" => std::f64::consts::PI / 35.0,
        "beta" => std::f64::consts::PI / 21.0,
        "erf" => std::f64::consts::PI / 22.0,
        "erfc" => std::f64::consts::PI / 23.0,
        "si" => std::f64::consts::PI / 24.0,
        "ci" => std::f64::consts::PI / 25.0,
        "fresnelc" => std::f64::consts::PI / 26.0,
        "fresnels" => std::f64::consts::PI / 27.0,
        "ei" => std::f64::consts::PI / 28.0,
        "li" => std::f64::consts::PI / 29.0,
        "sinc" => std::f64::consts::PI / 30.0,
        "ai" => std::f64::consts::PI / 31.0,
        "bi" => std::f64::consts::PI / 32.0,
        "theta4" => std::f64::consts::PI / 33.0,
        "integral" => std::f64::consts::PI / 34.0,
        "j3" => std::f64::consts::PI / 18.0,
        "!" => std::f64::consts::PI / 3.0,
        "error" => std::f64::consts::PI,
        _ => 0.0,
    }
}

fn base_phase_payload(op: &str) -> Value {
    json!({
        "op": op,
        "base_phase": phase_for_op(op)
    })
}

fn wrap_pi(mut x: f64) -> f64 {
    let two_pi = std::f64::consts::PI * 2.0;
    x = (x + std::f64::consts::PI) % two_pi;
    if x < 0.0 {
        x += two_pi;
    }
    x - std::f64::consts::PI
}

fn phase_distance(a: f64, b: f64) -> f64 {
    (wrap_pi(a - b)).abs()
}

fn resonance(theta: f64) -> f64 {
    (1.0 - theta.abs() / std::f64::consts::PI).max(0.0)
}

fn classify_phase(theta: f64, torsion_norm: f64, had_error: bool) -> (&'static str, &'static str) {
    if had_error {
        return ("NO", "no");
    }
    if theta.abs() > std::f64::consts::PI * 0.9 {
        return ("NO", "no");
    }
    if torsion_norm > 0.68 {
        return ("NO", "no");
    }
    if resonance(theta) > 0.92 && torsion_norm < 0.25 {
        return ("YES", "yes");
    }
    ("NEEDS_INPUT", "need")
}

fn crystal_rule_traces(state: &AppState, rule: &str) -> Vec<Value> {
    let Some(index) = state.bank_index.as_ref() else {
        return Vec::new();
    };

    retrieve_from_index(index, rule, 2)
        .into_iter()
        .map(|(entry_id, score)| {
            let e = &index.entries[entry_id];
            json!({
                "rule": rule,
                "score": score,
                "crystal_id": e.crystal_id,
                "edge_id": e.edge_id,
                "evidence": e.searchable_text
            })
        })
        .collect::<Vec<_>>()
}

fn phase_trajectory_from_steps(steps: &[MathStep]) -> Vec<PhaseStep> {
    let mut out = Vec::<PhaseStep>::new();
    for step in steps {
        let theta = step
            .geometry
            .as_ref()
            .and_then(|g| g.get("base_phase").and_then(Value::as_f64))
            .unwrap_or_else(|| phase_for_op(&step.rule));
        out.push(PhaseStep {
            op: step.rule.clone(),
            inputs: vec![step.expression.clone()],
            output: complex_to_text(step.complex_result),
            phase_theta: theta,
        });
    }
    out
}

fn torsion_residual(trajectory: &[PhaseStep]) -> f64 {
    if trajectory.len() < 2 {
        return 0.0;
    }
    let mut torsion_sum = 0.0;
    for idx in 1..trajectory.len() {
        torsion_sum += phase_distance(trajectory[idx].phase_theta, trajectory[idx - 1].phase_theta);
    }
    torsion_sum / (trajectory.len() as f64 - 1.0)
}

fn evaluate_math_expression(expr: &str, state: &AppState, options: MathOptions) -> Result<Value, String> {
    let tokens = tokenize_expression(expr)?;
    let parsed = parse_math_or_logic_tokens(&tokens)?;
    let numeric_determination = match options.mode {
        MathMode::Algebraic => "ieee754_binary_trace",
        MathMode::Geometric => "geometric_decimal_scaling",
    };
    let mode_txt = match options.mode {
        MathMode::Algebraic => "algebraic",
        MathMode::Geometric => "geometric",
    };
    let angle_txt = match options.angle_unit {
        AngleUnit::Radians => "radians",
        AngleUnit::Degrees => "degrees",
    };
    let time_randomness_payload =
        build_time_crystal_randomness_payload(expr, mode_txt, angle_txt);

    match parsed {
        ParsedMathInput::Scalar(ast) => {
            let normalized = ast_to_infix(&ast);
            let latex_expr = ast_to_latex(&ast);
            let mut steps = Vec::<MathStep>::new();
            let result = evaluate_ast_complex(&ast, state, options, &mut steps)?;
            let result_text = complex_to_text_with_mode(result, options.mode);
            let phase_trajectory = phase_trajectory_from_steps(&steps);
            let final_theta = complex_phase(result);
            let torsion = torsion_residual(&phase_trajectory);
            let torsion_norm = torsion / std::f64::consts::PI;
            let (crystal_state, crystal_class) = classify_phase(final_theta, torsion_norm, false);
            let unit_crystal = build_unit_crystal_payload(options, angle_txt, final_theta, torsion_norm);
            let unit_contradictions = unit_contradictions_from_payload(&unit_crystal);
            let unit_contradiction_signal = unit_crystal
                .get("contradiction_signal")
                .cloned()
                .unwrap_or_else(|| json!({
                    "triggered": false,
                    "stop_reason": null,
                    "max_loop_torsion_norm": 0.0,
                    "threshold": 0.0
                }));

            let primary_math_job = build_primary_math_job(&ast, &normalized, expr, &steps, result);
            let hidden_math_jobs = orchestrate_hidden_math_checks(&ast, result, crystal_state, torsion_norm);
            let mut math_jobs = Vec::with_capacity(1 + hidden_math_jobs.len());
            math_jobs.push(primary_math_job);
            math_jobs.extend(hidden_math_jobs);
            let logic_jobs = Vec::new();
            let final_outcome = synthesize_final_outcome(&math_jobs, &logic_jobs, Some(result), None);
            let semantic_jobs = build_semantic_jobs(
                expr,
                &PrimaryGoal::EvaluateNumeric,
                &ParsedUnit::ScalarExpr(ast.clone()),
                &final_outcome,
            );
            let job_influence_audit = build_job_influence_audit(&semantic_jobs, &math_jobs, &logic_jobs);
            let evaluation_envelope = build_success_envelope(
                expr,
                ParsedUnit::ScalarExpr(ast.clone()),
                PrimaryGoal::EvaluateNumeric,
                options,
                unit_contradictions,
                semantic_jobs,
                math_jobs,
                logic_jobs,
                job_influence_audit,
                final_outcome,
            );
            let anticrystal_lob = build_anticrystal_lob(&evaluation_envelope.consistency.contradictions);

            let derivation_steps = steps
                .iter()
                .enumerate()
                .map(|(idx, s)| {
                    let mut payload = json!({
                        "step": idx + 1,
                        "rule": s.rule,
                        "expression": s.expression,
                        "latex": s.latex,
                        "result": s.result,
                        "crystal_traces": s.crystal_traces
                    });
                    if let Some(geo) = &s.geometry {
                        payload["geometry"] = geo.clone();
                    }
                    payload
                })
                .collect::<Vec<_>>();

            let phase_path = phase_trajectory
                .iter()
                .enumerate()
                .map(|(idx, step)| {
                    json!({
                        "monotonic_index": idx + 1,
                        "op": step.op,
                        "inputs": step.inputs,
                        "output": step.output,
                        "phase_theta": step.phase_theta,
                    })
                })
                .collect::<Vec<_>>();

            let strict_rwif_export = json!({
                "schema_version": "rwif_v2_preview",
                "crystal_id": format!("calc_{}", unix_time_secs()),
                "crystal_label": "CSIF Math Engine Export",
                "domain": "csif_scientific_math",
                "lobe": "symbolic",
                "frozen": false,
                "nodes": [
                    {
                        "node_id": "node_expression",
                        "label": expr,
                        "aliases": [],
                        "lobe": "symbolic",
                        "provenance": {"source": "csif_agent_v2_rust", "role": "expression"}
                    },
                    {
                        "node_id": "node_result",
                        "label": result_text,
                        "aliases": [],
                        "lobe": "symbolic",
                        "provenance": {"source": "csif_agent_v2_rust", "role": "result"}
                    },
                    {
                        "node_id": unit_crystal["unit_id"],
                        "label": unit_crystal["label"],
                        "aliases": [unit_crystal["representation_system"]],
                        "lobe": "symbolic",
                        "provenance": {"source": "csif_agent_v2_rust", "role": "unit_crystal"}
                    }
                ],
                "edges": [
                    {
                        "edge_id": "edge_eval_path",
                        "source_node": "node_expression",
                        "relation": "evaluates_to",
                        "target_node": "node_result",
                        "lobe": "symbolic",
                        "reinforcing": crystal_state != "NO",
                        "base_phase": if phase_trajectory.is_empty() { final_theta } else { phase_trajectory[0].phase_theta },
                        "confidence_band": resonance(final_theta),
                        "phase_trajectory": phase_path,
                        "provenance": {"source": "csif_agent_v2_rust", "export_mode": "strict_rwif_v2"},
                        "state_encoding": "signed_i8_plus_intent_v2",
                        "numeric_range": {
                            "amplitude": {"min": -127, "max": 127},
                            "intent": {"min": -127, "max": 127}
                        },
                        "wrap_mode": "principal_pi",
                        "integer_wrap_mode": "clamp",
                        "integration_rule": "scientific_parser_v1",
                        "schema_version": "RWIF_EDGE_V2"
                    },
                    {
                        "edge_id": "edge_unit_projection",
                        "source_node": "node_result",
                        "relation": "represented_in_unit",
                        "target_node": unit_crystal["unit_id"],
                        "lobe": "symbolic",
                        "reinforcing": true,
                        "base_phase": unit_crystal["phase_signature"]["phase_theta"],
                        "confidence_band": unit_crystal["phase_signature"]["resonance"],
                        "phase_trajectory": unit_crystal["trajectory"],
                        "provenance": {
                            "source": "csif_agent_v2_rust",
                            "export_mode": "strict_rwif_v2",
                            "representation_system": unit_crystal["representation_system"]
                        },
                        "state_encoding": "signed_i8_plus_intent_v2",
                        "numeric_range": {
                            "amplitude": {"min": -127, "max": 127},
                            "intent": {"min": -127, "max": 127}
                        },
                        "wrap_mode": "principal_pi",
                        "integer_wrap_mode": "clamp",
                        "integration_rule": "unit_morphism_v1",
                        "schema_version": "RWIF_EDGE_V2"
                    }
                ],
                "version_history": [
                    {
                        "timestamp": unix_time_secs(),
                        "note": "Exported from CSIF Rust math engine"
                    }
                ],
                "stability_score": resonance(final_theta),
                "rwif_schema_version": "RWIF_V2"
            });

            let mut payload = json!({
                "object": "csif.math.result",
                "engine": "deterministic_math_v2",
                "mode": mode_txt,
                "reasoning_policy": {
                    "mode_isolation": "strict",
                    "numeric_determination": numeric_determination,
                    "preserve_binary_trace": options.mode == MathMode::Algebraic,
                    "preserve_decimal_geometric": options.mode == MathMode::Geometric
                },
                "angle_unit": angle_txt,
                "expression": expr,
                "normalized_expression": normalized,
                "latex_expression": latex_expr,
                "result": if result.is_real() { json!(result.re) } else { complex_to_json(result) },
                "result_latex": format!("{} = {}", latex_expr, result_text),
                "deterministic": true,
                "derivation_trace": derivation_steps,
                "bridge_audit": bridge_audit_value(&evaluation_envelope),
                "anticrystal_lob": anticrystal_lob,
                "unit_crystal": unit_crystal,
                "unit_contradiction_signal": unit_contradiction_signal,
                "phase_signature": {
                    "final_theta": final_theta,
                    "resonance": resonance(final_theta),
                    "torsion_residual": torsion,
                    "torsion_norm": torsion_norm,
                    "crystal_state": crystal_state,
                    "crystal_class": crystal_class,
                    "trajectory": phase_path
                },
                "rwif_export": strict_rwif_export
            });
            if let Some((time_crystal, randomness_appearance)) = time_randomness_payload.as_ref() {
                payload["time_crystal"] = time_crystal.clone();
                payload["randomness_appearance"] = randomness_appearance.clone();
            }
            Ok(payload)
        }
        ParsedMathInput::Logic(logic_expr) => {
            let normalized = logic_expr_to_text(&logic_expr);
            let latex_expr = logic_expr_to_latex(&logic_expr);
            let planned_logic_job_id = stable_bridge_id("logic_user", &normalized);
            let math_jobs = build_logic_operand_math_jobs(&logic_expr, state, options, &planned_logic_job_id)?;
            let mut steps = Vec::<MathStep>::new();
            let logic_result = evaluate_logic_expression(&logic_expr, state, options, &mut steps)?;
            let phase_trajectory = phase_trajectory_from_steps(&steps);
            let final_theta = steps.last().map(|s| complex_phase(s.complex_result)).unwrap_or(0.0);
            let torsion = torsion_residual(&phase_trajectory);
            let torsion_norm = if phase_trajectory.is_empty() { 0.0 } else { torsion / std::f64::consts::PI };
            let (crystal_state, crystal_class) = classify_phase(final_theta, torsion_norm, false);
            let unit_crystal = build_unit_crystal_payload(options, angle_txt, final_theta, torsion_norm);
            let unit_contradictions = unit_contradictions_from_payload(&unit_crystal);
            let unit_contradiction_signal = unit_crystal
                .get("contradiction_signal")
                .cloned()
                .unwrap_or_else(|| json!({
                    "triggered": false,
                    "stop_reason": null,
                    "max_loop_torsion_norm": 0.0,
                    "threshold": 0.0
                }));
            let logic_crystal = build_logic_crystal_payload(&logic_expr, &logic_result, final_theta, torsion_norm);
            let logic_connectives = build_logic_connective_payload(&logic_expr);
            let inference_morphisms = build_modus_ponens_morphisms(&logic_expr, final_theta, torsion_norm);
            let logic_contradiction_signal = build_logic_contradiction_signal(&inference_morphisms);
            let mut consistency_contradictions = unit_contradictions;
            consistency_contradictions.extend(logic_contradictions_from_payload(&logic_contradiction_signal));
            let truth_bool = matches!(logic_result.truth, TruthValue::True);
            let logic_jobs = vec![build_primary_logic_job(&logic_expr, &normalized, &steps, &logic_result)];
            let final_outcome = synthesize_final_outcome(&math_jobs, &logic_jobs, None, Some(logic_result.truth.clone()));
            let semantic_jobs = build_semantic_jobs(
                expr,
                &PrimaryGoal::CheckTruth,
                &ParsedUnit::LogicExpr(logic_expr.clone()),
                &final_outcome,
            );
            let job_influence_audit = build_job_influence_audit(&semantic_jobs, &math_jobs, &logic_jobs);
            let evaluation_envelope = build_success_envelope(
                expr,
                ParsedUnit::LogicExpr(logic_expr.clone()),
                PrimaryGoal::CheckTruth,
                options,
                consistency_contradictions,
                semantic_jobs,
                math_jobs,
                logic_jobs,
                job_influence_audit,
                final_outcome,
            );
            let anticrystal_lob = build_anticrystal_lob(&evaluation_envelope.consistency.contradictions);
            let derivation_steps = steps
                .iter()
                .enumerate()
                .map(|(idx, s)| {
                    let mut payload = json!({
                        "step": idx + 1,
                        "rule": s.rule,
                        "expression": s.expression,
                        "latex": s.latex,
                        "result": s.result,
                        "crystal_traces": s.crystal_traces
                    });
                    if let Some(geo) = &s.geometry {
                        payload["geometry"] = geo.clone();
                    }
                    payload
                })
                .collect::<Vec<_>>();

            let mut payload = json!({
                "object": "csif.math.result",
                "engine": "deterministic_math_v2",
                "mode": mode_txt,
                "angle_unit": angle_txt,
                "expression": expr,
                "normalized_expression": normalized,
                "latex_expression": latex_expr,
                "result": truth_bool,
                "result_latex": format!("{} = {}", latex_expr, if truth_bool { "\\mathrm{true}" } else { "\\mathrm{false}" }),
                "deterministic": true,
                "derivation_trace": derivation_steps,
                "logic_trace": vec![json!({
                    "rule": "comparison_evaluation",
                    "expression": logic_expr_to_text(&logic_expr),
                    "latex": logic_expr_to_latex(&logic_expr),
                    "truth": match logic_result.truth { TruthValue::True => true, TruthValue::False => false, _ => false },
                    "modality": format!("{:?}", logic_result.modality),
                })],
                "bridge_audit": bridge_audit_value(&evaluation_envelope),
                "anticrystal_lob": anticrystal_lob,
                "unit_crystal": unit_crystal,
                "unit_contradiction_signal": unit_contradiction_signal,
                "logic_crystal": logic_crystal,
                "logic_connectives": logic_connectives,
                "inference_morphisms": inference_morphisms,
                "logic_contradiction_signal": logic_contradiction_signal,
                "phase_signature": {
                    "final_theta": final_theta,
                    "resonance": resonance(final_theta),
                    "torsion_residual": torsion,
                    "torsion_norm": torsion_norm,
                    "crystal_state": crystal_state,
                    "crystal_class": crystal_class,
                    "trajectory": phase_trajectory.iter().enumerate().map(|(idx, step)| json!({
                        "monotonic_index": idx + 1,
                        "op": step.op,
                        "inputs": step.inputs,
                        "output": step.output,
                        "phase_theta": step.phase_theta,
                    })).collect::<Vec<_>>()
                }
            });
            if let Some((time_crystal, randomness_appearance)) = time_randomness_payload.as_ref() {
                payload["time_crystal"] = time_crystal.clone();
                payload["randomness_appearance"] = randomness_appearance.clone();
            }
            Ok(payload)
        }
    }
}

fn complex_to_math_value(value: ComplexValue) -> MathValue {
    if value.is_real() {
        MathValue::Real(value.re)
    } else {
        MathValue::Complex {
            re: value.re,
            im: value.im,
        }
    }
}

fn math_value_type_for_complex(value: ComplexValue) -> MathValueType {
    if value.is_real() {
        MathValueType::Real
    } else {
        MathValueType::Complex
    }
}

fn build_math_trace_steps(steps: &[MathStep]) -> Vec<MathTraceStep> {
    steps.iter()
        .map(|step| MathTraceStep {
            rule: step.rule.clone(),
            expression: step.expression.clone(),
            result: Some(complex_to_math_value(step.complex_result)),
            note: None,
        })
        .collect()
}

fn build_success_envelope(
    expr: &str,
    parsed_unit: ParsedUnit,
    primary_goal: PrimaryGoal,
    options: MathOptions,
    consistency_contradictions: Vec<ContradictionRecord>,
    semantic_jobs: Vec<SemanticJobRecord>,
    math_jobs: Vec<MathJobRecord>,
    logic_jobs: Vec<LogicJobRecord>,
    job_influence_audit: Vec<JobInfluenceRecord>,
    final_outcome: FinalOutcome,
) -> EvaluationEnvelope {
    let contradiction_count = consistency_contradictions.len();
    let mut final_outcome = final_outcome;
    final_outcome.machine_summary.contradiction_count = contradiction_count;
    if contradiction_count > 0 {
        if matches!(final_outcome.status, FinalStatus::Success) {
            final_outcome.status = FinalStatus::QualifiedSuccess;
        }
        if !final_outcome
            .responder_text
            .contains("contradiction-qualified")
        {
            final_outcome.responder_text = format!(
                "{} [contradiction-qualified: {} contradiction(s)]",
                final_outcome.responder_text,
                contradiction_count
            );
        }
    }

    let mut semantic_diagnostics = Vec::new();
    if matches!(primary_goal, PrimaryGoal::CheckTruth | PrimaryGoal::MixedReasoning) {
        semantic_diagnostics.push(DiagnosticEvent {
            code: "SEMANTIC_ROUTE_LOGIC".to_string(),
            message: "semantic layer routed request through math/logic bridge".to_string(),
            severity: CheckSeverity::Info,
        });
    }
    EvaluationEnvelope {
        envelope_id: stable_bridge_id("math_eval", expr),
        timestamp_unix_ms: 0,
        timeout_ms: None,
        source_text: expr.to_string(),
        source_kind: SourceKind::MathExpression,
        intent: IntentDescriptor {
            intent_id: "math_eval".to_string(),
            primary_goal,
            secondary_goals: Vec::new(),
            requested_output_mode: OutputMode::TextAndStructured,
        },
        semantic_context: EvalSemanticContext {
            resolved_entities: Vec::new(),
            ambiguity_state: AmbiguityState::Resolved,
            semantic_identity_signature: None,
            confidence: 1.0,
        },
        active_frame: EvalFrameContext {
            observer_frame: None,
            ontology_frame: Some("math_engine".to_string()),
            temporal_frame: None,
            modality_frame: Some(match options.mode {
                MathMode::Algebraic => "algebraic".to_string(),
                MathMode::Geometric => "geometric".to_string(),
            }),
            epistemic_source_frame: Some("deterministic_math_v2".to_string()),
        },
        assumptions: Vec::new(),
        parsed_units: vec![parsed_unit],
        symbol_table: SymbolTable::default(),
        semantic_jobs,
        math_jobs,
        logic_jobs,
        consistency: ConsistencyReport {
            semantic_math_alignment: AlignmentStatus::Aligned,
            semantic_logic_alignment: AlignmentStatus::Aligned,
            math_logic_alignment: if consistency_contradictions.is_empty() {
                AlignmentStatus::Aligned
            } else {
                AlignmentStatus::Conflicted
            },
            contradictions: consistency_contradictions,
            unresolved_ambiguities: Vec::new(),
        },
        routing_trace: vec![
            RouteEvent {
                stage: "parse".to_string(),
                decision: "expression_parsed".to_string(),
                rationale: Some("expression routed through shared math/logic bridge".to_string()),
            },
            RouteEvent {
                stage: "synthesize".to_string(),
                decision: "bridge_audit_emitted".to_string(),
                rationale: Some("job influence audit attached to response".to_string()),
            },
        ],
        job_influence_audit,
        diagnostics: semantic_diagnostics,
        final_outcome,
    }
}

fn bridge_audit_value(evaluation_envelope: &EvaluationEnvelope) -> Value {
    match serde_json::to_value(evaluation_envelope) {
        Ok(value) => value,
        Err(err) => json!({
            "serialization_error": err.to_string(),
            "final_outcome": evaluation_envelope.final_outcome,
            "job_influence_audit": evaluation_envelope.job_influence_audit,
        }),
    }
}

fn build_error_bridge_audit_value(
    expr: &str,
    error_message: &str,
    error_code: &str,
    status: &str,
) -> Value {
    let diagnostics = vec![DiagnosticEvent {
        code: error_code.to_string(),
        message: error_message.to_string(),
        severity: CheckSeverity::Error,
    }, DiagnosticEvent {
        code: "SEMANTIC_ROUTE_ERROR".to_string(),
        message: "semantic layer preserved error-path audit context".to_string(),
        severity: CheckSeverity::Info,
    }];
    let envelope = EvaluationEnvelope {
        envelope_id: stable_bridge_id("math_error", &format!("{}::{}", expr, error_code)),
        timestamp_unix_ms: 0,
        timeout_ms: None,
        source_text: expr.to_string(),
        source_kind: SourceKind::MathExpression,
        intent: IntentDescriptor {
            intent_id: "math_eval".to_string(),
            primary_goal: PrimaryGoal::EvaluateNumeric,
            secondary_goals: Vec::new(),
            requested_output_mode: OutputMode::TextAndStructured,
        },
        semantic_context: EvalSemanticContext {
            resolved_entities: Vec::new(),
            ambiguity_state: AmbiguityState::NeedsInput,
            semantic_identity_signature: None,
            confidence: 0.0,
        },
        active_frame: EvalFrameContext {
            observer_frame: None,
            ontology_frame: Some("math_engine".to_string()),
            temporal_frame: None,
            modality_frame: None,
            epistemic_source_frame: Some("deterministic_math_v2".to_string()),
        },
        assumptions: Vec::new(),
        parsed_units: Vec::new(),
        symbol_table: SymbolTable::default(),
        semantic_jobs: vec![
            SemanticJobRecord {
                job_id: stable_bridge_id("semantic_route", expr),
                invocation_mode: InvocationMode::InternallyTriggered,
                reasoning_scope: ReasoningScope::Committed,
                trigger_context: TriggerContext {
                    trigger_reason: "semantic_route_failed_request".to_string(),
                    triggered_by_job_id: None,
                    routed_from_stage: Some("csif_math".to_string()),
                },
                requested_operation: SemanticOperation::PreserveFailureContext,
                input_summary: expr.to_string(),
                result: SemanticJobResult {
                    status: SemanticStatus::Failed,
                    interpretation: format!("error preserved with code={}", error_code),
                    confidence: 1.0,
                    error: None,
                },
                trace: vec![SemanticTraceStep {
                    stage: "route_error".to_string(),
                    note: error_message.to_string(),
                    confidence: 1.0,
                }],
                influenced_final_answer: true,
                influence_notes: vec![format!("failed request context retained for {}", error_code)],
                diagnostics: Vec::new(),
            },
            SemanticJobRecord {
                job_id: stable_bridge_id("semantic_synthesis", &format!("{}::{}", expr, error_code)),
                invocation_mode: InvocationMode::InternallyTriggered,
                reasoning_scope: ReasoningScope::Committed,
                trigger_context: TriggerContext {
                    trigger_reason: "semantic_synthesize_failed_response".to_string(),
                    triggered_by_job_id: Some(stable_bridge_id("semantic_route", expr)),
                    routed_from_stage: Some("response_synthesis".to_string()),
                },
                requested_operation: SemanticOperation::SynthesizeResponse,
                input_summary: error_message.to_string(),
                result: SemanticJobResult {
                    status: SemanticStatus::Failed,
                    interpretation: "semantic layer synthesized final failure response".to_string(),
                    confidence: 1.0,
                    error: None,
                },
                trace: vec![SemanticTraceStep {
                    stage: "finalize_error".to_string(),
                    note: "failure envelope emitted".to_string(),
                    confidence: 1.0,
                }],
                influenced_final_answer: true,
                influence_notes: vec!["semantic failure synthesis".to_string()],
                diagnostics: Vec::new(),
            },
        ],
        math_jobs: Vec::new(),
        logic_jobs: Vec::new(),
        consistency: ConsistencyReport {
            semantic_math_alignment: AlignmentStatus::Unknown,
            semantic_logic_alignment: AlignmentStatus::Unknown,
            math_logic_alignment: AlignmentStatus::Unknown,
            contradictions: Vec::new(),
            unresolved_ambiguities: Vec::new(),
        },
        routing_trace: vec![RouteEvent {
            stage: "error".to_string(),
            decision: status.to_string(),
            rationale: Some(error_message.to_string()),
        }],
        job_influence_audit: vec![JobInfluenceRecord {
            job_id: stable_bridge_id("semantic_route", expr),
            job_kind: JobKind::Semantic,
            invocation_mode: InvocationMode::InternallyTriggered,
            reasoning_scope: ReasoningScope::Committed,
            used_in_final_answer: true,
            influence_role: InfluenceRole::DirectEvidence,
            explanation: format!("semantic layer preserved failed request context for {}", error_code),
        }, JobInfluenceRecord {
            job_id: stable_bridge_id("semantic_synthesis", &format!("{}::{}", expr, error_code)),
            job_kind: JobKind::Semantic,
            invocation_mode: InvocationMode::InternallyTriggered,
            reasoning_scope: ReasoningScope::Committed,
            used_in_final_answer: true,
            influence_role: InfluenceRole::CandidateRanking,
            explanation: "semantic layer synthesized final failure response".to_string(),
        }],
        diagnostics,
        final_outcome: FinalOutcome {
            status: FinalStatus::Failed,
            responder_text: error_message.to_string(),
            supporting_job_ids: Vec::new(),
            hidden_job_ids_used: Vec::new(),
            machine_summary: MachineSummary {
                final_value: None,
                final_truth: None,
                assumptions_applied: Vec::new(),
                contradiction_count: 0,
                confidence: 0.0,
            },
        },
    };
    bridge_audit_value(&envelope)
}

fn build_primary_math_job(
    ast: &AstNode,
    normalized: &str,
    expr: &str,
    steps: &[MathStep],
    result: ComplexValue,
) -> MathJobRecord {
    MathJobRecord {
        job_id: stable_bridge_id("math_user", &format!("{}::{}", normalized, expr)),
        invocation_mode: InvocationMode::UserRequested,
        reasoning_scope: ReasoningScope::Committed,
        trigger_context: TriggerContext {
            trigger_reason: "user_requested_numeric_evaluation".to_string(),
            triggered_by_job_id: None,
            routed_from_stage: Some("csif_math".to_string()),
        },
        input_expr: ast.clone(),
        normalized_expression: normalized.to_string(),
        requested_operation: MathOperation::Evaluate,
        domain_checks: Vec::new(),
        assumptions_used: Vec::new(),
        result: MathJobResult {
            status: MathStatus::Success,
            value: Some(complex_to_math_value(result)),
            value_type: math_value_type_for_complex(result),
            domain_ok: true,
            deterministic: true,
            precision_class: if result.is_real() {
                PrecisionClass::ExactDeterministic
            } else {
                PrecisionClass::ExactDeterministicComplex
            },
            error: None,
        },
        trace: build_math_trace_steps(steps),
        influenced_final_answer: true,
        influence_notes: vec!["primary user-visible numeric result".to_string()],
        diagnostics: Vec::new(),
    }
}

fn build_logic_trace_steps(expr: &LogicExprNode, result: &LogicResult) -> Vec<LogicTraceStep> {
    vec![LogicTraceStep {
        rule: match expr {
            LogicExprNode::Comparison { .. } => "comparison_evaluation".to_string(),
            LogicExprNode::Predicate { .. } => "predicate_evaluation".to_string(),
            LogicExprNode::Implies(_, _) => "implication_evaluation".to_string(),
            LogicExprNode::Equivalent(_, _) => "equivalence_evaluation".to_string(),
            _ => "logic_connective_evaluation".to_string(),
        },
        expression: logic_expr_to_text(expr),
        truth: result.truth.clone(),
        note: Some(format!("modality={:?}", result.modality)),
    }]
}

fn build_semantic_jobs(
    expr: &str,
    primary_goal: &PrimaryGoal,
    parsed_unit: &ParsedUnit,
    final_outcome: &FinalOutcome,
) -> Vec<SemanticJobRecord> {
    let route_job_id = stable_bridge_id("semantic_route", expr);
    let synth_job_id = stable_bridge_id("semantic_synthesis", expr);
    let parsed_summary = match parsed_unit {
        ParsedUnit::ScalarExpr(_) => "scalar_expression",
        ParsedUnit::LogicExpr(_) => "logic_expression",
        ParsedUnit::RelationExpr(_) => "relation_expression",
        ParsedUnit::ConstraintSet(_) => "constraint_set",
        ParsedUnit::Query(_) => "query",
    };
    vec![
        SemanticJobRecord {
            job_id: route_job_id.clone(),
            invocation_mode: InvocationMode::InternallyTriggered,
            reasoning_scope: ReasoningScope::Committed,
            trigger_context: TriggerContext {
                trigger_reason: "semantic_route_request".to_string(),
                triggered_by_job_id: None,
                routed_from_stage: Some("csif_math".to_string()),
            },
            requested_operation: SemanticOperation::RouteRequest,
            input_summary: expr.to_string(),
            result: SemanticJobResult {
                status: SemanticStatus::Success,
                interpretation: format!("primary_goal={:?}, parsed_unit={}", primary_goal, parsed_summary),
                confidence: 1.0,
                error: None,
            },
            trace: vec![SemanticTraceStep {
                stage: "route".to_string(),
                note: "expression routed through shared semantic bridge".to_string(),
                confidence: 1.0,
            }],
            influenced_final_answer: true,
            influence_notes: vec![format!("semantic route for {:?}", primary_goal)],
            diagnostics: Vec::new(),
        },
        SemanticJobRecord {
            job_id: synth_job_id,
            invocation_mode: InvocationMode::InternallyTriggered,
            reasoning_scope: ReasoningScope::Committed,
            trigger_context: TriggerContext {
                trigger_reason: "semantic_synthesize_response".to_string(),
                triggered_by_job_id: Some(route_job_id),
                routed_from_stage: Some("response_synthesis".to_string()),
            },
            requested_operation: SemanticOperation::SynthesizeResponse,
            input_summary: final_outcome.responder_text.clone(),
            result: SemanticJobResult {
                status: SemanticStatus::Success,
                interpretation: format!("final_status={:?}", final_outcome.status),
                confidence: final_outcome.machine_summary.confidence,
                error: None,
            },
            trace: vec![SemanticTraceStep {
                stage: "synthesize".to_string(),
                note: "final response assembled from semantic/math/logic jobs".to_string(),
                confidence: final_outcome.machine_summary.confidence,
            }],
            influenced_final_answer: true,
            influence_notes: vec!["semantic response synthesis".to_string()],
            diagnostics: Vec::new(),
        },
    ]
}

fn build_primary_logic_job(
    expr: &LogicExprNode,
    normalized: &str,
    steps: &[MathStep],
    result: &LogicResult,
) -> LogicJobRecord {
    let trigger_reason = if steps.is_empty() {
        "user_requested_logic_evaluation"
    } else {
        "user_requested_logic_evaluation_with_internal_math_operands"
    };
    LogicJobRecord {
        job_id: stable_bridge_id("logic_user", normalized),
        invocation_mode: InvocationMode::UserRequested,
        reasoning_scope: ReasoningScope::Committed,
        trigger_context: TriggerContext {
            trigger_reason: trigger_reason.to_string(),
            triggered_by_job_id: None,
            routed_from_stage: Some("csif_math".to_string()),
        },
        input_expr: expr.clone(),
        requested_operation: LogicOperation::EvaluateTruth,
        assumptions_used: Vec::new(),
        constraint_context: Vec::new(),
        result: result.clone(),
        trace: build_logic_trace_steps(expr, result),
        influenced_final_answer: true,
        influence_notes: vec!["primary user-visible logical truth result".to_string()],
        diagnostics: Vec::new(),
    }
}

fn build_logic_operand_math_jobs(
    expr: &LogicExprNode,
    state: &AppState,
    options: MathOptions,
    parent_logic_job_id: &str,
) -> Result<Vec<MathJobRecord>, String> {
    fn append_operand_job(
        jobs: &mut Vec<MathJobRecord>,
        operand_ast: &AstNode,
        state: &AppState,
        options: MathOptions,
        parent_logic_job_id: &str,
        label: &str,
    ) -> Result<(), String> {
        let mut operand_steps = Vec::<MathStep>::new();
        let result = evaluate_ast_complex(operand_ast, state, options, &mut operand_steps)?;
        jobs.push(MathJobRecord {
            job_id: stable_bridge_id("math_logic_operand", &format!("{}::{}", label, ast_to_infix(operand_ast))),
            invocation_mode: InvocationMode::InternallyTriggered,
            reasoning_scope: ReasoningScope::Committed,
            trigger_context: TriggerContext {
                trigger_reason: format!("logic_operand_evaluation:{}", label),
                triggered_by_job_id: Some(parent_logic_job_id.to_string()),
                routed_from_stage: Some("logic_evaluation".to_string()),
            },
            input_expr: operand_ast.clone(),
            normalized_expression: ast_to_infix(operand_ast),
            requested_operation: MathOperation::Evaluate,
            domain_checks: Vec::new(),
            assumptions_used: Vec::new(),
            result: MathJobResult {
                status: MathStatus::Success,
                value: Some(complex_to_math_value(result)),
                value_type: math_value_type_for_complex(result),
                domain_ok: true,
                deterministic: true,
                precision_class: if result.is_real() {
                    PrecisionClass::ExactDeterministic
                } else {
                    PrecisionClass::ExactDeterministicComplex
                },
                error: None,
            },
            trace: build_math_trace_steps(&operand_steps),
            influenced_final_answer: true,
            influence_notes: vec![format!("hidden numeric operand for logic evaluation ({})", label)],
            diagnostics: Vec::new(),
        });
        Ok(())
    }

    fn visit_logic_expr(
        expr: &LogicExprNode,
        jobs: &mut Vec<MathJobRecord>,
        state: &AppState,
        options: MathOptions,
        parent_logic_job_id: &str,
        counter: &mut usize,
    ) -> Result<(), String> {
        match expr {
            LogicExprNode::Comparison { left, right, .. } => {
                append_operand_job(jobs, left, state, options, parent_logic_job_id, &format!("cmp_left_{}", *counter))?;
                *counter += 1;
                append_operand_job(jobs, right, state, options, parent_logic_job_id, &format!("cmp_right_{}", *counter))?;
                *counter += 1;
            }
            LogicExprNode::Predicate { name, args } => {
                for (idx, arg) in args.iter().enumerate() {
                    append_operand_job(jobs, arg, state, options, parent_logic_job_id, &format!("pred_{}_arg_{}", name, idx))?;
                    *counter += 1;
                }
            }
            LogicExprNode::Not(inner) => visit_logic_expr(inner, jobs, state, options, parent_logic_job_id, counter)?,
            LogicExprNode::And(parts) | LogicExprNode::Or(parts) | LogicExprNode::Xor(parts) => {
                for part in parts {
                    visit_logic_expr(part, jobs, state, options, parent_logic_job_id, counter)?;
                }
            }
            LogicExprNode::Implies(lhs, rhs) | LogicExprNode::Equivalent(lhs, rhs) => {
                visit_logic_expr(lhs, jobs, state, options, parent_logic_job_id, counter)?;
                visit_logic_expr(rhs, jobs, state, options, parent_logic_job_id, counter)?;
            }
            LogicExprNode::BoolLiteral(_) => {}
        }
        Ok(())
    }

    let mut jobs = Vec::new();
    let mut counter = 0usize;
    visit_logic_expr(expr, &mut jobs, state, options, parent_logic_job_id, &mut counter)?;
    Ok(jobs)
}

fn make_hidden_math_job(
    ast: &AstNode,
    trigger_reason: &str,
    note: &str,
    result: ComplexValue,
    triggered_by_job_id: Option<String>,
) -> MathJobRecord {
    MathJobRecord {
        job_id: stable_bridge_id(
            "math_hidden",
            &format!("{}::{}::{}", trigger_reason, ast_to_infix(ast), note),
        ),
        invocation_mode: InvocationMode::InternallyTriggered,
        reasoning_scope: ReasoningScope::Committed,
        trigger_context: TriggerContext {
            trigger_reason: trigger_reason.to_string(),
            triggered_by_job_id,
            routed_from_stage: Some("response_synthesis".to_string()),
        },
        input_expr: ast.clone(),
        normalized_expression: ast_to_infix(ast),
        requested_operation: MathOperation::ResponseQualification,
        domain_checks: Vec::new(),
        assumptions_used: Vec::new(),
        result: MathJobResult {
            status: MathStatus::Success,
            value: Some(complex_to_math_value(result)),
            value_type: math_value_type_for_complex(result),
            domain_ok: true,
            deterministic: true,
            precision_class: if result.is_real() {
                PrecisionClass::ExactDeterministic
            } else {
                PrecisionClass::ExactDeterministicComplex
            },
            error: None,
        },
        trace: vec![MathTraceStep {
            rule: "hidden_math_check".to_string(),
            expression: note.to_string(),
            result: Some(complex_to_math_value(result)),
            note: Some(note.to_string()),
        }],
        influenced_final_answer: true,
        influence_notes: vec![note.to_string()],
        diagnostics: Vec::new(),
    }
}

fn orchestrate_hidden_math_checks(
    ast: &AstNode,
    result: ComplexValue,
    crystal_state: &str,
    torsion_norm: f64,
) -> Vec<MathJobRecord> {
    let mut jobs = Vec::new();
    if !result.is_real() {
        jobs.push(make_hidden_math_job(
            ast,
            "qualify_complex_result_for_response",
            "internal qualification: response requires complex-valued result context",
            result,
            None,
        ));
    }
    if crystal_state != "YES" {
        let parent_job_id = jobs.last().map(|job| job.job_id.clone());
        jobs.push(make_hidden_math_job(
            ast,
            "phase_signature_gate_for_response",
            &format!(
                "internal qualification: crystal_state={} with torsion_norm={:.6}",
                crystal_state, torsion_norm
            ),
            result,
            parent_job_id,
        ));
    }
    jobs
}

fn build_job_influence_audit(
    semantic_jobs: &[SemanticJobRecord],
    math_jobs: &[MathJobRecord],
    logic_jobs: &[LogicJobRecord],
) -> Vec<JobInfluenceRecord> {
    let mut audit = Vec::with_capacity(semantic_jobs.len() + math_jobs.len() + logic_jobs.len());
    for job in semantic_jobs {
        audit.push(JobInfluenceRecord {
            job_id: job.job_id.clone(),
            job_kind: JobKind::Semantic,
            invocation_mode: job.invocation_mode.clone(),
            reasoning_scope: job.reasoning_scope.clone(),
            used_in_final_answer: job.influenced_final_answer,
            influence_role: match job.requested_operation {
                SemanticOperation::RouteRequest | SemanticOperation::PreserveFailureContext => InfluenceRole::DirectEvidence,
                SemanticOperation::SynthesizeResponse => InfluenceRole::CandidateRanking,
            },
            explanation: if job.influence_notes.is_empty() {
                job.input_summary.clone()
            } else {
                job.influence_notes.join("; ")
            },
        });
    }
    for job in math_jobs {
        audit.push(JobInfluenceRecord {
            job_id: job.job_id.clone(),
            job_kind: JobKind::Math,
            invocation_mode: job.invocation_mode.clone(),
            reasoning_scope: job.reasoning_scope.clone(),
            used_in_final_answer: job.influenced_final_answer,
            influence_role: match job.invocation_mode {
                InvocationMode::UserRequested => InfluenceRole::FinalComputation,
                InvocationMode::InternallyTriggered => {
                    if job.trigger_context.trigger_reason.contains("gate") {
                        InfluenceRole::DomainGate
                    } else {
                        InfluenceRole::ConsistencyCheck
                    }
                }
            },
            explanation: if job.influence_notes.is_empty() {
                job.trigger_context.trigger_reason.clone()
            } else {
                job.influence_notes.join("; ")
            },
        });
    }
    for job in logic_jobs {
        audit.push(JobInfluenceRecord {
            job_id: job.job_id.clone(),
            job_kind: JobKind::Logic,
            invocation_mode: job.invocation_mode.clone(),
            reasoning_scope: job.reasoning_scope.clone(),
            used_in_final_answer: job.influenced_final_answer,
            influence_role: match job.invocation_mode {
                InvocationMode::UserRequested => InfluenceRole::DirectEvidence,
                InvocationMode::InternallyTriggered => InfluenceRole::ConsistencyCheck,
            },
            explanation: if job.influence_notes.is_empty() {
                job.trigger_context.trigger_reason.clone()
            } else {
                job.influence_notes.join("; ")
            },
        });
    }
    audit
}

fn cors_headers() -> [(header::HeaderName, &'static str); 3] {
    [
        (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*"),
        (header::ACCESS_CONTROL_ALLOW_METHODS, "POST, OPTIONS"),
        (header::ACCESS_CONTROL_ALLOW_HEADERS, "Content-Type"),
    ]
}

fn with_cors(response: axum::response::Response) -> axum::response::Response {
    let mut response = response;
    for (name, value) in cors_headers() {
        response.headers_mut().insert(name, value.parse().unwrap());
    }
    response
}

async fn csif_math_options() -> impl IntoResponse {
    (StatusCode::NO_CONTENT, cors_headers())
}

fn synthesize_final_outcome(
    math_jobs: &[MathJobRecord],
    logic_jobs: &[LogicJobRecord],
    final_value: Option<ComplexValue>,
    final_truth: Option<TruthValue>,
) -> FinalOutcome {
    let supporting_job_ids = math_jobs
        .iter()
        .filter(|job| job.influenced_final_answer)
        .map(|job| job.job_id.clone())
        .chain(
            logic_jobs
                .iter()
                .filter(|job| job.influenced_final_answer)
                .map(|job| job.job_id.clone()),
        )
        .collect::<Vec<_>>();
    let hidden_job_ids_used = math_jobs
        .iter()
        .filter(|job| {
            job.influenced_final_answer && matches!(job.invocation_mode, InvocationMode::InternallyTriggered)
        })
        .map(|job| job.job_id.clone())
        .collect::<Vec<_>>();
    let status = if hidden_job_ids_used.is_empty() {
        FinalStatus::Success
    } else {
        FinalStatus::QualifiedSuccess
    };
    FinalOutcome {
        status,
        responder_text: if hidden_job_ids_used.is_empty() {
            "deterministic math evaluation completed".to_string()
        } else {
            "deterministic math evaluation completed with internal qualification checks".to_string()
        },
        supporting_job_ids,
        hidden_job_ids_used,
        machine_summary: MachineSummary {
            final_value: final_value.map(complex_to_math_value),
            final_truth,
            assumptions_applied: Vec::new(),
            contradiction_count: 0,
            confidence: 1.0,
        },
    }
}

fn stable_bridge_id(prefix: &str, seed: &str) -> String {
    let hash = value_hash(&json!({
        "prefix": prefix,
        "seed": seed,
    }));
    format!("{}_{}", prefix, hash)
}

fn retrieval_payload(state: &AppState, query: &str, top_k: usize) -> Value {
    let Some(index) = state.bank_index.as_ref() else {
        return json!({
            "loaded": false,
            "query": query,
            "rewritten_query": query,
            "matches": [],
            "match_count": 0,
            "miss_diagnostics": {
                "reason": "index_unloaded",
                "query_tokens": tokenize(query),
                "rewritten_query_tokens": tokenize(query),
            },
            "message": "No RWIF bank index loaded"
        });
    };

    let query_tokens = tokenize(query);
    let (rewritten_query, rewrite_reasons) = rewrite_retrieval_query(query);
    let rewritten_query_tokens = tokenize(&rewritten_query);
    let hits = retrieve_from_index(index, &rewritten_query, top_k);
    let match_count = hits.len();
    let missing_tokens = rewritten_query_tokens
        .iter()
        .filter(|token| !index.postings.contains_key(*token))
        .cloned()
        .collect::<Vec<_>>();

    let miss_diagnostics = if match_count == 0 {
        json!({
            "reason": if rewritten_query_tokens.is_empty() {
                "empty_normalized_query"
            } else if missing_tokens.len() == rewritten_query_tokens.len() {
                "query_terms_absent_from_index"
            } else {
                "weak_overlap_after_rewrite"
            },
            "query_tokens": query_tokens,
            "rewritten_query_tokens": rewritten_query_tokens,
            "missing_tokens": missing_tokens,
            "rewrite_applied": !rewrite_reasons.is_empty(),
            "rewrite_reasons": rewrite_reasons,
        })
    } else {
        json!({
            "reason": "none",
            "query_tokens": query_tokens,
            "rewritten_query_tokens": rewritten_query_tokens,
            "missing_tokens": missing_tokens,
            "rewrite_applied": !rewrite_reasons.is_empty(),
            "rewrite_reasons": rewrite_reasons,
        })
    };

    json!({
        "loaded": true,
        "query": query,
        "rewritten_query": rewritten_query,
        "top_k": top_k,
        "matches": hits
            .into_iter()
            .map(|(entry_id, score)| {
                let e = &index.entries[entry_id];
                json!({
                    "score": score,
                    "crystal_id": e.crystal_id,
                    "edge_id": e.edge_id,
                    "source_node": e.source_node,
                    "relation": e.relation,
                    "target_node": e.target_node,
                    "searchable_text": e.searchable_text
                })
            })
            .collect::<Vec<_>>(),
        "match_count": match_count,
        "miss_diagnostics": miss_diagnostics,
    })
}

fn build_chat_answer(
    messages: &[ChatMessage],
    state: &AppState,
    requested_preferences: Option<&ChatPreferencesRequest>,
) -> (String, Value) {
    let prompt = last_user_prompt(messages);
    let intent = detect_chat_intent(&prompt);
    let preferences = resolve_chat_preferences(requested_preferences, &prompt);
    let concise = preferences.response_style == "concise";
    let recent_user = recent_user_prompts(messages, 3);
    let context_items = recent_user
        .iter()
        .take(recent_user.len().saturating_sub(1))
        .cloned()
        .collect::<Vec<_>>();

    let bank_meta = if let Some(summary) = &state.bank_summary {
        json!({
            "loaded": true,
            "bank_id": summary.bank_id.clone(),
            "crystal_count": summary.crystal_count,
            "edge_count": summary.edge_count,
            "event_count": summary.event_count
        })
    } else {
        json!({
            "loaded": false,
            "bank_id": Value::Null,
            "crystal_count": 0,
            "edge_count": 0,
            "event_count": 0
        })
    };

    let time_crystal_context = build_chat_time_crystal_context();

    let mut retrieval_matches = Vec::new();
    let mut retrieval_summary = json!({
        "enabled": false,
        "coverage": 0.0,
        "mean_score": 0.0,
        "readability_score": 1.0,
        "summary_quality": "none",
        "summary_text": "Retrieval summary disabled.",
    });
    let mut retrieval_meta = json!({
        "match_count": 0,
        "matches": [],
        "rewritten_query": prompt,
        "summary": retrieval_summary,
        "miss_diagnostics": {
            "reason": "index_unloaded",
            "query_tokens": tokenize(&prompt),
            "rewritten_query_tokens": tokenize(&prompt),
        }
    });
    let (opening_text, opening_randomness) = conversational_opening(
        intent,
        preferences.tone,
        preferences.warmth_ceiling,
        time_crystal_context.as_ref(),
    );
    let mut retrieval_fallback_randomness = json!({
        "enabled": false,
        "reason": "not_used",
    });

    let mut answer = String::new();
    answer.push_str(&opening_text);
    answer.push_str("\n\n");

    if let Some(context_bridge) = context_bridge_text(&context_items, concise) {
        answer.push_str(&context_bridge);
        answer.push_str("\n\n");
    }

    let mut has_retrieval_hits = false;

    if let Some(index) = state.bank_index.as_ref() {
        let (rewritten_prompt, rewrite_reasons) = rewrite_retrieval_query(&prompt);
        let hits = retrieve_from_index(index, &rewritten_prompt, preferences.retrieval_top_k);
        let rewritten_tokens = tokenize(&rewritten_prompt);
        let missing_tokens = rewritten_tokens
            .iter()
            .filter(|t| !index.postings.contains_key(*t))
            .cloned()
            .collect::<Vec<_>>();
        if hits.is_empty() {
            let (fallback_line, fallback_randomness) = retrieval_fallback_text(
                "no_match_hits",
                preferences.tone,
                concise,
                time_crystal_context.as_ref(),
            );
            retrieval_fallback_randomness = fallback_randomness;
            answer.push_str(&fallback_line);
            answer.push('\n');
            retrieval_meta = json!({
                "match_count": 0,
                "matches": [],
                "rewritten_query": rewritten_prompt,
                "summary": retrieval_summary,
                "miss_diagnostics": {
                    "reason": if missing_tokens.len() == rewritten_tokens.len() {
                        "query_terms_absent_from_index"
                    } else {
                        "weak_overlap_after_rewrite"
                    },
                    "query_tokens": tokenize(&prompt),
                    "rewritten_query_tokens": rewritten_tokens,
                    "missing_tokens": missing_tokens,
                    "rewrite_applied": !rewrite_reasons.is_empty(),
                    "rewrite_reasons": rewrite_reasons,
                }
            });
            if !concise {
                answer.push_str("I can still help with direct reasoning.\n\n");
            }
        } else {
            has_retrieval_hits = true;
            answer.push_str("I found matching evidence in your RWIF index:\n");
            for (entry_id, score) in hits {
                let e = &index.entries[entry_id];
                retrieval_matches.push(json!({
                    "score": score,
                    "crystal_id": e.crystal_id,
                    "edge_id": e.edge_id,
                    "source_node": e.source_node,
                    "relation": e.relation,
                    "target_node": e.target_node,
                    "searchable_text": e.searchable_text
                }));
                if preferences.depth == "deep" {
                    answer.push_str(&format!(
                        "- {} -> {} (relation: {}, score: {}, crystal: {}, edge: {})\n",
                        e.source_node, e.target_node, e.relation, score, e.crystal_id, e.edge_id
                    ));
                } else {
                    answer.push_str(&format!(
                        "- {} -> {} (relation: {}, score: {})\n",
                        e.source_node, e.target_node, e.relation, score
                    ));
                }
            }
            answer.push('\n');
            if preferences.retrieval_summary {
                retrieval_summary = summarize_retrieval_readability(
                    retrieval_matches.as_slice(),
                    preferences.retrieval_top_k,
                );
                if let Some(summary_text) = retrieval_summary.get("summary_text").and_then(Value::as_str) {
                    answer.push_str("Evidence summary: ");
                    answer.push_str(summary_text);
                    answer.push_str("\n\n");
                }
            }
            retrieval_meta = json!({
                "match_count": retrieval_matches.len(),
                "matches": retrieval_matches,
                "rewritten_query": rewritten_prompt,
                "summary": retrieval_summary,
                "miss_diagnostics": {
                    "reason": "none",
                    "query_tokens": tokenize(&prompt),
                    "rewritten_query_tokens": rewritten_tokens,
                    "missing_tokens": missing_tokens,
                    "rewrite_applied": !rewrite_reasons.is_empty(),
                    "rewrite_reasons": rewrite_reasons,
                }
            });
        }
    } else if !concise {
        let (fallback_line, fallback_randomness) = retrieval_fallback_text(
            "no_index_loaded",
            preferences.tone,
            concise,
            time_crystal_context.as_ref(),
        );
        retrieval_fallback_randomness = fallback_randomness;
        answer.push_str(&fallback_line);
        answer.push_str("\n\n");
    }

    let mut math_meta = Value::Null;
    let mut has_math_result = false;
    let math_candidate = normalize_chat_math_candidate(&prompt);
    if looks_like_math_expression(&prompt) {
        match evaluate_math_expression(math_candidate, state, MathOptions::default()) {
            Ok(payload) => {
                math_meta = json!({
                    "status": "ok",
                    "error_code": Value::Null,
                    "error_message": Value::Null,
                    "expression": math_candidate,
                    "result": payload.get("result").cloned().unwrap_or(Value::Null),
                    "result_latex": payload.get("result_latex").cloned().unwrap_or(Value::Null),
                    "phase_signature": payload.get("phase_signature").cloned().unwrap_or(Value::Null)
                });
                has_math_result = true;
                if let Some(result_latex) = payload.get("result_latex").and_then(Value::as_str) {
                    answer.push_str("Math result:\n");
                    answer.push_str(result_latex);
                    answer.push_str("\n\n");
                }
            }
            Err(e) => {
                let (status, code) = classify_math_error(&e);
                math_meta = json!({
                    "status": status,
                    "error_code": code,
                    "error_message": e,
                    "expression": math_candidate,
                    "result": Value::Null,
                    "result_latex": Value::Null,
                    "phase_signature": Value::Null,
                });
                answer.push_str("Math evaluation failed: ");
                answer.push_str(&e);
                answer.push_str("\n");
                answer.push_str("Try a valid function or expression and I will evaluate it deterministically.\n\n");
            }
        }
    }

    if !has_retrieval_hits && !has_math_result {
        answer.push_str(capability_hint_text(concise));
        answer.push_str("\n\n");
    }

    let suggestions = follow_up_suggestions(intent, has_retrieval_hits, has_math_result, concise);
    let (next_options_heading_text, next_options_randomness) =
        next_options_heading(preferences.tone, time_crystal_context.as_ref());
    answer.push_str(&next_options_heading_text);
    answer.push('\n');
    for item in &suggestions {
        answer.push_str("- ");
        answer.push_str(item);
        answer.push('\n');
    }

    let meta = json!({
        "schema_version": "csif_chat_meta_v1",
        "generated_at": unix_time_secs(),
        "mode": "deterministic_local_semantic_guard",
        "prompt": prompt,
        "context": {
            "recent_user_prompts": context_items
        },
        "bank": bank_meta,
        "retrieval": retrieval_meta,
        "math": math_meta,
        "conversation": {
            "intent": intent,
            "response_style": preferences.response_style,
            "depth": preferences.depth,
            "tone": preferences.tone,
            "warmth_ceiling": preferences.warmth_ceiling,
            "opening_randomness": opening_randomness,
            "retrieval_fallback_randomness": retrieval_fallback_randomness,
            "next_options_randomness": next_options_randomness,
            "retrieval_summary": preferences.retrieval_summary,
            "retrieval_top_k": preferences.retrieval_top_k,
            "suggestions": suggestions,
        }
    });

    (answer, meta)
}

fn coerce_embedding_inputs(input: &Value) -> Result<Vec<String>, String> {
    match input {
        Value::String(s) => Ok(vec![s.clone()]),
        Value::Array(arr) => {
            let mut out = Vec::with_capacity(arr.len());
            for (idx, item) in arr.iter().enumerate() {
                if let Some(s) = item.as_str() {
                    out.push(s.to_string());
                } else {
                    return Err(format!(
                        "input array entry at index {} must be a string",
                        idx
                    ));
                }
            }
            Ok(out)
        }
        _ => Err("input must be a string or array of strings".to_string()),
    }
}

fn embed_text_deterministic(text: &str) -> Vec<f32> {
    let mut vec = vec![0.0_f32; EMBEDDING_DIM];
    if text.is_empty() {
        return vec;
    }

    for (i, b) in text.as_bytes().iter().enumerate() {
        let idx = (i.wrapping_mul(131) + (*b as usize)) % EMBEDDING_DIM;
        let signed = (*b as f32 / 127.5_f32) - 1.0_f32;
        vec[idx] += signed;
    }

    let norm = vec.iter().map(|v| v * v).sum::<f32>().sqrt();
    if norm > 0.0 {
        for v in &mut vec {
            *v /= norm;
        }
    }

    vec
}

async fn health() -> Json<Value> {
    Json(json!({"status": "ok", "service": "csif-agent-rust-openai"}))
}

async fn list_models() -> Json<Value> {
    Json(json!({
        "object": "list",
        "data": [
            {
                "id": OPENAI_MODEL_ID,
                "object": "model",
                "owned_by": "csif-local"
            }
        ]
    }))
}

async fn csif_index(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(index_summary_payload(&state))
}

async fn csif_retrieve(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RetrieveRequest>,
) -> impl IntoResponse {
    let query = req.query.trim();
    if query.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": {
                    "message": "query must not be empty",
                    "type": "invalid_request_error"
                }
            })),
        )
            .into_response();
    }

    let top_k = req.top_k.unwrap_or(5).clamp(1, 50);
    (
        StatusCode::OK,
        Json(retrieval_payload(&state, query, top_k)),
    )
        .into_response()
}

async fn csif_math(
    State(state): State<Arc<AppState>>,
    Json(req): Json<MathRequest>,
) -> impl IntoResponse {
    let expr = req.expression.trim();
    if expr.is_empty() {
        let bridge_audit = build_error_bridge_audit_value(expr, "expression must not be empty", "MATH_PARSE_ERROR", "parse_error");
        return with_cors((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": {
                    "message": "expression must not be empty",
                    "type": "invalid_request_error"
                },
                "bridge_audit": bridge_audit,
            })),
        )
            .into_response());
    }

    let mode = match parse_math_mode(req.mode.as_deref()) {
        Ok(v) => v,
        Err(e) => {
            let bridge_audit = build_error_bridge_audit_value(expr, &e, "MATH_PARSE_ERROR", "parse_error");
            return with_cors((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": {
                        "message": e,
                        "type": "invalid_request_error"
                    },
                    "bridge_audit": bridge_audit,
                })),
            )
                .into_response());
        }
    };

    let angle_unit = match parse_angle_unit(req.angle_unit.as_deref()) {
        Ok(v) => v,
        Err(e) => {
            let bridge_audit = build_error_bridge_audit_value(expr, &e, "MATH_PARSE_ERROR", "parse_error");
            return with_cors((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": {
                        "message": e,
                        "type": "invalid_request_error"
                    },
                    "bridge_audit": bridge_audit,
                })),
            )
                .into_response());
        }
    };

    let options = MathOptions { mode, angle_unit };

    match evaluate_math_expression(expr, &state, options) {
        Ok(payload) => with_cors((StatusCode::OK, Json(payload)).into_response()),
        Err(e) => {
            let (status, error_code) = classify_math_error(&e);
            let bridge_audit = build_error_bridge_audit_value(expr, &e, error_code, status);
            with_cors((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": {
                        "message": e,
                        "type": "invalid_request_error",
                        "status": status,
                        "code": error_code,
                    }
                    ,"bridge_audit": bridge_audit
                })),
            )
                .into_response())
        }
    }
}

async fn csif_disambiguate(
    State(state): State<Arc<AppState>>,
    Json(req): Json<DisambiguateRequest>,
) -> impl IntoResponse {
    let token = req.token.trim();
    if token.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": {
                    "message": "token must not be empty",
                    "type": "invalid_request_error"
                }
            })),
        )
            .into_response();
    }

    let language = req.language.unwrap_or_else(|| "en".to_string());
    let context = req.context.unwrap_or_default();
    let top_k = req.top_k.unwrap_or(5).clamp(1, 20);
    let margin_threshold = req.margin.unwrap_or(0.75).max(0.0);
    let inertia_coefficient = req.inertia_coefficient.unwrap_or(0.35).max(0.0);
    let sandbox_on_inertia_block = req.sandbox_on_inertia_block.unwrap_or(true);
    let active_frame = resolve_frame_context(req.frame.as_ref());
    let prior_frame = resolve_frame_context(req.prior_frame.as_ref());
    let conservation_policy = resolve_conservation_policy(req.conservation_policy.as_ref());
    let lexicon_control = resolve_lexicon_control(req.lexicon_packs.as_ref());

    let mut prior_events = Vec::<Value>::new();
    if let Some(path) = state.sense_trajectory_log_path.as_ref() {
        if let Ok(events) = read_sense_trajectory_events(path) {
            prior_events = events;
        }
    }

    let inertia_profile = build_lexeme_inertia_profile(&prior_events, &language, token);
    let mut payload = disambiguate_payload_with_inertia_and_frame_and_policy(
        &state,
        &language,
        token,
        &context,
        top_k,
        margin_threshold,
        Some(&inertia_profile),
        inertia_coefficient,
        sandbox_on_inertia_block,
        &active_frame,
        &prior_frame,
        &conservation_policy,
        &lexicon_control,
    );

    if let Some(path) = state.sense_trajectory_log_path.as_ref() {
        let previous_event = latest_event_for_lexeme(&prior_events, &language, token);
        let event = build_disambiguation_event(
            &payload,
            &language,
            token,
            &context,
            top_k,
            margin_threshold,
            previous_event.as_ref(),
        );

        if let Some(obj) = payload.as_object_mut() {
            obj.insert(
                "trajectory_event".to_string(),
                json!({
                    "persisted": append_sense_trajectory_event(path, &event).is_ok(),
                    "schema_version": "csif_disambiguation_event_v1",
                    "event": event
                }),
            );
        }
    }

    (StatusCode::OK, Json(payload)).into_response()
}

async fn csif_disambiguation_trajectories(
    State(state): State<Arc<AppState>>,
    Query(query): Query<TrajectoryQuery>,
) -> impl IntoResponse {
    let token = query.token.as_deref().map(str::trim).filter(|v| !v.is_empty());
    let language = query
        .language
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let limit = query.limit.unwrap_or(25).clamp(1, 500);

    match trajectory_events_payload(&state, language, token, limit) {
        Ok(payload) => (StatusCode::OK, Json(payload)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": {
                    "message": e,
                    "type": "server_error"
                }
            })),
        )
            .into_response(),
    }
}

async fn csif_disambiguation_summary(
    State(state): State<Arc<AppState>>,
    Query(query): Query<TrajectorySummaryQuery>,
) -> impl IntoResponse {
    let token = query.token.as_deref().map(str::trim).filter(|v| !v.is_empty());
    let language = query
        .language
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty());
    let limit = query.limit.unwrap_or(200).clamp(1, 5000);

    match filtered_trajectory_events(&state, language, token, limit) {
        Ok(events) => (
            StatusCode::OK,
            Json(json!({
                "object": "csif.disambiguation.trajectory.summary",
                "schema_version": "csif_disambiguation_summary_v1",
                "deterministic": true,
                "filters": {
                    "language": language,
                    "token": token,
                    "limit": limit,
                },
                "metrics": summarize_trajectory_events(&events),
            })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": {
                    "message": e,
                    "type": "server_error"
                }
            })),
        )
            .into_response(),
    }
}

async fn csif_simulate(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SimulateRequest>,
) -> impl IntoResponse {
    let token = req.token.trim();
    if token.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": {
                    "message": "token must not be empty",
                    "type": "invalid_request_error"
                }
            })),
        )
            .into_response();
    }

    let language = req.language.unwrap_or_else(|| "en".to_string());
    let context = req.context.unwrap_or_default();
    let top_k = req.top_k.unwrap_or(5).clamp(1, 20);
    let margin_threshold = req.margin.unwrap_or(0.75).max(0.0);
    let inertia_coefficient = req.inertia_coefficient.unwrap_or(0.35).max(0.0);
    let branch_limit = req.branch_limit.unwrap_or(3).clamp(1, 8);
    let active_frame = resolve_frame_context(req.frame.as_ref());
    let prior_frame = resolve_frame_context(req.prior_frame.as_ref());
    let conservation_policy = resolve_conservation_policy(req.conservation_policy.as_ref());
    let lexicon_control = resolve_lexicon_control(req.lexicon_packs.as_ref());

    let mut prior_events = Vec::<Value>::new();
    if let Some(path) = state.sense_trajectory_log_path.as_ref() {
        if let Ok(events) = read_sense_trajectory_events(path) {
            prior_events = events;
        }
    }

    let inertia_profile = build_lexeme_inertia_profile(&prior_events, &language, token);
    let payload = disambiguate_payload_with_inertia_and_frame_and_policy(
        &state,
        &language,
        token,
        &context,
        top_k,
        margin_threshold,
        Some(&inertia_profile),
        inertia_coefficient,
        true,
        &active_frame,
        &prior_frame,
        &conservation_policy,
        &lexicon_control,
    );

    let simulation = build_sandbox_simulation(
        &payload,
        &language,
        token,
        &context,
        top_k,
        margin_threshold,
        inertia_coefficient,
        branch_limit,
        req.forced_sense_node.as_deref(),
    );

    (StatusCode::OK, Json(simulation)).into_response()
}

async fn csif_reconcile(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ReconcileRequest>,
) -> impl IntoResponse {
    let token = req.token.trim();
    if token.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": {
                    "message": "token must not be empty",
                    "type": "invalid_request_error"
                }
            })),
        )
            .into_response();
    }

    let language = req.language.unwrap_or_else(|| "en".to_string());
    let context = req.context.unwrap_or_default();
    let top_k = req.top_k.unwrap_or(5).clamp(1, 20);
    let margin_threshold = req.margin.unwrap_or(0.75).max(0.0);
    let inertia_coefficient = req.inertia_coefficient.unwrap_or(0.35).max(0.0);
    let branch_limit = req.branch_limit.unwrap_or(3).clamp(2, 8);
    let active_frame = resolve_frame_context(req.frame.as_ref());
    let prior_frame = resolve_frame_context(req.prior_frame.as_ref());
    let conservation_policy = resolve_conservation_policy(req.conservation_policy.as_ref());
    let lexicon_control = resolve_lexicon_control(req.lexicon_packs.as_ref());

    let mut prior_events = Vec::<Value>::new();
    if let Some(path) = state.sense_trajectory_log_path.as_ref() {
        if let Ok(events) = read_sense_trajectory_events(path) {
            prior_events = events;
        }
    }

    let inertia_profile = build_lexeme_inertia_profile(&prior_events, &language, token);
    let payload = disambiguate_payload_with_inertia_and_frame_and_policy(
        &state,
        &language,
        token,
        &context,
        top_k,
        margin_threshold,
        Some(&inertia_profile),
        inertia_coefficient,
        true,
        &active_frame,
        &prior_frame,
        &conservation_policy,
        &lexicon_control,
    );
    let simulation = build_sandbox_simulation(
        &payload,
        &language,
        token,
        &context,
        top_k,
        margin_threshold,
        inertia_coefficient,
        branch_limit,
        req.forced_sense_node.as_deref(),
    );

    match build_reconciliation_payload(&simulation, req.losing_branch_id.as_deref()) {
        Ok(payload) => (StatusCode::OK, Json(payload)).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": {
                    "message": e,
                    "type": "invalid_request_error"
                }
            })),
        )
            .into_response(),
    }
}

async fn chat_completions(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ChatCompletionsRequest>,
) -> impl IntoResponse {
    if req.messages.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": {
                    "message": "messages must not be empty",
                    "type": "invalid_request_error"
                }
            })),
        )
            .into_response();
    }

    let model = req.model.unwrap_or_else(|| OPENAI_MODEL_ID.to_string());
    let prompt = last_user_prompt(&req.messages);
    let (answer, csif_meta) = build_chat_answer(&req.messages, &state, req.preferences.as_ref());
    let prompt_tokens = token_count(&prompt);
    let completion_tokens = token_count(&answer);

    (
        StatusCode::OK,
        Json(json!({
            "id": format!("chatcmpl-{}", unix_time_secs()),
            "object": "chat.completion",
            "created": unix_time_secs(),
            "model": model,
            "choices": [
                {
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": answer
                    },
                    "finish_reason": "stop"
                }
            ],
            "usage": {
                "prompt_tokens": prompt_tokens,
                "completion_tokens": completion_tokens,
                "total_tokens": prompt_tokens + completion_tokens
            },
            "csif_meta": csif_meta
        })),
    )
        .into_response()
}

async fn embeddings(Json(req): Json<EmbeddingsRequest>) -> impl IntoResponse {
    let inputs = match coerce_embedding_inputs(&req.input) {
        Ok(v) if !v.is_empty() => v,
        Ok(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": {
                        "message": "input must not be empty",
                        "type": "invalid_request_error"
                    }
                })),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": {
                        "message": e,
                        "type": "invalid_request_error"
                    }
                })),
            )
                .into_response();
        }
    };

    let model = req.model.unwrap_or_else(|| OPENAI_MODEL_ID.to_string());
    let prompt_tokens = inputs.iter().map(|s| token_count(s)).sum::<usize>();

    let data = inputs
        .iter()
        .enumerate()
        .map(|(idx, text)| {
            json!({
                "object": "embedding",
                "index": idx,
                "embedding": embed_text_deterministic(text),
            })
        })
        .collect::<Vec<_>>();

    (
        StatusCode::OK,
        Json(json!({
            "object": "list",
            "data": data,
            "model": model,
            "usage": {
                "prompt_tokens": prompt_tokens,
                "total_tokens": prompt_tokens,
            }
        })),
    )
        .into_response()
}

fn parse_serve_args(args: &[String]) -> Result<ServeConfig, String> {
    let mut host = "127.0.0.1".to_string();
    let mut port = 8080u16;
    let mut bank_path: Option<String> = None;
    let mut sense_log_path: Option<String> = None;

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--host" => {
                let Some(v) = args.get(i + 1) else {
                    return Err("--host requires a value".to_string());
                };
                host = v.clone();
                i += 2;
            }
            "--port" => {
                let Some(v) = args.get(i + 1) else {
                    return Err("--port requires a value".to_string());
                };
                port = v
                    .parse::<u16>()
                    .map_err(|_| format!("invalid --port value: {}", v))?;
                i += 2;
            }
            "--bank-path" => {
                let Some(v) = args.get(i + 1) else {
                    return Err("--bank-path requires a value".to_string());
                };
                bank_path = Some(v.clone());
                i += 2;
            }
            "--sense-log-path" => {
                let Some(v) = args.get(i + 1) else {
                    return Err("--sense-log-path requires a value".to_string());
                };
                sense_log_path = Some(v.clone());
                i += 2;
            }
            other => {
                return Err(format!("unknown serve-openai option: {}", other));
            }
        }
    }

    Ok(ServeConfig {
        host,
        port,
        bank_path,
        sense_log_path,
    })
}

async fn run_openai_server(cfg: ServeConfig) -> Result<(), String> {
    let (bank_summary, bank_index) = if let Some(path) = cfg.bank_path.as_ref() {
        let bank = read_json(path)?;
        let (errors, warnings) = validate_bank(&bank);
        if !errors.is_empty() {
            return Err(format!(
                "bank validation failed at startup: {}",
                errors.join("; ")
            ));
        }
        if !warnings.is_empty() {
            eprintln!("bank validation warnings at startup: {}", warnings.join("; "));
        }
        (
            Some(summarize_bank(&bank)),
            Some(Arc::new(build_bank_index(&bank))),
        )
    } else {
        (None, None)
    };

    let sense_trajectory_log_path = cfg.sense_log_path.clone().or_else(|| {
        cfg.bank_path
            .as_ref()
            .map(|p| format!("{}.sense_trajectories.jsonl", p))
    });

    let state = Arc::new(AppState {
        bank_summary,
        bank_index,
        sense_trajectory_log_path,
    });
    let app = Router::new()
        .route("/health", get(health))
        .route("/v1/models", get(list_models))
        .route("/v1/chat/completions", post(chat_completions))
        .route("/v1/embeddings", post(embeddings))
        .route("/v1/csif/index", get(csif_index))
        .route("/v1/csif/retrieve", post(csif_retrieve))
        .route("/v1/csif/math", post(csif_math).options(csif_math_options))
        .route("/v1/csif/disambiguate", post(csif_disambiguate))
        .route("/v1/csif/simulate", post(csif_simulate))
        .route("/v1/csif/reconcile", post(csif_reconcile))
        .route(
            "/v1/csif/disambiguation/trajectories",
            get(csif_disambiguation_trajectories),
        )
        .route(
            "/v1/csif/disambiguation/summary",
            get(csif_disambiguation_summary),
        )
        .with_state(state);

    let addr = format!("{}:{}", cfg.host, cfg.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| format!("failed binding {}: {}", addr, e))?;
    println!("OpenAI-compatible server listening on http://{}", addr);
    axum::serve(listener, app)
        .await
        .map_err(|e| format!("server error: {}", e))
}

fn signed_i8_range() -> Value {
    json!({"min": -127, "max": 127})
}

fn read_json(path: &str) -> Result<Value, String> {
    let content = fs::read_to_string(path).map_err(|e| format!("failed reading {}: {}", path, e))?;
    serde_json::from_str(&content).map_err(|e| format!("failed parsing {}: {}", path, e))
}

fn write_json(path: &str, value: &Value) -> Result<(), String> {
    let pretty = serde_json::to_string_pretty(value).map_err(|e| format!("failed serializing json: {}", e))?;
    fs::write(path, format!("{}\n", pretty)).map_err(|e| format!("failed writing {}: {}", path, e))
}

fn as_object_mut(value: &mut Value) -> Option<&mut Map<String, Value>> {
    if let Value::Object(map) = value {
        Some(map)
    } else {
        None
    }
}

fn set_default(map: &mut Map<String, Value>, key: &str, val: Value) {
    if !map.contains_key(key) {
        map.insert(key.to_string(), val);
    }
}

fn migrate_event_v2(event: &Value) -> Value {
    let mut out = event.clone();
    let Some(obj) = as_object_mut(&mut out) else {
        return event.clone();
    };

    set_default(obj, "schema_version", Value::String(RWIF_EVENT_SCHEMA_VERSION.to_string()));
    set_default(obj, "state_encoding", Value::String("signed_i8_plus_intent_v2".to_string()));
    set_default(obj, "quantization_step", json!(1));
    set_default(obj, "amplitude_signed", Value::Null);
    set_default(obj, "intent_signed", Value::Null);
    if !obj.contains_key("phase_theta") {
        if let Some(phase) = obj.get("phase").cloned() {
            obj.insert("phase_theta".to_string(), phase);
        } else {
            obj.insert("phase_theta".to_string(), Value::Null);
        }
    }
    set_default(obj, "phase_omega", Value::Null);
    set_default(obj, "monotonic_index", Value::Null);
    out
}

fn migrate_edge_v2(edge: &Value) -> Value {
    let mut out = edge.clone();
    let Some(obj) = as_object_mut(&mut out) else {
        return edge.clone();
    };

    if let Some(Value::Array(events)) = obj.get_mut("phase_trajectory") {
        let migrated = events.iter().map(migrate_event_v2).collect::<Vec<_>>();
        *events = migrated;
    } else {
        obj.insert("phase_trajectory".to_string(), Value::Array(vec![]));
    }

    set_default(obj, "schema_version", Value::String(RWIF_EDGE_SCHEMA_VERSION.to_string()));
    set_default(obj, "state_encoding", Value::String("phase_scalar_v1".to_string()));
    set_default(
        obj,
        "numeric_range",
        json!({
            "amplitude": signed_i8_range(),
            "intent": signed_i8_range()
        }),
    );
    set_default(obj, "wrap_mode", Value::String("principal_pi".to_string()));
    set_default(obj, "integer_wrap_mode", Value::String("clamp".to_string()));
    set_default(obj, "integration_rule", Value::String("legacy_scalar".to_string()));
    out
}

fn migrate_crystal_v2(crystal: &Value) -> Value {
    let mut out = crystal.clone();
    let Some(obj) = as_object_mut(&mut out) else {
        return crystal.clone();
    };

    if let Some(Value::Array(edges)) = obj.get_mut("edges") {
        let migrated = edges.iter().map(migrate_edge_v2).collect::<Vec<_>>();
        *edges = migrated;
    } else {
        obj.insert("edges".to_string(), Value::Array(vec![]));
    }

    set_default(obj, "rwif_schema_version", Value::String(RWIF_SCHEMA_VERSION.to_string()));
    out
}

fn migrate_bank_v2(bank: &Value) -> Value {
    let mut out = bank.clone();
    let Some(obj) = as_object_mut(&mut out) else {
        return bank.clone();
    };

    if let Some(Value::Array(crystals)) = obj.get_mut("crystals") {
        let migrated = crystals.iter().map(migrate_crystal_v2).collect::<Vec<_>>();
        *crystals = migrated;
    } else {
        obj.insert("crystals".to_string(), Value::Array(vec![]));
    }

    obj.insert(
        "rwif_schema_version".to_string(),
        Value::String(RWIF_SCHEMA_VERSION.to_string()),
    );
    out
}

fn validate_bank(bank: &Value) -> (Vec<String>, Vec<String>) {
    let mut errors = Vec::<String>::new();
    let mut warnings = Vec::<String>::new();

    let Some(obj) = bank.as_object() else {
        errors.push("bank payload is not a JSON object".to_string());
        return (errors, warnings);
    };

    if !obj.contains_key("bank_id") {
        errors.push("missing bank_id".to_string());
    }

    let Some(crystals) = obj.get("crystals").and_then(Value::as_array) else {
        errors.push("missing or invalid crystals list".to_string());
        return (errors, warnings);
    };

    if obj
        .get("rwif_schema_version")
        .and_then(Value::as_str)
        != Some(RWIF_SCHEMA_VERSION)
    {
        warnings.push(format!(
            "bank rwif_schema_version is {:?}, expected {:?}",
            obj.get("rwif_schema_version"),
            RWIF_SCHEMA_VERSION
        ));
    }

    for crystal in crystals {
        let Some(cobj) = crystal.as_object() else {
            errors.push("crystal entry is not an object".to_string());
            continue;
        };

        let cid = cobj
            .get("crystal_id")
            .and_then(Value::as_str)
            .unwrap_or("<unknown_crystal>");

        if cobj
            .get("rwif_schema_version")
            .and_then(Value::as_str)
            != Some(RWIF_SCHEMA_VERSION)
        {
            warnings.push(format!(
                "{}: crystal rwif_schema_version not {}",
                cid, RWIF_SCHEMA_VERSION
            ));
        }

        let Some(edges) = cobj.get("edges").and_then(Value::as_array) else {
            errors.push(format!("{}: missing edges list", cid));
            continue;
        };

        for edge in edges {
            let Some(eobj) = edge.as_object() else {
                errors.push(format!("{}: edge entry is not an object", cid));
                continue;
            };
            let eid = eobj
                .get("edge_id")
                .and_then(Value::as_str)
                .unwrap_or("<unknown_edge>");

            if eobj.get("schema_version").and_then(Value::as_str) != Some(RWIF_EDGE_SCHEMA_VERSION) {
                warnings.push(format!(
                    "{}/{}: edge schema_version not {}",
                    cid, eid, RWIF_EDGE_SCHEMA_VERSION
                ));
            }

            if !eobj.contains_key("integer_wrap_mode") {
                errors.push(format!("{}/{}: missing integer_wrap_mode", cid, eid));
            }

            if let Some(events) = eobj.get("phase_trajectory").and_then(Value::as_array) {
                for event in events {
                    let Some(ev) = event.as_object() else {
                        errors.push(format!("{}/{}: event is not an object", cid, eid));
                        continue;
                    };

                    if ev.get("schema_version").and_then(Value::as_str) != Some(RWIF_EVENT_SCHEMA_VERSION) {
                        warnings.push(format!(
                            "{}/{}: event schema_version not {}",
                            cid, eid, RWIF_EVENT_SCHEMA_VERSION
                        ));
                    }
                    if !ev.contains_key("state_encoding") {
                        errors.push(format!("{}/{}: event missing state_encoding", cid, eid));
                    }
                    if !ev.contains_key("quantization_step") {
                        errors.push(format!("{}/{}: event missing quantization_step", cid, eid));
                    }
                    if let Some(mon) = ev.get("monotonic_index") {
                        if !mon.is_null() && !mon.is_i64() && !mon.is_u64() {
                            errors.push(format!("{}/{}: event monotonic_index must be int or null", cid, eid));
                        }
                    }
                }
            }
        }
    }

    (errors, warnings)
}

fn layer0_policy_default() -> Layer0Policy {
    Layer0Policy {
        tau_accept: 0.8,
        tau_reject: 0.2,
    }
}

fn layer0_analyze(graph: &Layer0Graph, policy: &Layer0Policy) -> Layer0Report {
    let mut contradictions = Vec::<Layer0Issue>::new();
    let mut warnings = Vec::<Layer0Issue>::new();

    let node_ids = graph
        .nodes
        .iter()
        .map(|n| n.node_id.as_str())
        .collect::<HashSet<_>>();

    let mut before_pairs = HashSet::<(String, String)>::new();
    let mut causes_pairs = Vec::<(String, String)>::new();
    let mut same_as_pairs = HashSet::<(String, String)>::new();
    let mut diff_pairs = HashSet::<(String, String)>::new();
    let mut anchored_nodes = HashSet::<String>::new();
    let mut has_unknown_support = false;
    let mut has_negation_conflict = false;

    for edge in &graph.edges {
        let _ = &edge.provenance;
        if !(0.0..=1.0).contains(&edge.confidence_band) {
            contradictions.push(Layer0Issue {
                code: "confidence_band_out_of_range".to_string(),
                message: format!(
                    "edge {} has confidence_band {} outside [0.0,1.0]",
                    edge.edge_id, edge.confidence_band
                ),
            });
        }

        if !node_ids.contains(edge.source_node.as_str()) {
            contradictions.push(Layer0Issue {
                code: "missing_source_node".to_string(),
                message: format!("edge {} references unknown source {}", edge.edge_id, edge.source_node),
            });
        }
        if !node_ids.contains(edge.target_node.as_str()) {
            contradictions.push(Layer0Issue {
                code: "missing_target_node".to_string(),
                message: format!("edge {} references unknown target {}", edge.edge_id, edge.target_node),
            });
        }

        match edge.relation.as_str() {
            "located_in" | "occurs_in" => {
                anchored_nodes.insert(edge.source_node.clone());
            }
            "before" => {
                before_pairs.insert((edge.source_node.clone(), edge.target_node.clone()));
            }
            "causes" => {
                causes_pairs.push((edge.source_node.clone(), edge.target_node.clone()));
            }
            "same_as" => {
                let key = ordered_pair(&edge.source_node, &edge.target_node);
                same_as_pairs.insert(key);
            }
            "different_from" => {
                let key = ordered_pair(&edge.source_node, &edge.target_node);
                diff_pairs.insert(key);
            }
            "part_of" => {
                if edge.source_node == edge.target_node {
                    contradictions.push(Layer0Issue {
                        code: "illegal_self_part".to_string(),
                        message: format!("part_of({}, {}) is not allowed", edge.source_node, edge.target_node),
                    });
                }
            }
            "negates" => {
                if edge.confidence_band >= policy.tau_accept {
                    has_negation_conflict = true;
                }
            }
            "measures" => {
                if edge.confidence_band <= policy.tau_reject {
                    has_unknown_support = true;
                }
            }
            _ => {}
        }
    }

    if has_temporal_cycle(&before_pairs) {
        contradictions.push(Layer0Issue {
            code: "temporal_cycle".to_string(),
            message: "before relation contains a cycle".to_string(),
        });
    }

    for pair in same_as_pairs {
        if diff_pairs.contains(&pair) {
            contradictions.push(Layer0Issue {
                code: "identity_conflict".to_string(),
                message: format!("same_as and different_from both asserted for {} and {}", pair.0, pair.1),
            });
        }
    }

    for (a, b) in causes_pairs {
        if before_pairs.contains(&(b.clone(), a.clone())) {
            contradictions.push(Layer0Issue {
                code: "causal_inversion".to_string(),
                message: format!("causes({}, {}) conflicts with before({}, {})", a, b, b, a),
            });
        }
    }

    for node in &graph.nodes {
        let _ = (&node.node_type, &node.label, &node.provenance);
        if !anchored_nodes.contains(&node.node_id) {
            warnings.push(Layer0Issue {
                code: "existence_anchoring_missing".to_string(),
                message: format!("node {} has no located_in/occurs_in anchoring", node.node_id),
            });
        }
    }

    if has_negation_conflict {
        warnings.push(Layer0Issue {
            code: "negation_consistency_review".to_string(),
            message: "committed negation edge detected; verify no co-committed positives in context window"
                .to_string(),
        });
    }

    let valid = contradictions.is_empty();
    let stop_reason = if valid {
        "path_found".to_string()
    } else {
        "contradiction_detected".to_string()
    };
    let verdict = if !contradictions.is_empty() {
        "NO".to_string()
    } else if has_unknown_support {
        "NEEDS_INPUT".to_string()
    } else {
        "YES".to_string()
    };

    Layer0Report {
        valid,
        contradictions,
        warnings,
        stop_reason,
        verdict,
        node_count: graph.nodes.len(),
        edge_count: graph.edges.len(),
    }
}

fn has_temporal_cycle(before_pairs: &HashSet<(String, String)>) -> bool {
    let mut adjacency = HashMap::<String, Vec<String>>::new();
    let mut indegree = HashMap::<String, usize>::new();

    for (a, b) in before_pairs {
        adjacency.entry(a.clone()).or_default().push(b.clone());
        indegree.entry(a.clone()).or_insert(0);
        *indegree.entry(b.clone()).or_insert(0) += 1;
    }

    let mut queue = VecDeque::<String>::new();
    for (node, degree) in &indegree {
        if *degree == 0 {
            queue.push_back(node.clone());
        }
    }

    let mut visited = 0usize;
    while let Some(node) = queue.pop_front() {
        visited += 1;
        for nxt in adjacency.get(&node).cloned().unwrap_or_default() {
            if let Some(d) = indegree.get_mut(&nxt) {
                *d -= 1;
                if *d == 0 {
                    queue.push_back(nxt);
                }
            }
        }
    }

    visited != indegree.len()
}

fn ordered_pair(a: &str, b: &str) -> (String, String) {
    if a <= b {
        (a.to_string(), b.to_string())
    } else {
        (b.to_string(), a.to_string())
    }
}

fn layer0_conformance_case(case_id: &str) -> Option<(Layer0Graph, bool, &'static str)> {
    let make_graph = |nodes: Vec<&str>, edges: Vec<(&str, &str, &str, &str, f64)>| -> Layer0Graph {
        Layer0Graph {
            layer0_version: Some("0.2".to_string()),
            nodes: nodes
                .into_iter()
                .map(|id| Layer0Node {
                    node_id: id.to_string(),
                    node_type: "entity".to_string(),
                    label: id.to_string(),
                    provenance: json!({"source": "conformance"}),
                })
                .collect(),
            edges: edges
                .into_iter()
                .map(|(eid, s, r, t, c)| Layer0Edge {
                    edge_id: eid.to_string(),
                    source_node: s.to_string(),
                    relation: r.to_string(),
                    target_node: t.to_string(),
                    confidence_band: c,
                    provenance: json!({"source": "conformance"}),
                })
                .collect(),
        }
    };

    match case_id {
        "C-001" => Some((
            make_graph(
                vec!["event_birth", "event_walk", "event_rest", "interval_t"],
                vec![
                    ("e1", "event_birth", "before", "event_walk", 1.0),
                    ("e2", "event_walk", "before", "event_rest", 1.0),
                    ("e3", "event_birth", "occurs_in", "interval_t", 1.0),
                ],
            ),
            true,
            "temporal order valid",
        )),
        "C-002" => Some((
            make_graph(
                vec!["a", "b", "interval_t"],
                vec![
                    ("e1", "a", "before", "b", 1.0),
                    ("e2", "b", "before", "a", 1.0),
                    ("e3", "a", "occurs_in", "interval_t", 1.0),
                ],
            ),
            false,
            "temporal cycle rejected",
        )),
        "C-004" => Some((
            make_graph(
                vec!["x", "y", "interval_t"],
                vec![
                    ("e1", "x", "same_as", "y", 1.0),
                    ("e2", "x", "different_from", "y", 1.0),
                    ("e3", "x", "occurs_in", "interval_t", 1.0),
                ],
            ),
            false,
            "identity conflict",
        )),
        "C-006" => Some((
            make_graph(
                vec!["event_a", "event_b", "interval_t"],
                vec![
                    ("e1", "event_a", "causes", "event_b", 1.0),
                    ("e2", "event_b", "before", "event_a", 1.0),
                    ("e3", "event_a", "occurs_in", "interval_t", 1.0),
                ],
            ),
            false,
            "causal inversion rejected",
        )),
        "C-008" => Some((
            make_graph(
                vec!["x", "region_a"],
                vec![
                    ("e1", "x", "part_of", "x", 1.0),
                    ("e2", "x", "located_in", "region_a", 1.0),
                ],
            ),
            false,
            "illegal self part",
        )),
        "C-012" => Some((
            make_graph(
                vec!["quantity_temp", "reactor_1", "region_lab"],
                vec![
                    ("e1", "reactor_1", "located_in", "region_lab", 1.0),
                    ("e2", "quantity_temp", "measures", "reactor_1", 0.0),
                ],
            ),
            true,
            "unknown handling",
        )),
        "C-013" => Some((
            make_graph(
                vec!["assertion_light_on", "light_2", "state_on", "region_room"],
                vec![
                    ("e1", "light_2", "has_state", "state_on", 1.0),
                    ("e2", "assertion_light_on", "negates", "assertion_light_on", 1.0),
                    ("e3", "light_2", "located_in", "region_room", 1.0),
                ],
            ),
            true,
            "negation consistency review",
        )),
        _ => None,
    }
}

fn layer0_run_conformance(selected_case: Option<&str>) -> Value {
    let case_ids = vec!["C-001", "C-002", "C-004", "C-006", "C-008", "C-012", "C-013"];
    let mut results = Vec::<Value>::new();
    let mut pass_count = 0usize;
    let mut fail_count = 0usize;
    let policy = layer0_policy_default();

    for case_id in case_ids {
        if selected_case.is_some() && selected_case != Some(case_id) {
            continue;
        }
        if let Some((graph, should_pass, label)) = layer0_conformance_case(case_id) {
            let report = layer0_analyze(&graph, &policy);
            let passed = report.valid == should_pass;
            if passed {
                pass_count += 1;
            } else {
                fail_count += 1;
            }
            results.push(json!({
                "case_id": case_id,
                "label": label,
                "expected_valid": should_pass,
                "actual_valid": report.valid,
                "passed": passed,
                "stop_reason": report.stop_reason,
                "verdict": report.verdict,
                "contradictions": report.contradictions,
                "warnings": report.warnings,
            }));
        }
    }

    json!({
        "object": "csif.layer0.conformance.report",
        "layer0_version": "0.2",
        "pass_count": pass_count,
        "fail_count": fail_count,
        "results": results,
    })
}

fn print_help(bin: &str) {
    println!("CSIF-Agent v2 Rust CLI\n");
    println!("Usage:");
    println!("  {} validate-bank <bank_path>", bin);
    println!("  {} migrate-bank <input_path> <output_path>", bin);
    println!("  {} index-bank <bank_path> <output_path>", bin);
    println!("  {} layer0-check <graph_path>", bin);
    println!("  {} layer0-conformance [--case C-001]", bin);
    println!("  {} math-eval [--mode algebraic|geometric] [--angle-unit radians|degrees] <expression>", bin);
    println!("  {} serve-openai [--host 127.0.0.1] [--port 8080] [--bank-path /path/to/bank.json]", bin);
    println!("  {} benchmark-determinism [--iterations 100]", bin);
}

fn percentile_ms(samples_ms: &[f64], pct: f64) -> f64 {
    if samples_ms.is_empty() {
        return 0.0;
    }
    let mut sorted = samples_ms.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let idx = ((sorted.len() as f64 - 1.0) * pct).round() as usize;
    sorted.get(idx).copied().unwrap_or(0.0)
}

fn canonicalize_json_value(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<_> = map.iter().collect();
            entries.sort_by(|(ka, _), (kb, _)| ka.cmp(kb));
            let mut canonical = serde_json::Map::new();
            for (k, v) in entries {
                canonical.insert(k.clone(), canonicalize_json_value(v));
            }
            Value::Object(canonical)
        }
        Value::Array(arr) => {
            let mut normalized = arr
                .iter()
                .map(canonicalize_json_value)
                .collect::<Vec<_>>();
            normalized.sort_by(|a, b| {
                let sa = serde_json::to_string(a).unwrap_or_default();
                let sb = serde_json::to_string(b).unwrap_or_default();
                sa.cmp(&sb)
            });
            Value::Array(normalized)
        }
        _ => value.clone(),
    }
}

fn value_hash(v: &Value) -> u64 {
    let canonical = canonicalize_json_value(v);
    let serialized = serde_json::to_string(&canonical).unwrap_or_else(|_| "{}".to_string());
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    serialized.hash(&mut hasher);
    hasher.finish()
}

fn benchmark_determinism(iterations: usize) -> Value {
    let bank = json!({
        "bank_id": "bench-bank",
        "rwif_schema_version": "RWIF_V2",
        "crystals": [{
            "crystal_id": "bench-c1",
            "rwif_schema_version": "RWIF_V2",
            "edges": [{
                "edge_id": "bench-e1",
                "source_node": "light",
                "relation": "dispels",
                "target_node": "darkness",
                "schema_version": "RWIF_EDGE_V2",
                "integer_wrap_mode": "clamp",
                "phase_trajectory": [{
                    "schema_version": "RWIF_EVENT_V2",
                    "state_encoding": "signed_i8_plus_intent_v2",
                    "quantization_step": 1,
                    "monotonic_index": null
                }]
            }]
        }]
    });

    let state = AppState {
        bank_summary: Some(summarize_bank(&bank)),
        bank_index: Some(Arc::new(build_bank_index(&bank))),
        sense_trajectory_log_path: None,
    };

    let run_case = |name: &str, f: &mut dyn FnMut() -> Value| -> Value {
        let mut samples = Vec::<f64>::new();
        let mut first_hash: Option<u64> = None;
        let mut all_hashes_equal = true;
        for _ in 0..iterations {
            let start = Instant::now();
            let payload = f();
            let elapsed = start.elapsed().as_secs_f64() * 1000.0;
            samples.push(elapsed);
            let h = value_hash(&payload);
            if let Some(prev) = first_hash {
                if prev != h {
                    all_hashes_equal = false;
                }
            } else {
                first_hash = Some(h);
            }
        }

        json!({
            "name": name,
            "iterations": iterations,
            "latency_ms": {
                "p50": percentile_ms(&samples, 0.50),
                "p95": percentile_ms(&samples, 0.95),
            },
            "output_hash": first_hash.unwrap_or(0),
            "deterministic_hash_stable": all_hashes_equal,
        })
    };

    let mut case_math = || {
        evaluate_math_expression(
            "((2+3i)^7 + conj(5-8i)^3 + exp(i*pi))/sqrt(7)",
            &state,
            MathOptions::default(),
        )
        .unwrap_or_else(|e| json!({"error": e}))
    };

    let mut case_retrieve = || retrieval_payload(&state, "luz light speed darkness", 5);

    let mut case_disambiguate = || {
        disambiguate_payload(
            &state,
            "en",
            "light",
            "lucidity and clarification illuminate reasoning",
            5,
            0.75,
        )
    };

    json!({
        "object": "csif.benchmark.determinism",
        "schema_version": "csif_benchmark_determinism_v1",
        "deterministic": true,
        "cases": [
            run_case("math_eval", &mut case_math),
            run_case("retrieve", &mut case_retrieve),
            run_case("disambiguate", &mut case_disambiguate),
        ]
    })
}

#[tokio::main]
async fn main() {
    let args = env::args().collect::<Vec<_>>();
    let bin = args
        .first()
        .map_or("csif_agent_v2_rust", |v| Path::new(v).file_name().and_then(|s| s.to_str()).unwrap_or(v));

    if args.len() < 2 {
        print_help(bin);
        std::process::exit(1);
    }

    match args[1].as_str() {
        "validate-bank" => {
            if args.len() != 3 {
                print_help(bin);
                std::process::exit(1);
            }
            let bank = match read_json(&args[2]) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("{}", e);
                    std::process::exit(1);
                }
            };
            let (errors, warnings) = validate_bank(&bank);
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({"errors": errors, "warnings": warnings}))
                    .expect("json pretty print should succeed")
            );
            if !errors.is_empty() {
                std::process::exit(1);
            }
        }
        "migrate-bank" => {
            if args.len() != 4 {
                print_help(bin);
                std::process::exit(1);
            }
            let bank = match read_json(&args[2]) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("{}", e);
                    std::process::exit(1);
                }
            };
            let migrated = migrate_bank_v2(&bank);
            if let Err(e) = write_json(&args[3], &migrated) {
                eprintln!("{}", e);
                std::process::exit(1);
            }
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({"migrated": true, "output": args[3]}))
                    .expect("json pretty print should succeed")
            );
        }
        "index-bank" => {
            if args.len() != 4 {
                print_help(bin);
                std::process::exit(1);
            }
            let bank = match read_json(&args[2]) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("{}", e);
                    std::process::exit(1);
                }
            };
            let (errors, warnings) = validate_bank(&bank);
            if !errors.is_empty() {
                eprintln!("bank validation failed: {}", errors.join("; "));
                std::process::exit(1);
            }
            if !warnings.is_empty() {
                eprintln!("bank validation warnings: {}", warnings.join("; "));
            }

            let payload = build_index_output(&bank);
            if let Err(e) = write_json(&args[3], &payload) {
                eprintln!("{}", e);
                std::process::exit(1);
            }
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({"indexed": true, "output": args[3]}))
                    .expect("json pretty print should succeed")
            );
        }
        "layer0-check" => {
            if args.len() != 3 {
                print_help(bin);
                std::process::exit(1);
            }
            let payload = match read_json(&args[2]) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("{}", e);
                    std::process::exit(1);
                }
            };
            let graph: Layer0Graph = match serde_json::from_value(payload) {
                Ok(g) => g,
                Err(e) => {
                    eprintln!("layer0 graph parse failed: {}", e);
                    std::process::exit(1);
                }
            };
            let _ = graph.layer0_version.as_deref();
            let report = layer0_analyze(&graph, &layer0_policy_default());
            println!(
                "{}",
                serde_json::to_string_pretty(&report)
                    .expect("json pretty print should succeed")
            );
            if !report.valid {
                std::process::exit(1);
            }
        }
        "layer0-conformance" => {
            let mut selected_case: Option<&str> = None;
            let mut i = 2usize;
            while i < args.len() {
                match args[i].as_str() {
                    "--case" => {
                        let Some(v) = args.get(i + 1) else {
                            eprintln!("--case requires a value");
                            std::process::exit(1);
                        };
                        selected_case = Some(v.as_str());
                        i += 2;
                    }
                    other => {
                        eprintln!("unknown layer0-conformance option: {}", other);
                        std::process::exit(1);
                    }
                }
            }

            let payload = layer0_run_conformance(selected_case);
            println!(
                "{}",
                serde_json::to_string_pretty(&payload)
                    .expect("json pretty print should succeed")
            );
            if payload.get("fail_count").and_then(Value::as_u64).unwrap_or(0) > 0 {
                std::process::exit(1);
            }
        }
        "math-eval" => {
            if args.len() < 3 {
                print_help(bin);
                std::process::exit(1);
            }
            let mut mode = MathMode::Algebraic;
            let mut angle_unit = AngleUnit::Radians;
            let mut expr_parts = Vec::<String>::new();
            let mut i = 2usize;
            while i < args.len() {
                match args[i].as_str() {
                    "--mode" => {
                        let Some(v) = args.get(i + 1) else {
                            eprintln!("--mode requires a value");
                            std::process::exit(1);
                        };
                        mode = match parse_math_mode(Some(v.as_str())) {
                            Ok(m) => m,
                            Err(e) => {
                                eprintln!("{}", e);
                                std::process::exit(1);
                            }
                        };
                        i += 2;
                    }
                    "--angle-unit" => {
                        let Some(v) = args.get(i + 1) else {
                            eprintln!("--angle-unit requires a value");
                            std::process::exit(1);
                        };
                        angle_unit = match parse_angle_unit(Some(v.as_str())) {
                            Ok(a) => a,
                            Err(e) => {
                                eprintln!("{}", e);
                                std::process::exit(1);
                            }
                        };
                        i += 2;
                    }
                    _ => {
                        expr_parts.push(args[i].clone());
                        i += 1;
                    }
                }
            }

            if expr_parts.is_empty() {
                eprintln!("expression must not be empty");
                std::process::exit(1);
            }

            let expression = expr_parts.join(" ");
            let state = AppState {
                bank_summary: None,
                bank_index: None,
            sense_trajectory_log_path: None,
            };
            let options = MathOptions { mode, angle_unit };
            match evaluate_math_expression(&expression, &state, options) {
                Ok(payload) => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&payload)
                            .expect("json pretty print should succeed")
                    );
                }
                Err(e) => {
                    eprintln!("{}", e);
                    std::process::exit(1);
                }
            }
        }
        "serve-openai" => {
            let cfg = match parse_serve_args(&args[2..]) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("{}", e);
                    print_help(bin);
                    std::process::exit(1);
                }
            };
            if let Err(e) = run_openai_server(cfg).await {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
        "benchmark-determinism" => {
            let mut iterations = 100usize;
            let mut i = 2usize;
            while i < args.len() {
                match args[i].as_str() {
                    "--iterations" => {
                        let Some(v) = args.get(i + 1) else {
                            eprintln!("--iterations requires a value");
                            std::process::exit(1);
                        };
                        iterations = match v.parse::<usize>() {
                            Ok(n) => n.clamp(1, 10_000),
                            Err(_) => {
                                eprintln!("invalid --iterations value: {}", v);
                                std::process::exit(1);
                            }
                        };
                        i += 2;
                    }
                    other => {
                        eprintln!("unknown benchmark-determinism option: {}", other);
                        std::process::exit(1);
                    }
                }
            }

            let payload = benchmark_determinism(iterations);
            println!(
                "{}",
                serde_json::to_string_pretty(&payload)
                    .expect("json pretty print should succeed")
            );
        }
        "-h" | "--help" | "help" => {
            print_help(bin);
        }
        _ => {
            print_help(bin);
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        ENV_LOCK.get_or_init(|| Mutex::new(()))
    }

    fn with_env_var<T>(key: &str, value: &str, f: impl FnOnce() -> T) -> T {
        with_env_vars(&[(key, value)], f)
    }

    fn with_env_vars<T>(pairs: &[(&str, &str)], f: impl FnOnce() -> T) -> T {
        let _guard = env_lock().lock().expect("env lock should not be poisoned");
        let prior = pairs
            .iter()
            .map(|(key, _)| ((*key).to_string(), std::env::var_os(key)))
            .collect::<Vec<_>>();

        // Tests need scoped process-env overrides for policy paths.
        for (key, value) in pairs {
            unsafe { std::env::set_var(key, value) };
        }

        let out = f();

        for (key, prev) in prior {
            match prev {
                Some(value) => {
                    unsafe { std::env::set_var(&key, value) };
                }
                None => {
                    unsafe { std::env::remove_var(&key) };
                }
            }
        }
        out
    }
    use axum::{body::Body, http::{Method, Request}};
    use tower::util::ServiceExt;

    fn sorted_object_keys(value: &Value) -> Vec<String> {
        value.as_object()
            .map(|object| {
                let mut keys = object.keys().cloned().collect::<Vec<_>>();
                keys.sort();
                keys
            })
            .unwrap_or_default()
    }

    fn bridge_audit_contract_snapshot(payload: &Value) -> Value {
        let bridge = payload
            .get("bridge_audit")
            .expect("bridge audit should exist");
        let semantic_jobs = bridge
            .get("semantic_jobs")
            .and_then(Value::as_array)
            .expect("semantic jobs should exist")
            .iter()
            .map(|job| {
                json!({
                    "requested_operation": job.get("requested_operation").cloned().unwrap_or(Value::Null),
                    "result": job.get("result").cloned().unwrap_or(Value::Null),
                    "trace": job.get("trace").cloned().unwrap_or(Value::Null),
                    "trigger_context": job.get("trigger_context").cloned().unwrap_or(Value::Null),
                })
            })
            .collect::<Vec<_>>();
        let math_jobs = bridge
            .get("math_jobs")
            .and_then(Value::as_array)
            .expect("math jobs should exist")
            .iter()
            .map(|job| {
                json!({
                    "normalized_expression": job.get("normalized_expression").cloned().unwrap_or(Value::Null),
                    "requested_operation": job.get("requested_operation").cloned().unwrap_or(Value::Null),
                    "result": job.get("result").cloned().unwrap_or(Value::Null),
                    "trigger_context": job.get("trigger_context").cloned().unwrap_or(Value::Null),
                    "influence_notes": job.get("influence_notes").cloned().unwrap_or(Value::Null),
                })
            })
            .collect::<Vec<_>>();
        let logic_jobs = bridge
            .get("logic_jobs")
            .and_then(Value::as_array)
            .expect("logic jobs should exist")
            .iter()
            .map(|job| {
                json!({
                    "requested_operation": job.get("requested_operation").cloned().unwrap_or(Value::Null),
                    "result": job.get("result").cloned().unwrap_or(Value::Null),
                    "trace": job.get("trace").cloned().unwrap_or(Value::Null),
                    "trigger_context": job.get("trigger_context").cloned().unwrap_or(Value::Null),
                })
            })
            .collect::<Vec<_>>();
        let influence_audit = bridge
            .get("job_influence_audit")
            .and_then(Value::as_array)
            .expect("job influence audit should exist")
            .iter()
            .map(|record| {
                json!({
                    "job_kind": record.get("job_kind").cloned().unwrap_or(Value::Null),
                    "invocation_mode": record.get("invocation_mode").cloned().unwrap_or(Value::Null),
                    "reasoning_scope": record.get("reasoning_scope").cloned().unwrap_or(Value::Null),
                    "used_in_final_answer": record.get("used_in_final_answer").cloned().unwrap_or(Value::Null),
                    "influence_role": record.get("influence_role").cloned().unwrap_or(Value::Null),
                    "explanation": record.get("explanation").cloned().unwrap_or(Value::Null),
                })
            })
            .collect::<Vec<_>>();

        json!({
            "top_level_keys": sorted_object_keys(bridge),
            "source_kind": bridge.get("source_kind").cloned().unwrap_or(Value::Null),
            "source_text": bridge.get("source_text").cloned().unwrap_or(Value::Null),
            "intent": bridge.get("intent").cloned().unwrap_or(Value::Null),
            "semantic_context": bridge.get("semantic_context").cloned().unwrap_or(Value::Null),
            "active_frame": bridge.get("active_frame").cloned().unwrap_or(Value::Null),
            "parsed_units": bridge.get("parsed_units").cloned().unwrap_or(Value::Null),
            "consistency": bridge.get("consistency").cloned().unwrap_or(Value::Null),
            "routing_trace": bridge.get("routing_trace").cloned().unwrap_or(Value::Null),
            "diagnostics": bridge.get("diagnostics").cloned().unwrap_or(Value::Null),
            "semantic_jobs": semantic_jobs,
            "math_jobs": math_jobs,
            "logic_jobs": logic_jobs,
            "job_influence_audit": influence_audit,
            "final_outcome": json!({
                "status": bridge.get("final_outcome").and_then(|outcome| outcome.get("status")).cloned().unwrap_or(Value::Null),
                "responder_text": bridge.get("final_outcome").and_then(|outcome| outcome.get("responder_text")).cloned().unwrap_or(Value::Null),
                "supporting_job_count": bridge.get("final_outcome").and_then(|outcome| outcome.get("supporting_job_ids")).and_then(Value::as_array).map(|items| items.len()).unwrap_or(0),
                "hidden_job_count": bridge.get("final_outcome").and_then(|outcome| outcome.get("hidden_job_ids_used")).and_then(Value::as_array).map(|items| items.len()).unwrap_or(0),
                "machine_summary": bridge.get("final_outcome").and_then(|outcome| outcome.get("machine_summary")).cloned().unwrap_or(Value::Null),
            }),
        })
    }

    fn sample_v1_bank() -> Value {
        json!({
            "bank_id": "bank-1",
            "unknown_bank_field": {"keep": true},
            "crystals": [
                {
                    "crystal_id": "crystal-1",
                    "unknown_crystal_field": 123,
                    "edges": [
                        {
                            "edge_id": "edge-1",
                            "relation": "is_a",
                            "unknown_edge_field": "retain",
                            "phase_trajectory": [
                                {
                                    "timestamp": "2026-05-27T12:00:00Z",
                                    "phase": 0.5236,
                                    "unknown_event_field": "retain"
                                }
                            ]
                        }
                    ]
                }
            ]
        })
    }

    #[test]
    fn migration_adds_rwif_v2_defaults() {
        let migrated = migrate_bank_v2(&sample_v1_bank());

        let bank = migrated.as_object().expect("bank should be object");
        assert_eq!(
            bank.get("rwif_schema_version").and_then(Value::as_str),
            Some(RWIF_SCHEMA_VERSION)
        );

        let crystal = bank
            .get("crystals")
            .and_then(Value::as_array)
            .and_then(|v| v.first())
            .and_then(Value::as_object)
            .expect("crystal should exist");

        assert_eq!(
            crystal.get("rwif_schema_version").and_then(Value::as_str),
            Some(RWIF_SCHEMA_VERSION)
        );

        let edge = crystal
            .get("edges")
            .and_then(Value::as_array)
            .and_then(|v| v.first())
            .and_then(Value::as_object)
            .expect("edge should exist");

        assert_eq!(
            edge.get("schema_version").and_then(Value::as_str),
            Some(RWIF_EDGE_SCHEMA_VERSION)
        );
        assert_eq!(
            edge.get("state_encoding").and_then(Value::as_str),
            Some("phase_scalar_v1")
        );
        assert_eq!(
            edge.get("wrap_mode").and_then(Value::as_str),
            Some("principal_pi")
        );
        assert_eq!(
            edge.get("integer_wrap_mode").and_then(Value::as_str),
            Some("clamp")
        );
        assert_eq!(
            edge.get("integration_rule").and_then(Value::as_str),
            Some("legacy_scalar")
        );

        let event = edge
            .get("phase_trajectory")
            .and_then(Value::as_array)
            .and_then(|v| v.first())
            .and_then(Value::as_object)
            .expect("event should exist");

        assert_eq!(
            event.get("schema_version").and_then(Value::as_str),
            Some(RWIF_EVENT_SCHEMA_VERSION)
        );
        assert_eq!(
            event.get("state_encoding").and_then(Value::as_str),
            Some("signed_i8_plus_intent_v2")
        );
        assert_eq!(event.get("quantization_step"), Some(&json!(1)));
        assert_eq!(event.get("amplitude_signed"), Some(&Value::Null));
        assert_eq!(event.get("intent_signed"), Some(&Value::Null));
        assert_eq!(event.get("phase_theta"), Some(&json!(0.5236)));
        assert_eq!(event.get("phase_omega"), Some(&Value::Null));
        assert_eq!(event.get("monotonic_index"), Some(&Value::Null));
    }

    #[test]
    fn migration_preserves_unknown_and_existing_fields() {
        let source = json!({
            "bank_id": "bank-1",
            "rwif_schema_version": "CUSTOM_VERSION",
            "crystals": [
                {
                    "crystal_id": "crystal-1",
                    "rwif_schema_version": "CUSTOM_CRYSTAL_VERSION",
                    "edges": [
                        {
                            "edge_id": "edge-1",
                            "schema_version": "CUSTOM_EDGE_VERSION",
                            "integer_wrap_mode": "overflow_modulo",
                            "phase_trajectory": [
                                {
                                    "schema_version": "CUSTOM_EVENT_VERSION",
                                    "state_encoding": "custom_encoding",
                                    "quantization_step": 7,
                                    "phase": 1.25,
                                    "phase_theta": 2.5,
                                    "amplitude_signed": 8,
                                    "intent_signed": -1,
                                    "monotonic_index": 99,
                                    "unknown_event_field": "retain"
                                }
                            ],
                            "unknown_edge_field": "retain"
                        }
                    ],
                    "unknown_crystal_field": "retain"
                }
            ],
            "unknown_bank_field": "retain"
        });

        let migrated = migrate_bank_v2(&source);

        let bank = migrated.as_object().expect("bank should be object");
        assert_eq!(bank.get("unknown_bank_field"), Some(&json!("retain")));
        assert_eq!(
            bank.get("rwif_schema_version").and_then(Value::as_str),
            Some(RWIF_SCHEMA_VERSION)
        );

        let crystal = bank
            .get("crystals")
            .and_then(Value::as_array)
            .and_then(|v| v.first())
            .and_then(Value::as_object)
            .expect("crystal should exist");
        assert_eq!(crystal.get("unknown_crystal_field"), Some(&json!("retain")));
        assert_eq!(
            crystal.get("rwif_schema_version").and_then(Value::as_str),
            Some("CUSTOM_CRYSTAL_VERSION")
        );

        let edge = crystal
            .get("edges")
            .and_then(Value::as_array)
            .and_then(|v| v.first())
            .and_then(Value::as_object)
            .expect("edge should exist");
        assert_eq!(edge.get("unknown_edge_field"), Some(&json!("retain")));
        assert_eq!(
            edge.get("schema_version").and_then(Value::as_str),
            Some("CUSTOM_EDGE_VERSION")
        );
        assert_eq!(
            edge.get("integer_wrap_mode").and_then(Value::as_str),
            Some("overflow_modulo")
        );

        let event = edge
            .get("phase_trajectory")
            .and_then(Value::as_array)
            .and_then(|v| v.first())
            .and_then(Value::as_object)
            .expect("event should exist");
        assert_eq!(
            event.get("schema_version").and_then(Value::as_str),
            Some("CUSTOM_EVENT_VERSION")
        );
        assert_eq!(
            event.get("state_encoding").and_then(Value::as_str),
            Some("custom_encoding")
        );
        assert_eq!(event.get("quantization_step"), Some(&json!(7)));
        assert_eq!(event.get("phase_theta"), Some(&json!(2.5)));
        assert_eq!(event.get("amplitude_signed"), Some(&json!(8)));
        assert_eq!(event.get("intent_signed"), Some(&json!(-1)));
        assert_eq!(event.get("monotonic_index"), Some(&json!(99)));
        assert_eq!(event.get("unknown_event_field"), Some(&json!("retain")));
    }

    #[test]
    fn migration_is_idempotent() {
        let once = migrate_bank_v2(&sample_v1_bank());
        let twice = migrate_bank_v2(&once);
        assert_eq!(once, twice);
    }

    #[test]
    fn validator_flags_missing_integer_wrap_mode() {
        let bank = json!({
            "bank_id": "bank-1",
            "rwif_schema_version": "RWIF_V2",
            "crystals": [
                {
                    "crystal_id": "crystal-1",
                    "rwif_schema_version": "RWIF_V2",
                    "edges": [
                        {
                            "edge_id": "edge-1",
                            "schema_version": "RWIF_EDGE_V2",
                            "phase_trajectory": [
                                {
                                    "schema_version": "RWIF_EVENT_V2",
                                    "state_encoding": "signed_i8_plus_intent_v2",
                                    "quantization_step": 1,
                                    "monotonic_index": null
                                }
                            ]
                        }
                    ]
                }
            ]
        });

        let (errors, warnings) = validate_bank(&bank);
        assert!(warnings.is_empty());
        assert_eq!(errors.len(), 1);
        assert!(
            errors
                .first()
                .expect("one error expected")
                .contains("missing integer_wrap_mode")
        );
    }

    #[test]
    fn embedding_is_deterministic_and_dimensioned() {
        let e1 = embed_text_deterministic("semantic status");
        let e2 = embed_text_deterministic("semantic status");
        assert_eq!(e1, e2);
        assert_eq!(e1.len(), EMBEDDING_DIM);
    }

    #[test]
    fn embedding_changes_with_input() {
        let e1 = embed_text_deterministic("alpha");
        let e2 = embed_text_deterministic("beta");
        assert_ne!(e1, e2);
    }

    #[test]
    fn index_builds_and_retrieves_matches() {
        let bank = json!({
            "bank_id": "b1",
            "rwif_schema_version": "RWIF_V2",
            "crystals": [
                {
                    "crystal_id": "c1",
                    "rwif_schema_version": "RWIF_V2",
                    "edges": [
                        {
                            "edge_id": "e1",
                            "source_node": "light",
                            "relation": "dispels",
                            "target_node": "darkness",
                            "schema_version": "RWIF_EDGE_V2",
                            "integer_wrap_mode": "clamp",
                            "phase_trajectory": [
                                {
                                    "schema_version": "RWIF_EVENT_V2",
                                    "state_encoding": "signed_i8_plus_intent_v2",
                                    "quantization_step": 1,
                                    "monotonic_index": null
                                }
                            ]
                        },
                        {
                            "edge_id": "e2",
                            "source_node": "whale",
                            "relation": "is_a",
                            "target_node": "mammal",
                            "schema_version": "RWIF_EDGE_V2",
                            "integer_wrap_mode": "clamp",
                            "phase_trajectory": [
                                {
                                    "schema_version": "RWIF_EVENT_V2",
                                    "state_encoding": "signed_i8_plus_intent_v2",
                                    "quantization_step": 1,
                                    "monotonic_index": null
                                }
                            ]
                        }
                    ]
                }
            ]
        });

        let index = build_bank_index(&bank);
        assert_eq!(index.entries.len(), 2);
        assert!(index.postings.contains_key("light"));

        let hits = retrieve_from_index(&index, "light darkness", 3);
        assert!(!hits.is_empty());
        let top = &index.entries[hits[0].0];
        assert_eq!(top.edge_id, "e1");
    }

    #[test]
    fn index_summary_reports_loaded_state() {
        let bank = json!({
            "bank_id": "b2",
            "rwif_schema_version": "RWIF_V2",
            "crystals": [
                {
                    "crystal_id": "c1",
                    "rwif_schema_version": "RWIF_V2",
                    "edges": [
                        {
                            "edge_id": "e1",
                            "source_node": "a",
                            "relation": "is_a",
                            "target_node": "b",
                            "schema_version": "RWIF_EDGE_V2",
                            "integer_wrap_mode": "clamp",
                            "phase_trajectory": [
                                {
                                    "schema_version": "RWIF_EVENT_V2",
                                    "state_encoding": "signed_i8_plus_intent_v2",
                                    "quantization_step": 1,
                                    "monotonic_index": null
                                }
                            ]
                        }
                    ]
                }
            ]
        });

        let state = AppState {
            bank_summary: Some(summarize_bank(&bank)),
            bank_index: Some(Arc::new(build_bank_index(&bank))),
        sense_trajectory_log_path: None,
        };
        let payload = index_summary_payload(&state);
        assert_eq!(payload.get("loaded"), Some(&Value::Bool(true)));
        assert_eq!(payload.get("bank_id"), Some(&Value::String("b2".to_string())));
    }

    #[test]
    fn retrieval_payload_reports_unloaded_state() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
        sense_trajectory_log_path: None,
        };
        let payload = retrieval_payload(&state, "alpha", 3);
        assert_eq!(payload.get("loaded"), Some(&Value::Bool(false)));
        assert_eq!(payload.get("match_count"), Some(&json!(0)));
    }

    #[test]
    fn retrieval_payload_returns_matches() {
        let bank = json!({
            "bank_id": "b3",
            "rwif_schema_version": "RWIF_V2",
            "crystals": [
                {
                    "crystal_id": "c1",
                    "rwif_schema_version": "RWIF_V2",
                    "edges": [
                        {
                            "edge_id": "e1",
                            "source_node": "light",
                            "relation": "dispels",
                            "target_node": "darkness",
                            "schema_version": "RWIF_EDGE_V2",
                            "integer_wrap_mode": "clamp",
                            "phase_trajectory": [
                                {
                                    "schema_version": "RWIF_EVENT_V2",
                                    "state_encoding": "signed_i8_plus_intent_v2",
                                    "quantization_step": 1,
                                    "monotonic_index": null
                                }
                            ]
                        }
                    ]
                }
            ]
        });

        let state = AppState {
            bank_summary: Some(summarize_bank(&bank)),
            bank_index: Some(Arc::new(build_bank_index(&bank))),
        sense_trajectory_log_path: None,
        };
        let payload = retrieval_payload(&state, "light darkness", 3);
        assert_eq!(payload.get("loaded"), Some(&Value::Bool(true)));
        assert_eq!(payload.get("match_count"), Some(&json!(1)));
        let matches = payload
            .get("matches")
            .and_then(Value::as_array)
            .expect("matches array should exist");
        assert_eq!(matches.len(), 1);
        assert!(
            payload
                .get("rewritten_query")
                .and_then(Value::as_str)
                .is_some()
        );
    }

    #[test]
    fn retrieval_payload_emits_miss_diagnostics_for_unseen_terms() {
        let bank = json!({
            "bank_id": "b4",
            "rwif_schema_version": "RWIF_V2",
            "crystals": [
                {
                    "crystal_id": "c1",
                    "rwif_schema_version": "RWIF_V2",
                    "edges": [
                        {
                            "edge_id": "e1",
                            "source_node": "light",
                            "relation": "dispels",
                            "target_node": "darkness",
                            "schema_version": "RWIF_EDGE_V2",
                            "integer_wrap_mode": "clamp",
                            "phase_trajectory": [
                                {
                                    "schema_version": "RWIF_EVENT_V2",
                                    "state_encoding": "signed_i8_plus_intent_v2",
                                    "quantization_step": 1,
                                    "monotonic_index": null
                                }
                            ]
                        }
                    ]
                }
            ]
        });

        let state = AppState {
            bank_summary: Some(summarize_bank(&bank)),
            bank_index: Some(Arc::new(build_bank_index(&bank))),
            sense_trajectory_log_path: None,
        };
        let payload = retrieval_payload(&state, "quasarword blorf", 3);
        assert_eq!(payload.get("match_count"), Some(&json!(0)));
        assert_eq!(
            payload
                .get("miss_diagnostics")
                .and_then(|v| v.get("reason"))
                .and_then(Value::as_str),
            Some("query_terms_absent_from_index")
        );
    }

    #[test]
    fn math_eval_is_deterministic_and_has_latex() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
        sense_trajectory_log_path: None,
        };
        let options = MathOptions::default();
        let p1 = evaluate_math_expression("2*(3+4)^2", &state, options).expect("math eval should pass");
        let p2 = evaluate_math_expression("2*(3+4)^2", &state, options).expect("math eval should pass");
        assert_eq!(p1, p2);
        let result = p1
            .get("result")
            .and_then(Value::as_f64)
            .expect("result should be numeric");
        assert!((result - 98.0).abs() < 1e-12);
        assert!(
            p1.get("result_latex")
                .and_then(Value::as_str)
                .expect("result_latex should exist")
                .contains("=")
        );
    }

    #[test]
    fn math_eval_rejects_division_by_zero() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
        sense_trajectory_log_path: None,
        };
        let err = evaluate_math_expression("4/(2-2)", &state, MathOptions::default())
            .expect_err("should fail");
        assert!(err.contains("division by zero"));
    }

    #[test]
    fn logic_implication_emits_first_class_semantic_jobs() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };

        let payload = evaluate_math_expression(
            "1 < 2 implies isreal(5)",
            &state,
            MathOptions::default(),
        )
        .expect("implication should evaluate");

        assert_eq!(payload.get("result"), Some(&json!(true)));
        let bridge = payload
            .get("bridge_audit")
            .and_then(Value::as_object)
            .expect("bridge audit should exist");
        let semantic_jobs = bridge
            .get("semantic_jobs")
            .and_then(Value::as_array)
            .expect("semantic jobs should exist");
        assert_eq!(semantic_jobs.len(), 2);
        let logic_jobs = bridge
            .get("logic_jobs")
            .and_then(Value::as_array)
            .expect("logic jobs should exist");
        assert_eq!(logic_jobs.len(), 1);
        assert_eq!(
            logic_jobs[0]
                .get("trace")
                .and_then(Value::as_array)
                .and_then(|trace| trace.first())
                .and_then(|step| step.get("rule"))
                .and_then(Value::as_str),
            Some("implication_evaluation")
        );

        let logic_crystal = payload
            .get("logic_crystal")
            .expect("logic_crystal should exist");
        assert_eq!(
            logic_crystal.get("node_type"),
            Some(&json!("logic_prop"))
        );
        assert!(
            logic_crystal
                .get("phase_signature")
                .and_then(|p| p.get("phase_theta"))
                .and_then(Value::as_f64)
                .is_some()
        );

        let connectives = payload
            .get("logic_connectives")
            .and_then(Value::as_array)
            .expect("logic_connectives should exist");
        assert!(connectives.iter().any(|c| {
            c.get("operator") == Some(&json!("implies"))
        }));

        let morphisms = payload
            .get("inference_morphisms")
            .and_then(Value::as_array)
            .expect("inference_morphisms should exist");
        assert!(morphisms.iter().any(|m| {
            m.get("morphism_type") == Some(&json!("inference_modus_ponens"))
                && m.get("hops").and_then(Value::as_array).map(|h| h.len() == 2).unwrap_or(false)
        }));

        let logic_signal = payload
            .get("logic_contradiction_signal")
            .expect("logic_contradiction_signal should exist");
        assert!(logic_signal.get("triggered").and_then(Value::as_bool).is_some());
    }

    #[test]
    fn logic_equivalence_and_predicates_emit_hidden_operand_jobs() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };

        let payload = evaluate_math_expression(
            "iszero(0) equiv isfinite(3+4i)",
            &state,
            MathOptions::default(),
        )
        .expect("equivalence should evaluate");

        assert_eq!(payload.get("result"), Some(&json!(true)));
        let bridge = payload
            .get("bridge_audit")
            .and_then(Value::as_object)
            .expect("bridge audit should exist");
        let math_jobs = bridge
            .get("math_jobs")
            .and_then(Value::as_array)
            .expect("operand math jobs should exist");
        assert_eq!(math_jobs.len(), 2);
        assert_eq!(
            bridge
                .get("logic_jobs")
                .and_then(Value::as_array)
                .and_then(|jobs| jobs.first())
                .and_then(|job| job.get("trace"))
                .and_then(Value::as_array)
                .and_then(|trace| trace.first())
                .and_then(|step| step.get("rule"))
                .and_then(Value::as_str),
            Some("equivalence_evaluation")
        );
    }

    #[test]
    fn symbolic_logic_operators_are_supported() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };

        let payload = evaluate_math_expression(
            "!(false) && (1 < 2 || false)",
            &state,
            MathOptions::default(),
        )
        .expect("symbolic logic operators should evaluate");

        assert_eq!(payload.get("result"), Some(&json!(true)));
        assert_eq!(
            payload
                .get("bridge_audit")
                .and_then(Value::as_object)
                .and_then(|bridge| bridge.get("logic_jobs"))
                .and_then(Value::as_array)
                .and_then(|jobs| jobs.first())
                .and_then(|job| job.get("result"))
                .and_then(|result| result.get("truth"))
                .and_then(Value::as_str),
            Some("True")
        );
    }

    #[test]
    fn symbolic_xor_and_alias_operators_are_supported() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };

        let xor_payload = evaluate_math_expression(
            "true ^^ false",
            &state,
            MathOptions::default(),
        )
        .expect("symbolic xor should evaluate");
        let implies_payload = evaluate_math_expression(
            "1 < 2 => isreal(5)",
            &state,
            MathOptions::default(),
        )
        .expect("=> alias should evaluate");
        let equiv_payload = evaluate_math_expression(
            "iszero(0) ↔ isfinite(3+4i)",
            &state,
            MathOptions::default(),
        )
        .expect("unicode equivalence alias should evaluate");

        assert_eq!(xor_payload.get("result"), Some(&json!(true)));
        assert_eq!(implies_payload.get("result"), Some(&json!(true)));
        assert_eq!(equiv_payload.get("result"), Some(&json!(true)));
    }

    #[test]
    fn long_mixed_logic_chain_supports_nested_negation_and_comparison_symbols() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };

        let payload = evaluate_math_expression(
            "!(!true) && (1 < 2) && (2 <= 2) && (3 >= 2) && (4 == 4) && (5 != 6) && isnonzero(1) && (true || false || false)",
            &state,
            MathOptions::default(),
        )
        .expect("long mixed logic chain should evaluate");

        assert_eq!(payload.get("result"), Some(&json!(true)));
    }

    #[test]
    fn grouped_implication_and_equivalence_precedence_is_respected() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };

        let ungrouped = evaluate_math_expression(
            "false -> true <-> false",
            &state,
            MathOptions::default(),
        )
        .expect("ungrouped precedence expression should evaluate");
        let grouped = evaluate_math_expression(
            "false -> (true <-> false)",
            &state,
            MathOptions::default(),
        )
        .expect("grouped precedence expression should evaluate");

        assert_eq!(ungrouped.get("result"), Some(&json!(false)));
        assert_eq!(grouped.get("result"), Some(&json!(true)));
    }

    #[tokio::test]
    async fn math_endpoint_options_returns_cors_headers() {
        let state = Arc::new(AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        });
        let app = Router::new()
            .route("/v1/csif/math", post(csif_math).options(csif_math_options))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::OPTIONS)
                    .uri("/v1/csif/math")
                    .header("Origin", "file://")
                    .header("Access-Control-Request-Method", "POST")
                    .header("Access-Control-Request-Headers", "Content-Type")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("options request should succeed");

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            response.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN).and_then(|v| v.to_str().ok()),
            Some("*")
        );
        assert_eq!(
            response.headers().get(header::ACCESS_CONTROL_ALLOW_METHODS).and_then(|v| v.to_str().ok()),
            Some("POST, OPTIONS")
        );
        assert_eq!(
            response.headers().get(header::ACCESS_CONTROL_ALLOW_HEADERS).and_then(|v| v.to_str().ok()),
            Some("Content-Type")
        );
    }

    #[tokio::test]
    async fn math_endpoint_post_locks_bridge_envelope_shape() {
        let state = Arc::new(AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        });
        let app = Router::new()
            .route("/v1/csif/math", post(csif_math).options(csif_math_options))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/v1/csif/math")
                    .header("Origin", "file://")
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"expression":"1 < 2 -> isreal(5)"}"#))
                    .expect("request should build"),
            )
            .await
            .expect("post request should succeed");

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN).and_then(|v| v.to_str().ok()),
            Some("*")
        );

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should read");
        let payload: Value = serde_json::from_slice(&body).expect("payload should be valid json");

        assert_eq!(payload.get("result"), Some(&json!(true)));
        let bridge = payload
            .get("bridge_audit")
            .and_then(Value::as_object)
            .expect("bridge audit should exist");
        assert!(bridge.contains_key("semantic_jobs"));
        assert!(bridge.contains_key("math_jobs"));
        assert!(bridge.contains_key("logic_jobs"));
        assert!(bridge.contains_key("job_influence_audit"));
        assert!(bridge.contains_key("final_outcome"));
        assert_eq!(
            bridge
                .get("semantic_jobs")
                .and_then(Value::as_array)
                .map(|items| items.len()),
            Some(2)
        );
        assert_eq!(
            bridge
                .get("logic_jobs")
                .and_then(Value::as_array)
                .and_then(|jobs| jobs.first())
                .and_then(|job| job.get("trace"))
                .and_then(Value::as_array)
                .and_then(|trace| trace.first())
                .and_then(|step| step.get("rule"))
                .and_then(Value::as_str),
            Some("implication_evaluation")
        );
    }

    #[test]
    fn bridge_audit_contract_snapshot_matches_expected_shape() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };

        let payload = evaluate_math_expression(
            "1 < 2 -> isreal(5)",
            &state,
            MathOptions::default(),
        )
        .expect("bridge snapshot expression should evaluate");

        let snapshot = bridge_audit_contract_snapshot(&payload);
        assert_eq!(snapshot, json!({
            "top_level_keys": [
                "active_frame",
                "assumptions",
                "consistency",
                "diagnostics",
                "envelope_id",
                "final_outcome",
                "intent",
                "job_influence_audit",
                "logic_jobs",
                "math_jobs",
                "parsed_units",
                "routing_trace",
                "semantic_context",
                "semantic_jobs",
                "source_kind",
                "source_text",
                "symbol_table",
                "timeout_ms",
                "timestamp_unix_ms"
            ],
            "source_kind": "MathExpression",
            "source_text": "1 < 2 -> isreal(5)",
            "intent": {
                "intent_id": "math_eval",
                "primary_goal": "CheckTruth",
                "requested_output_mode": "TextAndStructured",
                "secondary_goals": []
            },
            "semantic_context": {
                "ambiguity_state": "Resolved",
                "confidence": 1.0,
                "resolved_entities": [],
                "semantic_identity_signature": null
            },
            "active_frame": {
                "epistemic_source_frame": "deterministic_math_v2",
                "modality_frame": "algebraic",
                "observer_frame": null,
                "ontology_frame": "math_engine",
                "temporal_frame": null
            },
            "parsed_units": [
                {
                    "LogicExpr": {
                        "Implies": [
                            {"Comparison": {"left": {"Number": 1.0}, "op": "Lt", "right": {"Number": 2.0}}},
                            {"Predicate": {"args": [{"Number": 5.0}], "name": "isreal"}}
                        ]
                    }
                }
            ],
            "consistency": {
                "contradictions": [],
                "math_logic_alignment": "Aligned",
                "semantic_logic_alignment": "Aligned",
                "semantic_math_alignment": "Aligned",
                "unresolved_ambiguities": []
            },
            "routing_trace": [
                {
                    "decision": "expression_parsed",
                    "rationale": "expression routed through shared math/logic bridge",
                    "stage": "parse"
                },
                {
                    "decision": "bridge_audit_emitted",
                    "rationale": "job influence audit attached to response",
                    "stage": "synthesize"
                }
            ],
            "diagnostics": [
                {
                    "code": "SEMANTIC_ROUTE_LOGIC",
                    "message": "semantic layer routed request through math/logic bridge",
                    "severity": "Info"
                }
            ],
            "semantic_jobs": [
                {
                    "requested_operation": "RouteRequest",
                    "result": {
                        "confidence": 1.0,
                        "error": null,
                        "interpretation": "primary_goal=CheckTruth, parsed_unit=logic_expression",
                        "status": "Success"
                    },
                    "trace": [
                        {"confidence": 1.0, "note": "expression routed through shared semantic bridge", "stage": "route"}
                    ],
                    "trigger_context": {
                        "routed_from_stage": "csif_math",
                        "trigger_reason": "semantic_route_request",
                        "triggered_by_job_id": null
                    }
                },
                {
                    "requested_operation": "SynthesizeResponse",
                    "result": {
                        "confidence": 1.0,
                        "error": null,
                        "interpretation": "final_status=QualifiedSuccess",
                        "status": "Success"
                    },
                    "trace": [
                        {"confidence": 1.0, "note": "final response assembled from semantic/math/logic jobs", "stage": "synthesize"}
                    ],
                    "trigger_context": {
                        "routed_from_stage": "response_synthesis",
                        "trigger_reason": "semantic_synthesize_response",
                        "triggered_by_job_id": Value::String(stable_bridge_id("semantic_route", "1 < 2 -> isreal(5)"))
                    }
                }
            ],
            "math_jobs": [
                {
                    "normalized_expression": "1",
                    "requested_operation": "Evaluate",
                    "result": {
                        "deterministic": true,
                        "domain_ok": true,
                        "error": null,
                        "precision_class": "ExactDeterministic",
                        "status": "Success",
                        "value": {"Real": 1.0},
                        "value_type": "Real"
                    },
                    "trigger_context": {
                        "routed_from_stage": "logic_evaluation",
                        "trigger_reason": "logic_operand_evaluation:cmp_left_0",
                        "triggered_by_job_id": Value::String(stable_bridge_id("logic_user", "(1 < 2) -> (isreal(5))"))
                    },
                    "influence_notes": ["hidden numeric operand for logic evaluation (cmp_left_0)"]
                },
                {
                    "normalized_expression": "2",
                    "requested_operation": "Evaluate",
                    "result": {
                        "deterministic": true,
                        "domain_ok": true,
                        "error": null,
                        "precision_class": "ExactDeterministic",
                        "status": "Success",
                        "value": {"Real": 2.0},
                        "value_type": "Real"
                    },
                    "trigger_context": {
                        "routed_from_stage": "logic_evaluation",
                        "trigger_reason": "logic_operand_evaluation:cmp_right_1",
                        "triggered_by_job_id": Value::String(stable_bridge_id("logic_user", "(1 < 2) -> (isreal(5))"))
                    },
                    "influence_notes": ["hidden numeric operand for logic evaluation (cmp_right_1)"]
                },
                {
                    "normalized_expression": "5",
                    "requested_operation": "Evaluate",
                    "result": {
                        "deterministic": true,
                        "domain_ok": true,
                        "error": null,
                        "precision_class": "ExactDeterministic",
                        "status": "Success",
                        "value": {"Real": 5.0},
                        "value_type": "Real"
                    },
                    "trigger_context": {
                        "routed_from_stage": "logic_evaluation",
                        "trigger_reason": "logic_operand_evaluation:pred_isreal_arg_0",
                        "triggered_by_job_id": Value::String(stable_bridge_id("logic_user", "(1 < 2) -> (isreal(5))"))
                    },
                    "influence_notes": ["hidden numeric operand for logic evaluation (pred_isreal_arg_0)"]
                }
            ],
            "logic_jobs": [
                {
                    "requested_operation": "EvaluateTruth",
                    "result": {
                        "blocking_conditions": [],
                        "error": null,
                        "modality": "Must",
                        "models_found": null,
                        "satisfiable": true,
                        "status": "Success",
                        "truth": "True"
                    },
                    "trace": [
                        {
                            "expression": "(1 < 2) -> (isreal(5))",
                            "note": "modality=Must",
                            "rule": "implication_evaluation",
                            "truth": "True"
                        }
                    ],
                    "trigger_context": {
                        "routed_from_stage": "csif_math",
                        "trigger_reason": "user_requested_logic_evaluation",
                        "triggered_by_job_id": null
                    }
                }
            ],
            "job_influence_audit": [
                {
                    "job_kind": "Semantic",
                    "invocation_mode": "InternallyTriggered",
                    "reasoning_scope": "Committed",
                    "used_in_final_answer": true,
                    "influence_role": "DirectEvidence",
                    "explanation": "semantic route for CheckTruth"
                },
                {
                    "job_kind": "Semantic",
                    "invocation_mode": "InternallyTriggered",
                    "reasoning_scope": "Committed",
                    "used_in_final_answer": true,
                    "influence_role": "CandidateRanking",
                    "explanation": "semantic response synthesis"
                },
                {
                    "job_kind": "Math",
                    "invocation_mode": "InternallyTriggered",
                    "reasoning_scope": "Committed",
                    "used_in_final_answer": true,
                    "influence_role": "ConsistencyCheck",
                    "explanation": "hidden numeric operand for logic evaluation (cmp_left_0)"
                },
                {
                    "job_kind": "Math",
                    "invocation_mode": "InternallyTriggered",
                    "reasoning_scope": "Committed",
                    "used_in_final_answer": true,
                    "influence_role": "ConsistencyCheck",
                    "explanation": "hidden numeric operand for logic evaluation (cmp_right_1)"
                },
                {
                    "job_kind": "Math",
                    "invocation_mode": "InternallyTriggered",
                    "reasoning_scope": "Committed",
                    "used_in_final_answer": true,
                    "influence_role": "ConsistencyCheck",
                    "explanation": "hidden numeric operand for logic evaluation (pred_isreal_arg_0)"
                },
                {
                    "job_kind": "Logic",
                    "invocation_mode": "UserRequested",
                    "reasoning_scope": "Committed",
                    "used_in_final_answer": true,
                    "influence_role": "DirectEvidence",
                    "explanation": "primary user-visible logical truth result"
                }
            ],
            "final_outcome": {
                "status": "QualifiedSuccess",
                "responder_text": "deterministic math evaluation completed with internal qualification checks",
                "supporting_job_count": 4,
                "hidden_job_count": 3,
                "machine_summary": {
                    "assumptions_applied": [],
                    "confidence": 1.0,
                    "contradiction_count": 0,
                    "final_truth": "True",
                    "final_value": null
                }
            }
        }));
    }

    #[test]
    fn bridge_audit_failure_snapshot_matches_expected_shape() {
        let snapshot = bridge_audit_contract_snapshot(&json!({
            "bridge_audit": build_error_bridge_audit_value(
                "foo(",
                "unexpected trailing tokens",
                "MATH_PARSE_ERROR",
                "parse_error",
            )
        }));

        assert_eq!(snapshot, json!({
            "top_level_keys": [
                "active_frame",
                "assumptions",
                "consistency",
                "diagnostics",
                "envelope_id",
                "final_outcome",
                "intent",
                "job_influence_audit",
                "logic_jobs",
                "math_jobs",
                "parsed_units",
                "routing_trace",
                "semantic_context",
                "semantic_jobs",
                "source_kind",
                "source_text",
                "symbol_table",
                "timeout_ms",
                "timestamp_unix_ms"
            ],
            "source_kind": "MathExpression",
            "source_text": "foo(",
            "intent": {
                "intent_id": "math_eval",
                "primary_goal": "EvaluateNumeric",
                "requested_output_mode": "TextAndStructured",
                "secondary_goals": []
            },
            "semantic_context": {
                "ambiguity_state": "NeedsInput",
                "confidence": 0.0,
                "resolved_entities": [],
                "semantic_identity_signature": null
            },
            "active_frame": {
                "epistemic_source_frame": "deterministic_math_v2",
                "modality_frame": null,
                "observer_frame": null,
                "ontology_frame": "math_engine",
                "temporal_frame": null
            },
            "parsed_units": [],
            "consistency": {
                "contradictions": [],
                "math_logic_alignment": "Unknown",
                "semantic_logic_alignment": "Unknown",
                "semantic_math_alignment": "Unknown",
                "unresolved_ambiguities": []
            },
            "routing_trace": [
                {
                    "decision": "parse_error",
                    "rationale": "unexpected trailing tokens",
                    "stage": "error"
                }
            ],
            "diagnostics": [
                {
                    "code": "MATH_PARSE_ERROR",
                    "message": "unexpected trailing tokens",
                    "severity": "Error"
                },
                {
                    "code": "SEMANTIC_ROUTE_ERROR",
                    "message": "semantic layer preserved error-path audit context",
                    "severity": "Info"
                }
            ],
            "semantic_jobs": [
                {
                    "requested_operation": "PreserveFailureContext",
                    "result": {
                        "confidence": 1.0,
                        "error": null,
                        "interpretation": "error preserved with code=MATH_PARSE_ERROR",
                        "status": "Failed"
                    },
                    "trace": [
                        {"confidence": 1.0, "note": "unexpected trailing tokens", "stage": "route_error"}
                    ],
                    "trigger_context": {
                        "routed_from_stage": "csif_math",
                        "trigger_reason": "semantic_route_failed_request",
                        "triggered_by_job_id": null
                    }
                },
                {
                    "requested_operation": "SynthesizeResponse",
                    "result": {
                        "confidence": 1.0,
                        "error": null,
                        "interpretation": "semantic layer synthesized final failure response",
                        "status": "Failed"
                    },
                    "trace": [
                        {"confidence": 1.0, "note": "failure envelope emitted", "stage": "finalize_error"}
                    ],
                    "trigger_context": {
                        "routed_from_stage": "response_synthesis",
                        "trigger_reason": "semantic_synthesize_failed_response",
                        "triggered_by_job_id": Value::String(stable_bridge_id("semantic_route", "foo("))
                    }
                }
            ],
            "math_jobs": [],
            "logic_jobs": [],
            "job_influence_audit": [
                {
                    "job_kind": "Semantic",
                    "invocation_mode": "InternallyTriggered",
                    "reasoning_scope": "Committed",
                    "used_in_final_answer": true,
                    "influence_role": "DirectEvidence",
                    "explanation": "semantic layer preserved failed request context for MATH_PARSE_ERROR"
                },
                {
                    "job_kind": "Semantic",
                    "invocation_mode": "InternallyTriggered",
                    "reasoning_scope": "Committed",
                    "used_in_final_answer": true,
                    "influence_role": "CandidateRanking",
                    "explanation": "semantic layer synthesized final failure response"
                }
            ],
            "final_outcome": {
                "status": "Failed",
                "responder_text": "unexpected trailing tokens",
                "supporting_job_count": 0,
                "hidden_job_count": 0,
                "machine_summary": {
                    "assumptions_applied": [],
                    "confidence": 0.0,
                    "contradiction_count": 0,
                    "final_truth": null,
                    "final_value": null
                }
            }
        }));
    }

    #[test]
    fn geometric_mode_supports_scientific_trig_in_degrees() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
        sense_trajectory_log_path: None,
        };
        let payload = evaluate_math_expression(
            "sin(30) + cos(60)",
            &state,
            MathOptions {
                mode: MathMode::Geometric,
                angle_unit: AngleUnit::Degrees,
            },
        )
        .expect("geometric trig should evaluate");

        let result = payload
            .get("result")
            .and_then(Value::as_f64)
            .expect("result should be numeric");
        assert!((result - 1.0).abs() < 1e-12);

        let steps = payload
            .get("derivation_trace")
            .and_then(Value::as_array)
            .expect("derivation trace should exist");
        assert!(steps.iter().any(|s| s.get("geometry").is_some()));
    }

    #[test]
    fn geometric_mode_avoids_decimal_binary_float_trap() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };
        let payload = evaluate_math_expression(
            "0.1+0.2",
            &state,
            MathOptions {
                mode: MathMode::Geometric,
                angle_unit: AngleUnit::Radians,
            },
        )
        .expect("geometric decimal scaling should evaluate");

        let result = payload
            .get("result")
            .and_then(Value::as_f64)
            .expect("result should be numeric");
        assert!((result - 0.3).abs() < 1e-12);
        assert_ne!(result, 0.30000000000000004_f64);

        let policy = payload
            .get("reasoning_policy")
            .expect("reasoning_policy should exist");
        assert_eq!(
            policy.get("numeric_determination"),
            Some(&json!("geometric_decimal_scaling"))
        );

        let unit = payload.get("unit_crystal").expect("unit_crystal should exist");
        assert_eq!(
            unit.get("unit_id"),
            Some(&json!("unit.decimal.geometric"))
        );
        assert_eq!(
            unit.get("policy")
                .and_then(|p| p.get("loop_torsion_norm_threshold"))
                .and_then(Value::as_f64),
            Some(0.2)
        );
        assert!(
            unit.get("trajectory")
                .and_then(Value::as_array)
                .map(|t| !t.is_empty())
                .unwrap_or(false)
        );
        let morphisms = unit
            .get("conversion_morphisms")
            .and_then(Value::as_array)
            .expect("conversion_morphisms should exist");
        assert!(morphisms.iter().any(|m| {
            m.get("morphism_type") == Some(&json!("representation_change"))
                && m.get("hops").and_then(Value::as_array).map(|h| h.len() == 2).unwrap_or(false)
                && m.get("loop_metrics")
                    .and_then(|lm| lm.get("loop_torsion_norm"))
                    .and_then(Value::as_f64)
                    .is_some()
        }));
        assert_eq!(
            payload
                .get("unit_contradiction_signal")
                .and_then(|s| s.get("triggered")),
            Some(&json!(false))
        );

        let rwif_edges = payload
            .get("rwif_export")
            .and_then(|v| v.get("edges"))
            .and_then(Value::as_array)
            .expect("rwif edges should exist");
        assert!(rwif_edges.iter().any(|e| {
            e.get("relation") == Some(&json!("represented_in_unit"))
        }));
    }

    #[test]
    fn algebraic_mode_preserves_binary_trace_float_result() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };
        let payload = evaluate_math_expression(
            "0.1+0.2",
            &state,
            MathOptions {
                mode: MathMode::Algebraic,
                angle_unit: AngleUnit::Radians,
            },
        )
        .expect("algebraic binary trace should evaluate");

        let result = payload
            .get("result")
            .and_then(Value::as_f64)
            .expect("result should be numeric");
        assert_eq!(result, 0.30000000000000004_f64);
        assert!(
            payload
                .get("result_latex")
                .and_then(Value::as_str)
                .expect("result_latex should exist")
                .contains("0.30000000000000004")
        );

        let policy = payload
            .get("reasoning_policy")
            .expect("reasoning_policy should exist");
        assert_eq!(
            policy.get("numeric_determination"),
            Some(&json!("ieee754_binary_trace"))
        );
        assert_eq!(policy.get("preserve_binary_trace"), Some(&json!(true)));
        assert_eq!(policy.get("preserve_decimal_geometric"), Some(&json!(false)));

        let unit = payload.get("unit_crystal").expect("unit_crystal should exist");
        assert_eq!(
            unit.get("unit_id"),
            Some(&json!("unit.binary.ieee754.f64"))
        );
        let morphisms = unit
            .get("conversion_morphisms")
            .and_then(Value::as_array)
            .expect("conversion_morphisms should exist");
        assert!(morphisms.iter().any(|m| {
            m.get("morphism_type") == Some(&json!("angle_coordinate_change"))
                && m.get("hops").and_then(Value::as_array).map(|h| h.len() == 2).unwrap_or(false)
                && m.get("loop_metrics")
                    .and_then(|lm| lm.get("loop_resonance"))
                    .and_then(Value::as_f64)
                    .is_some()
        }));
    }

    #[test]
    fn unit_conversion_torsion_threshold_emits_explicit_contradiction_signal() {
        let unit = build_unit_crystal_payload(
            MathOptions {
                mode: MathMode::Geometric,
                angle_unit: AngleUnit::Radians,
            },
            "radians",
            0.0,
            1.0,
        );

        let signal = unit
            .get("contradiction_signal")
            .expect("contradiction_signal should exist");
        assert_eq!(signal.get("triggered"), Some(&json!(true)));
        assert_eq!(
            signal.get("stop_reason"),
            Some(&json!("unit_conversion_loop_torsion_exceeded"))
        );

        let contradictions = unit
            .get("contradictions")
            .and_then(Value::as_array)
            .expect("unit contradictions should exist");
        assert!(contradictions.iter().any(|c| {
            c.get("code") == Some(&json!("unit_conversion_loop_torsion_exceeded"))
        }));

        let bridge_contradictions = unit_contradictions_from_payload(&unit);
        let envelope = build_success_envelope(
            "1+0",
            ParsedUnit::ScalarExpr(AstNode::Number(1.0)),
            PrimaryGoal::EvaluateNumeric,
            MathOptions {
                mode: MathMode::Geometric,
                angle_unit: AngleUnit::Radians,
            },
            bridge_contradictions,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            synthesize_final_outcome(&[], &[], Some(ComplexValue::new(1.0, 0.0)), None),
        );
        let bridge = bridge_audit_value(&envelope);
        let consistency = bridge
            .get("consistency")
            .expect("bridge consistency should exist");
        assert_eq!(
            consistency.get("math_logic_alignment"),
            Some(&json!("Conflicted"))
        );
        let bridge_list = consistency
            .get("contradictions")
            .and_then(Value::as_array)
            .expect("bridge contradictions should exist");
        assert!(!bridge_list.is_empty());
        assert!(bridge_list.iter().any(|c| {
            c.get("contradiction_id")
                .and_then(Value::as_str)
                .map(|id| id.contains("unit_conversion_loop_torsion_exceeded"))
                .unwrap_or(false)
        }));
        let contradiction_count = bridge
            .get("final_outcome")
            .and_then(|fo| fo.get("machine_summary"))
            .and_then(|ms| ms.get("contradiction_count"))
            .and_then(Value::as_u64)
            .expect("machine_summary contradiction_count should exist");
        assert_eq!(contradiction_count as usize, bridge_list.len());
        assert_eq!(
            bridge
                .get("final_outcome")
                .and_then(|fo| fo.get("status")),
            Some(&json!("QualifiedSuccess"))
        );
        assert!(
            bridge
                .get("final_outcome")
                .and_then(|fo| fo.get("responder_text"))
                .and_then(Value::as_str)
                .expect("responder_text should exist")
                .contains("contradiction-qualified")
        );
    }

    #[test]
    fn logic_inference_torsion_threshold_emits_explicit_contradiction_signal() {
        let expr = LogicExprNode::Implies(
            Box::new(LogicExprNode::BoolLiteral(true)),
            Box::new(LogicExprNode::BoolLiteral(true)),
        );
        let inference_morphisms = build_modus_ponens_morphisms(&expr, 0.0, 1.0);
        let signal = build_logic_contradiction_signal(&inference_morphisms);

        assert_eq!(signal.get("triggered"), Some(&json!(true)));
        assert_eq!(
            signal.get("stop_reason"),
            Some(&json!("logic_inference_torsion_exceeded"))
        );

        let contradictions = signal
            .get("contradictions")
            .and_then(Value::as_array)
            .expect("logic contradictions should exist");
        assert!(contradictions.iter().any(|c| {
            c.get("code") == Some(&json!("logic_inference_torsion_exceeded"))
        }));

        let bridge_contradictions = logic_contradictions_from_payload(&signal);
        let envelope = build_success_envelope(
            "true implies true",
            ParsedUnit::LogicExpr(expr),
            PrimaryGoal::CheckTruth,
            MathOptions {
                mode: MathMode::Geometric,
                angle_unit: AngleUnit::Radians,
            },
            bridge_contradictions,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            synthesize_final_outcome(&[], &[], None, Some(TruthValue::True)),
        );
        let bridge = bridge_audit_value(&envelope);
        let consistency = bridge
            .get("consistency")
            .expect("bridge consistency should exist");
        assert_eq!(
            consistency.get("math_logic_alignment"),
            Some(&json!("Conflicted"))
        );
        let bridge_list = consistency
            .get("contradictions")
            .and_then(Value::as_array)
            .expect("bridge contradictions should exist");
        assert!(!bridge_list.is_empty());
        assert!(bridge_list.iter().any(|c| {
            c.get("contradiction_id")
                .and_then(Value::as_str)
                .map(|id| id.contains("logic_inference_torsion_exceeded"))
                .unwrap_or(false)
        }));
        let contradiction_count = bridge
            .get("final_outcome")
            .and_then(|fo| fo.get("machine_summary"))
            .and_then(|ms| ms.get("contradiction_count"))
            .and_then(Value::as_u64)
            .expect("machine_summary contradiction_count should exist");
        assert_eq!(contradiction_count as usize, bridge_list.len());
        assert_eq!(
            bridge
                .get("final_outcome")
                .and_then(|fo| fo.get("status")),
            Some(&json!("QualifiedSuccess"))
        );
        assert!(
            bridge
                .get("final_outcome")
                .and_then(|fo| fo.get("responder_text"))
                .and_then(Value::as_str)
                .expect("responder_text should exist")
                .contains("contradiction-qualified")
        );
    }

    #[test]
    fn logic_t504_low_threshold_propagates_via_evaluate_math_expression() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };

        let payload = with_env_var("CSIF_LOGIC_INFERENCE_TORSION_THRESHOLD", "0.0", || {
            evaluate_math_expression("1 < 2 implies isreal(5)", &state, MathOptions::default())
                .expect("logic implication should evaluate under low threshold profile")
        });

        let logic_signal = payload
            .get("logic_contradiction_signal")
            .expect("logic_contradiction_signal should exist");
        assert_eq!(logic_signal.get("triggered"), Some(&json!(true)));
        assert_eq!(
            logic_signal.get("stop_reason"),
            Some(&json!("logic_inference_torsion_exceeded"))
        );

        let anticrystal_lob = payload
            .get("anticrystal_lob")
            .expect("anticrystal_lob should exist");
        assert_eq!(anticrystal_lob.get("lobe"), Some(&json!("anticrystal")));
        let anti_entries = anticrystal_lob
            .get("entries")
            .and_then(Value::as_array)
            .expect("anticrystal entries should exist");
        assert!(!anti_entries.is_empty());
        assert!(anti_entries.iter().any(|entry| {
            entry
                .get("contradiction_id")
                .and_then(Value::as_str)
                .map(|id| id.contains("logic_inference_torsion_exceeded"))
                .unwrap_or(false)
                && entry
                    .get("this_is_wrong_because")
                    .and_then(Value::as_str)
                    .map(|msg| msg.contains("exceeded torsion threshold"))
                    .unwrap_or(false)
        }));

        let bridge = payload
            .get("bridge_audit")
            .expect("bridge_audit should exist");
        let consistency = bridge
            .get("consistency")
            .expect("bridge consistency should exist");
        let bridge_list = consistency
            .get("contradictions")
            .and_then(Value::as_array)
            .expect("bridge contradictions should exist");
        assert!(bridge_list.iter().any(|c| {
            c.get("contradiction_id")
                .and_then(Value::as_str)
                .map(|id| id.contains("logic_inference_torsion_exceeded"))
                .unwrap_or(false)
        }));

        let contradiction_count = bridge
            .get("final_outcome")
            .and_then(|fo| fo.get("machine_summary"))
            .and_then(|ms| ms.get("contradiction_count"))
            .and_then(Value::as_u64)
            .expect("machine_summary contradiction_count should exist");
        assert_eq!(contradiction_count as usize, bridge_list.len());
        assert_eq!(
            bridge
                .get("final_outcome")
                .and_then(|fo| fo.get("status")),
            Some(&json!("QualifiedSuccess"))
        );
        assert!(
            bridge
                .get("final_outcome")
                .and_then(|fo| fo.get("responder_text"))
                .and_then(Value::as_str)
                .expect("responder_text should exist")
                .contains("contradiction-qualified")
        );
    }

    #[test]
    fn time_crystal_randomness_profile_conformance_stub_emits_deterministic_replay_keys() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };

        let payload_a = with_env_vars(
            &[
                ("CSIF_EMIT_TIME_CRYSTAL_RANDOMNESS", "true"),
                ("CSIF_TIME_CRYSTAL_T_NS", "1717171717000000000"),
            ],
            || {
                evaluate_math_expression("1 < 2 implies isreal(5)", &state, MathOptions::default())
                    .expect("logic implication should evaluate with time crystal profile")
            },
        );

        // Conformance stub for Section 13.6 profile wiring.
        let time_crystal = payload_a
            .get("time_crystal")
            .expect("time_crystal should be emitted when profile is enabled");
        assert_eq!(
            time_crystal.get("t_ns"),
            Some(&json!(1717171717000000000_u128))
        );
        assert!(
            time_crystal
                .get("phase_theta")
                .and_then(Value::as_f64)
                .is_some()
        );
        assert!(
            time_crystal
                .get("torsion_norm")
                .and_then(Value::as_f64)
                .is_some()
        );

        let randomness_a = payload_a
            .get("randomness_appearance")
            .expect("randomness_appearance should be emitted when profile is enabled");
        assert_eq!(
            randomness_a.get("mode"),
            Some(&json!("deterministic_time_chaos"))
        );
        let replay_key_a = randomness_a
            .get("replay_key")
            .and_then(Value::as_str)
            .expect("replay_key should exist")
            .to_string();
        let audit_trace_id_a = randomness_a
            .get("audit_trace_id")
            .and_then(Value::as_str)
            .expect("audit_trace_id should exist")
            .to_string();

        let payload_b = with_env_vars(
            &[
                ("CSIF_EMIT_TIME_CRYSTAL_RANDOMNESS", "true"),
                ("CSIF_TIME_CRYSTAL_T_NS", "1717171717000000000"),
            ],
            || {
                evaluate_math_expression("1 < 2 implies isreal(5)", &state, MathOptions::default())
                    .expect("logic implication should evaluate with fixed time crystal")
            },
        );
        let randomness_b = payload_b
            .get("randomness_appearance")
            .expect("randomness_appearance should exist on repeated evaluation");
        let replay_key_b = randomness_b
            .get("replay_key")
            .and_then(Value::as_str)
            .expect("replay_key should exist")
            .to_string();
        let audit_trace_id_b = randomness_b
            .get("audit_trace_id")
            .and_then(Value::as_str)
            .expect("audit_trace_id should exist")
            .to_string();

        assert_eq!(replay_key_a, replay_key_b);
        assert_eq!(audit_trace_id_a, audit_trace_id_b);
    }

    #[test]
    fn time_crystal_randomness_profile_conformance_stub_omits_fields_without_opt_in() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };

        let payload = with_env_var("CSIF_EMIT_TIME_CRYSTAL_RANDOMNESS", "false", || {
            evaluate_math_expression("1 < 2 implies isreal(5)", &state, MathOptions::default())
                .expect("logic implication should evaluate without opt-in profile")
        });

        assert!(
            payload.get("time_crystal").is_none(),
            "time_crystal should be absent when opt-in is disabled"
        );
        assert!(
            payload.get("randomness_appearance").is_none(),
            "randomness_appearance should be absent when opt-in is disabled"
        );
    }

    #[test]
    fn geometric_mode_scales_imaginary_decimal_sum() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };
        let payload = evaluate_math_expression(
            "0.1i+0.2i",
            &state,
            MathOptions {
                mode: MathMode::Geometric,
                angle_unit: AngleUnit::Radians,
            },
        )
        .expect("imaginary decimal sum should evaluate");

        let result = payload.get("result").expect("result should exist");
        let re = result.get("re").and_then(Value::as_f64).expect("complex real part should exist");
        let im = result.get("im").and_then(Value::as_f64).expect("complex imaginary part should exist");
        assert!(re.abs() < 1e-12);
        assert!((im - 0.3).abs() < 1e-12);
        assert_ne!(im, 0.30000000000000004_f64);
    }

    #[test]
    fn geometric_mode_preserves_component_separation_with_scaled_magnitudes() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };
        let payload = evaluate_math_expression(
            "0.1+0.2i",
            &state,
            MathOptions {
                mode: MathMode::Geometric,
                angle_unit: AngleUnit::Radians,
            },
        )
        .expect("mixed-axis expression should evaluate");

        let result = payload.get("result").expect("result should exist");
        let re = result.get("re").and_then(Value::as_f64).expect("complex real part should exist");
        let im = result.get("im").and_then(Value::as_f64).expect("complex imaginary part should exist");
        assert!((re - 0.1).abs() < 1e-12);
        assert!((im - 0.2).abs() < 1e-12);
    }

    #[test]
    fn geometric_mode_nested_mixed_axes_yield_exact_scaled_components() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };
        let payload = evaluate_math_expression(
            "(0.1i+0.2i)+(0.1+0.2)",
            &state,
            MathOptions {
                mode: MathMode::Geometric,
                angle_unit: AngleUnit::Radians,
            },
        )
        .expect("nested mixed expression should evaluate");

        let result = payload.get("result").expect("result should exist");
        let re = result.get("re").and_then(Value::as_f64).expect("complex real part should exist");
        let im = result.get("im").and_then(Value::as_f64).expect("complex imaginary part should exist");
        assert!((re - 0.3).abs() < 1e-12);
        assert!((im - 0.3).abs() < 1e-12);
    }

    #[test]
    fn scientific_constants_and_functions_work() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
        sense_trajectory_log_path: None,
        };
        let payload = evaluate_math_expression("sqrt(9) + ln(e)", &state, MathOptions::default())
            .expect("scientific functions should evaluate");
        let result = payload
            .get("result")
            .and_then(Value::as_f64)
            .expect("result should be numeric");
        assert!((result - 4.0).abs() < 1e-12);
    }

    #[test]
    fn hyperbolic_and_inverse_hyperbolic_functions_work() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };
        let payload = evaluate_math_expression(
            "cosh(0) + sinh(0) + tanh(0) + asinh(0) + acosh(1) + atanh(0)",
            &state,
            MathOptions::default(),
        )
        .expect("hyperbolic functions should evaluate");

        let result = payload
            .get("result")
            .and_then(Value::as_f64)
            .expect("result should be numeric");
        assert!((result - 1.0).abs() < 1e-12);
    }

    #[test]
    fn comb_and_c_alias_support_two_arguments() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };
        let payload = evaluate_math_expression("comb(12,5) + C(9 4)", &state, MathOptions::default())
            .expect("comb and C should evaluate");
        let result = payload
            .get("result")
            .and_then(Value::as_f64)
            .expect("result should be numeric");
        assert!((result - 918.0).abs() < 1e-12);
    }

    #[test]
    fn atan2_supports_two_arguments() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };
        let payload = evaluate_math_expression("atan2(-3,2)", &state, MathOptions::default())
            .expect("atan2 should evaluate");
        let result = payload
            .get("result")
            .and_then(Value::as_f64)
            .expect("result should be numeric");
        assert!((result - (-3.0f64).atan2(2.0)).abs() < 1e-12);
    }

    #[test]
    fn bessel_j012_are_supported() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };
        let payload = evaluate_math_expression("J0(0)+J1(0)+J2(0)", &state, MathOptions::default())
            .expect("bessel functions should evaluate");
        let result = payload
            .get("result")
            .and_then(Value::as_f64)
            .expect("result should be numeric");
        assert!((result - 1.0).abs() < 1e-12);
    }

    #[test]
    fn gamma_supports_real_and_complex_inputs() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };

        let gamma_one = evaluate_math_expression("gamma(1)", &state, MathOptions::default())
            .expect("gamma(1) should evaluate");
        let g1 = gamma_one
            .get("result")
            .and_then(Value::as_f64)
            .expect("gamma(1) result should be numeric");
        assert!((g1 - 1.0).abs() < 1e-12);

        let gamma_complex = evaluate_math_expression(
            "gamma(0.3+0.8i)",
            &state,
            MathOptions::default(),
        )
        .expect("complex gamma should evaluate");
        let complex_result = gamma_complex
            .get("result")
            .and_then(Value::as_object)
            .expect("complex gamma result should be an object");
        let re = complex_result
            .get("re")
            .and_then(Value::as_f64)
            .expect("gamma complex result should have re");
        let im = complex_result
            .get("im")
            .and_then(Value::as_f64)
            .expect("gamma complex result should have im");
        assert!(re.is_finite() && im.is_finite());
    }

    #[test]
    fn hard_expression_with_gamma_comb_bessel_and_atan2_runs_end_to_end() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };

        let payload = evaluate_math_expression(
            "( J1(J0(1.3)) * C(14 6) - exp(-0.7 + 1.9i) + gamma(0.3 + 0.8i) ) * ( log(-2 + 0.00001i) + sqrt(-3 - 5i) + atan2(5 -7) )",
            &state,
            MathOptions::default(),
        )
        .expect("hard mixed expression should evaluate");

        assert_eq!(payload.get("deterministic"), Some(&json!(true)));
        let complex_result = payload
            .get("result")
            .and_then(Value::as_object)
            .expect("result should be complex object");
        assert!(
            complex_result
                .get("re")
                .and_then(Value::as_f64)
                .expect("result re should exist")
                .is_finite()
        );
        assert!(
            complex_result
                .get("im")
                .and_then(Value::as_f64)
                .expect("result im should exist")
                .is_finite()
        );
    }

    #[test]
    fn zeta_supports_critical_strip_inputs() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };

        let payload = evaluate_math_expression("zeta(0.5+14i)", &state, MathOptions::default())
            .expect("zeta should evaluate for Re(s)>0");
        let complex_result = payload
            .get("result")
            .and_then(Value::as_object)
            .expect("zeta result should be complex object");
        assert!(
            complex_result
                .get("re")
                .and_then(Value::as_f64)
                .expect("zeta re should exist")
                .is_finite()
        );
        assert!(
            complex_result
                .get("im")
                .and_then(Value::as_f64)
                .expect("zeta im should exist")
                .is_finite()
        );
    }

    #[test]
    fn hard_expression_with_zeta_gamma_bessel_comb_and_atan2_runs_end_to_end() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };

        let payload = evaluate_math_expression(
            "( J2(J1(0.9)) * C(16 7) + exp(0.4 - 2.3i) - gamma(1.2 - 0.6i) + zeta(0.5 + 14i) ) * ( log(-3 + 0.000001i) + sqrt(-6 + 2i) + atan2(-4 9) )",
            &state,
            MathOptions::default(),
        )
        .expect("hard zeta expression should evaluate");

        assert_eq!(payload.get("deterministic"), Some(&json!(true)));
        let complex_result = payload
            .get("result")
            .and_then(Value::as_object)
            .expect("result should be complex object");
        assert!(
            complex_result
                .get("re")
                .and_then(Value::as_f64)
                .expect("result re should exist")
                .is_finite()
        );
        assert!(
            complex_result
                .get("im")
                .and_then(Value::as_f64)
                .expect("result im should exist")
                .is_finite()
        );
    }

    #[test]
    fn lambertw_supports_complex_inputs() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };

        let payload = evaluate_math_expression("lambertw(-1+3i)", &state, MathOptions::default())
            .expect("lambertw should evaluate");
        let complex_result = payload
            .get("result")
            .and_then(Value::as_object)
            .expect("lambertw result should be complex object");
        let re = complex_result
            .get("re")
            .and_then(Value::as_f64)
            .expect("lambertw re should exist");
        let im = complex_result
            .get("im")
            .and_then(Value::as_f64)
            .expect("lambertw im should exist");
        assert!(re.is_finite() && im.is_finite());
    }

    #[test]
    fn hard_expression_with_lambertw_j3_and_left_half_plane_zeta_runs_end_to_end() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };

        let payload = evaluate_math_expression(
            "( lambertw( -1 + 3i ) + J1( gamma(0.4 + 1.1i) ) - C(18 9) * J3(0.7) + zeta( -1.3 + 9i ) ) * ( log(-4 + 0.0000001i) + sqrt(-8 - 3i) + atan2(11 -5) )",
            &state,
            MathOptions::default(),
        )
        .expect("hard expression with lambertw/j3/zeta should evaluate");

        assert_eq!(payload.get("deterministic"), Some(&json!(true)));
        let complex_result = payload
            .get("result")
            .and_then(Value::as_object)
            .expect("result should be complex object");
        assert!(
            complex_result
                .get("re")
                .and_then(Value::as_f64)
                .expect("result re should exist")
                .is_finite()
        );
        assert!(
            complex_result
                .get("im")
                .and_then(Value::as_f64)
                .expect("result im should exist")
                .is_finite()
        );
    }

    #[test]
    fn polylog_gammainc_and_spherical_bessel_support() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };

        let payload = evaluate_math_expression(
            "polylog(2, 0.6 + 0.4i) + gammainc(1.3, 0.7 - 1.1i) - C(15 6) * j_sph(2, 1.1) + zeta(0.2 + 7i)",
            &state,
            MathOptions::default(),
        )
        .expect("polylog/gammainc/j_sph expression should evaluate");

        let complex_result = payload
            .get("result")
            .and_then(Value::as_object)
            .expect("result should be complex object");
        assert!(
            complex_result
                .get("re")
                .and_then(Value::as_f64)
                .expect("result re should exist")
                .is_finite()
        );
        assert!(
            complex_result
                .get("im")
                .and_then(Value::as_f64)
                .expect("result im should exist")
                .is_finite()
        );
    }

    #[test]
    fn deep_nested_polylog_expression_runs_end_to_end() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };

        let payload = evaluate_math_expression(
            "( polylog(4, polylog(3, zeta(0.8 + 4i))) + gammainc(2.7, lambertw(-0.2 + 3.1i) * zeta(-0.6 + 9i)) - C(22 11) * j_sph(6, 1.2) + zeta(0.3 + 17i) * J2(lambertw(0.4 - 2.8i)) + j_sph(3, gamma(0.9 - 1.4i)) - polylog(2, polylog(2, lambertw(1.1 + 0.9i) * zeta(0.1 + 12i))) ) * ( log(-9 + 0.00000000001i) + sqrt(-13 - 8i) + atan2(12, -13) + J1(1.4) * j_sph(4, 0.9) )",
            &state,
            MathOptions::default(),
        )
        .expect("deep nested polylog expression should evaluate");

        assert_eq!(payload.get("deterministic"), Some(&json!(true)));
        let complex_result = payload
            .get("result")
            .and_then(Value::as_object)
            .expect("result should be complex object");
        assert!(
            complex_result
                .get("re")
                .and_then(Value::as_f64)
                .expect("result re should exist")
                .is_finite()
        );
        assert!(
            complex_result
                .get("im")
                .and_then(Value::as_f64)
                .expect("result im should exist")
                .is_finite()
        );
    }

    #[test]
    fn det_and_permanent_scalar_wrappers_are_supported() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };

        let payload = evaluate_math_expression(
            "det(exp(0.4+3i) * zeta(0.5+11i)) + permanent(exp(j_sph(3,0.9) * C(12 6)))",
            &state,
            MathOptions::default(),
        )
        .expect("det/permanent scalar wrappers should evaluate");

        let result = payload
            .get("result")
            .and_then(Value::as_object)
            .expect("result should be complex object");
        assert!(
            result
                .get("re")
                .and_then(Value::as_f64)
                .expect("result re should exist")
                .is_finite()
        );
        assert!(
            result
                .get("im")
                .and_then(Value::as_f64)
                .expect("result im should exist")
                .is_finite()
        );
    }

    #[test]
    fn chat_math_failure_emits_structured_error_metadata() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };
        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: Value::String("/math frob(1)".to_string()),
        }];

        let (_answer, meta) = build_chat_answer(&messages, &state, None);
        assert_eq!(
            meta.get("math")
                .and_then(|v| v.get("status"))
                .and_then(Value::as_str),
            Some("unsupported_function")
        );
        assert_eq!(
            meta.get("math")
                .and_then(|v| v.get("error_code"))
                .and_then(Value::as_str),
            Some("MATH_UNSUPPORTED_FUNCTION")
        );
    }

    #[test]
    fn chat_identity_prompt_reports_intent_and_friendly_intro() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };
        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: Value::String("Who are you and what can you do?".to_string()),
        }];

        let (answer, meta) = build_chat_answer(&messages, &state, None);
        assert!(answer.contains("I am UGC-Model"));
        assert!(answer.contains("Next options:"));
        assert_eq!(
            meta.get("conversation")
                .and_then(|v| v.get("intent"))
                .and_then(Value::as_str),
            Some("identity")
        );
    }

    #[test]
    fn chat_concise_prompt_sets_concise_response_style() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };
        let messages = vec![
            ChatMessage {
                role: "user".to_string(),
                content: Value::String("I am exploring your capabilities.".to_string()),
            },
            ChatMessage {
                role: "user".to_string(),
                content: Value::String("Give me a brief answer on what you can do.".to_string()),
            },
        ];

        let (answer, meta) = build_chat_answer(&messages, &state, None);
        assert!(answer.contains("Context carryover:"));
        assert_eq!(
            meta.get("conversation")
                .and_then(|v| v.get("response_style"))
                .and_then(Value::as_str),
            Some("concise")
        );
    }

    #[test]
    fn chat_greeting_uses_time_crystal_opening_variation_with_replay_override() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };
        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: Value::String("hello".to_string()),
        }];

        let (_answer, meta) = with_env_vars(
            &[
                ("CSIF_CHAT_TIME_CRYSTAL_OPENING_VARIATION", "true"),
                ("CSIF_TIME_CRYSTAL_T_NS", "1717171717000000000"),
            ],
            || build_chat_answer(&messages, &state, None),
        );

        assert_eq!(
            meta.get("conversation")
                .and_then(|v| v.get("opening_randomness"))
                .and_then(|v| v.get("enabled")),
            Some(&Value::Bool(true))
        );
        assert_eq!(
            meta.get("conversation")
                .and_then(|v| v.get("opening_randomness"))
                .and_then(|v| v.get("time_crystal"))
                .and_then(|v| v.get("coordinate_source"))
                .and_then(Value::as_str),
            Some("env:CSIF_TIME_CRYSTAL_T_NS")
        );
        assert_eq!(
            meta.get("conversation")
                .and_then(|v| v.get("retrieval_fallback_randomness"))
                .and_then(|v| v.get("enabled")),
            Some(&Value::Bool(true))
        );
        assert_eq!(
            meta.get("conversation")
                .and_then(|v| v.get("next_options_randomness"))
                .and_then(|v| v.get("enabled")),
            Some(&Value::Bool(true))
        );
    }

    #[test]
    fn chat_greeting_opening_variation_can_be_disabled() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };
        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: Value::String("hello".to_string()),
        }];

        let (answer, meta) = with_env_vars(
            &[("CSIF_CHAT_TIME_CRYSTAL_OPENING_VARIATION", "false")],
            || build_chat_answer(&messages, &state, None),
        );

        assert!(answer.starts_with("Hey, great to connect. I am ready to help."));
        assert_eq!(
            meta.get("conversation")
                .and_then(|v| v.get("opening_randomness"))
                .and_then(|v| v.get("enabled")),
            Some(&Value::Bool(false))
        );
        assert_eq!(
            meta.get("conversation")
                .and_then(|v| v.get("opening_randomness"))
                .and_then(|v| v.get("reason"))
                .and_then(Value::as_str),
            Some("disabled_by_env")
        );
        assert_eq!(
            meta.get("conversation")
                .and_then(|v| v.get("retrieval_fallback_randomness"))
                .and_then(|v| v.get("enabled")),
            Some(&Value::Bool(false))
        );
        assert_eq!(
            meta.get("conversation")
                .and_then(|v| v.get("next_options_randomness"))
                .and_then(|v| v.get("enabled")),
            Some(&Value::Bool(false))
        );
    }

    #[test]
    fn chat_preferences_override_style_depth_and_tone() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };
        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: Value::String("help me map the best rollout path".to_string()),
        }];
        let prefs = ChatPreferencesRequest {
            response_style: Some("standard".to_string()),
            depth: Some("deep".to_string()),
            tone: Some("professional".to_string()),
            warmth_ceiling: Some("subtle".to_string()),
            retrieval_summary: Some(true),
            retrieval_top_k: Some(7),
        };

        let (answer, meta) = build_chat_answer(&messages, &state, Some(&prefs));
        assert!(answer.starts_with("Please share your target outcome"));
        assert_eq!(
            meta.get("conversation")
                .and_then(|v| v.get("response_style"))
                .and_then(Value::as_str),
            Some("standard")
        );
        assert_eq!(
            meta.get("conversation")
                .and_then(|v| v.get("depth"))
                .and_then(Value::as_str),
            Some("deep")
        );
        assert_eq!(
            meta.get("conversation")
                .and_then(|v| v.get("tone"))
                .and_then(Value::as_str),
            Some("professional")
        );
        assert_eq!(
            meta.get("conversation")
                .and_then(|v| v.get("warmth_ceiling"))
                .and_then(Value::as_str),
            Some("subtle")
        );
    }

    #[test]
    fn chat_greeting_warmth_ceiling_env_is_applied() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };
        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: Value::String("hello".to_string()),
        }];

        let (_answer, meta) = with_env_vars(
            &[
                ("CSIF_CHAT_TIME_CRYSTAL_OPENING_VARIATION", "true"),
                ("CSIF_CHAT_GREETING_WARMTH_CEILING", "subtle"),
                ("CSIF_TIME_CRYSTAL_T_NS", "1717171717000000000"),
            ],
            || build_chat_answer(&messages, &state, None),
        );

        assert_eq!(
            meta.get("conversation")
                .and_then(|v| v.get("warmth_ceiling"))
                .and_then(Value::as_str),
            Some("subtle")
        );
    }

    #[test]
    fn chat_greeting_warmth_ceiling_payload_overrides_env() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };
        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: Value::String("hello".to_string()),
        }];
        let prefs = ChatPreferencesRequest {
            response_style: None,
            depth: None,
            tone: Some("friendly".to_string()),
            warmth_ceiling: Some("expressive".to_string()),
            retrieval_summary: None,
            retrieval_top_k: None,
        };

        let (_answer, meta) = with_env_vars(
            &[
                ("CSIF_CHAT_TIME_CRYSTAL_OPENING_VARIATION", "true"),
                ("CSIF_CHAT_GREETING_WARMTH_CEILING", "subtle"),
                ("CSIF_TIME_CRYSTAL_T_NS", "1717171717000000000"),
            ],
            || build_chat_answer(&messages, &state, Some(&prefs)),
        );

        assert_eq!(
            meta.get("conversation")
                .and_then(|v| v.get("warmth_ceiling"))
                .and_then(Value::as_str),
            Some("expressive")
        );
    }

    #[test]
    fn conversation_benchmark_prompt_set_contract() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };
        let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("benchmarks")
            .join("conversation_prompt_set_v1.json");
        let fixture_text =
            std::fs::read_to_string(&fixture_path).expect("conversation benchmark fixture should load");
        let suite: Value = serde_json::from_str(&fixture_text).expect("fixture should be valid json");
        let cases = suite
            .as_array()
            .expect("conversation benchmark fixture should be an array");
        assert!(!cases.is_empty(), "conversation benchmark fixture should not be empty");

        for case in cases {
            let prompt = case
                .get("prompt")
                .and_then(Value::as_str)
                .expect("benchmark case should include prompt");
            let expected_intent = case
                .get("expected_intent")
                .and_then(Value::as_str)
                .expect("benchmark case should include expected_intent");
            let expected_style = case
                .get("expected_style")
                .and_then(Value::as_str)
                .expect("benchmark case should include expected_style");

            let preferences = case
                .get("preferences")
                .cloned()
                .map(serde_json::from_value::<ChatPreferencesRequest>)
                .transpose()
                .expect("preferences object should deserialize when provided");

            let messages = vec![ChatMessage {
                role: "user".to_string(),
                content: Value::String(prompt.to_string()),
            }];
            let (answer, meta) = build_chat_answer(&messages, &state, preferences.as_ref());

            assert!(!answer.trim().is_empty(), "answer should not be empty for prompt: {prompt}");
            assert!(answer.contains("Next options:"), "answer shape should include Next options for prompt: {prompt}");
            assert_eq!(
                meta.get("conversation")
                    .and_then(|v| v.get("intent"))
                    .and_then(Value::as_str),
                Some(expected_intent),
                "intent mismatch for prompt: {prompt}"
            );
            assert_eq!(
                meta.get("conversation")
                    .and_then(|v| v.get("response_style"))
                    .and_then(Value::as_str),
                Some(expected_style),
                "response style mismatch for prompt: {prompt}"
            );
            assert!(
                meta.get("conversation")
                    .and_then(|v| v.get("suggestions"))
                    .and_then(Value::as_array)
                    .map(|a| !a.is_empty())
                    .unwrap_or(false),
                "conversation suggestions should be present for prompt: {prompt}"
            );

            if let Some(required) = case.get("required_substrings").and_then(Value::as_array) {
                for token in required.iter().filter_map(Value::as_str) {
                    assert!(
                        answer.contains(token),
                        "answer should contain required token '{token}' for prompt: {prompt}"
                    );
                }
            }
        }
    }

    #[test]
    fn disambiguation_resolves_visible_light() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
        sense_trajectory_log_path: None,
        };

        let payload = disambiguate_payload(
            &state,
            "en",
            "light",
            "the light helped me see with my eyes in the room",
            5,
            0.75,
        );

        assert_eq!(payload.get("status"), Some(&json!("resolved")));
        let selected = payload
            .get("selected_sense")
            .and_then(|v| v.get("node_id"))
            .and_then(Value::as_str)
            .expect("selected sense should exist");
        assert_eq!(selected, "sense_light_visible_em");
    }

    #[test]
    fn disambiguation_resolves_insight_light() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
        sense_trajectory_log_path: None,
        };

        let payload = disambiguate_payload(
            &state,
            "en",
            "light",
            "the lecture gave me insight and understanding",
            5,
            0.75,
        );

        assert_eq!(payload.get("status"), Some(&json!("resolved")));
        let selected = payload
            .get("selected_sense")
            .and_then(|v| v.get("node_id"))
            .and_then(Value::as_str)
            .expect("selected sense should exist");
        assert_eq!(selected, "sense_light_insight");
    }

    #[test]
    fn disambiguation_marks_unknown_lexeme() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
        sense_trajectory_log_path: None,
        };

        let payload = disambiguate_payload(&state, "en", "quasarword", "some context", 5, 0.75);
        assert_eq!(payload.get("status"), Some(&json!("unknown_lexeme")));
        assert_eq!(payload.get("selected_sense"), Some(&Value::Null));
    }

    #[test]
    fn disambiguation_surfaces_compact_lexicon_coverage_and_edges() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };

        let payload = disambiguate_payload(
            &state,
            "es",
            "luz",
            "la luz y la luminosidad ayudan a ver",
            5,
            0.75,
        );

        assert_eq!(
            payload
                .get("lexicon")
                .and_then(|v| v.get("pack"))
                .and_then(Value::as_str),
            Some("csif_compact_lexicon_v1")
        );

        let coverage_ratio = payload
            .get("lexicon")
            .and_then(|v| v.get("coverage"))
            .and_then(|v| v.get("coverage_ratio"))
            .and_then(Value::as_f64)
            .expect("lexicon coverage ratio should exist");
        assert!(coverage_ratio > 0.0);

        let edges = payload
            .get("lexicon")
            .and_then(|v| v.get("matched_lexicon_edges"))
            .and_then(Value::as_array)
            .expect("matched lexicon edges should exist");
        assert!(!edges.is_empty());
        assert!(edges.iter().any(|edge| {
            edge.get("language").and_then(Value::as_str) == Some("es")
                && edge.get("concept_node").and_then(Value::as_str)
                    == Some("concept_light_visible")
        }));
    }

    #[test]
    fn disambiguation_candidates_include_lexicon_support_feature() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };

        let payload = disambiguate_payload(
            &state,
            "fr",
            "lumière",
            "la lumière est visible et la vision est claire",
            5,
            0.75,
        );

        let candidates = payload
            .get("candidates")
            .and_then(Value::as_array)
            .expect("candidates should exist");
        let visible = candidates
            .iter()
            .find(|c| {
                c.get("sense_node")
                    .and_then(|v| v.get("node_id"))
                    .and_then(Value::as_str)
                    == Some("sense_light_visible_em")
            })
            .expect("visible sense candidate should exist");

        let lexicon_support = visible
            .get("features")
            .and_then(|v| v.get("lexicon_support"))
            .and_then(Value::as_f64)
            .expect("lexicon support feature should exist");
        assert!(lexicon_support > 0.0);
    }

    #[test]
    fn unknown_lexeme_marks_lexicon_gap_when_no_edges_match() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };

        let payload = disambiguate_payload(&state, "en", "quasarword", "blorf snark", 5, 0.75);
        assert_eq!(payload.get("status"), Some(&json!("unknown_lexeme")));
        assert_eq!(
            payload
                .get("lexicon")
                .and_then(|v| v.get("unknown_due_to_lexicon_gap"))
                .and_then(Value::as_bool),
            Some(true)
        );
    }

    #[test]
    fn legacy_pack_matches_older_vocabulary_in_existing_languages() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };

        let payload = disambiguate_payload(
            &state,
            "en",
            "light",
            "the effulgence and refulgence filled the hall",
            5,
            0.75,
        );

        let edges = payload
            .get("lexicon")
            .and_then(|v| v.get("matched_lexicon_edges"))
            .and_then(Value::as_array)
            .expect("matched lexicon edges should exist");
        assert!(edges.iter().any(|edge| {
            edge.get("pack").and_then(Value::as_str) == Some("csif_compact_lexicon_legacy_v2")
                && edge.get("lemma").and_then(Value::as_str) == Some("effulgence")
        }));
    }

    #[test]
    fn reasoning_pack_matches_uncommon_dividual_style_terms() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };

        let payload = disambiguate_payload(
            &state,
            "en",
            "light",
            "dividual analysis improves individual understanding",
            5,
            0.75,
        );

        let edges = payload
            .get("lexicon")
            .and_then(|v| v.get("matched_lexicon_edges"))
            .and_then(Value::as_array)
            .expect("matched lexicon edges should exist");
        assert!(edges.iter().any(|edge| {
            edge.get("pack").and_then(Value::as_str)
                == Some("csif_compact_lexicon_reasoning_v2")
                && edge.get("lemma").and_then(Value::as_str) == Some("dividual")
        }));

        let packs = payload
            .get("lexicon")
            .and_then(|v| v.get("packs"))
            .and_then(Value::as_array)
            .expect("lexicon packs should exist");
        assert!(packs.iter().any(|p| p == &json!("csif_compact_lexicon_reasoning_v2")));
    }

    #[test]
    fn etymology_pack_matches_luc_clar_family_in_english() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };

        let payload = disambiguate_payload(
            &state,
            "en",
            "light",
            "lucidity helps elucidate and clarify difficult ideas",
            5,
            0.75,
        );

        let edges = payload
            .get("lexicon")
            .and_then(|v| v.get("matched_lexicon_edges"))
            .and_then(Value::as_array)
            .expect("matched lexicon edges should exist");
        assert!(edges.iter().any(|edge| {
            edge.get("pack").and_then(Value::as_str)
                == Some("csif_compact_lexicon_etymology_v3")
                && edge.get("lemma").and_then(Value::as_str) == Some("lucidity")
        }));
    }

    #[test]
    fn etymology_pack_matches_luc_clar_family_in_french() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };

        let payload = disambiguate_payload(
            &state,
            "fr",
            "lumière",
            "la lucidite permet d elucider et clarifier le propos",
            5,
            0.75,
        );

        let edges = payload
            .get("lexicon")
            .and_then(|v| v.get("matched_lexicon_edges"))
            .and_then(Value::as_array)
            .expect("matched lexicon edges should exist");
        assert!(edges.iter().any(|edge| {
            edge.get("pack").and_then(Value::as_str)
                == Some("csif_compact_lexicon_etymology_v3")
                && edge.get("lemma").and_then(Value::as_str) == Some("lucidite")
        }));
    }

    #[test]
    fn disambiguation_respects_lexicon_pack_toggles_for_ab_controls() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };

        let active = default_frame_context();
        let prior = default_frame_context();
        let policy = default_conservation_policy();
        let only_v1 = resolve_lexicon_control(Some(&vec!["csif_compact_lexicon_v1".to_string()]));

        let payload = disambiguate_payload_with_inertia_and_frame_and_policy(
            &state,
            "en",
            "light",
            "lucidity helps elucidate and clarify difficult ideas",
            5,
            0.75,
            None,
            0.35,
            true,
            &active,
            &prior,
            &policy,
            &only_v1,
        );

        let packs = payload
            .get("lexicon")
            .and_then(|v| v.get("packs"))
            .and_then(Value::as_array)
            .expect("lexicon packs should exist");
        assert_eq!(packs, &vec![json!("csif_compact_lexicon_v1")]);
        assert_eq!(
            payload
                .get("lexicon")
                .and_then(|v| v.get("coverage"))
                .and_then(|v| v.get("matched_token_count"))
                .and_then(Value::as_u64),
            Some(0)
        );
    }

    #[test]
    fn benchmark_suite_reports_latency_and_hash_stability() {
        let payload = benchmark_determinism(3);
        assert_eq!(
            payload.get("schema_version").and_then(Value::as_str),
            Some("csif_benchmark_determinism_v1")
        );
        let cases = payload
            .get("cases")
            .and_then(Value::as_array)
            .expect("benchmark cases should exist");
        assert!(!cases.is_empty());
        assert!(cases.iter().all(|case| {
            case.get("deterministic_hash_stable").and_then(Value::as_bool) == Some(true)
        }));
    }

    #[test]
    fn trajectory_event_persists_and_can_be_loaded() {
        let mut path = std::env::temp_dir();
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be monotonic")
            .as_nanos();
        path.push(format!("csif_sense_trajectory_test_{}.jsonl", nonce));
        let path_str = path.to_string_lossy().to_string();

        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: Some(path_str.clone()),
        };
        let payload = disambiguate_payload(
            &state,
            "en",
            "light",
            "the light helped me see with my eyes",
            5,
            0.75,
        );
        let event = build_disambiguation_event(
            &payload,
            "en",
            "light",
            "the light helped me see with my eyes",
            5,
            0.75,
            None,
        );

        append_sense_trajectory_event(&path_str, &event).expect("event append should succeed");
        let events = read_sense_trajectory_events(&path_str).expect("events should load");
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0]
                .get("input")
                .and_then(|v| v.get("token"))
                .and_then(Value::as_str),
            Some("light")
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn trajectory_payload_filters_by_token() {
        let mut path = std::env::temp_dir();
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be monotonic")
            .as_nanos();
        path.push(format!("csif_sense_trajectory_filter_test_{}.jsonl", nonce));
        let path_str = path.to_string_lossy().to_string();

        let event_a = json!({
            "object": "csif.disambiguation.event",
            "schema_version": "csif_disambiguation_event_v1",
            "created_at": unix_time_secs(),
            "input": {"language": "en", "token": "light"},
            "status": "resolved",
            "selected_sense": "sense_light_visible_em"
        });
        let event_b = json!({
            "object": "csif.disambiguation.event",
            "schema_version": "csif_disambiguation_event_v1",
            "created_at": unix_time_secs(),
            "input": {"language": "en", "token": "mouse"},
            "status": "resolved",
            "selected_sense": "sense_mouse_device"
        });
        append_sense_trajectory_event(&path_str, &event_a).expect("event a append should succeed");
        append_sense_trajectory_event(&path_str, &event_b).expect("event b append should succeed");

        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: Some(path_str.clone()),
        };
        let filtered = trajectory_events_payload(&state, Some("en"), Some("light"), 10)
            .expect("payload should build");
        let events = filtered
            .get("events")
            .and_then(Value::as_array)
            .expect("events array should exist");
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0]
                .get("input")
                .and_then(|v| v.get("token"))
                .and_then(Value::as_str),
            Some("light")
        );

        let _ = fs::remove_file(path);
    }

    #[test]
    fn disambiguation_resolves_cross_language_alias_luz() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };

        let payload = disambiguate_payload(
            &state,
            "es",
            "luz",
            "la luz me ayuda a ver con mis ojos",
            5,
            0.75,
        );

        assert_eq!(payload.get("status"), Some(&json!("resolved")));
        let selected = payload
            .get("selected_sense")
            .and_then(|v| v.get("node_id"))
            .and_then(Value::as_str)
            .expect("selected sense should exist");
        assert_eq!(selected, "sense_light_visible_em");

        let semantic = payload
            .get("semantic_identity")
            .and_then(|v| v.get("canonical_lexeme_node"))
            .and_then(Value::as_str)
            .expect("canonical lexeme node should exist");
        assert_eq!(semantic, "lexeme_semantic_light");
    }

    #[test]
    fn trajectory_summary_metrics_capture_torsion_and_stability() {
        let events = vec![
            json!({
                "status": "resolved",
                "selected_sense": "sense_light_visible_em",
                "confidence": {"ambiguity_margin": 2.0},
                "contradiction": {"encountered": false}
            }),
            json!({
                "status": "resolved",
                "selected_sense": "sense_light_visible_em",
                "confidence": {"ambiguity_margin": 2.5},
                "contradiction": {"encountered": false}
            }),
            json!({
                "status": "resolved",
                "selected_sense": "sense_light_insight",
                "confidence": {"ambiguity_margin": 1.0},
                "contradiction": {"encountered": true}
            }),
            json!({
                "status": "ambiguous",
                "selected_sense": Value::Null,
                "confidence": {"ambiguity_margin": 0.1},
                "contradiction": {"encountered": false}
            }),
        ];

        let summary = summarize_trajectory_events(&events);
        assert_eq!(summary.get("event_count"), Some(&json!(4)));
        assert_eq!(summary.get("resolved_count"), Some(&json!(3)));
        assert_eq!(summary.get("ambiguous_count"), Some(&json!(1)));

        let contradiction_rate = summary
            .get("contradiction_rate")
            .and_then(Value::as_f64)
            .expect("contradiction rate must be numeric");
        assert!((contradiction_rate - 0.25).abs() < 1e-12);

        let stability = summary
            .get("stability_score")
            .and_then(Value::as_f64)
            .expect("stability score must be numeric");
        assert!((stability - 0.5).abs() < 1e-12);
    }

    #[test]
    fn semantic_inertia_blocks_reassignment_when_threshold_not_met() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };
        let profile = LexemeInertiaProfile {
            crystallization_depth: 1.0,
            last_selected_sense: Some("sense_light_visible_em".to_string()),
            current_streak: 5,
            resolved_count: 5,
        };

        let payload = disambiguate_payload_with_inertia(
            &state,
            "en",
            "light",
            "the lecture gave me understanding and insight",
            5,
            0.75,
            Some(&profile),
            20.0,
            true,
        );

        assert_eq!(payload.get("status"), Some(&json!("ambiguous")));
        assert_eq!(
            payload
                .get("inertia_decision")
                .and_then(|v| v.get("blocked"))
                .and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            payload
                .get("inertia_decision")
                .and_then(|v| v.get("recommended_action"))
                .and_then(Value::as_str),
            Some("sandbox_review")
        );
        assert_eq!(payload.get("selected_sense"), Some(&Value::Null));
    }

    #[test]
    fn semantic_inertia_does_not_block_same_sense_reaffirmation() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };
        let profile = LexemeInertiaProfile {
            crystallization_depth: 1.0,
            last_selected_sense: Some("sense_light_visible_em".to_string()),
            current_streak: 5,
            resolved_count: 5,
        };

        let payload = disambiguate_payload_with_inertia(
            &state,
            "en",
            "light",
            "the light helped me see with my eyes",
            5,
            0.75,
            Some(&profile),
            8.0,
            true,
        );

        assert_eq!(payload.get("status"), Some(&json!("resolved")));
        let selected = payload
            .get("selected_sense")
            .and_then(|v| v.get("node_id"))
            .and_then(Value::as_str)
            .expect("selected sense should exist");
        assert_eq!(selected, "sense_light_visible_em");
        assert_eq!(
            payload
                .get("inertia_decision")
                .and_then(|v| v.get("blocked"))
                .and_then(Value::as_bool),
            Some(false)
        );
    }

    #[test]
    fn sandbox_simulation_emits_modal_branches_without_commit() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };
        let profile = LexemeInertiaProfile {
            crystallization_depth: 0.8,
            last_selected_sense: Some("sense_light_visible_em".to_string()),
            current_streak: 4,
            resolved_count: 5,
        };
        let payload = disambiguate_payload_with_inertia(
            &state,
            "en",
            "light",
            "the lecture gave me insight and understanding",
            5,
            0.75,
            Some(&profile),
            10.0,
            true,
        );
        let simulation = build_sandbox_simulation(
            &payload,
            "en",
            "light",
            "the lecture gave me insight and understanding",
            5,
            0.75,
            10.0,
            3,
            Some("sense_light_insight"),
        );

        assert_eq!(simulation.get("sandbox"), Some(&Value::Bool(true)));
        assert_eq!(simulation.get("committed"), Some(&Value::Bool(false)));
        assert_eq!(
            simulation
                .get("sandbox_decision")
                .and_then(|v| v.get("competitive_selection"))
                .and_then(Value::as_bool),
            Some(true)
        );
        let branches = simulation
            .get("branches")
            .and_then(Value::as_array)
            .expect("branches should exist");
        assert!(!branches.is_empty());
        assert_eq!(
            branches[0]
                .get("modal_semantics")
                .and_then(|v| v.get("certainty_mode"))
                .and_then(Value::as_str),
            Some("sandbox")
        );
        assert!(
            simulation
                .get("sandbox_decision")
                .and_then(|v| v.get("winner_branch_id"))
                .and_then(Value::as_str)
                .is_some()
        );
    }

    #[test]
    fn sandbox_simulation_can_flag_counterfactual_forced_branch() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };
        let payload = disambiguate_payload_with_inertia(
            &state,
            "es",
            "luz",
            "la luz me ayuda a ver con mis ojos",
            5,
            0.75,
            None,
            0.35,
            true,
        );
        let simulation = build_sandbox_simulation(
            &payload,
            "es",
            "luz",
            "la luz me ayuda a ver con mis ojos",
            5,
            0.75,
            0.35,
            3,
            Some("sense_light_insight"),
        );

        let branches = simulation
            .get("branches")
            .and_then(Value::as_array)
            .expect("branches should exist");
        assert!(branches.iter().any(|branch| {
            branch
                .get("modal_semantics")
                .and_then(|v| v.get("counterfactual"))
                .and_then(Value::as_bool)
                == Some(true)
        }));
        assert!(branches.iter().any(|branch| {
            branch
                .get("rejection")
                .and_then(|v| v.get("rejection_causes"))
                .and_then(Value::as_array)
                .map(|items| items.iter().any(|item| item == &json!("counterfactual_forced_branch")))
                .unwrap_or(false)
        }));
    }

    #[test]
    fn sandbox_simulation_ranks_competing_futures() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };
        let payload = disambiguate_payload_with_inertia(
            &state,
            "en",
            "light",
            "the lecture gave me understanding and insight",
            5,
            0.75,
            None,
            0.35,
            true,
        );

        let simulation = build_sandbox_simulation(
            &payload,
            "en",
            "light",
            "the lecture gave me understanding and insight",
            5,
            0.75,
            0.35,
            3,
            None,
        );

        let decision = simulation
            .get("sandbox_decision")
            .expect("sandbox decision should exist");
        let winner_branch_id = decision
            .get("winner_branch_id")
            .and_then(Value::as_str)
            .expect("winner branch id should exist");
        let ranked = decision
            .get("ranked_branch_ids")
            .and_then(Value::as_array)
            .expect("ranked branches should exist");
        assert_eq!(ranked.first().and_then(Value::as_str), Some(winner_branch_id));
        assert!(
            decision
                .get("trajectory_coherence_score")
                .and_then(Value::as_f64)
                .is_some()
        );
    }

    #[test]
    fn disambiguation_includes_phrase_layer_for_compositional_context() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };
        let payload = disambiguate_payload_with_inertia(
            &state,
            "en",
            "light",
            "the light speed limit is constant",
            5,
            0.75,
            None,
            0.35,
            true,
        );

        let selected_phrase = payload
            .get("phrase_layer")
            .and_then(|v| v.get("selected_phrase"))
            .cloned()
            .unwrap_or(Value::Null);
        assert_ne!(selected_phrase, Value::Null);
        assert_eq!(
            selected_phrase.get("node_id").and_then(Value::as_str),
            Some("phrase_light_speed")
        );
        assert_eq!(
            payload
                .get("phrase_layer")
                .and_then(|v| v.get("mode"))
                .and_then(Value::as_str),
            Some("deterministic_multilingual_lattice_v2")
        );
    }

    #[test]
    fn disambiguation_maps_multilingual_phrase_aliases_to_shared_phrase_identity() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };
        let payload = disambiguate_payload_with_inertia(
            &state,
            "es",
            "luz",
            "la velocidad de la luz es constante",
            5,
            0.75,
            None,
            0.35,
            true,
        );

        assert_eq!(
            payload
                .get("phrase_layer")
                .and_then(|v| v.get("selected_phrase"))
                .and_then(|v| v.get("node_id"))
                .and_then(Value::as_str),
            Some("phrase_light_speed")
        );
        assert_eq!(
            payload
                .get("phrase_layer")
                .and_then(|v| v.get("selected_phrase"))
                .and_then(|v| v.get("matched_alias"))
                .and_then(|v| v.get("language"))
                .and_then(Value::as_str),
            Some("es")
        );
    }

    #[test]
    fn phrase_layer_emits_overlap_aware_chunk_topology() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };
        let payload = disambiguate_payload_with_inertia(
            &state,
            "en",
            "light",
            "the light speed engine prototype is unstable",
            5,
            0.75,
            None,
            0.35,
            true,
        );

        let phrase_candidates = payload
            .get("phrase_layer")
            .and_then(|v| v.get("phrase_candidates"))
            .and_then(Value::as_array)
            .expect("phrase candidates should exist");

        let light_speed = phrase_candidates
            .iter()
            .find(|candidate| candidate.get("node_id").and_then(Value::as_str) == Some("phrase_light_speed"))
            .expect("light speed phrase should exist");
        assert!(
            light_speed
                .get("chunk_topology")
                .and_then(|v| v.get("overlap_count"))
                .and_then(Value::as_u64)
                .unwrap_or(0)
                >= 1
        );
        assert!(
            payload
                .get("phrase_layer")
                .and_then(|v| v.get("lattice"))
                .and_then(|v| v.get("groups"))
                .and_then(Value::as_array)
                .map(|groups| groups.iter().any(|group| {
                    group
                        .get("phrase_nodes")
                        .and_then(Value::as_array)
                        .map(|node_ids| {
                            node_ids.iter().any(|id| id == &json!("phrase_light_speed"))
                                && node_ids.iter().any(|id| id == &json!("phrase_speed_engine"))
                        })
                        .unwrap_or(false)
                }))
                .unwrap_or(false)
        );
    }

    #[test]
    fn frame_conflict_keeps_torsion_unresolved_without_forced_collapse() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };
        let active_frame = FrameContext {
            observer_frame: "third_person_external".to_string(),
            ontology_frame: "quantum_interaction".to_string(),
            temporal_frame: "counterfactual_future".to_string(),
            modality_frame: "hypothetical".to_string(),
            epistemic_source_frame: "derived_inference".to_string(),
        };
        let prior_frame = default_frame_context();

        let payload = disambiguate_payload_with_inertia_and_frame(
            &state,
            "en",
            "light",
            "the light helped me see with my eyes",
            5,
            0.75,
            None,
            0.35,
            true,
            &active_frame,
            &prior_frame,
        );

        assert_eq!(payload.get("status"), Some(&json!("ambiguous")));
        assert_eq!(payload.get("selected_sense"), Some(&Value::Null));
        assert_eq!(
            payload
                .get("frame_semantics")
                .and_then(|v| v.get("unresolved_torsion"))
                .and_then(|v| v.get("kind"))
                .and_then(Value::as_str),
            Some("frame_conflict")
        );
        assert_eq!(
            payload
                .get("frame_semantics")
                .and_then(|v| v.get("unresolved_torsion"))
                .and_then(|v| v.get("collapse_allowed"))
                .and_then(Value::as_bool),
            Some(false)
        );
    }

    #[test]
    fn sandbox_reports_frame_transition_metrics() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };
        let active_frame = FrameContext {
            observer_frame: "internal_observer".to_string(),
            ontology_frame: "electromagnetic_physics".to_string(),
            temporal_frame: "present".to_string(),
            modality_frame: "assertive".to_string(),
            epistemic_source_frame: "instrumented_measurement".to_string(),
        };
        let prior_frame = default_frame_context();

        let payload = disambiguate_payload_with_inertia_and_frame(
            &state,
            "en",
            "light",
            "the light speed limit is constant",
            5,
            0.75,
            None,
            0.35,
            true,
            &active_frame,
            &prior_frame,
        );
        let simulation = build_sandbox_simulation(
            &payload,
            "en",
            "light",
            "the light speed limit is constant",
            5,
            0.75,
            0.35,
            3,
            None,
        );

        assert!(
            simulation
                .get("sandbox_decision")
                .and_then(|v| v.get("frame_transition_cost"))
                .and_then(Value::as_f64)
                .is_some()
        );
        assert!(
            simulation
                .get("sandbox_decision")
                .and_then(|v| v.get("typed_contradiction"))
                .and_then(Value::as_str)
                .is_some()
        );
    }

    #[test]
    fn disambiguation_emits_cross_frame_translation_operators() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };
        let active_frame = FrameContext {
            observer_frame: "internal_observer".to_string(),
            ontology_frame: "quantum_interaction".to_string(),
            temporal_frame: "present".to_string(),
            modality_frame: "assertive".to_string(),
            epistemic_source_frame: "instrumented_measurement".to_string(),
        };
        let prior_frame = FrameContext {
            observer_frame: "internal_observer".to_string(),
            ontology_frame: "classical_optics".to_string(),
            temporal_frame: "present".to_string(),
            modality_frame: "assertive".to_string(),
            epistemic_source_frame: "instrumented_measurement".to_string(),
        };

        let payload = disambiguate_payload_with_inertia_and_frame(
            &state,
            "en",
            "light",
            "light behaves as both wave and particle",
            5,
            0.75,
            None,
            0.35,
            true,
            &active_frame,
            &prior_frame,
        );

        assert_eq!(
            payload
                .get("frame_operators")
                .and_then(|v| v.get("transform_policy"))
                .and_then(|v| v.get("implicit_transforms"))
                .and_then(Value::as_bool),
            Some(false)
        );
        assert!(
            payload
                .get("frame_operators")
                .and_then(|v| v.get("eligible_operators"))
                .and_then(Value::as_array)
                .map(|ops| {
                    ops.iter().any(|op| {
                        op.get("operator_id").and_then(Value::as_str)
                            == Some("op_classical_optics_to_quantum_interaction")
                    })
                })
                .unwrap_or(false)
        );
        assert!(
            payload
                .get("frame_operators")
                .and_then(|v| v.get("projected_candidates"))
                .and_then(Value::as_array)
                .map(|items| !items.is_empty())
                .unwrap_or(false)
        );
    }

    #[test]
    fn conservation_policy_blocks_transform_when_required_invariant_fails() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };
        let active_frame = FrameContext {
            observer_frame: "third_person_external".to_string(),
            ontology_frame: "quantum_interaction".to_string(),
            temporal_frame: "present".to_string(),
            modality_frame: "assertive".to_string(),
            epistemic_source_frame: "instrumented_measurement".to_string(),
        };
        let prior_frame = FrameContext {
            observer_frame: "internal_observer".to_string(),
            ontology_frame: "classical_optics".to_string(),
            temporal_frame: "present".to_string(),
            modality_frame: "assertive".to_string(),
            epistemic_source_frame: "instrumented_measurement".to_string(),
        };
        let policy = ConservationPolicy {
            required_invariants: vec!["observer_consistency".to_string()],
            allow_lossy: false,
            max_total_loss: 0.2,
        };
        let lexicon_control = resolve_lexicon_control(None);

        let payload = disambiguate_payload_with_inertia_and_frame_and_policy(
            &state,
            "en",
            "light",
            "light behaves as both wave and particle",
            5,
            0.75,
            None,
            0.35,
            true,
            &active_frame,
            &prior_frame,
            &policy,
            &lexicon_control,
        );

        let blocked_projection = payload
            .get("frame_operators")
            .and_then(|v| v.get("projected_candidates"))
            .and_then(Value::as_array)
            .and_then(|items| {
                items.iter().find(|item| {
                    item.get("conservation_profile")
                        .and_then(|v| v.get("admissible"))
                        .and_then(Value::as_bool)
                        == Some(false)
                })
            })
            .expect("expected inadmissible projection");
        assert_eq!(
            blocked_projection
                .get("conservation_blocked")
                .and_then(Value::as_bool),
            Some(true)
        );
    }

    #[test]
    fn sandbox_emits_transform_branches_for_operator_projections() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };
        let active_frame = FrameContext {
            observer_frame: "internal_observer".to_string(),
            ontology_frame: "quantum_interaction".to_string(),
            temporal_frame: "present".to_string(),
            modality_frame: "assertive".to_string(),
            epistemic_source_frame: "instrumented_measurement".to_string(),
        };
        let prior_frame = FrameContext {
            observer_frame: "internal_observer".to_string(),
            ontology_frame: "classical_optics".to_string(),
            temporal_frame: "present".to_string(),
            modality_frame: "assertive".to_string(),
            epistemic_source_frame: "instrumented_measurement".to_string(),
        };

        let payload = disambiguate_payload_with_inertia_and_frame(
            &state,
            "en",
            "light",
            "light behaves as both wave and particle",
            5,
            0.75,
            None,
            0.35,
            true,
            &active_frame,
            &prior_frame,
        );
        let simulation = build_sandbox_simulation(
            &payload,
            "en",
            "light",
            "light behaves as both wave and particle",
            5,
            0.75,
            0.35,
            3,
            None,
        );

        assert!(
            simulation
                .get("sandbox_decision")
                .and_then(|v| v.get("transform_branch_count"))
                .and_then(Value::as_u64)
                .unwrap_or(0)
                > 0
        );
        assert!(
            simulation
                .get("branches")
                .and_then(Value::as_array)
                .map(|branches| {
                    branches.iter().any(|branch| {
                        branch
                            .get("trajectory")
                            .and_then(|v| v.get("torsion"))
                            .and_then(Value::as_str)
                            == Some("frame_transform")
                    })
                })
                .unwrap_or(false)
        );
    }

    #[test]
    fn sandbox_marks_conservation_violation_in_rejection_causes() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };
        let active_frame = FrameContext {
            observer_frame: "third_person_external".to_string(),
            ontology_frame: "quantum_interaction".to_string(),
            temporal_frame: "present".to_string(),
            modality_frame: "assertive".to_string(),
            epistemic_source_frame: "instrumented_measurement".to_string(),
        };
        let prior_frame = FrameContext {
            observer_frame: "internal_observer".to_string(),
            ontology_frame: "classical_optics".to_string(),
            temporal_frame: "present".to_string(),
            modality_frame: "assertive".to_string(),
            epistemic_source_frame: "instrumented_measurement".to_string(),
        };
        let policy = ConservationPolicy {
            required_invariants: vec!["observer_consistency".to_string()],
            allow_lossy: false,
            max_total_loss: 0.2,
        };
        let lexicon_control = resolve_lexicon_control(None);

        let payload = disambiguate_payload_with_inertia_and_frame_and_policy(
            &state,
            "en",
            "light",
            "light behaves as both wave and particle",
            5,
            0.75,
            None,
            0.35,
            true,
            &active_frame,
            &prior_frame,
            &policy,
            &lexicon_control,
        );
        let simulation = build_sandbox_simulation(
            &payload,
            "en",
            "light",
            "light behaves as both wave and particle",
            5,
            0.75,
            0.35,
            3,
            None,
        );

        assert!(
            simulation
                .get("branches")
                .and_then(Value::as_array)
                .map(|branches| {
                    branches.iter().any(|branch| {
                        branch
                            .get("rejection")
                            .and_then(|v| v.get("rejection_causes"))
                            .and_then(Value::as_array)
                            .map(|causes| causes.iter().any(|cause| cause == &json!("conservation_violation")))
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(false)
        );
    }

    #[test]
    fn reconciliation_includes_operator_audit_for_transform_branch() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };
        let active_frame = FrameContext {
            observer_frame: "internal_observer".to_string(),
            ontology_frame: "quantum_interaction".to_string(),
            temporal_frame: "present".to_string(),
            modality_frame: "assertive".to_string(),
            epistemic_source_frame: "instrumented_measurement".to_string(),
        };
        let prior_frame = FrameContext {
            observer_frame: "internal_observer".to_string(),
            ontology_frame: "classical_optics".to_string(),
            temporal_frame: "present".to_string(),
            modality_frame: "assertive".to_string(),
            epistemic_source_frame: "instrumented_measurement".to_string(),
        };

        let payload = disambiguate_payload_with_inertia_and_frame(
            &state,
            "en",
            "light",
            "light behaves as both wave and particle",
            5,
            0.75,
            None,
            0.35,
            true,
            &active_frame,
            &prior_frame,
        );
        let simulation = build_sandbox_simulation(
            &payload,
            "en",
            "light",
            "light behaves as both wave and particle",
            5,
            0.75,
            0.35,
            3,
            None,
        );
        let transform_loser = simulation
            .get("branches")
            .and_then(Value::as_array)
            .and_then(|branches| {
                branches
                    .iter()
                    .find(|branch| {
                        branch
                            .get("trajectory")
                            .and_then(|v| v.get("torsion"))
                            .and_then(Value::as_str)
                            == Some("frame_transform")
                    })
                    .and_then(|branch| branch.get("branch_id"))
                    .and_then(Value::as_str)
            })
            .expect("transform branch should exist");

        let reconciliation = build_reconciliation_payload(&simulation, Some(transform_loser))
            .expect("reconciliation should succeed");
        assert!(
            reconciliation
                .get("topology_explanation")
                .and_then(|v| v.get("loser_operator_audit"))
                .and_then(|v| v.get("operator_id"))
                .and_then(Value::as_str)
                .is_some()
        );
        assert!(
            reconciliation
                .get("topology_explanation")
                .and_then(|v| v.get("loser_conservation_profile"))
                .is_some()
        );
    }

    #[test]
    fn sandbox_simulation_emits_phrase_branches_when_detected() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };
        let payload = disambiguate_payload_with_inertia(
            &state,
            "en",
            "light",
            "the light speed limit is constant",
            5,
            0.75,
            None,
            0.35,
            true,
        );
        let simulation = build_sandbox_simulation(
            &payload,
            "en",
            "light",
            "the light speed limit is constant",
            5,
            0.75,
            0.35,
            3,
            None,
        );

        let branches = simulation
            .get("branches")
            .and_then(Value::as_array)
            .expect("branches should exist");
        assert!(branches.iter().any(|branch| {
            branch
                .get("candidate")
                .and_then(|v| v.get("sense_node"))
                .and_then(|v| v.get("node_type"))
                .and_then(Value::as_str)
                == Some("semantic_phrase")
        }));
    }

    #[test]
    fn reconciliation_explains_winner_vs_loser_topology() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };
        let payload = disambiguate_payload_with_inertia(
            &state,
            "en",
            "light",
            "the lecture gave me understanding and insight",
            5,
            0.75,
            None,
            0.35,
            true,
        );
        let simulation = build_sandbox_simulation(
            &payload,
            "en",
            "light",
            "the lecture gave me understanding and insight",
            5,
            0.75,
            0.35,
            3,
            None,
        );
        let reconciliation = build_reconciliation_payload(&simulation, None)
            .expect("reconciliation should succeed");

        assert_eq!(
            reconciliation.get("object"),
            Some(&json!("csif.reconciliation.result"))
        );
        assert!(reconciliation.get("winner_branch_id").is_some());
        assert!(reconciliation.get("losing_branch_id").is_some());
        assert!(
            reconciliation
                .get("topology_explanation")
                .and_then(|v| v.get("identity_persistence"))
                .and_then(Value::as_f64)
                .is_some()
        );
        assert!(
            reconciliation
                .get("rejected_topology")
                .and_then(|v| v.get("contradiction_pressure"))
                .and_then(Value::as_f64)
                .is_some()
        );
    }

    #[test]
    fn reconciliation_marks_phrase_topology_mode_when_phrase_branch_present() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };
        let payload = disambiguate_payload_with_inertia(
            &state,
            "en",
            "light",
            "the light speed limit is constant",
            5,
            0.75,
            None,
            0.35,
            true,
        );
        let simulation = build_sandbox_simulation(
            &payload,
            "en",
            "light",
            "the light speed limit is constant",
            5,
            0.75,
            0.35,
            3,
            None,
        );

        let loser_branch_id = simulation
            .get("branches")
            .and_then(Value::as_array)
            .and_then(|branches| {
                branches
                    .iter()
                    .find(|branch| {
                        branch
                            .get("candidate")
                            .and_then(|v| v.get("sense_node"))
                            .and_then(|v| v.get("node_type"))
                            .and_then(Value::as_str)
                            == Some("semantic_phrase")
                    })
                    .and_then(|branch| branch.get("branch_id"))
                    .and_then(Value::as_str)
            })
            .expect("phrase loser branch should exist");

        let reconciliation = build_reconciliation_payload(&simulation, Some(loser_branch_id))
            .expect("reconciliation should succeed");
        assert_eq!(
            reconciliation
                .get("topology_explanation")
                .and_then(|v| v.get("phrase_reconciliation_mode"))
                .and_then(Value::as_str),
            Some("phrase_vs_token_or_phrase")
        );
        assert!(
            reconciliation
                .get("topology_explanation")
                .and_then(|v| v.get("loser_chunk_topology"))
                .is_some()
        );
        assert!(
            reconciliation
                .get("topology_explanation")
                .and_then(|v| v.get("frame_transition_cost"))
                .and_then(Value::as_f64)
                .is_some()
        );
    }

    #[test]
    fn reconciliation_rejects_unknown_losing_branch() {
        let state = AppState {
            bank_summary: None,
            bank_index: None,
            sense_trajectory_log_path: None,
        };
        let payload = disambiguate_payload_with_inertia(
            &state,
            "en",
            "light",
            "the lecture gave me understanding and insight",
            5,
            0.75,
            None,
            0.35,
            true,
        );
        let simulation = build_sandbox_simulation(
            &payload,
            "en",
            "light",
            "the lecture gave me understanding and insight",
            5,
            0.75,
            0.35,
            3,
            None,
        );

        let err = build_reconciliation_payload(&simulation, Some("missing-branch"))
            .expect_err("unknown loser branch should fail");
        assert!(err.contains("not found"));
    }
}
