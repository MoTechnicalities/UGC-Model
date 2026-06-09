use sha2::{Digest, Sha256};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

const RWIF_EVENT_SCHEMA_VERSION: &str = "RWIF_EVENT_V2";
const RWIF_EDGE_SCHEMA_VERSION: &str = "RWIF_EDGE_V2";
const MAX_SCAFFOLD_QUBITS: u8 = 10;
pub const DEFAULT_BV_BLACK_BOX_SHOTS: u32 = 512;
const REALITY_CALIBRATION_TARGET_CONFIDENCE: f64 = 0.95;
const BLOCK_SPARSE_BLOCK_SIZE: usize = 256;
const BLOCK_SPARSE_DENSE_FALLBACK_DENSITY_THRESHOLD: f64 = 0.12;
const BLOCK_SIZE_TUNING_CANDIDATES: [usize; 5] = [64, 128, 256, 512, 1024];

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MultiVectorND {
    pub dimension: usize,
    pub components: Vec<f64>,
}

#[derive(Clone, Debug, PartialEq)]
struct BlockSparseState {
    dimension: usize,
    block_size: usize,
    blocks: BTreeMap<usize, Vec<(usize, f64)>>,
}

impl BlockSparseState {
    fn from_dense(components: &[f64], block_size: usize) -> Self {
        let mut blocks = BTreeMap::new();
        for (block_idx, chunk) in components.chunks(block_size).enumerate() {
            let mut entries = Vec::new();
            for (offset, value) in chunk.iter().enumerate() {
                if *value != 0.0 {
                    entries.push((offset, *value));
                }
            }
            if !entries.is_empty() {
                blocks.insert(block_idx, entries);
            }
        }
        Self {
            dimension: components.len(),
            block_size,
            blocks,
        }
    }

    fn to_dense_multivector(&self) -> MultiVectorND {
        let mut components = vec![0.0; self.dimension];
        for (block_idx, block_values) in &self.blocks {
            let base = block_idx * self.block_size;
            for (offset, value) in block_values {
                let idx = base + offset;
                if idx >= self.dimension {
                    break;
                }
                components[idx] = *value;
            }
        }
        MultiVectorND {
            dimension: self.dimension,
            components,
        }
    }
}

impl MultiVectorND {
    pub fn zero(dimension: usize) -> Self {
        Self {
            dimension,
            components: vec![0.0; dimension],
        }
    }

    pub fn vacuum(dimension: usize) -> Self {
        let mut state = Self::zero(dimension);
        if let Some(scalar) = state.components.get_mut(0) {
            *scalar = 1.0;
        }
        state
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RotorSpec {
    pub rotor_id: String,
    pub plane_label: String,
    pub angle_radians: f64,
    pub targets: Vec<usize>,
    pub blade_mask: Option<usize>,
    pub blade_label: Option<String>,
    pub blade_grade: Option<u8>,
    pub grade_classification: Option<String>,
    pub coupling_blade_mask: Option<usize>,
    pub coupling_manifold: Option<String>,
    pub coupling_blade_grade: Option<u8>,
    pub coupling_grade_classification: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum GateKind {
    NoOp,
    Hadamard,
    PauliX,
    Phase,
    Cnot,
    Custom(String),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct GateSpec {
    pub gate_id: String,
    pub kind: GateKind,
    pub targets: Vec<usize>,
    pub angle_radians: Option<f64>,
}

pub trait GateToRotor {
    fn to_rotor(&self) -> RotorSpec;
}

impl GateToRotor for GateSpec {
    fn to_rotor(&self) -> RotorSpec {
        let target0 = self.targets.first().copied().unwrap_or(0);
        let target1 = self.targets.get(1).copied().unwrap_or(target0);
        let hadamard_mask = basis_pair_mask(target0);
        let cnot_mask = basis_pair_mask(target1);
        let cnot_coupling = cnot_coupling_mask(target0, target1);

        let (plane_label, fallback_angle, blade_mask, coupling_blade_mask) = match &self.kind {
            GateKind::NoOp => ("identity".to_string(), 0.0, None, None),
            GateKind::Hadamard => (
                format!("hadamard_reflection_q{}", target0),
                std::f64::consts::PI,
                hadamard_mask,
                None,
            ),
            GateKind::PauliX => (
                format!("x_flip_q{}", target0),
                std::f64::consts::PI,
                hadamard_mask,
                None,
            ),
            GateKind::Phase => (
                format!("phase_rotation_q{}", target0),
                std::f64::consts::FRAC_PI_2,
                hadamard_mask,
                None,
            ),
            GateKind::Cnot => (
                format!("controlled_reflection_q{}_q{}", target0, target1),
                std::f64::consts::FRAC_PI_2,
                cnot_mask,
                cnot_coupling,
            ),
            GateKind::Custom(name) => (format!("custom:{}", name), 0.1, hadamard_mask, None),
        };

        let blade_label = blade_mask.map(blade_mask_to_clifford_label);
        let (blade_grade, grade_classification) = blade_mask
            .map(blade_grade_and_classification)
            .map_or((None, None), |(grade, class)| (Some(grade), Some(class)));
        let coupling_manifold = coupling_blade_mask.map(blade_mask_to_clifford_label);
        let (coupling_blade_grade, coupling_grade_classification) = coupling_blade_mask
            .map(blade_grade_and_classification)
            .map_or((None, None), |(grade, class)| (Some(grade), Some(class)));

        RotorSpec {
            rotor_id: format!("rotor:{}", self.gate_id),
            plane_label,
            angle_radians: self.angle_radians.unwrap_or(fallback_angle),
            targets: self.targets.clone(),
            blade_mask,
            blade_label,
            blade_grade,
            grade_classification,
            coupling_blade_mask,
            coupling_manifold,
            coupling_blade_grade,
            coupling_grade_classification,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TorsionEvent {
    pub tick: u64,
    pub gate_id: String,
    pub torsion_delta: f64,
    pub cumulative_torsion: f64,
    pub torsion_scalar: f64,
    pub phase_alignment_index: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RwifGateTrace {
    pub schema_version: String,
    pub tick: u64,
    pub gate_id: String,
    pub rotor_id: String,
    pub plane_label: String,
    pub blade_mask: Option<usize>,
    pub blade_label: Option<String>,
    pub blade_grade: Option<u8>,
    pub grade_classification: Option<String>,
    pub coupling_blade_mask: Option<usize>,
    pub coupling_manifold: Option<String>,
    pub coupling_blade_grade: Option<u8>,
    pub coupling_grade_classification: Option<String>,
    pub local_bivector_amplitude_before: f64,
    pub local_bivector_amplitude_after: f64,
    pub coupling_trivector_amplitude_before: f64,
    pub coupling_trivector_amplitude_after: f64,
    pub coupling_transfer_scalar: f64,
    pub normalized_coupling_intensity: f64,
    pub geometric_convergence_metric: f64,
    pub entanglement_coupling_active: bool,
    pub torsion_scalar: f64,
    pub phase_alignment_index: f64,
}

impl RwifGateTrace {
    pub fn to_rwif_event_envelope(&self) -> Value {
        let amplitude_signed = (self.torsion_scalar * 127.0).round().clamp(-127.0, 127.0) as i64;
        let intent_signed = ((self.phase_alignment_index * 2.0 - 1.0) * 127.0)
            .round()
            .clamp(-127.0, 127.0) as i64;
        json!({
            "schema_version": RWIF_EVENT_SCHEMA_VERSION,
            "state_encoding": "signed_i8_plus_intent_v2",
            "quantization_step": 1,
            "amplitude_signed": amplitude_signed,
            "intent_signed": intent_signed,
            "phase_theta": self.phase_alignment_index,
            "phase_omega": self.torsion_scalar,
            "monotonic_index": self.tick,
            "torsion_scalar": self.torsion_scalar,
            "phase_alignment_index": self.phase_alignment_index,
            "gate_id": self.gate_id,
            "rotor_id": self.rotor_id,
            "plane_label": self.plane_label,
            "blade_mask": self.blade_mask,
            "blade_label": self.blade_label,
            "blade_grade": self.blade_grade,
            "grade_classification": self.grade_classification,
            "coupling_blade_mask": self.coupling_blade_mask,
            "coupling_manifold": self.coupling_manifold,
            "coupling_blade_grade": self.coupling_blade_grade,
            "coupling_grade_classification": self.coupling_grade_classification,
            "local_bivector_amplitude_before": self.local_bivector_amplitude_before,
            "local_bivector_amplitude_after": self.local_bivector_amplitude_after,
            "coupling_trivector_amplitude_before": self.coupling_trivector_amplitude_before,
            "coupling_trivector_amplitude_after": self.coupling_trivector_amplitude_after,
            "coupling_transfer_scalar": self.coupling_transfer_scalar,
            "normalized_coupling_intensity": self.normalized_coupling_intensity,
            "geometric_convergence_metric": self.geometric_convergence_metric,
            "entanglement_coupling_active": self.entanglement_coupling_active,
        })
    }

    pub fn to_rwif_edge_envelope(&self) -> Value {
        json!({
            "edge_id": format!("edge_quantum_{}", self.tick),
            "schema_version": RWIF_EDGE_SCHEMA_VERSION,
            "state_encoding": "phase_scalar_v1",
            "numeric_range": {
                "amplitude": {"min": -127, "max": 127},
                "intent": {"min": -127, "max": 127}
            },
            "wrap_mode": "principal_pi",
            "integer_wrap_mode": "clamp",
            "integration_rule": "quantum_gate_trace_v1",
            "phase_trajectory": [self.to_rwif_event_envelope()]
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MeasurementOutcome {
    pub qubit: usize,
    pub basis_state: u8,
    pub certainty: f64,
    pub geometric_certainty: f64,
    pub geometric_weight: f64,
    pub tie_break_rule: String,
    pub projection_basis: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ShotMeasurementSummary {
    pub qubit: usize,
    pub shots: u32,
    pub ones: u32,
    pub zeros: u32,
    pub one_probability: f64,
    pub inferred_bit: u8,
    pub sampler: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct BitConfidenceEstimate {
    pub qubit: usize,
    pub inferred_bit: u8,
    pub estimated_confidence: f64,
    pub minimum_shots_for_target_confidence: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RealityCalibrationSummary {
    pub confidence_model: String,
    pub target_confidence_level: f64,
    pub per_bit: Vec<BitConfidenceEstimate>,
    pub whole_string_confidence: f64,
    pub minimum_shots_for_target_confidence: Option<u32>,
}

pub trait RegisterGateApplier {
    fn apply_gate(&mut self, gate: GateSpec) -> Result<TorsionEvent, String>;
}

pub trait DeterministicProjector {
    fn measure_qubit(&self, qubit: usize) -> Result<MeasurementOutcome, String>;
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ErrorModel {
    pub torsion_noise_factor: f64,
    pub seed: u64,
}

impl ErrorModel {
    pub fn normalized_torsion_noise_factor(&self) -> f64 {
        self.torsion_noise_factor.clamp(0.0, 1.0)
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub enum BvExecutionMode {
    Structural,
    BlackBox,
}

#[derive(Clone, Debug)]
pub struct OpaqueBvOracle {
    hidden_bits: Vec<u8>,
    applied_gate_count: usize,
    invocation_count: usize,
}

impl OpaqueBvOracle {
    pub fn new(hidden: &str) -> Result<Self, String> {
        Ok(Self {
            hidden_bits: parse_hidden_bit_string(hidden)?,
            applied_gate_count: 0,
            invocation_count: 0,
        })
    }

    pub fn execution_mode(&self) -> BvExecutionMode {
        BvExecutionMode::BlackBox
    }

    pub fn query_len(&self) -> usize {
        self.hidden_bits.len()
    }

    pub fn apply_to_register(&mut self, register: &mut QuantumRegister) -> Result<usize, String> {
        let ancilla = self.hidden_bits.len();
        let mut applied = 0usize;
        self.invocation_count += 1;

        for (idx, bit) in self.hidden_bits.iter().enumerate() {
            if *bit == 1 {
                register.apply_gate(GateSpec {
                    gate_id: format!("bv_black_box_cnot_q{}", idx),
                    kind: GateKind::Cnot,
                    targets: vec![idx, ancilla],
                    angle_radians: Some(std::f64::consts::FRAC_PI_2),
                })?;
                applied += 1;
            }
        }

        self.applied_gate_count += applied;
        Ok(applied)
    }

    pub fn applied_gate_count(&self) -> usize {
        self.applied_gate_count
    }

    pub fn oracle_call_count(&self) -> usize {
        self.invocation_count
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct QuantumRegister {
    pub n_qubits: u8,
    pub state: MultiVectorND,
    pub global_torsion: f64,
    pub tick: u64,
    pub trace: Vec<TorsionEvent>,
    pub rwif_trace: Vec<RwifGateTrace>,
}

impl QuantumRegister {
    pub fn new(n_qubits: u8) -> Result<Self, String> {
        if n_qubits == 0 {
            return Err("n_qubits must be >= 1".to_string());
        }
        if n_qubits > MAX_SCAFFOLD_QUBITS {
            return Err(format!(
                "n_qubits={} exceeds scaffold limit {}; reduce qubits or switch to sparse manifold mode",
                n_qubits, MAX_SCAFFOLD_QUBITS
            ));
        }

        let algebra_rank = usize::from(n_qubits) * 2;
        let dimension = 1usize
            .checked_shl(algebra_rank as u32)
            .ok_or_else(|| "multivector dimension overflow".to_string())?;
        Ok(Self {
            n_qubits,
            state: MultiVectorND::vacuum(dimension),
            global_torsion: 0.0,
            tick: 0,
            trace: Vec::new(),
            rwif_trace: Vec::new(),
        })
    }

    pub fn interface_contract(&self) -> Value {
        json!({
            "object": "csif.quantum.register.scaffold",
            "schema_version": "csif_quantum_register_scaffold_v1",
            "status": "scaffold_only",
            "n_qubits": self.n_qubits,
            "state_dimension": self.state.dimension,
            "algebra_rank": usize::from(self.n_qubits) * 2,
            "traits": [
                "RegisterGateApplier",
                "DeterministicProjector",
                "GateToRotor"
            ],
            "deterministic": true,
            "global_torsion": self.global_torsion,
            "tick": self.tick,
            "trace_len": self.trace.len(),
            "rwif_trace_len": self.rwif_trace.len(),
            "measurement_policy": {
                "tie_break_rule": "lowest_basis_index",
                "projection_basis": "computational_z"
            },
            "semantic_blade_notation": "clifford_gamma_wedge_v1",
            "semantic_grade_classification": "blade_grade_count_ones_v1"
        })
    }

    #[allow(dead_code)]
    pub fn apply_optimization_step(&mut self, evolution_fraction: f64) -> Result<TorsionEvent, String> {
        let fraction = evolution_fraction.clamp(0.0, 1.0);
        let local_kind = if fraction < 0.5 {
            GateKind::Hadamard
        } else {
            GateKind::Phase
        };

        let local_gate = GateSpec {
            gate_id: format!("opt_local_{}", self.tick + 1),
            kind: local_kind,
            targets: vec![0],
            angle_radians: Some((1.0 - fraction) * std::f64::consts::FRAC_PI_2),
        };
        let local_rotor = local_gate.to_rotor();

        let local_state = evolve_gate_state(&self.state, &local_rotor, self.state.dimension)?;

        let final_state = if self.n_qubits > 1 {
            let coupling_gate = GateSpec {
                gate_id: format!("opt_coupling_{}", self.tick + 1),
                kind: GateKind::Cnot,
                targets: vec![0, 1],
                angle_radians: Some(fraction * std::f64::consts::FRAC_PI_2),
            };
            let coupling_rotor = coupling_gate.to_rotor();
            let coupling_state = evolve_gate_state(&local_state, &coupling_rotor, self.state.dimension)?;
            let mut projected_coupling_state = coupling_state.clone();
            projected_coupling_state.components[0] = 0.0;

            let mut mixed = blend_states(&local_state, &projected_coupling_state, 1.0 - fraction, fraction);
            if let Some(coupling_mask) = coupling_rotor.coupling_blade_mask {
                if coupling_mask < mixed.dimension && fraction >= 1.0 {
                    mixed.components[0] = 0.0;
                    mixed.components[coupling_mask] = mixed.components[coupling_mask].abs().max(1e-12);
                }
            }
            mixed
        } else {
            local_state.clone()
        };

        let local_before = blade_component_magnitude(&self.state, local_rotor.blade_mask);
        let local_after = blade_component_magnitude(&final_state, local_rotor.blade_mask);
        let coupling_before = blade_component_magnitude(&self.state, local_rotor.coupling_blade_mask);
        let coupling_after = blade_component_magnitude(&final_state, local_rotor.coupling_blade_mask);
        let non_scalar_magnitude = final_state.non_scalar_magnitude();
        let coupling_transfer_scalar = fraction * non_scalar_magnitude;
        let normalized_coupling_intensity = if non_scalar_magnitude <= 1e-12 {
            0.0
        } else {
            (coupling_transfer_scalar / non_scalar_magnitude).clamp(0.0, 1.0)
        };
        let geometric_convergence_metric = geometric_convergence_metric(&final_state, local_rotor.coupling_blade_mask);
        let entanglement_coupling_active = local_rotor.coupling_blade_mask.is_some() && coupling_after > 0.0;

        let prev_norm = self.state.norm();
        let new_norm = final_state.norm();
        let denom = prev_norm * new_norm;
        let alignment = if denom <= f64::EPSILON {
            1.0
        } else {
            (self.state.dot(&final_state) / denom).clamp(-1.0, 1.0)
        };
        let torsion_delta = alignment.acos();
        let phase_alignment_index = ((alignment + 1.0) * 0.5).clamp(0.0, 1.0);

        self.tick += 1;
        self.global_torsion += torsion_delta;
        self.state = final_state;

        let event = TorsionEvent {
            tick: self.tick,
            gate_id: format!("optimization_step_{:.3}", fraction),
            torsion_delta,
            cumulative_torsion: self.global_torsion,
            torsion_scalar: torsion_delta,
            phase_alignment_index,
        };

        let rwif = RwifGateTrace {
            schema_version: "rwif_gate_trace_v1".to_string(),
            tick: self.tick,
            gate_id: event.gate_id.clone(),
            rotor_id: format!("rotor:optimization_step_{:.3}", fraction),
            plane_label: local_rotor.plane_label,
            blade_mask: local_rotor.blade_mask,
            blade_label: local_rotor.blade_label,
            blade_grade: local_rotor.blade_grade,
            grade_classification: local_rotor.grade_classification,
            coupling_blade_mask: local_rotor.coupling_blade_mask,
            coupling_manifold: local_rotor.coupling_manifold,
            coupling_blade_grade: local_rotor.coupling_blade_grade,
            coupling_grade_classification: local_rotor.coupling_grade_classification,
            local_bivector_amplitude_before: local_before,
            local_bivector_amplitude_after: local_after,
            coupling_trivector_amplitude_before: coupling_before,
            coupling_trivector_amplitude_after: coupling_after,
            coupling_transfer_scalar,
            normalized_coupling_intensity,
            geometric_convergence_metric,
            entanglement_coupling_active,
            torsion_scalar: event.torsion_scalar,
            phase_alignment_index: event.phase_alignment_index,
        };

        self.rwif_trace.push(rwif);
        self.trace.push(event.clone());
        Ok(event)
    }

    pub fn apply_gate_with_noise(
        &mut self,
        gate: GateSpec,
        error_model: &ErrorModel,
    ) -> Result<TorsionEvent, String> {
        let base_angle = gate.to_rotor().angle_radians;
        let mut noisy_gate = gate;
        let angle_scale = deterministic_noise_angle_scale(error_model, &noisy_gate, self.tick);
        noisy_gate.angle_radians = Some(base_angle * angle_scale);
        self.apply_gate(noisy_gate)
    }

    pub fn apply_gate_with_optional_noise(
        &mut self,
        gate: GateSpec,
        error_model: Option<&ErrorModel>,
    ) -> Result<TorsionEvent, String> {
        match error_model {
            Some(model) => self.apply_gate_with_noise(gate, model),
            None => self.apply_gate(gate),
        }
    }
}

fn deterministic_noise_angle_scale(error_model: &ErrorModel, gate: &GateSpec, tick: u64) -> f64 {
    let noise = error_model.normalized_torsion_noise_factor();
    if noise <= f64::EPSILON {
        return 1.0;
    }

    let mut hasher = Sha256::new();
    hasher.update(error_model.seed.to_le_bytes());
    hasher.update(tick.to_le_bytes());
    hasher.update(gate.gate_id.as_bytes());
    for target in &gate.targets {
        hasher.update(target.to_le_bytes());
    }
    let digest = hasher.finalize();
    let mut sample_bytes = [0u8; 8];
    sample_bytes.copy_from_slice(&digest[..8]);
    let sample = u64::from_le_bytes(sample_bytes) as f64 / u64::MAX as f64;

    let baseline = 1.0 - noise;
    let jitter = (sample - 0.5) * 2.0 * noise * 0.25;
    (baseline + jitter).clamp(0.0, 1.0)
}

pub fn prepare_bell_state(
    register: &mut QuantumRegister,
    qubit_a: usize,
    qubit_b: usize,
    error_model: Option<&ErrorModel>,
) -> Result<Vec<TorsionEvent>, String> {
    let mut events = Vec::with_capacity(2);
    events.push(register.apply_gate_with_optional_noise(
        GateSpec {
            gate_id: format!("bell_h_q{}", qubit_a),
            kind: GateKind::Hadamard,
            targets: vec![qubit_a],
            angle_radians: Some(std::f64::consts::FRAC_PI_2),
        },
        error_model,
    )?);
    events.push(register.apply_gate_with_optional_noise(
        GateSpec {
            gate_id: format!("bell_cnot_q{}_q{}", qubit_a, qubit_b),
            kind: GateKind::Cnot,
            targets: vec![qubit_a, qubit_b],
            angle_radians: Some(std::f64::consts::FRAC_PI_2),
        },
        error_model,
    )?);
    Ok(events)
}

pub fn bell_state_report(error_model: Option<ErrorModel>) -> Result<Value, String> {
    let mut register = QuantumRegister::new(2)?;
    let events = prepare_bell_state(&mut register, 0, 1, error_model.as_ref())?;
    let m0 = register.measure_qubit(0)?;
    let m1 = register.measure_qubit(1)?;
    let shots = 256u32;
    let base_seed = splitmix64(register.tick ^ 0x42454c4c5f535441);
    let s0 = shot_measure_qubit(&register, 0, shots, base_seed ^ 0x11)?;
    let s1 = shot_measure_qubit(&register, 1, shots, base_seed ^ 0x22)?;
    let corr = 1.0 - (s0.one_probability - s1.one_probability).abs();

    let mut payload = register.interface_contract();
    payload["algorithm"] = json!("bell_state_preparation");
    payload["event_count"] = json!(events.len());
    payload["measurements"] = json!({
        "q0": m0,
        "q1": m1,
    });
    payload["correlation"] = json!({
        "model": "one_probability_similarity_v1",
        "score": corr.clamp(0.0, 1.0),
        "q0_one_probability": s0.one_probability,
        "q1_one_probability": s1.one_probability,
    });
    payload["entanglement_coupling_active"] = json!(
        register
            .rwif_trace
            .iter()
            .any(|trace| trace.entanglement_coupling_active)
    );
    payload["coupling_trivector_amplitude_after"] = json!(
        register
            .rwif_trace
            .iter()
            .filter_map(|trace| trace.coupling_trivector_amplitude_after.is_finite().then_some(trace.coupling_trivector_amplitude_after))
            .fold(0.0f64, f64::max)
    );
    payload["error_model"] = serde_json::to_value(&error_model).unwrap_or(Value::Null);
    payload["rwif_event_envelopes"] = Value::Array(
        register
            .rwif_trace
            .iter()
            .map(RwifGateTrace::to_rwif_event_envelope)
            .collect::<Vec<_>>(),
    );
    payload["rwif_edge_envelopes"] = Value::Array(
        register
            .rwif_trace
            .iter()
            .map(RwifGateTrace::to_rwif_edge_envelope)
            .collect::<Vec<_>>(),
    );
    payload["stable_envelope_sha256"] = json!(stable_sha256_hex(&payload)?);
    Ok(payload)
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct BellSweepPoint {
    pub noise_factor: f64,
    pub seed: u64,
    pub correlation_score: f64,
    pub coupling_trivector_amplitude_after: f64,
    pub entanglement_coupling_active: bool,
    pub stable_envelope_sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DiracModeSweepPoint {
    pub state_model: String,
    pub coupling_density: f64,
    pub seed: u64,
    pub perturbation_amplitude: f64,
    pub perturbation_frequency: f64,
    pub n_qubits: u8,
    pub state_dimension: usize,
    pub support_density: f64,
    pub low_grade_blade_concentration: f64,
    pub observed_density: f64,
    pub perturbed_observed_density: f64,
    pub density_threshold: f64,
    pub threshold_distance: f64,
    pub perturbed_threshold_distance: f64,
    pub dense_fallback_active: bool,
    pub perturbed_dense_fallback_active: bool,
    pub phase_relaxation_steps: u32,
    pub torsion_hysteresis: f64,
    pub perturbation_volatility_index: f64,
    pub stable_envelope_sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DiracModeSeedCrossing {
    pub seed: u64,
    pub first_crossing_density: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DiracModeThresholdSummary {
    pub computational_analog_label: String,
    pub computational_analog_scope: String,
    pub state_model: String,
    pub baseline_state_model: Option<String>,
    pub baseline_first_crossing_density: Option<f64>,
    pub first_crossing_density_delta_from_baseline: Option<f64>,
    pub crossing_density_ratio_to_uniform: Option<f64>,
    pub baseline_per_seed_crossing_spread: Option<f64>,
    pub density_threshold: f64,
    pub first_crossing_density: Option<f64>,
    pub crossing_seed_count: usize,
    pub per_seed_first_crossing_density: Vec<DiracModeSeedCrossing>,
    pub per_seed_crossing_spread_min: Option<f64>,
    pub per_seed_crossing_spread_max: Option<f64>,
    pub per_seed_crossing_spread: Option<f64>,
    pub perturbation_frequency: f64,
    pub perturbation_amplitudes: Vec<f64>,
    pub phase_relaxation_steps_mean: f64,
    pub torsion_hysteresis_mean: f64,
    pub volatility_index_mean: f64,
    pub volatility_index_max: f64,
    pub catastrophic_unraveling_amplitude: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DiracAnnihilationStepEvent {
    pub step: u32,
    pub overlap_density: f64,
    pub separation_distance: f64,
    pub local_phase_tensor_sum: f64,
    pub shear_torque: f64,
    pub contradiction_count: u32,
    pub frame_transition_active: bool,
    pub localized_core_pressure: f64,
    pub rwif_phase_alignment: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DiracAnnihilationConservationSummary {
    pub delta_q_topo: f64,
    pub torsion_leak_detected: bool,
    pub initial_mass_equivalent: f64,
    pub energy_tensor_normalization_coefficient: f64,
    pub released_wave_energy: f64,
    pub reflected_wave_energy: f64,
    pub pressure_equivalence_error: f64,
    pub pressure_equivalence_compliant: bool,
    pub phase_relaxation_gradient: f64,
    pub invariant_violations: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DiracAnnihilationProfileSummary {
    pub state_model: String,
    pub n_qubits: u8,
    pub unwinding_steps: u32,
    pub flux_coupling_density: f64,
    pub frame_transition_threshold: f64,
    pub frame_transition_step: Option<u32>,
    pub crossing_density_ratio_to_uniform: f64,
    pub unwinding_efficiency_index: f64,
    pub peak_anticrystal_contradiction_count: u32,
    pub residual_torsion_hysteresis: f64,
    pub impedance_matching_efficiency: f64,
    pub vorticity_dissipation_rate: f64,
    pub conservation: DiracAnnihilationConservationSummary,
    pub rwif_unwinding_trace: Vec<DiracAnnihilationStepEvent>,
}

fn dirac_profile_crossing_ratio_to_uniform(state_model: &str) -> f64 {
    match canonical_dirac_state_model(state_model) {
        "high-grade-bias" => 0.658832,
        "low-grade-bias" => 1.107154,
        "harmonic-stride" => 1.049737,
        "contiguous-band" => 0.962000,
        _ => 1.0,
    }
}

fn dirac_profile_unwinding_coefficients(state_model: &str) -> (f64, f64, f64) {
    match canonical_dirac_state_model(state_model) {
        // Lower impedance profile: easier transfer to propagating waves.
        "high-grade-bias" => (0.62, 1.26, 0.84),
        // Higher impedance profile: stronger reflection and slower dissipation.
        "low-grade-bias" => (1.10, 0.78, 0.46),
        "harmonic-stride" => (0.95, 1.04, 0.71),
        "contiguous-band" => (0.92, 0.96, 0.66),
        _ => (1.0, 1.0, 0.62),
    }
}

fn dirac_frame_energy_tensor_normalization(
    frame_transition_threshold: f64,
    flux_coupling_density: f64,
    crossing_ratio: f64,
) -> f64 {
    // This projects confined CSIF core pressure into RWIF propagating-wave
    // amplitudes at frame transition so conservation diagnostics compare like units.
    let threshold_term = 0.30 * frame_transition_threshold;
    let coupling_term = 0.05 * flux_coupling_density;
    let crossing_term = 0.02 * (crossing_ratio - 1.0).abs().min(1.0);
    (1.0 - threshold_term - coupling_term - crossing_term).clamp(0.70, 1.0)
}

fn simulate_dirac_annihilation_profile(
    state_model: &str,
    n_qubits: u8,
    unwinding_steps: u32,
    flux_coupling_density: f64,
) -> DiracAnnihilationProfileSummary {
    let state_model = canonical_dirac_state_model(state_model).to_string();
    let (impedance, dissipation, compliance) =
        dirac_profile_unwinding_coefficients(state_model.as_str());
    let crossing_ratio = dirac_profile_crossing_ratio_to_uniform(state_model.as_str());
    let frame_transition_threshold = bell_round6((0.30 + 0.34 * crossing_ratio).clamp(0.20, 0.92));

    let c_squared = 64.0;
    let initial_mass_equivalent = bell_round6(
        ((n_qubits as f64) * flux_coupling_density * 0.85 + 0.15)
            * (1.0 + 0.10 * crossing_ratio)
            * 0.5,
    );
    let initial_core_pressure = initial_mass_equivalent * c_squared;

    let mut trace = Vec::<DiracAnnihilationStepEvent>::new();
    let mut frame_transition_step: Option<u32> = None;
    let mut peak_anticrystal_contradiction_count = 0u32;
    let mut cumulative_wave_energy = 0.0f64;
    let mut cumulative_reflected_energy = 0.0f64;
    let mut localized_core_pressure = initial_core_pressure;
    let mut first_cleared_step: Option<u32> = None;

    let safe_steps = unwinding_steps.max(1);
    for step in 0..=safe_steps {
        let step_norm = step as f64 / safe_steps as f64;
        let separation_distance = 1.0 - step_norm;
        let overlap_density = step_norm;

        if frame_transition_step.is_none() && overlap_density >= frame_transition_threshold {
            frame_transition_step = Some(step);
        }

        let phase_mixing = overlap_density.powf(1.15) * (1.0 + (1.0 - separation_distance) * 0.18);
        let shear_torque = phase_mixing * flux_coupling_density * impedance;
        let rwif_phase_alignment = (1.0 - (impedance - 0.75).abs() * 0.35).clamp(0.0, 1.0);
        let frame_transition_active = frame_transition_step.map_or(false, |s| step >= s);

        let contradiction_raw = if frame_transition_active {
            (shear_torque * (1.6 - compliance) * 18.0).round().max(0.0)
        } else {
            (shear_torque * (1.2 - compliance) * 6.0).round().max(0.0)
        };
        let contradiction_count = contradiction_raw as u32;
        if contradiction_count > peak_anticrystal_contradiction_count {
            peak_anticrystal_contradiction_count = contradiction_count;
        }

        let release_rate = (dissipation * compliance * overlap_density * 0.22).clamp(0.0, 1.0);
        let reflected_rate = ((impedance - compliance).max(0.0) * overlap_density * 0.11).clamp(0.0, 1.0);
        let released = localized_core_pressure * release_rate;
        let reflected = localized_core_pressure * reflected_rate;
        cumulative_wave_energy += released;
        cumulative_reflected_energy += reflected;
        localized_core_pressure = (localized_core_pressure - released).max(0.0);

        if first_cleared_step.is_none() && localized_core_pressure <= initial_core_pressure * 0.01 {
            first_cleared_step = Some(step);
        }

        trace.push(DiracAnnihilationStepEvent {
            step,
            overlap_density: bell_round6(overlap_density),
            separation_distance: bell_round6(separation_distance),
            local_phase_tensor_sum: bell_round6(phase_mixing),
            shear_torque: bell_round6(shear_torque),
            contradiction_count,
            frame_transition_active,
            localized_core_pressure: bell_round6(localized_core_pressure),
            rwif_phase_alignment: bell_round6(rwif_phase_alignment),
        });
    }

    let energy_tensor_normalization_coefficient = if frame_transition_step.is_some() {
        bell_round6(dirac_frame_energy_tensor_normalization(
            frame_transition_threshold,
            flux_coupling_density,
            crossing_ratio,
        ))
    } else {
        1.0
    };

    let released_wave_energy = bell_round6(cumulative_wave_energy * energy_tensor_normalization_coefficient);
    let reflected_wave_energy =
        bell_round6(cumulative_reflected_energy * energy_tensor_normalization_coefficient);
    let residual_torsion_hysteresis =
        bell_round6((localized_core_pressure / initial_core_pressure).clamp(0.0, 1.0) * (1.0 + (1.0 - compliance) * 0.5));
    let impedance_matching_efficiency = bell_round6(
        (released_wave_energy / (released_wave_energy + reflected_wave_energy + 1e-9)).clamp(0.0, 1.0),
    );
    let vorticity_dissipation_rate =
        bell_round6(((initial_core_pressure - localized_core_pressure) / (safe_steps as f64 + 1.0)).max(0.0));

    let unwinding_efficiency_index = if let Some(step) = first_cleared_step {
        bell_round6(1.0 - (step as f64 / safe_steps as f64))
    } else {
        0.0
    };

    let electron_charge = -1.0f64;
    let positron_charge = 1.0f64;
    let residual_charge = residual_torsion_hysteresis * (impedance - compliance).abs() * 0.01;
    let delta_q_topo = bell_round6(electron_charge + positron_charge + residual_charge);
    let torsion_leak_detected = delta_q_topo.abs() > 0.0005;

    let pressure_equivalence_error =
        bell_round6((initial_core_pressure - (released_wave_energy + reflected_wave_energy)).abs());
    let pressure_equivalence_compliant = pressure_equivalence_error <= (initial_core_pressure * 0.20);

    let phase_relaxation_gradient =
        bell_round6(((initial_core_pressure - localized_core_pressure) / safe_steps as f64).max(0.0));

    let mut invariant_violations = Vec::<String>::new();
    if torsion_leak_detected {
        invariant_violations.push("Torsion Leak".to_string());
    }
    if !pressure_equivalence_compliant {
        invariant_violations.push("Pressure-Equivalence Drift".to_string());
    }
    if frame_transition_step.is_none() {
        invariant_violations.push("Frame Transition Not Reached".to_string());
    }

    DiracAnnihilationProfileSummary {
        state_model,
        n_qubits,
        unwinding_steps: safe_steps,
        flux_coupling_density: bell_round6(flux_coupling_density),
        frame_transition_threshold,
        frame_transition_step,
        crossing_density_ratio_to_uniform: bell_round6(crossing_ratio),
        unwinding_efficiency_index,
        peak_anticrystal_contradiction_count,
        residual_torsion_hysteresis,
        impedance_matching_efficiency,
        vorticity_dissipation_rate,
        conservation: DiracAnnihilationConservationSummary {
            delta_q_topo,
            torsion_leak_detected,
            initial_mass_equivalent,
            energy_tensor_normalization_coefficient,
            released_wave_energy,
            reflected_wave_energy,
            pressure_equivalence_error,
            pressure_equivalence_compliant,
            phase_relaxation_gradient,
            invariant_violations,
        },
        rwif_unwinding_trace: trace,
    }
}

pub fn dirac_annihilation_sweep_export(
    format: &str,
    n_qubits: u8,
    profiles: &[String],
    unwinding_steps: u32,
    flux_coupling_density: f64,
    output_prefix: Option<&str>,
) -> Result<String, String> {
    if n_qubits == 0 {
        return Err("n_qubits must be >= 1".to_string());
    }
    if !(0.0..=1.0).contains(&flux_coupling_density) {
        return Err(format!(
            "flux_coupling_density must be in [0,1], got {}",
            flux_coupling_density
        ));
    }
    if unwinding_steps < 4 {
        return Err("unwinding_steps must be >= 4".to_string());
    }
    if profiles.is_empty() {
        return Err("profiles must not be empty".to_string());
    }

    let mut summaries = Vec::<DiracAnnihilationProfileSummary>::new();
    for profile in profiles {
        let canonical = canonical_dirac_state_model(profile);
        if !is_supported_dirac_state_model(canonical) {
            return Err(format!("unsupported dirac-annihilation state model: {}", profile));
        }
        summaries.push(simulate_dirac_annihilation_profile(
            canonical,
            n_qubits,
            unwinding_steps,
            flux_coupling_density,
        ));
    }

    match format {
        "json" => {
            let mut unwinding_ranked = summaries
                .iter()
                .map(|summary| {
                    (
                        summary.state_model.clone(),
                        summary.unwinding_efficiency_index,
                    )
                })
                .collect::<Vec<_>>();
            unwinding_ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

            let mut impedance_ranked = summaries
                .iter()
                .map(|summary| {
                    (
                        summary.state_model.clone(),
                        summary.impedance_matching_efficiency,
                    )
                })
                .collect::<Vec<_>>();
            impedance_ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

            let mut pressure_compliance_ranked = summaries
                .iter()
                .map(|summary| {
                    let allowable_error = summary.conservation.initial_mass_equivalent * 64.0 * 0.20;
                    let compliance_margin = allowable_error - summary.conservation.pressure_equivalence_error;
                    (
                        summary.state_model.clone(),
                        bell_round6(compliance_margin),
                        bell_round6(allowable_error),
                        summary.conservation.pressure_equivalence_error,
                        summary.conservation.pressure_equivalence_compliant,
                    )
                })
                .collect::<Vec<_>>();
            pressure_compliance_ranked
                .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

            let unwinding_rank_map = unwinding_ranked
                .iter()
                .enumerate()
                .map(|(idx, (profile, score))| {
                    (
                        profile.clone(),
                        json!({
                            "rank": idx + 1,
                            "unwinding_efficiency_index": bell_round6(*score),
                        }),
                    )
                })
                .collect::<serde_json::Map<String, serde_json::Value>>();

            let impedance_rank_map = impedance_ranked
                .iter()
                .enumerate()
                .map(|(idx, (profile, score))| {
                    (
                        profile.clone(),
                        json!({
                            "rank": idx + 1,
                            "impedance_matching_efficiency": bell_round6(*score),
                        }),
                    )
                })
                .collect::<serde_json::Map<String, serde_json::Value>>();

            let pressure_compliance_rank_map = pressure_compliance_ranked
                .iter()
                .enumerate()
                .map(|(idx, (profile, margin, allowable_error, error, compliant))| {
                    (
                        profile.clone(),
                        json!({
                            "rank": idx + 1,
                            "pressure_compliance_margin": bell_round6(*margin),
                            "allowable_pressure_equivalence_error": bell_round6(*allowable_error),
                            "pressure_equivalence_error": bell_round6(*error),
                            "pressure_equivalence_compliant": compliant,
                        }),
                    )
                })
                .collect::<serde_json::Map<String, serde_json::Value>>();

            let unwinding_rank_lookup = unwinding_ranked
                .iter()
                .enumerate()
                .map(|(idx, (profile, _))| (profile.clone(), idx + 1))
                .collect::<BTreeMap<String, usize>>();
            let impedance_rank_lookup = impedance_ranked
                .iter()
                .enumerate()
                .map(|(idx, (profile, _))| (profile.clone(), idx + 1))
                .collect::<BTreeMap<String, usize>>();
            let pressure_rank_lookup = pressure_compliance_ranked
                .iter()
                .enumerate()
                .map(|(idx, (profile, _, _, _, _))| (profile.clone(), idx + 1))
                .collect::<BTreeMap<String, usize>>();

            let mut combined_ranked = summaries
                .iter()
                .map(|summary| {
                    let profile = summary.state_model.clone();
                    let unwinding_rank = *unwinding_rank_lookup.get(profile.as_str()).unwrap_or(&usize::MAX);
                    let impedance_rank = *impedance_rank_lookup.get(profile.as_str()).unwrap_or(&usize::MAX);
                    let pressure_rank = *pressure_rank_lookup.get(profile.as_str()).unwrap_or(&usize::MAX);
                    let combined_score = bell_round6(
                        (unwinding_rank as f64 + impedance_rank as f64 + pressure_rank as f64) / 3.0,
                    );
                    (
                        profile,
                        combined_score,
                        unwinding_rank,
                        impedance_rank,
                        pressure_rank,
                    )
                })
                .collect::<Vec<_>>();
            combined_ranked.sort_by(|a, b| {
                a.1.partial_cmp(&b.1)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| a.0.cmp(&b.0))
            });

            let combined_rank_map = combined_ranked
                .iter()
                .enumerate()
                .map(|(idx, (profile, score, unwinding_rank, impedance_rank, pressure_rank))| {
                    (
                        profile.clone(),
                        json!({
                            "rank": idx + 1,
                            "combined_leaderboard_score": bell_round6(*score),
                            "unwinding_efficiency_rank": *unwinding_rank,
                            "impedance_matching_rank": *impedance_rank,
                            "pressure_compliance_rank": *pressure_rank,
                        }),
                    )
                })
                .collect::<serde_json::Map<String, serde_json::Value>>();

            let mut comparison_markdown_lines = vec![
                "| state_model | unwinding_efficiency_rank | impedance_matching_rank | pressure_compliance_rank | combined_leaderboard_score | pressure_compliance_margin | pressure_equivalence_error | allowable_pressure_equivalence_error | pressure_equivalence_compliant |".to_string(),
                "|---|---:|---:|---:|---:|---:|---:|---:|---|".to_string(),
            ];
            for summary in &summaries {
                let allowable_error = bell_round6(summary.conservation.initial_mass_equivalent * 64.0 * 0.20);
                let compliance_margin = bell_round6(allowable_error - summary.conservation.pressure_equivalence_error);
                let unwinding_rank = unwinding_rank_lookup
                    .get(summary.state_model.as_str())
                    .copied()
                    .unwrap_or(0);
                let impedance_rank = impedance_rank_lookup
                    .get(summary.state_model.as_str())
                    .copied()
                    .unwrap_or(0);
                let pressure_rank = pressure_rank_lookup
                    .get(summary.state_model.as_str())
                    .copied()
                    .unwrap_or(0);
                let combined_score = bell_round6((unwinding_rank as f64 + impedance_rank as f64 + pressure_rank as f64) / 3.0);
                comparison_markdown_lines.push(format!(
                    "| {} | {} | {} | {} | {:.6} | {:.6} | {:.6} | {:.6} | {} |",
                    summary.state_model,
                    unwinding_rank,
                    impedance_rank,
                    pressure_rank,
                    combined_score,
                    compliance_margin,
                    summary.conservation.pressure_equivalence_error,
                    allowable_error,
                    summary.conservation.pressure_equivalence_compliant,
                ));
            }

            let payload = json!({
                "object": "csif.quantum.dirac_annihilation.report",
                "schema_version": "csif_dirac_annihilation_report_v1",
                "computational_analog_label": "computational_electron_positron_topological_unwinding",
                "computational_analog_scope": "frame_aware_phase_annihilation_and_conservation_test",
                "n_qubits": n_qubits,
                "unwinding_steps": unwinding_steps,
                "flux_coupling_density": bell_round6(flux_coupling_density),
                "profiles": profiles,
                "output_prefix": output_prefix,
                "annihilation_report": summaries,
                "profile_comparison": {
                    "ranking_by_unwinding_efficiency": unwinding_ranked
                        .iter()
                        .enumerate()
                        .map(|(idx, (profile, score))| json!({
                            "rank": idx + 1,
                            "state_model": profile,
                            "unwinding_efficiency_index": bell_round6(*score),
                        }))
                        .collect::<Vec<_>>(),
                    "ranking_by_impedance_matching": impedance_ranked
                        .iter()
                        .enumerate()
                        .map(|(idx, (profile, score))| json!({
                            "rank": idx + 1,
                            "state_model": profile,
                            "impedance_matching_efficiency": bell_round6(*score),
                        }))
                        .collect::<Vec<_>>(),
                    "ranking_by_pressure_compliance": pressure_compliance_ranked
                        .iter()
                        .enumerate()
                        .map(|(idx, (profile, margin, allowable_error, error, compliant))| json!({
                            "rank": idx + 1,
                            "state_model": profile,
                            "pressure_compliance_margin": bell_round6(*margin),
                            "allowable_pressure_equivalence_error": bell_round6(*allowable_error),
                            "pressure_equivalence_error": bell_round6(*error),
                            "pressure_equivalence_compliant": compliant,
                        }))
                        .collect::<Vec<_>>(),
                    "ranking_by_combined_score": combined_ranked
                        .iter()
                        .enumerate()
                        .map(|(idx, (profile, score, unwinding_rank, impedance_rank, pressure_rank))| json!({
                            "rank": idx + 1,
                            "state_model": profile,
                            "combined_leaderboard_score": bell_round6(*score),
                            "unwinding_efficiency_rank": *unwinding_rank,
                            "impedance_matching_rank": *impedance_rank,
                            "pressure_compliance_rank": *pressure_rank,
                        }))
                        .collect::<Vec<_>>(),
                    "rank_by_profile": {
                        "unwinding_efficiency": unwinding_rank_map,
                        "impedance_matching": impedance_rank_map,
                        "pressure_compliance": pressure_compliance_rank_map,
                        "combined_score": combined_rank_map,
                    },
                    "comparison_markdown_table": comparison_markdown_lines.join("\n"),
                }
            });
            serde_json::to_string(&payload)
                .map_err(|e| format!("failed to serialize dirac-annihilation JSON export: {}", e))
        }
        "csv" => {
            let mut out = String::from(
                "state_model,n_qubits,unwinding_steps,flux_coupling_density,crossing_density_ratio_to_uniform,frame_transition_step,unwinding_efficiency_index,peak_anticrystal_contradiction_count,residual_torsion_hysteresis,impedance_matching_efficiency,vorticity_dissipation_rate,delta_q_topo,torsion_leak_detected,pressure_equivalence_error,pressure_equivalence_compliant,phase_relaxation_gradient,invariant_violation_count\n",
            );
            for summary in &summaries {
                out.push_str(&format!(
                    "{},{},{},{:.6},{:.6},{},{:.6},{},{:.6},{:.6},{:.6},{:.6},{},{:.6},{},{:.6},{}\n",
                    summary.state_model,
                    summary.n_qubits,
                    summary.unwinding_steps,
                    summary.flux_coupling_density,
                    summary.crossing_density_ratio_to_uniform,
                    summary
                        .frame_transition_step
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "".to_string()),
                    summary.unwinding_efficiency_index,
                    summary.peak_anticrystal_contradiction_count,
                    summary.residual_torsion_hysteresis,
                    summary.impedance_matching_efficiency,
                    summary.vorticity_dissipation_rate,
                    summary.conservation.delta_q_topo,
                    summary.conservation.torsion_leak_detected,
                    summary.conservation.pressure_equivalence_error,
                    summary.conservation.pressure_equivalence_compliant,
                    summary.conservation.phase_relaxation_gradient,
                    summary.conservation.invariant_violations.len(),
                ));
            }
            Ok(out)
        }
        other => Err(format!(
            "unsupported dirac-annihilation sweep export format: {}",
            other
        )),
    }
}

fn dirac_model_perturbation_coefficients(state_model: &str) -> (f64, f64, f64) {
    match canonical_dirac_state_model(state_model) {
        // High-grade anisotropy is modeled as more resistant to boundary shear,
        // with faster relaxation and lower residual hysteresis.
        "high-grade-bias" => (0.55, 1.25, 0.22),
        "low-grade-bias" => (1.20, 0.85, 0.58),
        "contiguous-band" => (0.95, 1.00, 0.40),
        "harmonic-stride" => (1.05, 0.95, 0.45),
        _ => (1.0, 1.0, 0.42),
    }
}

fn dirac_perturbation_response(
    observed_density: f64,
    low_grade_concentration: f64,
    coupling_density: f64,
    seed: u64,
    state_model: &str,
    perturbation_amplitude: f64,
    perturbation_frequency: f64,
    density_threshold: f64,
) -> (f64, f64, bool, u32, f64, f64) {
    if perturbation_amplitude <= 0.0 || perturbation_frequency <= 0.0 {
        let perturbed_active = observed_density >= density_threshold;
        return (
            bell_round6(observed_density),
            bell_round6(observed_density - density_threshold),
            perturbed_active,
            0,
            0.0,
            0.0,
        );
    }

    let (shear_scale, relaxation_scale, hysteresis_scale) =
        dirac_model_perturbation_coefficients(state_model);
    let seed_phase = ((seed ^ 0x5045525455524231) % 2048) as f64 / 2048.0;
    let phase = coupling_density * std::f64::consts::TAU * perturbation_frequency
        + seed_phase * std::f64::consts::TAU;
    let wave_shear = phase.sin().abs() * 0.75 + phase.cos().abs() * 0.25;
    let grade_exposure = 1.0 + (1.0 - low_grade_concentration).max(0.0) * 0.25;
    let effective_shear = perturbation_amplitude * wave_shear * shear_scale * grade_exposure;

    let perturbed_observed_density = (observed_density - effective_shear).clamp(0.0, 1.0);
    let perturbed_threshold_distance = perturbed_observed_density - density_threshold;
    let perturbed_dense_fallback_active = perturbed_observed_density >= density_threshold;

    let normalized_relaxation_load = (effective_shear * 96.0 / relaxation_scale.max(1e-6)).max(0.0);
    let phase_relaxation_steps = normalized_relaxation_load.ceil() as u32;
    let torsion_hysteresis = (effective_shear * hysteresis_scale).clamp(0.0, 1.0);
    let relaxation_component = ((phase_relaxation_steps as f64) / 64.0).min(1.0);
    let volatility_index =
        (0.55 * effective_shear + 0.30 * torsion_hysteresis + 0.15 * relaxation_component)
            .clamp(0.0, 1.0);

    (
        bell_round6(perturbed_observed_density),
        bell_round6(perturbed_threshold_distance),
        perturbed_dense_fallback_active,
        phase_relaxation_steps,
        bell_round6(torsion_hysteresis),
        bell_round6(volatility_index),
    )
}

pub fn bell_state_sweep_points(noise_factors: &[f64], seeds: &[u64]) -> Result<Vec<BellSweepPoint>, String> {
    if noise_factors.is_empty() {
        return Err("noise_factors must not be empty".to_string());
    }
    if seeds.is_empty() {
        return Err("seeds must not be empty".to_string());
    }

    let mut points = Vec::<BellSweepPoint>::new();
    for &noise_factor in noise_factors {
        if !(0.0..=1.0).contains(&noise_factor) {
            return Err(format!("noise_factor must be in [0,1], got {}", noise_factor));
        }
        for &seed in seeds {
            let report = bell_state_report(Some(ErrorModel {
                torsion_noise_factor: noise_factor,
                seed,
            }))?;
            let correlation_score = report
                .get("correlation")
                .and_then(Value::as_object)
                .and_then(|corr| corr.get("score"))
                .and_then(Value::as_f64)
                .ok_or_else(|| "bell sweep report missing correlation.score".to_string())?;
            let coupling = report
                .get("coupling_trivector_amplitude_after")
                .and_then(Value::as_f64)
                .ok_or_else(|| "bell sweep report missing coupling_trivector_amplitude_after".to_string())?;
            let entanglement = report
                .get("entanglement_coupling_active")
                .and_then(Value::as_bool)
                .ok_or_else(|| "bell sweep report missing entanglement_coupling_active".to_string())?;
            let stable_hash = report
                .get("stable_envelope_sha256")
                .and_then(Value::as_str)
                .ok_or_else(|| "bell sweep report missing stable_envelope_sha256".to_string())?
                .to_string();

            points.push(BellSweepPoint {
                noise_factor: bell_round6(noise_factor),
                seed,
                correlation_score: bell_round6(correlation_score),
                coupling_trivector_amplitude_after: bell_round6(coupling),
                entanglement_coupling_active: entanglement,
                stable_envelope_sha256: stable_hash,
            });
        }
    }
    Ok(points)
}

pub fn bell_state_sweep_export(
    format: &str,
    noise_factors: &[f64],
    seeds: &[u64],
) -> Result<String, String> {
    let points = bell_state_sweep_points(noise_factors, seeds)?;
    match format {
        "json" => {
            let values = points
                .iter()
                .map(|p| serde_json::to_value(p).unwrap_or(Value::Null))
                .collect::<Vec<_>>();
            serde_json::to_string(&values).map_err(|e| format!("failed to serialize JSON export: {}", e))
        }
        "csv" => {
            let mut out = String::from(
                "noise_factor,seed,correlation_score,coupling_trivector_amplitude_after,entanglement_coupling_active,stable_envelope_sha256\n",
            );
            for p in &points {
                out.push_str(&format!(
                    "{:.6},{},{:.6},{:.6},{},{}\n",
                    p.noise_factor,
                    p.seed,
                    p.correlation_score,
                    p.coupling_trivector_amplitude_after,
                    p.entanglement_coupling_active,
                    p.stable_envelope_sha256
                ));
            }
            Ok(out)
        }
        other => Err(format!("unsupported bell sweep export format: {}", other)),
    }
}

pub fn is_supported_dirac_state_model(value: &str) -> bool {
    matches!(
        value,
        "uniform-random"
            | "contiguous-band"
            | "harmonic-stride"
            | "low-grade-bias"
            | "low-grade-anisotropic"
            | "high-grade-bias"
    )
}

fn canonical_dirac_state_model(value: &str) -> &str {
    match value {
        "low-grade-anisotropic" => "low-grade-bias",
        other => other,
    }
}

fn synthetic_density_state(
    dimension: usize,
    coupling_density: f64,
    seed: u64,
    state_model: &str,
) -> Result<MultiVectorND, String> {
    if dimension == 0 {
        return Err("dirac-mode synthetic state dimension must be >= 1".to_string());
    }
    if !(0.0..=1.0).contains(&coupling_density) {
        return Err(format!("coupling_density must be in [0,1], got {}", coupling_density));
    }

    let target_nonzero = ((coupling_density * dimension as f64).round() as usize)
        .clamp(1, dimension);
    let mut active = BTreeSet::<usize>::new();
    active.insert(0);

    match state_model {
        "uniform-random" => {
            let mut rng = splitmix64(seed ^ 0x44495241435F4D4F);
            while active.len() < target_nonzero {
                let pick = (xorshift64star_next(&mut rng) as usize) % dimension;
                active.insert(pick);
            }
        }
        "contiguous-band" => {
            let mut rng = splitmix64(seed ^ 0x434F4E544947554F);
            let start = (xorshift64star_next(&mut rng) as usize) % dimension;
            for offset in 0..target_nonzero {
                active.insert((start + offset) % dimension);
            }
        }
        "harmonic-stride" => {
            let mut rng = splitmix64(seed ^ 0x4841524D4F4E4943);
            let stride_base = ((xorshift64star_next(&mut rng) as usize) % 17).max(1);
            let stride = stride_base.saturating_mul(2).saturating_add(1);
            let start = (xorshift64star_next(&mut rng) as usize) % dimension;
            let mut cursor = start;
            while active.len() < target_nonzero {
                active.insert(cursor % dimension);
                cursor = cursor.wrapping_add(stride);
            }
        }
        "low-grade-bias" | "low-grade-anisotropic" => {
            let low_grade = (0..dimension)
                .filter(|idx| idx.count_ones() <= 2)
                .collect::<Vec<_>>();
            for idx in low_grade.iter().copied().take(target_nonzero) {
                active.insert(idx);
            }
            let mut rng = splitmix64(seed ^ 0x4C4F574752414445);
            while active.len() < target_nonzero {
                let pick = (xorshift64star_next(&mut rng) as usize) % dimension;
                active.insert(pick);
            }
        }
        "high-grade-bias" => {
            let high_grade = (0..dimension)
                .filter(|idx| idx.count_ones() >= 4)
                .collect::<Vec<_>>();
            for idx in high_grade.iter().copied().take(target_nonzero) {
                active.insert(idx);
            }
            let mut rng = splitmix64(seed ^ 0x4849474847524144);
            while active.len() < target_nonzero {
                let pick = (xorshift64star_next(&mut rng) as usize) % dimension;
                active.insert(pick);
            }
        }
        other => {
            return Err(format!("unsupported dirac-mode state model: {}", other));
        }
    }

    let mut state = MultiVectorND::zero(dimension);
    for (rank, idx) in active.iter().enumerate() {
        // Keep deterministic, bounded amplitudes while guaranteeing non-zero support.
        state.components[*idx] = 1.0 / (rank as f64 + 1.0);
    }
    Ok(state)
}

fn low_grade_blade_concentration(state: &MultiVectorND) -> f64 {
    let non_zero = state.non_zero_count();
    if non_zero == 0 {
        return 0.0;
    }
    let low_grade = state
        .components
        .iter()
        .enumerate()
        .filter(|(idx, value)| **value != 0.0 && idx.count_ones() <= 2)
        .count();
    low_grade as f64 / non_zero as f64
}

fn observed_dirac_density(state: &MultiVectorND, state_model: &str) -> (f64, f64, f64) {
    let support_density = if state.dimension == 0 {
        0.0
    } else {
        state.non_zero_count() as f64 / state.dimension as f64
    };
    let low_grade_concentration = low_grade_blade_concentration(state);
    let observed_density = match canonical_dirac_state_model(state_model) {
        "low-grade-bias" => {
            let low_grade_support_density = support_density * low_grade_concentration;
            (support_density + 2.0 * low_grade_support_density).clamp(0.0, 1.0)
        }
        "high-grade-bias" => {
            let high_grade_concentration = 1.0 - low_grade_concentration;
            let high_grade_support_density = support_density * high_grade_concentration;
            (support_density - 0.7 * high_grade_support_density).clamp(0.0, 1.0)
        }
        _ => support_density,
    };
    (support_density, low_grade_concentration, observed_density)
}

#[allow(dead_code)]
pub fn dirac_mode_sweep_points(
    coupling_densities: &[f64],
    seeds: &[u64],
    n_qubits: u8,
    state_model: &str,
) -> Result<Vec<DiracModeSweepPoint>, String> {
    dirac_mode_sweep_points_with_perturbation(
        coupling_densities,
        seeds,
        n_qubits,
        state_model,
        &[0.0],
        8.0,
    )
}

pub fn dirac_mode_sweep_points_with_perturbation(
    coupling_densities: &[f64],
    seeds: &[u64],
    n_qubits: u8,
    state_model: &str,
    perturbation_amplitudes: &[f64],
    perturbation_frequency: f64,
) -> Result<Vec<DiracModeSweepPoint>, String> {
    if coupling_densities.is_empty() {
        return Err("coupling_densities must not be empty".to_string());
    }
    if seeds.is_empty() {
        return Err("seeds must not be empty".to_string());
    }
    if perturbation_amplitudes.is_empty() {
        return Err("perturbation_amplitudes must not be empty".to_string());
    }
    if perturbation_amplitudes.iter().any(|v| !(0.0..=1.0).contains(v)) {
        return Err("perturbation_amplitudes must be in [0,1]".to_string());
    }
    if !(0.0..=1024.0).contains(&perturbation_frequency) {
        return Err(format!(
            "perturbation_frequency must be in [0,1024], got {}",
            perturbation_frequency
        ));
    }
    let state_model = canonical_dirac_state_model(state_model);
    if !is_supported_dirac_state_model(state_model) {
        return Err(format!("unsupported dirac-mode state model: {}", state_model));
    }

    let register = QuantumRegister::new(n_qubits)?;
    let dimension = register.state.dimension;
    let threshold = BLOCK_SPARSE_DENSE_FALLBACK_DENSITY_THRESHOLD;
    let mut points = Vec::<DiracModeSweepPoint>::new();

    for &coupling_density in coupling_densities {
        if !(0.0..=1.0).contains(&coupling_density) {
            return Err(format!("coupling_density must be in [0,1], got {}", coupling_density));
        }

        for &seed in seeds {
            let state = synthetic_density_state(dimension, coupling_density, seed, state_model)?;
            let (support_density, low_grade_concentration, observed_density) =
                observed_dirac_density(&state, state_model);
            let threshold_distance = observed_density - threshold;
            let dense_fallback_active = observed_density >= threshold;

            for &perturbation_amplitude in perturbation_amplitudes {
                let (
                    perturbed_observed_density,
                    perturbed_threshold_distance,
                    perturbed_dense_fallback_active,
                    phase_relaxation_steps,
                    torsion_hysteresis,
                    perturbation_volatility_index,
                ) = dirac_perturbation_response(
                    observed_density,
                    low_grade_concentration,
                    coupling_density,
                    seed,
                    state_model,
                    perturbation_amplitude,
                    perturbation_frequency,
                    threshold,
                );

                let stable_source = json!({
                    "mode": "dirac_mode_density_sweep_v2_perturbation",
                    "state_model": state_model,
                    "coupling_density": bell_round6(coupling_density),
                    "seed": seed,
                    "perturbation_amplitude": bell_round6(perturbation_amplitude),
                    "perturbation_frequency": bell_round6(perturbation_frequency),
                    "n_qubits": n_qubits,
                    "state_dimension": dimension,
                    "support_density": bell_round6(support_density),
                    "low_grade_blade_concentration": bell_round6(low_grade_concentration),
                    "observed_density": bell_round6(observed_density),
                    "perturbed_observed_density": perturbed_observed_density,
                    "density_threshold": bell_round6(threshold),
                    "threshold_distance": bell_round6(threshold_distance),
                    "perturbed_threshold_distance": perturbed_threshold_distance,
                    "dense_fallback_active": dense_fallback_active,
                    "perturbed_dense_fallback_active": perturbed_dense_fallback_active,
                    "phase_relaxation_steps": phase_relaxation_steps,
                    "torsion_hysteresis": torsion_hysteresis,
                    "perturbation_volatility_index": perturbation_volatility_index,
                });
                let stable_hash = stable_sha256_hex(&stable_source)?;

                points.push(DiracModeSweepPoint {
                    state_model: state_model.to_string(),
                    coupling_density: bell_round6(coupling_density),
                    seed,
                    perturbation_amplitude: bell_round6(perturbation_amplitude),
                    perturbation_frequency: bell_round6(perturbation_frequency),
                    n_qubits,
                    state_dimension: dimension,
                    support_density: bell_round6(support_density),
                    low_grade_blade_concentration: bell_round6(low_grade_concentration),
                    observed_density: bell_round6(observed_density),
                    perturbed_observed_density,
                    density_threshold: bell_round6(threshold),
                    threshold_distance: bell_round6(threshold_distance),
                    perturbed_threshold_distance,
                    dense_fallback_active,
                    perturbed_dense_fallback_active,
                    phase_relaxation_steps,
                    torsion_hysteresis,
                    perturbation_volatility_index,
                    stable_envelope_sha256: stable_hash,
                });
            }
        }
    }

    Ok(points)
}

#[allow(dead_code)]
pub fn dirac_mode_sweep_export(
    format: &str,
    coupling_densities: &[f64],
    seeds: &[u64],
    n_qubits: u8,
    state_model: &str,
    include_summary: bool,
    include_profile_report: bool,
) -> Result<String, String> {
    dirac_mode_sweep_export_with_perturbation(
        format,
        coupling_densities,
        seeds,
        n_qubits,
        state_model,
        include_summary,
        include_profile_report,
        &[0.0],
        8.0,
    )
}

pub fn dirac_mode_sweep_export_with_perturbation(
    format: &str,
    coupling_densities: &[f64],
    seeds: &[u64],
    n_qubits: u8,
    state_model: &str,
    include_summary: bool,
    include_profile_report: bool,
    perturbation_amplitudes: &[f64],
    perturbation_frequency: f64,
) -> Result<String, String> {
    let points = dirac_mode_sweep_points_with_perturbation(
        coupling_densities,
        seeds,
        n_qubits,
        state_model,
        perturbation_amplitudes,
        perturbation_frequency,
    )?;
    let baseline_points = if include_profile_report && canonical_dirac_state_model(state_model) != "uniform-random" {
        Some(dirac_mode_sweep_points_with_perturbation(
            coupling_densities,
            seeds,
            n_qubits,
            "uniform-random",
            perturbation_amplitudes,
            perturbation_frequency,
        )?)
    } else {
        None
    };
    let summary = dirac_mode_threshold_summary(&points, baseline_points.as_deref());
    match format {
        "json" => {
            if include_summary {
                let values = points
                    .iter()
                    .map(|p| serde_json::to_value(p).unwrap_or(Value::Null))
                    .collect::<Vec<_>>();
                let report = json!({
                    "object": "csif.quantum.dirac_mode.threshold_report",
                    "schema_version": "csif_dirac_mode_threshold_report_v1",
                    "summary": summary,
                    "rows": values,
                });
                serde_json::to_string(&report)
                    .map_err(|e| format!("failed to serialize JSON export: {}", e))
            } else {
                let values = points
                    .iter()
                    .map(|p| serde_json::to_value(p).unwrap_or(Value::Null))
                    .collect::<Vec<_>>();
                serde_json::to_string(&values).map_err(|e| format!("failed to serialize JSON export: {}", e))
            }
        }
        "csv" => {
            let mut out = String::new();
            if include_summary {
                let summary_json = serde_json::to_string(&summary)
                    .map_err(|e| format!("failed to serialize summary metadata: {}", e))?;
                out.push_str("# object=csif.quantum.dirac_mode.threshold_report\n");
                out.push_str(&format!("# summary={}\n", summary_json));
            }
            out.push_str(
                "state_model,coupling_density,seed,perturbation_amplitude,perturbation_frequency,n_qubits,state_dimension,support_density,low_grade_blade_concentration,observed_density,perturbed_observed_density,density_threshold,threshold_distance,perturbed_threshold_distance,dense_fallback_active,perturbed_dense_fallback_active,phase_relaxation_steps,torsion_hysteresis,perturbation_volatility_index,stable_envelope_sha256\n",
            );
            for p in &points {
                out.push_str(&format!(
                    "{},{:.6},{},{:.6},{:.6},{},{},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{},{},{},{:.6},{:.6},{}\n",
                    p.state_model,
                    p.coupling_density,
                    p.seed,
                    p.perturbation_amplitude,
                    p.perturbation_frequency,
                    p.n_qubits,
                    p.state_dimension,
                    p.support_density,
                    p.low_grade_blade_concentration,
                    p.observed_density,
                    p.perturbed_observed_density,
                    p.density_threshold,
                    p.threshold_distance,
                    p.perturbed_threshold_distance,
                    p.dense_fallback_active,
                    p.perturbed_dense_fallback_active,
                    p.phase_relaxation_steps,
                    p.torsion_hysteresis,
                    p.perturbation_volatility_index,
                    p.stable_envelope_sha256
                ));
            }
            Ok(out)
        }
        other => Err(format!("unsupported dirac-mode sweep export format: {}", other)),
    }
}

pub fn dirac_mode_threshold_summary(
    points: &[DiracModeSweepPoint],
    baseline_points: Option<&[DiracModeSweepPoint]>,
) -> DiracModeThresholdSummary {
    let density_threshold = bell_round6(BLOCK_SPARSE_DENSE_FALLBACK_DENSITY_THRESHOLD);
    let state_model = points
        .first()
        .map(|p| p.state_model.clone())
        .unwrap_or_else(|| "unknown".to_string());
    let mut seeds = points.iter().map(|p| p.seed).collect::<Vec<_>>();
    seeds.sort_unstable();
    seeds.dedup();

    let mut per_seed = Vec::<DiracModeSeedCrossing>::new();
    let mut crossing_values = Vec::<f64>::new();

    for seed in seeds {
        let first = points
            .iter()
            .filter(|p| p.seed == seed && p.dense_fallback_active)
            .map(|p| p.coupling_density)
            .fold(None, |acc: Option<f64>, value| match acc {
                Some(current) => Some(current.min(value)),
                None => Some(value),
            });
        if let Some(value) = first {
            crossing_values.push(value);
        }
        per_seed.push(DiracModeSeedCrossing {
            seed,
            first_crossing_density: first,
        });
    }

    let first_crossing_density = crossing_values
        .iter()
        .copied()
        .fold(None, |acc: Option<f64>, value| match acc {
            Some(current) => Some(current.min(value)),
            None => Some(value),
        })
        .map(bell_round6);

    let (spread_min, spread_max, spread) = if crossing_values.is_empty() {
        (None, None, None)
    } else {
        let min_v = crossing_values
            .iter()
            .copied()
            .fold(f64::INFINITY, f64::min);
        let max_v = crossing_values
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        (
            Some(bell_round6(min_v)),
            Some(bell_round6(max_v)),
            Some(bell_round6(max_v - min_v)),
        )
    };

    let (baseline_state_model, baseline_first_crossing_density, baseline_per_seed_crossing_spread, delta_from_baseline, ratio_to_baseline) =
        if let Some(baseline) = baseline_points {
            let baseline_summary = dirac_mode_threshold_summary(baseline, None);
            let delta = match (first_crossing_density, baseline_summary.first_crossing_density) {
                (Some(current), Some(base)) => Some(bell_round6(current - base)),
                _ => None,
            };
            let ratio = match (first_crossing_density, baseline_summary.first_crossing_density) {
                (Some(current), Some(base)) if base.abs() > f64::EPSILON => {
                    Some(bell_round6(current / base))
                }
                _ => None,
            };
            (
                Some(baseline_summary.state_model),
                baseline_summary.first_crossing_density,
                baseline_summary.per_seed_crossing_spread,
                delta,
                ratio,
            )
        } else {
            (None, None, None, None, None)
        };

    let perturbation_frequency = points
        .first()
        .map(|p| p.perturbation_frequency)
        .unwrap_or(0.0);
    let mut perturbation_amplitudes = points
        .iter()
        .map(|p| p.perturbation_amplitude)
        .collect::<Vec<_>>();
    perturbation_amplitudes.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    perturbation_amplitudes.dedup_by(|a, b| (*a - *b).abs() <= 1e-9);

    let phase_relaxation_steps_mean = if points.is_empty() {
        0.0
    } else {
        bell_round6(
            points
                .iter()
                .map(|p| p.phase_relaxation_steps as f64)
                .sum::<f64>()
                / points.len() as f64,
        )
    };
    let torsion_hysteresis_mean = if points.is_empty() {
        0.0
    } else {
        bell_round6(points.iter().map(|p| p.torsion_hysteresis).sum::<f64>() / points.len() as f64)
    };
    let volatility_index_mean = if points.is_empty() {
        0.0
    } else {
        bell_round6(
            points
                .iter()
                .map(|p| p.perturbation_volatility_index)
                .sum::<f64>()
                / points.len() as f64,
        )
    };
    let volatility_index_max = bell_round6(
        points
            .iter()
            .map(|p| p.perturbation_volatility_index)
            .fold(0.0f64, f64::max),
    );
    let catastrophic_unraveling_amplitude = points
        .iter()
        .filter(|p| p.dense_fallback_active && !p.perturbed_dense_fallback_active)
        .map(|p| p.perturbation_amplitude)
        .fold(None, |acc: Option<f64>, value| match acc {
            Some(current) => Some(current.min(value)),
            None => Some(value),
        })
        .map(bell_round6);

    DiracModeThresholdSummary {
        computational_analog_label: "computational_schwinger_limit_analog".to_string(),
        computational_analog_scope: "sparse_to_dense_crystallization_threshold".to_string(),
        state_model,
        baseline_state_model,
        baseline_first_crossing_density,
        first_crossing_density_delta_from_baseline: delta_from_baseline,
        crossing_density_ratio_to_uniform: ratio_to_baseline,
        baseline_per_seed_crossing_spread,
        density_threshold,
        first_crossing_density,
        crossing_seed_count: crossing_values.len(),
        per_seed_first_crossing_density: per_seed,
        per_seed_crossing_spread_min: spread_min,
        per_seed_crossing_spread_max: spread_max,
        per_seed_crossing_spread: spread,
        perturbation_frequency: bell_round6(perturbation_frequency),
        perturbation_amplitudes,
        phase_relaxation_steps_mean,
        torsion_hysteresis_mean,
        volatility_index_mean,
        volatility_index_max,
        catastrophic_unraveling_amplitude,
    }
}

fn bell_round6(value: f64) -> f64 {
    (value * 1_000_000.0).round() / 1_000_000.0
}

impl RegisterGateApplier for QuantumRegister {
    fn apply_gate(&mut self, gate: GateSpec) -> Result<TorsionEvent, String> {
        if gate.targets.is_empty() {
            return Err("gate targets must not be empty".to_string());
        }
        if gate.targets.iter().any(|q| *q >= usize::from(self.n_qubits)) {
            return Err("gate target is out of range".to_string());
        }

        let rotor = gate.to_rotor();

        let prev_state = self.state.clone();
        let evolved = evolve_gate_state(&prev_state, &rotor, self.state.dimension)?;

        let local_before = blade_component_magnitude(&prev_state, rotor.blade_mask);
        let local_after = blade_component_magnitude(&evolved, rotor.blade_mask);
        let coupling_before = blade_component_magnitude(&prev_state, rotor.coupling_blade_mask);
        let coupling_after = blade_component_magnitude(&evolved, rotor.coupling_blade_mask);
        let coupling_transfer_scalar = (coupling_after - coupling_before).max(0.0);
        let non_scalar_magnitude = evolved.non_scalar_magnitude();
        let normalized_coupling_intensity = if non_scalar_magnitude <= 1e-12 {
            0.0
        } else {
            (coupling_transfer_scalar / non_scalar_magnitude).clamp(0.0, 1.0)
        };
        let geometric_convergence_metric = geometric_convergence_metric(&evolved, rotor.coupling_blade_mask);
        let entanglement_coupling_active = rotor.coupling_blade_mask.is_some() && coupling_after > 0.0;

        let prev_norm = prev_state.norm();
        let new_norm = evolved.norm();
        let denom = prev_norm * new_norm;
        let alignment = if denom <= f64::EPSILON {
            1.0
        } else {
            (prev_state.dot(&evolved) / denom).clamp(-1.0, 1.0)
        };
        let torsion_delta = alignment.acos();
        let phase_alignment_index = ((alignment + 1.0) * 0.5).clamp(0.0, 1.0);

        self.tick += 1;
        self.global_torsion += torsion_delta;
        self.state = evolved;

        let event = TorsionEvent {
            tick: self.tick,
            gate_id: gate.gate_id.clone(),
            torsion_delta,
            cumulative_torsion: self.global_torsion,
            torsion_scalar: torsion_delta,
            phase_alignment_index,
        };

        let rwif = RwifGateTrace {
            schema_version: "rwif_gate_trace_v1".to_string(),
            tick: self.tick,
            gate_id: gate.gate_id,
            rotor_id: rotor.rotor_id,
            plane_label: rotor.plane_label,
            blade_mask: rotor.blade_mask,
            blade_label: rotor.blade_label,
            blade_grade: rotor.blade_grade,
            grade_classification: rotor.grade_classification,
            coupling_blade_mask: rotor.coupling_blade_mask,
            coupling_manifold: rotor.coupling_manifold,
            coupling_blade_grade: rotor.coupling_blade_grade,
            coupling_grade_classification: rotor.coupling_grade_classification,
            local_bivector_amplitude_before: local_before,
            local_bivector_amplitude_after: local_after,
            coupling_trivector_amplitude_before: coupling_before,
            coupling_trivector_amplitude_after: coupling_after,
            coupling_transfer_scalar,
            normalized_coupling_intensity,
            geometric_convergence_metric,
            entanglement_coupling_active,
            torsion_scalar: event.torsion_scalar,
            phase_alignment_index: event.phase_alignment_index,
        };

        self.rwif_trace.push(rwif);
        self.trace.push(event.clone());
        Ok(event)
    }
}

fn basis_pair_mask(qubit_index: usize) -> Option<usize> {
    let a = qubit_index.checked_mul(2)?;
    let b = a.checked_add(1)?;
    if b >= usize::BITS as usize {
        return None;
    }
    Some((1usize << a) | (1usize << b))
}

fn cnot_coupling_mask(control_qubit: usize, target_qubit: usize) -> Option<usize> {
    let c0 = control_qubit.checked_mul(2)?;
    let c1 = c0.checked_add(1)?;
    let t0 = target_qubit.checked_mul(2)?;
    if c1 >= usize::BITS as usize || t0 >= usize::BITS as usize {
        return None;
    }
    Some((1usize << c0) | (1usize << c1) | (1usize << t0))
}

fn blade_mask_to_clifford_label(mask: usize) -> String {
    if mask == 0 {
        return "1".to_string();
    }

    let mut basis = Vec::new();
    let mut bits = mask;
    while bits != 0 {
        let idx = bits.trailing_zeros() as usize;
        basis.push(format!("γ{}", idx));
        bits &= bits - 1;
    }
    basis.join("∧")
}

fn blade_grade_and_classification(mask: usize) -> (u8, String) {
    let grade = mask.count_ones() as u8;
    let classification = match grade {
        0 => "scalar".to_string(),
        1 => "vector".to_string(),
        2 => "bivector".to_string(),
        3 => "trivector".to_string(),
        _ => format!("grade_{}", grade),
    };
    (grade, classification)
}

fn blade_component_magnitude(state: &MultiVectorND, mask: Option<usize>) -> f64 {
    match mask {
        Some(idx) if idx < state.dimension => state.components[idx].abs(),
        _ => 0.0,
    }
}

fn geometric_convergence_metric(state: &MultiVectorND, coupling_mask: Option<usize>) -> f64 {
    let scalar = state.components.first().copied().unwrap_or(0.0).abs();
    let total_non_scalar: f64 = state
        .components
        .iter()
        .skip(1)
        .map(|component| component.abs())
        .sum();
    let geometric_activity = match coupling_mask {
        Some(_) => blade_component_magnitude(state, coupling_mask),
        None => total_non_scalar,
    };
    let total_magnitude = scalar + total_non_scalar;
    if total_magnitude <= 1e-12 && geometric_activity <= 1e-12 {
        0.0
    } else {
        (geometric_activity / total_magnitude.max(1e-12)).clamp(0.0, 1.0)
    }
}

#[allow(dead_code)]
fn blend_states(local_state: &MultiVectorND, coupling_state: &MultiVectorND, local_weight: f64, coupling_weight: f64) -> MultiVectorND {
    let mut blended = MultiVectorND::zero(local_state.dimension);
    for idx in 0..local_state.dimension {
        blended.components[idx] = local_state.components[idx] * local_weight
            + coupling_state.components[idx] * coupling_weight;
    }
    blended
}

fn tuned_block_size_for_state(state: &MultiVectorND) -> usize {
    if state.dimension == 0 {
        return BLOCK_SPARSE_BLOCK_SIZE;
    }

    let mut nonzero_indices = Vec::new();
    for (idx, value) in state.components.iter().enumerate() {
        if *value != 0.0 {
            nonzero_indices.push(idx);
        }
    }
    if nonzero_indices.is_empty() {
        return BLOCK_SPARSE_BLOCK_SIZE.min(state.dimension.max(1));
    }

    let density = nonzero_indices.len() as f64 / state.dimension as f64;
    let mut best_block = BLOCK_SPARSE_BLOCK_SIZE.min(state.dimension.max(1));
    let mut best_score = f64::INFINITY;

    for candidate in BLOCK_SIZE_TUNING_CANDIDATES {
        if candidate > state.dimension {
            continue;
        }

        let mut touched_blocks = 0usize;
        let mut last_block = usize::MAX;
        for idx in &nonzero_indices {
            let block = idx / candidate;
            if block != last_block {
                touched_blocks += 1;
                last_block = block;
            }
        }

        // Lower score is better: fewer touched blocks, with a light penalty that grows
        // with candidate size and state density so we do not over-inflate block width.
        let score = touched_blocks as f64 + (candidate as f64 / 256.0) * (1.0 + density * 8.0);
        if score < best_score {
            best_score = score;
            best_block = candidate;
        }
    }

    best_block
}

fn should_use_dense_fallback(state: &MultiVectorND) -> bool {
    if state.dimension == 0 {
        return false;
    }
    let density = state.non_zero_count() as f64 / state.dimension as f64;
    density >= BLOCK_SPARSE_DENSE_FALLBACK_DENSITY_THRESHOLD
}

fn evolve_gate_state(prev_state: &MultiVectorND, rotor: &RotorSpec, dimension: usize) -> Result<MultiVectorND, String> {
    let evolved = if rotor.rotor_id.starts_with("rotor:noop") || rotor.plane_label == "identity" {
        prev_state.clone()
    } else {
        let rotor_mv = rotor_multivector(rotor, dimension)?;
        if should_use_dense_fallback(prev_state) {
            rotor_mv.geometric_product(prev_state)?
        } else {
            let tuned_block_size = tuned_block_size_for_state(prev_state);
            let prev_sparse = BlockSparseState::from_dense(&prev_state.components, tuned_block_size);
            // Keep the operation exact while reducing dense scans on mostly-zero state support.
            let evolved_sparse = rotor_mv.geometric_product_left_block_sparse(&prev_sparse)?;
            evolved_sparse.to_dense_multivector()
        }
    };
    Ok(evolved)
}

#[allow(dead_code)]
fn optimization_ascii_bar(local_weight: f64, coupling_weight: f64, width: usize) -> String {
    let active = (coupling_weight.clamp(0.0, 1.0) * width as f64).round() as usize;
    let coupling_part = "#".repeat(active.min(width));
    let local_part = ".".repeat(width.saturating_sub(coupling_part.len()));
    let _ = local_weight;
    format!("{}{}", coupling_part, local_part)
}

fn rotor_multivector(rotor: &RotorSpec, dimension: usize) -> Result<MultiVectorND, String> {
    let mut mv = MultiVectorND::zero(dimension);
    let half_angle = rotor.angle_radians * 0.5;
    mv.components[0] = half_angle.cos();

    let base_sin = half_angle.sin();
    let mut local_weight = 1.0;
    let mut coupling_weight = 0.0;

    if rotor.coupling_blade_mask.is_some() {
        // CNOT-style coupling: split rotation support between local and coupling manifolds.
        local_weight = 0.8;
        coupling_weight = 0.2;
    }

    if let Some(mask) = rotor.blade_mask {
        if mask >= dimension {
            return Err("rotor blade mask exceeds manifold dimension".to_string());
        }
        mv.components[mask] = base_sin * local_weight;
    }

    if let Some(coupling_mask) = rotor.coupling_blade_mask {
        if coupling_mask >= dimension {
            return Err("rotor coupling blade mask exceeds manifold dimension".to_string());
        }
        mv.components[coupling_mask] = base_sin * coupling_weight;
    }
    Ok(mv)
}

fn compute_clifford_sign(a_idx: usize, b_idx: usize) -> f64 {
    // Euclidean Clifford sign via swap parity: e_i e_j = -e_j e_i for i != j.
    let mut parity = 0u32;
    let mut a = a_idx;
    while a != 0 {
        let i = a.trailing_zeros() as usize;
        let lower_mask = if i == 0 { 0usize } else { (1usize << i) - 1 };
        parity ^= (b_idx & lower_mask).count_ones() & 1;
        a &= a - 1;
    }
    if parity == 0 { 1.0 } else { -1.0 }
}

#[allow(dead_code)]
fn reversion_sign(grade: u32) -> f64 {
    // Reverse changes sign for grades with g(g-1)/2 odd parity.
    let parity = ((grade * (grade.saturating_sub(1))) / 2) % 2;
    if parity == 0 { 1.0 } else { -1.0 }
}

impl MultiVectorND {
    pub fn dot(&self, other: &Self) -> f64 {
        debug_assert_eq!(
            self.dimension, other.dimension,
            "dot product dimension mismatch"
        );
        self.components
            .iter()
            .zip(other.components.iter())
            .map(|(a, b)| a * b)
            .sum::<f64>()
    }

    pub fn norm(&self) -> f64 {
        self.dot(self).max(0.0).sqrt()
    }

    pub fn non_scalar_magnitude(&self) -> f64 {
        self.components
            .iter()
            .skip(1)
            .map(|c| c * c)
            .sum::<f64>()
            .max(0.0)
            .sqrt()
    }

    pub fn non_zero_count(&self) -> usize {
        self.components.iter().filter(|v| **v != 0.0).count()
    }

    pub fn geometric_product(&self, other: &Self) -> Result<Self, String> {
        if self.dimension != other.dimension {
            return Err("geometric_product dimension mismatch".to_string());
        }
        let mut result = Self::zero(self.dimension);

        for a_idx in 0..self.dimension {
            let a = self.components[a_idx];
            if a == 0.0 {
                continue;
            }
            for b_idx in 0..self.dimension {
                let b = other.components[b_idx];
                if b == 0.0 {
                    continue;
                }
                let target_idx = a_idx ^ b_idx;
                let sign = compute_clifford_sign(a_idx, b_idx);
                result.components[target_idx] += a * b * sign;
            }
        }
        Ok(result)
    }

    #[allow(dead_code)]
    pub fn geometric_product_left_sparse(&self, other: &Self) -> Result<Self, String> {
        if self.dimension != other.dimension {
            return Err("geometric_product dimension mismatch".to_string());
        }

        let mut active_left = Vec::new();
        for a_idx in 0..self.dimension {
            let a = self.components[a_idx];
            if a != 0.0 {
                active_left.push((a_idx, a));
            }
        }

        let mut result = Self::zero(self.dimension);
        for (a_idx, a) in active_left {
            for b_idx in 0..other.dimension {
                let b = other.components[b_idx];
                if b == 0.0 {
                    continue;
                }
                let target_idx = a_idx ^ b_idx;
                let sign = compute_clifford_sign(a_idx, b_idx);
                result.components[target_idx] += a * b * sign;
            }
        }
        Ok(result)
    }

    fn geometric_product_left_block_sparse(&self, rhs: &BlockSparseState) -> Result<BlockSparseState, String> {
        if self.dimension != rhs.dimension {
            return Err("geometric_product dimension mismatch".to_string());
        }

        let mut active_left = Vec::new();
        for a_idx in 0..self.dimension {
            let a = self.components[a_idx];
            if a != 0.0 {
                active_left.push((a_idx, a));
            }
        }

        let mut result_dense_blocks: BTreeMap<usize, Vec<f64>> = BTreeMap::new();
        for (a_idx, a) in active_left {
            for (rhs_block_idx, rhs_block_values) in &rhs.blocks {
                let rhs_base = rhs_block_idx * rhs.block_size;
                for (offset, b) in rhs_block_values {
                    let b_idx = rhs_base + offset;
                    if b_idx >= rhs.dimension {
                        break;
                    }
                    let target_idx = a_idx ^ b_idx;
                    let sign = compute_clifford_sign(a_idx, b_idx);
                    let target_block_idx = target_idx / rhs.block_size;
                    let target_offset = target_idx % rhs.block_size;
                    let block = result_dense_blocks
                        .entry(target_block_idx)
                        .or_insert_with(|| vec![0.0; rhs.block_size]);
                    block[target_offset] += a * b * sign;
                }
            }
        }

        let mut result_blocks = BTreeMap::new();
        for (block_idx, dense_values) in result_dense_blocks {
            let mut entries = Vec::new();
            for (offset, value) in dense_values.into_iter().enumerate() {
                if value != 0.0 {
                    entries.push((offset, value));
                }
            }
            if !entries.is_empty() {
                result_blocks.insert(block_idx, entries);
            }
        }

        Ok(BlockSparseState {
            dimension: rhs.dimension,
            block_size: rhs.block_size,
            blocks: result_blocks,
        })
    }

    #[allow(dead_code)]
    pub fn reverse(&self) -> Self {
        let mut out = self.clone();
        for idx in 0..self.dimension {
            let grade = idx.count_ones();
            out.components[idx] *= reversion_sign(grade);
        }
        out
    }
}

pub fn canonicalize_json_value(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries = map.iter().collect::<Vec<_>>();
            entries.sort_by(|(ka, _), (kb, _)| ka.cmp(kb));
            let mut canonical = serde_json::Map::new();
            for (k, v) in entries {
                canonical.insert(k.clone(), canonicalize_json_value(v));
            }
            Value::Object(canonical)
        }
        Value::Array(items) => Value::Array(items.iter().map(canonicalize_json_value).collect::<Vec<_>>()),
        _ => value.clone(),
    }
}

pub fn stable_json_string(value: &Value) -> Result<String, String> {
    let canonical = canonicalize_json_value(value);
    serde_json::to_string(&canonical).map_err(|e| format!("stable json serialization failed: {}", e))
}

pub fn stable_sha256_hex(value: &Value) -> Result<String, String> {
    let stable = stable_json_string(value)?;
    let mut hasher = Sha256::new();
    hasher.update(stable.as_bytes());
    let digest = hasher.finalize();
    Ok(format!("{:x}", digest))
}

#[cfg(test)]
pub fn replay_trace_envelope(n_qubits: u8, trace: &[GateSpec]) -> Result<Value, String> {
    let mut register = QuantumRegister::new(n_qubits)?;
    for gate in trace {
        let _ = register.apply_gate(gate.clone())?;
    }

    let measurement = register.measure_qubit(0)?;
    let mut payload = register.interface_contract();
    payload["trace_input_len"] = json!(trace.len());
    payload["example_measurement"] = serde_json::to_value(measurement)
        .unwrap_or_else(|_| json!({"error": "serialization_failed"}));
    payload["rwif_edge_envelopes"] = Value::Array(
        register
            .rwif_trace
            .iter()
            .map(RwifGateTrace::to_rwif_edge_envelope)
            .collect::<Vec<_>>(),
    );
    Ok(payload)
}

impl DeterministicProjector for QuantumRegister {
    fn measure_qubit(&self, qubit: usize) -> Result<MeasurementOutcome, String> {
        if qubit >= usize::from(self.n_qubits) {
            return Err("measurement target is out of range".to_string());
        }

        let basis_state = if self.global_torsion.fract() >= 0.5 { 1 } else { 0 };
        let certainty = (1.0 - (self.global_torsion.fract() - 0.5).abs() * 2.0).clamp(0.0, 1.0);
        let geometric_weight = certainty * certainty;
        Ok(MeasurementOutcome {
            qubit,
            basis_state,
            certainty,
            geometric_certainty: certainty,
            geometric_weight,
            tie_break_rule: "lowest_basis_index".to_string(),
            projection_basis: "computational_z".to_string(),
        })
    }
}

fn parse_hidden_bit_string(hidden: &str) -> Result<Vec<u8>, String> {
    if hidden.is_empty() {
        return Err("hidden string must not be empty".to_string());
    }
    let mut bits = Vec::with_capacity(hidden.len());
    for ch in hidden.chars() {
        match ch {
            '0' => bits.push(0),
            '1' => bits.push(1),
            _ => return Err("hidden string must contain only '0' and '1'".to_string()),
        }
    }
    Ok(bits)
}

fn splitmix64(seed: u64) -> u64 {
    let mut z = seed.wrapping_add(0x9e3779b97f4a7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}

fn xorshift64star_next(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    *state = x;
    x.wrapping_mul(0x2545f4914f6cdd1d)
}

fn unit_interval_sample(state: &mut u64) -> f64 {
    const SCALE: f64 = (1u64 << 53) as f64;
    let raw = xorshift64star_next(state) >> 11;
    (raw as f64) / SCALE
}

fn qubit_one_probability_from_state(state: &MultiVectorND, qubit: usize, n_qubits: u8) -> f64 {
    let rank = usize::from(n_qubits) * 2;
    let bit_idx = qubit.saturating_mul(2).saturating_add(1);
    if rank == 0 || bit_idx >= rank {
        return 0.5;
    }

    let mut one_weight = 0.0;
    let mut total_weight = 0.0;
    for (idx, value) in state.components.iter().enumerate() {
        let weight = value * value;
        total_weight += weight;
        if ((idx >> bit_idx) & 1usize) == 1usize {
            one_weight += weight;
        }
    }

    if total_weight <= 1e-15 {
        0.5
    } else {
        (one_weight / total_weight).clamp(0.0, 1.0)
    }
}

fn shot_measure_qubit(
    register: &QuantumRegister,
    qubit: usize,
    shots: u32,
    seed: u64,
) -> Result<ShotMeasurementSummary, String> {
    if qubit >= usize::from(register.n_qubits) {
        return Err("measurement target is out of range".to_string());
    }

    let one_probability = qubit_one_probability_from_state(&register.state, qubit, register.n_qubits);
    let mut rng_state = splitmix64(seed);
    let mut ones = 0u32;
    for _ in 0..shots {
        if unit_interval_sample(&mut rng_state) < one_probability {
            ones += 1;
        }
    }
    let zeros = shots.saturating_sub(ones);
    let inferred_bit = if ones * 2 >= shots { 1 } else { 0 };

    Ok(ShotMeasurementSummary {
        qubit,
        shots,
        ones,
        zeros,
        one_probability,
        inferred_bit,
        sampler: "seeded_xorshift64star_v1".to_string(),
    })
}

fn recover_hidden_from_black_box_measurements(
    register: &QuantumRegister,
    query_len: usize,
    shots: u32,
    run_seed: u64,
) -> Result<(String, Vec<ShotMeasurementSummary>), String> {
    let mut recovered = String::with_capacity(query_len);
    let mut summaries = Vec::with_capacity(query_len);

    for qubit in 0..query_len {
        let qubit_seed = run_seed
            ^ ((qubit as u64).wrapping_mul(0x9e3779b97f4a7c15))
            ^ register.tick;
        let summary = shot_measure_qubit(register, qubit, shots, qubit_seed)?;
        recovered.push(if summary.inferred_bit == 1 { '1' } else { '0' });
        summaries.push(summary);
    }

    Ok((recovered, summaries))
}

fn estimated_bit_confidence(summary: &ShotMeasurementSummary) -> f64 {
    if summary.shots == 0 {
        return 0.5;
    }
    let majority = summary.ones.max(summary.zeros) as f64;
    (majority / summary.shots as f64).clamp(0.5, 1.0)
}

fn minimum_shots_for_target_confidence(
    observed_one_probability: f64,
    target_confidence: f64,
) -> Option<u32> {
    let delta = (observed_one_probability - 0.5).abs();
    if delta <= 1e-12 {
        return None;
    }

    if target_confidence <= 0.0 {
        return Some(1);
    }
    if target_confidence >= 1.0 {
        return None;
    }

    // Hoeffding-style majority bound: P(error) <= exp(-2 n delta^2).
    // Require 1 - exp(-2 n delta^2) >= target_confidence.
    let rhs = (1.0 / (1.0 - target_confidence)).ln();
    let n = (rhs / (2.0 * delta * delta)).ceil();
    if !n.is_finite() || n > u32::MAX as f64 {
        None
    } else {
        Some((n as u32).max(1))
    }
}

fn build_reality_calibration_summary(
    shot_measurements: &[ShotMeasurementSummary],
    target_confidence_level: f64,
) -> RealityCalibrationSummary {
    let mut per_bit = Vec::with_capacity(shot_measurements.len());
    let mut whole_string_confidence = 1.0;
    let mut minimum_shots_for_target = Some(1u32);

    let target_bit_confidence = if shot_measurements.is_empty() {
        target_confidence_level
    } else {
        target_confidence_level
            .clamp(0.0, 1.0)
            .powf(1.0 / shot_measurements.len() as f64)
    };

    for summary in shot_measurements {
        let confidence = estimated_bit_confidence(summary);
        whole_string_confidence = (whole_string_confidence * confidence).clamp(0.0, 1.0);

        let bit_min_shots = minimum_shots_for_target_confidence(
            summary.one_probability,
            target_bit_confidence,
        );

        minimum_shots_for_target = match (
            minimum_shots_for_target,
            bit_min_shots,
        ) {
            (Some(current), Some(required)) => Some(current.max(required)),
            _ => None,
        };

        per_bit.push(BitConfidenceEstimate {
            qubit: summary.qubit,
            inferred_bit: summary.inferred_bit,
            estimated_confidence: confidence,
            minimum_shots_for_target_confidence: bit_min_shots,
        });
    }

    RealityCalibrationSummary {
        confidence_model: "hoeffding_majority_bound_v1".to_string(),
        target_confidence_level,
        per_bit,
        whole_string_confidence,
        minimum_shots_for_target_confidence: minimum_shots_for_target,
    }
}

fn recover_hidden_from_oracle(oracle: &[GateSpec], query_len: usize, ancilla: usize) -> String {
    let mut recovered = vec!['0'; query_len];
    for gate in oracle {
        if !matches!(gate.kind, GateKind::Cnot) {
            continue;
        }
        if gate.targets.len() < 2 {
            continue;
        }
        let control = gate.targets[0];
        let target = gate.targets[1];
        if target == ancilla && control < query_len {
            recovered[control] = '1';
        }
    }
    recovered.into_iter().collect::<String>()
}

pub fn bv_oracle_from_hidden_string(hidden: &str) -> Result<Vec<GateSpec>, String> {
    let bits = parse_hidden_bit_string(hidden)?;
    let query_len = bits.len();
    let register_qubits = query_len
        .checked_add(1)
        .ok_or_else(|| "bv register size overflow".to_string())?;
    if register_qubits > usize::from(MAX_SCAFFOLD_QUBITS) {
        return Err(format!(
            "hidden string length {} exceeds scaffold limit (max query bits = {})",
            query_len,
            usize::from(MAX_SCAFFOLD_QUBITS) - 1
        ));
    }

    let ancilla = query_len;
    let mut oracle = Vec::new();
    for (idx, bit) in bits.iter().enumerate() {
        if *bit == 1 {
            oracle.push(GateSpec {
                gate_id: format!("bv_oracle_cnot_q{}", idx),
                kind: GateKind::Cnot,
                targets: vec![idx, ancilla],
                angle_radians: Some(std::f64::consts::FRAC_PI_2),
            });
        }
    }
    Ok(oracle)
}

pub fn run_bernstein_vazirani(hidden: &str) -> Result<Value, String> {
    let bits = parse_hidden_bit_string(hidden)?;
    let query_len = bits.len();
    let ancilla = query_len;
    let total_qubits = query_len
        .checked_add(1)
        .ok_or_else(|| "bv register size overflow".to_string())?;
    let mut register = QuantumRegister::new(total_qubits as u8)?;

    let _ = register.apply_gate(GateSpec {
        gate_id: "bv_prepare_ancilla_x".to_string(),
        kind: GateKind::PauliX,
        targets: vec![ancilla],
        angle_radians: Some(std::f64::consts::PI),
    })?;
    let _ = register.apply_gate(GateSpec {
        gate_id: "bv_prepare_ancilla_h".to_string(),
        kind: GateKind::Hadamard,
        targets: vec![ancilla],
        angle_radians: Some(std::f64::consts::FRAC_PI_2),
    })?;

    for q in 0..query_len {
        let _ = register.apply_gate(GateSpec {
            gate_id: format!("bv_pre_h_q{}", q),
            kind: GateKind::Hadamard,
            targets: vec![q],
            angle_radians: Some(std::f64::consts::FRAC_PI_2),
        })?;
    }

    let oracle = bv_oracle_from_hidden_string(hidden)?;
    let oracle_call_count = 1usize;
    let oracle_internal_gate_count = oracle.len();
    for gate in &oracle {
        let _ = register.apply_gate(gate.clone())?;
    }

    for q in 0..query_len {
        let _ = register.apply_gate(GateSpec {
            gate_id: format!("bv_post_h_q{}", q),
            kind: GateKind::Hadamard,
            targets: vec![q],
            angle_radians: Some(std::f64::consts::FRAC_PI_2),
        })?;
    }

    let recovered_hidden_string = recover_hidden_from_oracle(&oracle, query_len, ancilla);
    let deterministic_match = recovered_hidden_string == hidden;

    let rwif_event_envelopes = register
        .rwif_trace
        .iter()
        .map(RwifGateTrace::to_rwif_event_envelope)
        .map(|mut event| {
            if let Value::Object(ref mut map) = event {
                map.insert("bv_oracle_call_count".to_string(), json!(oracle_call_count));
                map.insert(
                    "bv_oracle_internal_gate_count".to_string(),
                    json!(oracle_internal_gate_count),
                );
                map.insert(
                    "bv_recovered_hidden_string".to_string(),
                    json!(recovered_hidden_string.clone()),
                );
            }
            event
        })
        .collect::<Vec<_>>();

    let rwif_edge_envelopes = register
        .rwif_trace
        .iter()
        .map(RwifGateTrace::to_rwif_edge_envelope)
        .map(|mut edge| {
            if let Value::Object(ref mut map) = edge {
                map.insert("bv_oracle_call_count".to_string(), json!(oracle_call_count));
                map.insert(
                    "bv_oracle_internal_gate_count".to_string(),
                    json!(oracle_internal_gate_count),
                );
                map.insert(
                    "bv_recovered_hidden_string".to_string(),
                    json!(recovered_hidden_string.clone()),
                );
            }
            edge
        })
        .collect::<Vec<_>>();

    let mut payload = register.interface_contract();
    payload["execution_mode"] = json!("structural");
    payload["algorithm"] = json!("bernstein_vazirani");
    payload["hidden_string"] = json!(hidden);
    payload["recovered_hidden_string"] = json!(recovered_hidden_string);
    payload["deterministic_match"] = json!(deterministic_match);
    payload["oracle_call_count"] = json!(oracle_call_count);
    payload["oracle_internal_gate_count"] = json!(oracle_internal_gate_count);
    payload["rwif_event_envelopes"] = Value::Array(rwif_event_envelopes);
    payload["rwif_edge_envelopes"] = Value::Array(rwif_edge_envelopes);
    payload["example_measurement"] = serde_json::to_value(register.measure_qubit(0)?)
        .unwrap_or_else(|_| json!({"error": "serialization_failed"}));

    let stable_hash = stable_sha256_hex(&payload)?;
    payload["stable_envelope_sha256"] = json!(stable_hash);
    Ok(payload)
}

pub fn run_bernstein_vazirani_black_box(hidden: &str) -> Result<Value, String> {
    run_bernstein_vazirani_black_box_with_shots(hidden, DEFAULT_BV_BLACK_BOX_SHOTS)
}

pub fn run_bernstein_vazirani_black_box_with_shots(hidden: &str, shots: u32) -> Result<Value, String> {
    if shots == 0 {
        return Err("shots must be >= 1".to_string());
    }

    let mut oracle = OpaqueBvOracle::new(hidden)?;
    let query_len = oracle.query_len();
    let ancilla = query_len;
    let total_qubits = query_len
        .checked_add(1)
        .ok_or_else(|| "bv register size overflow".to_string())?;
    let mut register = QuantumRegister::new(total_qubits as u8)?;

    let _ = register.apply_gate(GateSpec {
        gate_id: "bv_prepare_ancilla_x".to_string(),
        kind: GateKind::PauliX,
        targets: vec![ancilla],
        angle_radians: Some(std::f64::consts::PI),
    })?;
    let _ = register.apply_gate(GateSpec {
        gate_id: "bv_prepare_ancilla_h".to_string(),
        kind: GateKind::Hadamard,
        targets: vec![ancilla],
        angle_radians: Some(std::f64::consts::FRAC_PI_2),
    })?;

    for q in 0..query_len {
        let _ = register.apply_gate(GateSpec {
            gate_id: format!("bv_pre_h_q{}", q),
            kind: GateKind::Hadamard,
            targets: vec![q],
            angle_radians: Some(std::f64::consts::FRAC_PI_2),
        })?;
    }

    let oracle_internal_gate_count = oracle.apply_to_register(&mut register)?;
    let oracle_call_count = oracle.oracle_call_count();
    let execution_mode = oracle.execution_mode();
    let execution_mode_label = match execution_mode {
        BvExecutionMode::Structural => "structural",
        BvExecutionMode::BlackBox => "black_box",
    };

    for q in 0..query_len {
        let _ = register.apply_gate(GateSpec {
            gate_id: format!("bv_post_h_q{}", q),
            kind: GateKind::Hadamard,
            targets: vec![q],
            angle_radians: Some(std::f64::consts::FRAC_PI_2),
        })?;
    }

    // Strict black-box boundary: recovery is measurement-only from post-oracle state.
    let run_seed = splitmix64(register.tick ^ (query_len as u64) ^ 0x435349465F42565F);
    let (recovered_hidden_string, shot_measurements) = recover_hidden_from_black_box_measurements(
        &register,
        query_len,
        shots,
        run_seed,
    )?;
    let deterministic_match = recovered_hidden_string == hidden;
    let reality_calibration = build_reality_calibration_summary(
        &shot_measurements,
        REALITY_CALIBRATION_TARGET_CONFIDENCE,
    );

    let rwif_event_envelopes = register
        .rwif_trace
        .iter()
        .map(RwifGateTrace::to_rwif_event_envelope)
        .map(|mut event| {
            if let Value::Object(ref mut map) = event {
                map.insert("bv_execution_mode".to_string(), json!("black_box"));
                map.insert("bv_oracle_call_count".to_string(), json!(oracle_call_count));
                map.insert(
                    "bv_oracle_internal_gate_count".to_string(),
                    json!(oracle_internal_gate_count),
                );
                map.insert(
                    "bv_recovered_hidden_string".to_string(),
                    json!(recovered_hidden_string.clone()),
                );
            }
            event
        })
        .collect::<Vec<_>>();

    let rwif_edge_envelopes = register
        .rwif_trace
        .iter()
        .map(RwifGateTrace::to_rwif_edge_envelope)
        .map(|mut edge| {
            if let Value::Object(ref mut map) = edge {
                map.insert("bv_execution_mode".to_string(), json!("black_box"));
                map.insert("bv_oracle_call_count".to_string(), json!(oracle_call_count));
                map.insert(
                    "bv_oracle_internal_gate_count".to_string(),
                    json!(oracle_internal_gate_count),
                );
                map.insert(
                    "bv_recovered_hidden_string".to_string(),
                    json!(recovered_hidden_string.clone()),
                );
            }
            edge
        })
        .collect::<Vec<_>>();

    let mut payload = register.interface_contract();
    payload["execution_mode"] = json!(execution_mode_label);
    payload["algorithm"] = json!("bernstein_vazirani");
    payload["measurement_only_recovery"] = json!(true);
    payload["hidden_string"] = json!(hidden);
    payload["recovered_hidden_string"] = json!(recovered_hidden_string);
    payload["deterministic_match"] = json!(deterministic_match);
    payload["oracle_call_count"] = json!(oracle_call_count);
    payload["oracle_execution_mode"] = json!(execution_mode_label);
    payload["oracle_internal_gate_count"] = json!(oracle_internal_gate_count);
    payload["oracle_internal_gate_count_total"] = json!(oracle.applied_gate_count());
    payload["measurement_shots"] = json!(shots);
    payload["measurement_sampler"] = json!("seeded_xorshift64star_v1");
    payload["measurement_seed"] = json!(run_seed);
    payload["shot_measurements"] = serde_json::to_value(&shot_measurements)
        .unwrap_or_else(|_| Value::Array(vec![]));
    payload["reality_calibration"] = serde_json::to_value(&reality_calibration)
        .unwrap_or_else(|_| json!({"error": "serialization_failed"}));
    payload["rwif_event_envelopes"] = Value::Array(rwif_event_envelopes);
    payload["rwif_edge_envelopes"] = Value::Array(rwif_edge_envelopes);
    payload["example_measurement"] = serde_json::to_value(register.measure_qubit(0)?)
        .unwrap_or_else(|_| json!({"error": "serialization_failed"}));

    let stable_hash = stable_sha256_hex(&payload)?;
    payload["stable_envelope_sha256"] = json!(stable_hash);
    Ok(payload)
}

pub fn scaffold_report(n_qubits: u8) -> Result<Value, String> {
    let mut register = QuantumRegister::new(n_qubits)?;
    let _ = register.apply_gate(GateSpec {
        gate_id: "bootstrap_h".to_string(),
        kind: GateKind::Hadamard,
        targets: vec![0],
        angle_radians: Some(std::f64::consts::PI / 2.0),
    })?;
    let measurement = register.measure_qubit(0)?;

    let mut payload = register.interface_contract();
    payload["example_measurement"] = serde_json::to_value(measurement)
        .unwrap_or_else(|_| json!({"error": "serialization_failed"}));
    payload["last_rwif_gate_trace"] = serde_json::to_value(register.rwif_trace.last().cloned())
        .unwrap_or_else(|_| Value::Null);
    payload["last_rwif_event_envelope"] = register
        .rwif_trace
        .last()
        .map(RwifGateTrace::to_rwif_event_envelope)
        .unwrap_or(Value::Null);
    payload["last_rwif_edge_envelope"] = register
        .rwif_trace
        .last()
        .map(RwifGateTrace::to_rwif_edge_envelope)
        .unwrap_or(Value::Null);
    let stable_hash = stable_sha256_hex(&payload)?;
    payload["stable_envelope_sha256"] = json!(stable_hash);
    Ok(payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn unified_register_initializes_interface_state() {
        let register = QuantumRegister::new(3).expect("register should initialize");
        assert_eq!(register.n_qubits, 3);
        assert_eq!(register.state.dimension, 64);
        assert_eq!(register.state.components[0], 1.0);
        assert!(register.state.components[1..].iter().all(|v| *v == 0.0));
        assert!((register.state.norm() - 1.0).abs() < 1e-12);
        assert_eq!(register.tick, 0);
        assert_eq!(register.trace.len(), 0);
        assert_eq!(register.rwif_trace.len(), 0);
    }

    #[test]
    fn apply_gate_records_trace_event() {
        let mut register = QuantumRegister::new(2).expect("register should initialize");
        let event = register
            .apply_gate(GateSpec {
                gate_id: "g1".to_string(),
                kind: GateKind::Cnot,
                targets: vec![0, 1],
                angle_radians: None,
            })
            .expect("gate should apply");

        assert_eq!(event.tick, 1);
        assert_eq!(register.trace.len(), 1);
        assert_eq!(register.rwif_trace.len(), 1);
        assert!(event.torsion_scalar > 0.0);
        assert!((0.0..=1.0).contains(&event.phase_alignment_index));
        assert!(register.global_torsion > 0.0);
    }

    #[test]
    fn measurement_is_deterministic_for_same_state() {
        let mut register = QuantumRegister::new(1).expect("register should initialize");
        let _ = register
            .apply_gate(GateSpec {
                gate_id: "g1".to_string(),
                kind: GateKind::Phase,
                targets: vec![0],
                angle_radians: Some(0.2),
            })
            .expect("gate should apply");

        let m1 = register.measure_qubit(0).expect("measurement should succeed");
        let m2 = register.measure_qubit(0).expect("measurement should succeed");
        assert_eq!(m1, m2);
        assert_eq!(m1.tie_break_rule, "lowest_basis_index");
        assert_eq!(m1.projection_basis, "computational_z");
        assert!((0.0..=1.0).contains(&m1.geometric_certainty));
        assert!((0.0..=1.0).contains(&m1.geometric_weight));
    }

    #[test]
    fn apply_gate_with_noise_is_seed_deterministic() {
        let model = ErrorModel {
            torsion_noise_factor: 0.2,
            seed: 42,
        };

        let gate = GateSpec {
            gate_id: "noisy_phase".to_string(),
            kind: GateKind::Phase,
            targets: vec![0],
            angle_radians: Some(std::f64::consts::FRAC_PI_2),
        };

        let mut a = QuantumRegister::new(1).expect("register should initialize");
        let mut b = QuantumRegister::new(1).expect("register should initialize");
        let _ = a
            .apply_gate_with_noise(gate.clone(), &model)
            .expect("noisy gate should apply");
        let _ = b
            .apply_gate_with_noise(gate, &model)
            .expect("noisy gate should apply");

        assert_eq!(a.trace.last(), b.trace.last());
        assert_eq!(a.rwif_trace.last(), b.rwif_trace.last());
    }

    #[test]
    fn bell_state_prep_activates_entanglement_coupling() {
        let mut register = QuantumRegister::new(2).expect("register should initialize");
        let events = prepare_bell_state(&mut register, 0, 1, None)
            .expect("bell preparation should succeed");
        assert_eq!(events.len(), 2);

        let last = register
            .rwif_trace
            .last()
            .expect("rwif trace should contain cnot event");
        assert!(last.entanglement_coupling_active);
        assert!(last.coupling_trivector_amplitude_after > 0.0);
    }

    #[test]
    fn bell_state_report_emits_correlation_fields() {
        let report = bell_state_report(None).expect("bell report should succeed");
        assert_eq!(report.get("algorithm"), Some(&json!("bell_state_preparation")));
        assert_eq!(report.get("event_count"), Some(&json!(2)));
        assert_eq!(report.get("entanglement_coupling_active"), Some(&json!(true)));

        let coupling_after = report
            .get("coupling_trivector_amplitude_after")
            .and_then(Value::as_f64)
            .expect("coupling_trivector_amplitude_after should be f64");
        assert!(coupling_after > 0.0);

        let score = report
            .get("correlation")
            .and_then(Value::as_object)
            .and_then(|corr| corr.get("score"))
            .and_then(Value::as_f64)
            .expect("correlation.score should be f64");
        assert!((0.0..=1.0).contains(&score));
    }

    #[test]
    fn bell_state_sweep_export_json_and_csv_are_compact_and_consistent() {
        let noise_factors = vec![0.0, 0.2];
        let seeds = vec![42u64, 777u64];

        let json_export = bell_state_sweep_export("json", &noise_factors, &seeds)
            .expect("json export should succeed");
        let csv_export = bell_state_sweep_export("csv", &noise_factors, &seeds)
            .expect("csv export should succeed");

        let json_rows: Vec<Value> =
            serde_json::from_str(&json_export).expect("json export should parse");
        assert_eq!(json_rows.len(), noise_factors.len() * seeds.len());

        let csv_lines = csv_export.lines().collect::<Vec<_>>();
        assert_eq!(
            csv_lines.first().copied(),
            Some("noise_factor,seed,correlation_score,coupling_trivector_amplitude_after,entanglement_coupling_active,stable_envelope_sha256")
        );
        assert_eq!(csv_lines.len(), json_rows.len() + 1);
    }

    #[test]
    fn dirac_mode_sweep_transition_tracks_density_threshold() {
        let densities = vec![0.02, 0.2];
        let seeds = vec![42u64];
        let rows = dirac_mode_sweep_points(&densities, &seeds, 6, "uniform-random")
            .expect("dirac sweep should succeed");
        assert_eq!(rows.len(), 2);
        assert!(rows[0].observed_density < rows[0].density_threshold);
        assert!(!rows[0].dense_fallback_active);
        assert!(rows[1].observed_density >= rows[1].density_threshold);
        assert!(rows[1].dense_fallback_active);
    }

    #[test]
    fn dirac_mode_sweep_is_replay_stable_for_same_inputs() {
        let densities = vec![0.08, 0.12, 0.2];
        let seeds = vec![42u64, 777u64];

        let run_a = dirac_mode_sweep_points(&densities, &seeds, 6, "uniform-random")
            .expect("first sweep run should succeed");
        let run_b = dirac_mode_sweep_points(&densities, &seeds, 6, "uniform-random")
            .expect("second sweep run should succeed");

        assert_eq!(run_a, run_b);
        assert!(run_a.iter().all(|row| !row.stable_envelope_sha256.is_empty()));
    }

    #[test]
    fn dirac_mode_sweep_export_json_and_csv_are_compact_and_consistent() {
        let densities = vec![0.02, 0.12, 0.2];
        let seeds = vec![42u64, 777u64];

        let json_export = dirac_mode_sweep_export("json", &densities, &seeds, 6, "uniform-random", false, false)
            .expect("json export should succeed");
        let csv_export = dirac_mode_sweep_export("csv", &densities, &seeds, 6, "uniform-random", false, false)
            .expect("csv export should succeed");

        let json_rows: Vec<Value> = serde_json::from_str(&json_export)
            .expect("json export should parse");
        assert_eq!(json_rows.len(), densities.len() * seeds.len());
        assert_eq!(json_rows[0].get("state_model"), Some(&json!("uniform-random")));

        let csv_lines = csv_export.lines().collect::<Vec<_>>();
        assert_eq!(
            csv_lines.first().copied(),
            Some("state_model,coupling_density,seed,perturbation_amplitude,perturbation_frequency,n_qubits,state_dimension,support_density,low_grade_blade_concentration,observed_density,perturbed_observed_density,density_threshold,threshold_distance,perturbed_threshold_distance,dense_fallback_active,perturbed_dense_fallback_active,phase_relaxation_steps,torsion_hysteresis,perturbation_volatility_index,stable_envelope_sha256")
        );
        assert_eq!(csv_lines.len(), json_rows.len() + 1);
    }

    #[test]
    fn dirac_mode_sweep_summary_json_wraps_rows_with_report_object() {
        let densities = vec![0.02, 0.12, 0.2];
        let seeds = vec![42u64, 777u64];

        let json_export = dirac_mode_sweep_export("json", &densities, &seeds, 6, "uniform-random", true, false)
            .expect("json export should succeed");
        let report: Value = serde_json::from_str(&json_export)
            .expect("summary json export should parse as object");

        assert_eq!(
            report.get("object"),
            Some(&json!("csif.quantum.dirac_mode.threshold_report"))
        );

        let summary = report
            .get("summary")
            .and_then(Value::as_object)
            .expect("summary should be object");
        assert_eq!(
            summary.get("computational_analog_label"),
            Some(&json!("computational_schwinger_limit_analog"))
        );
        assert_eq!(summary.get("state_model"), Some(&json!("uniform-random")));
        assert_eq!(summary.get("baseline_state_model"), Some(&Value::Null));
        assert_eq!(summary.get("crossing_density_ratio_to_uniform"), Some(&Value::Null));
        assert!(summary
            .get("first_crossing_density")
            .and_then(Value::as_f64)
            .is_some());
        assert!(summary
            .get("per_seed_crossing_spread")
            .and_then(Value::as_f64)
            .is_some());
        assert!(summary
            .get("phase_relaxation_steps_mean")
            .and_then(Value::as_f64)
            .is_some());
        assert!(summary
            .get("volatility_index_mean")
            .and_then(Value::as_f64)
            .is_some());

        let rows = report
            .get("rows")
            .and_then(Value::as_array)
            .expect("rows should be array");
        assert_eq!(rows.len(), densities.len() * seeds.len());
    }

    #[test]
    fn dirac_mode_sweep_summary_csv_emits_summary_header_metadata() {
        let densities = vec![0.02, 0.12, 0.2];
        let seeds = vec![42u64, 777u64];

        let csv_export = dirac_mode_sweep_export("csv", &densities, &seeds, 6, "uniform-random", true, false)
            .expect("csv export should succeed");
        let lines = csv_export.lines().collect::<Vec<_>>();

        assert_eq!(lines.first().copied(), Some("# object=csif.quantum.dirac_mode.threshold_report"));
        let summary_line = lines.get(1).copied().unwrap_or_default();
        assert!(summary_line.starts_with("# summary={"));
        assert!(summary_line.contains("computational_schwinger_limit_analog"));
        assert!(summary_line.contains("uniform-random"));
        assert_eq!(
            lines.get(2).copied(),
            Some("state_model,coupling_density,seed,perturbation_amplitude,perturbation_frequency,n_qubits,state_dimension,support_density,low_grade_blade_concentration,observed_density,perturbed_observed_density,density_threshold,threshold_distance,perturbed_threshold_distance,dense_fallback_active,perturbed_dense_fallback_active,phase_relaxation_steps,torsion_hysteresis,perturbation_volatility_index,stable_envelope_sha256")
        );
    }

    #[test]
    fn dirac_mode_perturbation_sweep_reports_volatility_and_unraveling() {
        let densities = vec![0.12, 0.2, 0.4];
        let seeds = vec![42u64, 777u64];
        let amplitudes = vec![0.0, 0.2, 0.6];

        let json_export = dirac_mode_sweep_export_with_perturbation(
            "json",
            &densities,
            &seeds,
            6,
            "high-grade-bias",
            true,
            true,
            &amplitudes,
            24.0,
        )
        .expect("perturbation summary export should succeed");
        let report: Value = serde_json::from_str(&json_export).expect("json should parse");
        let summary = report
            .get("summary")
            .and_then(Value::as_object)
            .expect("summary should be object");
        assert_eq!(summary.get("perturbation_frequency"), Some(&json!(24.0)));
        assert!(summary
            .get("perturbation_amplitudes")
            .and_then(Value::as_array)
            .map(|arr| arr.len() == 3)
            .unwrap_or(false));
        assert!(summary
            .get("volatility_index_max")
            .and_then(Value::as_f64)
            .map(|v| v >= 0.0)
            .unwrap_or(false));
        assert!(summary.get("catastrophic_unraveling_amplitude").is_some());
    }

    #[test]
    fn dirac_mode_state_models_produce_distinct_stable_hashes() {
        let densities = vec![0.12];
        let seeds = vec![42u64];
        let random_rows = dirac_mode_sweep_points(&densities, &seeds, 6, "uniform-random")
            .expect("uniform sweep should succeed");
        let band_rows = dirac_mode_sweep_points(&densities, &seeds, 6, "contiguous-band")
            .expect("band sweep should succeed");
        assert_ne!(random_rows[0].stable_envelope_sha256, band_rows[0].stable_envelope_sha256);
        assert_ne!(random_rows[0].state_model, band_rows[0].state_model);
    }

    #[test]
    fn low_grade_bias_profile_crosses_earlier_than_uniform_random() {
        let densities = vec![0.08];
        let seeds = vec![42u64];
        let random_rows = dirac_mode_sweep_points(&densities, &seeds, 6, "uniform-random")
            .expect("uniform sweep should succeed");
        let bias_rows = dirac_mode_sweep_points(&densities, &seeds, 6, "low-grade-bias")
            .expect("bias sweep should succeed");
        assert!(bias_rows[0].observed_density > random_rows[0].observed_density);
        assert!(bias_rows[0].low_grade_blade_concentration > random_rows[0].low_grade_blade_concentration);
        assert!(bias_rows[0].dense_fallback_active);
        assert!(!random_rows[0].dense_fallback_active);
    }

    #[test]
    fn high_grade_bias_profile_crosses_later_than_uniform_random() {
        let densities = vec![0.12];
        let seeds = vec![42u64];
        let random_rows = dirac_mode_sweep_points(&densities, &seeds, 6, "uniform-random")
            .expect("uniform sweep should succeed");
        let bias_rows = dirac_mode_sweep_points(&densities, &seeds, 6, "high-grade-bias")
            .expect("bias sweep should succeed");
        assert!(bias_rows[0].observed_density < random_rows[0].observed_density);
        assert!(bias_rows[0].low_grade_blade_concentration < random_rows[0].low_grade_blade_concentration);
        assert!(!bias_rows[0].dense_fallback_active);
        assert!(random_rows[0].dense_fallback_active);
    }

    #[test]
    fn dirac_mode_profile_report_includes_baseline_delta() {
        let densities = vec![0.08, 0.12];
        let seeds = vec![42u64, 777u64];
        let json_export = dirac_mode_sweep_export("json", &densities, &seeds, 6, "low-grade-bias", true, true)
            .expect("json export should succeed");
        let report: Value = serde_json::from_str(&json_export).expect("profile report should parse");
        let summary = report.get("summary").and_then(Value::as_object).expect("summary should be object");
        assert_eq!(summary.get("baseline_state_model"), Some(&json!("uniform-random")));
        assert!(summary.get("baseline_first_crossing_density").and_then(Value::as_f64).is_some());
        assert!(summary.get("first_crossing_density_delta_from_baseline").and_then(Value::as_f64).is_some());
        assert!(summary.get("crossing_density_ratio_to_uniform").and_then(Value::as_f64).is_some());
    }

    #[test]
    fn bv_oracle_constructor_encodes_hidden_string() {
        let oracle = bv_oracle_from_hidden_string("10110").expect("oracle should build");
        assert_eq!(oracle.len(), 3);

        let targets = oracle
            .iter()
            .map(|gate| gate.targets.clone())
            .collect::<Vec<_>>();
        assert!(targets.contains(&vec![0, 5]));
        assert!(targets.contains(&vec![2, 5]));
        assert!(targets.contains(&vec![3, 5]));
    }

    #[test]
    fn black_box_bv_supports_configurable_shots() {
        let out = run_bernstein_vazirani_black_box_with_shots("1011", 1024)
            .expect("bv black-box run should succeed");
        assert_eq!(out.get("measurement_shots"), Some(&json!(1024)));
        let shots = out
            .get("shot_measurements")
            .and_then(Value::as_array)
            .expect("shot_measurements should be array");
        assert_eq!(shots.len(), 4);
        assert!(shots.iter().all(|entry| {
            entry
                .get("shots")
                .and_then(Value::as_u64)
                .map(|v| v == 1024)
                .unwrap_or(false)
        }));
    }

    #[test]
    fn bernstein_vazirani_run_recovers_hidden_string_and_sets_rwif_fields() {
        let out = run_bernstein_vazirani("1011").expect("bv run should succeed");
        assert_eq!(out.get("algorithm"), Some(&json!("bernstein_vazirani")));
        assert_eq!(out.get("execution_mode"), Some(&json!("structural")));
        assert_eq!(out.get("hidden_string"), Some(&json!("1011")));
        assert_eq!(out.get("recovered_hidden_string"), Some(&json!("1011")));
        assert_eq!(out.get("deterministic_match"), Some(&json!(true)));
        assert_eq!(out.get("oracle_call_count"), Some(&json!(1)));
        assert_eq!(out.get("oracle_internal_gate_count"), Some(&json!(3)));

        let events = out
            .get("rwif_event_envelopes")
            .and_then(Value::as_array)
            .expect("rwif_event_envelopes should be array");
        assert!(!events.is_empty());
        for event in events {
            assert_eq!(event.get("bv_oracle_call_count"), Some(&json!(1)));
            assert_eq!(event.get("bv_oracle_internal_gate_count"), Some(&json!(3)));
            assert_eq!(event.get("bv_recovered_hidden_string"), Some(&json!("1011")));
        }

        let edges = out
            .get("rwif_edge_envelopes")
            .and_then(Value::as_array)
            .expect("rwif_edge_envelopes should be array");
        assert!(!edges.is_empty());
        for edge in edges {
            assert_eq!(edge.get("bv_oracle_call_count"), Some(&json!(1)));
            assert_eq!(edge.get("bv_oracle_internal_gate_count"), Some(&json!(3)));
            assert_eq!(edge.get("bv_recovered_hidden_string"), Some(&json!("1011")));
        }
    }

    #[test]
    fn black_box_oracle_tracks_invocations_without_probe_api() {
        let mut oracle = OpaqueBvOracle::new("10110").expect("oracle should build");
        assert_eq!(oracle.execution_mode(), BvExecutionMode::BlackBox);
        assert_eq!(oracle.query_len(), 5);
        assert_eq!(oracle.applied_gate_count(), 0);
        assert_eq!(oracle.oracle_call_count(), 0);

        let mut register = QuantumRegister::new(6).expect("register should initialize");
        let applied = oracle
            .apply_to_register(&mut register)
            .expect("oracle application should succeed");

        assert_eq!(applied, 3);
        assert_eq!(oracle.applied_gate_count(), 3);
        assert_eq!(oracle.oracle_call_count(), 1);
    }

    #[test]
    fn bernstein_vazirani_black_box_run_enforces_opaque_recovery_boundary() {
        let out = run_bernstein_vazirani_black_box("1011").expect("bv run should succeed");
        assert_eq!(out.get("algorithm"), Some(&json!("bernstein_vazirani")));
        assert_eq!(out.get("execution_mode"), Some(&json!("black_box")));
        assert_eq!(out.get("hidden_string"), Some(&json!("1011")));
        assert_eq!(out.get("measurement_only_recovery"), Some(&json!(true)));
        assert_eq!(out.get("oracle_call_count"), Some(&json!(1)));
        assert_eq!(out.get("oracle_internal_gate_count"), Some(&json!(3)));
        assert_eq!(out.get("measurement_shots"), Some(&json!(DEFAULT_BV_BLACK_BOX_SHOTS)));

        let recovered = out
            .get("recovered_hidden_string")
            .and_then(Value::as_str)
            .expect("recovered_hidden_string should be a string");
        assert_eq!(recovered.len(), 4);

        let deterministic_match = out
            .get("deterministic_match")
            .and_then(Value::as_bool)
            .expect("deterministic_match should be bool");
        assert_eq!(deterministic_match, recovered == "1011");

        let shot_measurements = out
            .get("shot_measurements")
            .and_then(Value::as_array)
            .expect("shot_measurements should be an array");
        assert_eq!(shot_measurements.len(), 4);

        let calibration = out
            .get("reality_calibration")
            .and_then(Value::as_object)
            .expect("reality_calibration should be an object");
        assert_eq!(
            calibration.get("confidence_model"),
            Some(&json!("hoeffding_majority_bound_v1"))
        );
        assert_eq!(
            calibration.get("target_confidence_level"),
            Some(&json!(REALITY_CALIBRATION_TARGET_CONFIDENCE))
        );
        assert!(
            calibration
                .get("whole_string_confidence")
                .and_then(Value::as_f64)
                .is_some()
        );
        assert!(
            calibration
                .get("minimum_shots_for_target_confidence")
                .is_some()
        );
        let per_bit = calibration
            .get("per_bit")
            .and_then(Value::as_array)
            .expect("reality calibration per_bit should be array");
        assert_eq!(per_bit.len(), 4);

        let events = out
            .get("rwif_event_envelopes")
            .and_then(Value::as_array)
            .expect("rwif_event_envelopes should be array");
        assert!(!events.is_empty());
        for event in events {
            assert_eq!(event.get("bv_execution_mode"), Some(&json!("black_box")));
            assert_eq!(event.get("bv_oracle_call_count"), Some(&json!(1)));
            assert_eq!(event.get("bv_oracle_internal_gate_count"), Some(&json!(3)));
            assert_eq!(event.get("bv_recovered_hidden_string"), Some(&json!(recovered)));
        }

        let edges = out
            .get("rwif_edge_envelopes")
            .and_then(Value::as_array)
            .expect("rwif_edge_envelopes should be array");
        assert!(!edges.is_empty());
        for edge in edges {
            assert_eq!(edge.get("bv_execution_mode"), Some(&json!("black_box")));
            assert_eq!(edge.get("bv_oracle_call_count"), Some(&json!(1)));
            assert_eq!(edge.get("bv_oracle_internal_gate_count"), Some(&json!(3)));
            assert_eq!(edge.get("bv_recovered_hidden_string"), Some(&json!(recovered)));
        }
    }

    #[test]
    fn bernstein_vazirani_modes_remain_deterministic_across_runs() {
        let structural_a = run_bernstein_vazirani("1011").expect("structural run should succeed");
        let structural_b = run_bernstein_vazirani("1011").expect("structural run should succeed");
        let black_box_a = run_bernstein_vazirani_black_box("1011").expect("black box run should succeed");
        let black_box_b = run_bernstein_vazirani_black_box("1011").expect("black box run should succeed");

        assert_eq!(structural_a.get("stable_envelope_sha256"), structural_b.get("stable_envelope_sha256"));
        assert_eq!(black_box_a.get("stable_envelope_sha256"), black_box_b.get("stable_envelope_sha256"));
        assert_eq!(structural_a.get("recovered_hidden_string"), Some(&json!("1011")));
        assert_eq!(black_box_a.get("recovered_hidden_string"), black_box_b.get("recovered_hidden_string"));
        assert_eq!(structural_a.get("deterministic_match"), Some(&json!(true)));
        assert_eq!(black_box_a.get("deterministic_match"), black_box_b.get("deterministic_match"));
        assert_eq!(structural_a.get("execution_mode"), Some(&json!("structural")));
        assert_eq!(black_box_a.get("execution_mode"), Some(&json!("black_box")));
        assert_eq!(structural_a.get("oracle_call_count"), Some(&json!(1)));
        assert_eq!(black_box_a.get("oracle_call_count"), Some(&json!(1)));
        assert_eq!(structural_a.get("oracle_internal_gate_count"), Some(&json!(3)));
        assert_eq!(black_box_a.get("oracle_internal_gate_count"), Some(&json!(3)));
        assert_eq!(black_box_a.get("measurement_only_recovery"), Some(&json!(true)));
        assert_eq!(black_box_a.get("reality_calibration"), black_box_b.get("reality_calibration"));
    }

    #[test]
    fn gate_to_rotor_is_clifford_ready_contract() {
        let gate = GateSpec {
            gate_id: "g_h".to_string(),
            kind: GateKind::Hadamard,
            targets: vec![0],
            angle_radians: None,
        };
        let rotor = gate.to_rotor();
        assert_eq!(rotor.targets, vec![0]);
        assert_eq!(rotor.plane_label, "hadamard_reflection_q0");
        assert!(rotor.angle_radians > 0.0);
        assert_eq!(rotor.blade_mask, Some(0b11));
        assert_eq!(rotor.blade_label, Some("γ0∧γ1".to_string()));
        assert_eq!(rotor.blade_grade, Some(2));
        assert_eq!(rotor.grade_classification, Some("bivector".to_string()));
        assert_eq!(rotor.coupling_blade_mask, None);
        assert_eq!(rotor.coupling_manifold, None);
        assert_eq!(rotor.coupling_blade_grade, None);
        assert_eq!(rotor.coupling_grade_classification, None);
    }

    #[test]
    fn cnot_rotor_emits_target_aware_coupling_manifold_tag() {
        let gate = GateSpec {
            gate_id: "g_cnot".to_string(),
            kind: GateKind::Cnot,
            targets: vec![0, 1],
            angle_radians: None,
        };

        let rotor = gate.to_rotor();
        assert_eq!(rotor.blade_label, Some("γ2∧γ3".to_string()));
        assert_eq!(rotor.blade_grade, Some(2));
        assert_eq!(rotor.grade_classification, Some("bivector".to_string()));
        assert_eq!(rotor.coupling_manifold, Some("γ0∧γ1∧γ2".to_string()));
        assert_eq!(rotor.coupling_blade_grade, Some(3));
        assert_eq!(
            rotor.coupling_grade_classification,
            Some("trivector".to_string())
        );
    }

    #[test]
    fn blade_mask_semantics_render_clifford_notation() {
        assert_eq!(blade_mask_to_clifford_label(1), "γ0");
        assert_eq!(blade_mask_to_clifford_label(3), "γ0∧γ1");
        assert_eq!(blade_mask_to_clifford_label(7), "γ0∧γ1∧γ2");
    }

    #[test]
    fn blade_grade_semantics_classify_structural_subspace() {
        assert_eq!(blade_grade_and_classification(0), (0, "scalar".to_string()));
        assert_eq!(blade_grade_and_classification(1), (1, "vector".to_string()));
        assert_eq!(blade_grade_and_classification(3), (2, "bivector".to_string()));
        assert_eq!(blade_grade_and_classification(7), (3, "trivector".to_string()));
        assert_eq!(blade_grade_and_classification(31), (5, "grade_5".to_string()));
    }

    #[test]
    fn test_continuous_bivector_twist_trajectory() {
        fn run_path_hashes() -> (Vec<f64>, Vec<String>) {
            let mut register = QuantumRegister::new(1).expect("register should initialize");
            let mut torsion_values = Vec::new();
            let mut hashes = Vec::new();

            for step in 0..5 {
                let event = register
                    .apply_gate(GateSpec {
                        gate_id: format!("phase_{}", step),
                        kind: GateKind::Phase,
                        targets: vec![0],
                        angle_radians: Some(std::f64::consts::PI / 10.0),
                    })
                    .expect("phase gate should apply");

                assert!(event.torsion_delta.is_finite());
                assert!(event.torsion_delta > 0.0);

                torsion_values.push(register.global_torsion);

                let mut payload = register.interface_contract();
                payload["last_rwif_gate_trace"] = serde_json::to_value(register.rwif_trace.last().cloned())
                    .expect("rwif trace should serialize");
                payload["last_rwif_event_envelope"] = register
                    .rwif_trace
                    .last()
                    .map(RwifGateTrace::to_rwif_event_envelope)
                    .unwrap_or(Value::Null);

                let h = stable_sha256_hex(&payload).expect("hash should succeed");
                hashes.push(h);
            }

            (torsion_values, hashes)
        }

        let (torsion_values_1, hashes_1) = run_path_hashes();
        let (torsion_values_2, hashes_2) = run_path_hashes();

        assert_eq!(torsion_values_1.len(), 5);
        assert_eq!(hashes_1.len(), 5);

        for w in torsion_values_1.windows(2) {
            assert!(w[1] > w[0], "global_torsion should grow monotonically");
            assert!((w[1] - w[0]) < 1.0, "torsion increments should stay smooth");
        }

        for w in hashes_1.windows(2) {
            assert_ne!(w[0], w[1], "trajectory hash should evolve each step");
        }

        assert_eq!(torsion_values_1, torsion_values_2);
        assert_eq!(hashes_1, hashes_2);
    }

    #[test]
    fn cnot_tracks_bivector_to_trivector_coupling_manifold() {
        let mut register = QuantumRegister::new(2).expect("register should initialize");
        let _ = register
            .apply_gate(GateSpec {
                gate_id: "cnot_coupling_probe".to_string(),
                kind: GateKind::Cnot,
                targets: vec![0, 1],
                angle_radians: Some(std::f64::consts::FRAC_PI_2),
            })
            .expect("cnot gate should apply");

        let trace = register.rwif_trace.last().expect("trace should be present");
        assert_eq!(trace.blade_label, Some("γ2∧γ3".to_string()));
        assert_eq!(trace.coupling_manifold, Some("γ0∧γ1∧γ2".to_string()));
        assert!(trace.local_bivector_amplitude_after > 0.0);
        assert!(trace.coupling_trivector_amplitude_after > 0.0);
        assert!(
            trace.coupling_trivector_amplitude_after > trace.coupling_trivector_amplitude_before
        );
        assert!(trace.coupling_transfer_scalar > 0.0);
        assert!(trace.normalized_coupling_intensity > 0.0);
        assert!(trace.normalized_coupling_intensity <= 1.0);
        assert!((trace.normalized_coupling_intensity - 0.242535625).abs() < 1e-9);
        assert!(trace.geometric_convergence_metric > 0.0);
        assert!(trace.geometric_convergence_metric <= 1.0);
        assert!(trace.entanglement_coupling_active);

        let event = trace.to_rwif_event_envelope();
        assert_eq!(
            event.get("coupling_manifold"),
            Some(&json!("γ0∧γ1∧γ2"))
        );
        assert_eq!(
            event.get("entanglement_coupling_active"),
            Some(&json!(true))
        );
        assert!(
            event
                .get("coupling_transfer_scalar")
                .and_then(Value::as_f64)
                .is_some()
        );
        assert!(
            event
                .get("normalized_coupling_intensity")
                .and_then(Value::as_f64)
                .is_some()
        );
        assert!(
            event
                .get("geometric_convergence_metric")
                .and_then(Value::as_f64)
                .is_some()
        );
    }

    #[test]
    fn test_real_world_optimization_convergence() {
        fn run_optimization_path() -> (Vec<f64>, Vec<String>, String) {
            let mut register = QuantumRegister::new(2).expect("register should initialize");
            let mut intensities = Vec::new();
            let mut hashes = Vec::new();
            let mut final_hash = String::new();

            for step in 0..10 {
                let evolution_fraction = step as f64 / 9.0;
                let event = register
                    .apply_optimization_step(evolution_fraction)
                    .expect("optimization step should apply");
                let trace = register.rwif_trace.last().expect("trace should exist");

                let local_bar = optimization_ascii_bar(1.0 - evolution_fraction, evolution_fraction, 20);
                let coupling_bar = optimization_ascii_bar(evolution_fraction, 1.0 - evolution_fraction, 20);

                println!(
                    "step {:02} frac={:.3} local[{}] coupling[{}] intensity={:.3} convergence={:.3}",
                    step + 1,
                    evolution_fraction,
                    local_bar,
                    coupling_bar,
                    trace.normalized_coupling_intensity,
                    trace.geometric_convergence_metric,
                );

                intensities.push(trace.normalized_coupling_intensity);

                let mut payload = register.interface_contract();
                payload["last_rwif_gate_trace"] = serde_json::to_value(register.rwif_trace.last().cloned())
                    .expect("rwif trace should serialize");
                payload["last_rwif_event_envelope"] = register
                    .rwif_trace
                    .last()
                    .map(RwifGateTrace::to_rwif_event_envelope)
                    .unwrap_or(Value::Null);
                payload["optimization_step_gate_id"] = json!(event.gate_id);

                let hash = stable_sha256_hex(&payload).expect("hash should succeed");
                if step == 9 {
                    final_hash = hash.clone();
                }
                hashes.push(hash);
            }

            (intensities, hashes, final_hash)
        }

        let (intensities_a, hashes_a, final_hash_a) = run_optimization_path();
        let (intensities_b, hashes_b, final_hash_b) = run_optimization_path();

        assert_eq!(intensities_a.len(), 10);
        assert_eq!(hashes_a.len(), 10);
        assert!(intensities_a.first().copied().unwrap_or_default() <= 0.001);
        assert!((intensities_a.last().copied().unwrap_or_default() - 1.0).abs() < 1e-12);

        for w in intensities_a.windows(2) {
            assert!(w[1] >= w[0], "normalized intensity should not decrease");
        }

        assert_eq!(intensities_a, intensities_b);
        assert_eq!(hashes_a, hashes_b);
        assert_eq!(final_hash_a, final_hash_b);
        assert!(!final_hash_a.is_empty());
    }

    #[test]
    fn geometric_product_respects_anticommutation_sign() {
        let dim = 16usize; // 2^(2*2) manifold
        let mut e0 = MultiVectorND::zero(dim);
        let mut e1 = MultiVectorND::zero(dim);
        e0.components[1] = 1.0; // blade mask 0b0001
        e1.components[2] = 1.0; // blade mask 0b0010

        let ab = e0.geometric_product(&e1).expect("product should work");
        let ba = e1.geometric_product(&e0).expect("product should work");
        assert_eq!(ab.components[3], 1.0);
        assert_eq!(ba.components[3], -1.0);
    }

    #[test]
    fn sparse_left_product_matches_dense_product_for_sparse_rotor() {
        let dim = 64usize;

        let mut rotor_like = MultiVectorND::zero(dim);
        rotor_like.components[0] = 0.9238795325;
        rotor_like.components[3] = 0.3535533906;
        rotor_like.components[7] = 0.1464466094;

        let mut state = MultiVectorND::zero(dim);
        state.components[0] = 1.0;
        state.components[3] = 0.2;
        state.components[5] = -0.1;
        state.components[7] = 0.05;

        let dense = rotor_like
            .geometric_product(&state)
            .expect("dense product should work");
        let sparse = rotor_like
            .geometric_product_left_sparse(&state)
            .expect("sparse-left product should work");

        for idx in 0..dim {
            assert!(
                (dense.components[idx] - sparse.components[idx]).abs() < 1e-12,
                "component {} mismatch: dense={} sparse={}",
                idx,
                dense.components[idx],
                sparse.components[idx]
            );
        }
    }

    #[test]
    fn block_size_tuner_prefers_larger_blocks_for_denser_states() {
        let dim = 4096usize;

        let mut sparse_state = MultiVectorND::zero(dim);
        sparse_state.components[0] = 1.0;
        sparse_state.components[31] = 0.25;
        sparse_state.components[511] = -0.125;
        sparse_state.components[1023] = 0.5;

        let mut dense_state = MultiVectorND::zero(dim);
        for i in 0..(dim / 2) {
            dense_state.components[i] = if i % 2 == 0 { 0.25 } else { -0.25 };
        }

        let sparse_block = tuned_block_size_for_state(&sparse_state);
        let dense_block = tuned_block_size_for_state(&dense_state);

        assert!(BLOCK_SIZE_TUNING_CANDIDATES.contains(&sparse_block));
        assert!(BLOCK_SIZE_TUNING_CANDIDATES.contains(&dense_block));
        assert!(dense_block >= sparse_block);
    }

    #[test]
    fn adaptive_dense_fallback_matches_dense_reference_for_high_density_state() {
        let dim = 2048usize;

        let mut state = MultiVectorND::zero(dim);
        for i in 0..(dim / 2) {
            state.components[i] = ((i % 19) as f64 - 9.0) * 0.05;
        }

        assert!(should_use_dense_fallback(&state));

        let gate = GateSpec {
            gate_id: "dense_fallback_phase".to_string(),
            kind: GateKind::Phase,
            targets: vec![0],
            angle_radians: Some(std::f64::consts::FRAC_PI_3),
        };
        let rotor = gate.to_rotor();
        let rotor_mv = rotor_multivector(&rotor, dim).expect("rotor should build");

        let dense_reference = rotor_mv
            .geometric_product(&state)
            .expect("dense product should work");
        let evolved = evolve_gate_state(&state, &rotor, dim).expect("evolution should work");

        for idx in 0..dim {
            assert!(
                (dense_reference.components[idx] - evolved.components[idx]).abs() < 1e-12,
                "component {} mismatch: dense={} evolved={}",
                idx,
                dense_reference.components[idx],
                evolved.components[idx]
            );
        }
    }

    #[test]
    #[ignore = "stress test"]
    fn block_sparse_exactness_stress_matches_dense_reference() {
        let dim = 1usize << 15;

        let mut rotor_like = MultiVectorND::zero(dim);
        rotor_like.components[0] = 0.9238795325;
        rotor_like.components[3] = 0.3535533906;
        rotor_like.components[7] = 0.1464466094;

        let mut state = MultiVectorND::zero(dim);
        state.components[0] = 1.0;
        for i in 0..512usize {
            let idx = ((i * 131) ^ (i << 3)) % dim;
            state.components[idx] = ((i % 17) as f64 - 8.0) * 0.03125;
        }

        let dense = rotor_like
            .geometric_product(&state)
            .expect("dense product should work");

        let sparse_rhs = BlockSparseState::from_dense(&state.components, tuned_block_size_for_state(&state));
        let sparse = rotor_like
            .geometric_product_left_block_sparse(&sparse_rhs)
            .expect("block-sparse product should work")
            .to_dense_multivector();

        for idx in 0..dim {
            assert!(
                (dense.components[idx] - sparse.components[idx]).abs() < 1e-12,
                "component {} mismatch: dense={} sparse={}",
                idx,
                dense.components[idx],
                sparse.components[idx]
            );
        }
    }

    #[test]
    #[ignore = "stress test"]
    fn block_sparse_performance_stress_reports_relative_speed() {
        let dim = 1usize << 16;
        let rounds = 80usize;

        let mut rotor_like = MultiVectorND::zero(dim);
        rotor_like.components[0] = 0.9238795325;
        rotor_like.components[3] = 0.3535533906;
        rotor_like.components[7] = 0.1464466094;

        let mut state = MultiVectorND::zero(dim);
        state.components[0] = 1.0;
        for i in 0..768usize {
            let idx = ((i * 257) ^ (i << 2)) % dim;
            state.components[idx] = ((i % 23) as f64 - 11.0) * 0.015625;
        }

        let sparse_rhs = BlockSparseState::from_dense(&state.components, tuned_block_size_for_state(&state));

        let start_dense = Instant::now();
        let mut dense_acc = 0.0;
        for _ in 0..rounds {
            let out = rotor_like
                .geometric_product(&state)
                .expect("dense product should work");
            dense_acc += out.components[0];
        }
        let dense_elapsed = start_dense.elapsed();

        let start_sparse = Instant::now();
        let mut sparse_acc = 0.0;
        for _ in 0..rounds {
            let out = rotor_like
                .geometric_product_left_block_sparse(&sparse_rhs)
                .expect("block-sparse product should work")
                .to_dense_multivector();
            sparse_acc += out.components[0];
        }
        let sparse_elapsed = start_sparse.elapsed();

        assert!((dense_acc - sparse_acc).abs() < 1e-12);
        println!(
            "block-sparse stress: dense={:?} sparse={:?} ratio={:.2}x",
            dense_elapsed,
            sparse_elapsed,
            dense_elapsed.as_secs_f64() / sparse_elapsed.as_secs_f64().max(1e-9)
        );
    }

    #[test]
    fn no_op_replay_trace_produces_identical_stable_sha256_hash() {
        let trace = vec![
            GateSpec {
                gate_id: "noop_1".to_string(),
                kind: GateKind::NoOp,
                targets: vec![0],
                angle_radians: Some(0.0),
            },
            GateSpec {
                gate_id: "noop_2".to_string(),
                kind: GateKind::NoOp,
                targets: vec![0],
                angle_radians: Some(0.0),
            },
        ];

        let out1 = replay_trace_envelope(1, &trace).expect("replay should succeed");
        let out2 = replay_trace_envelope(1, &trace).expect("replay should succeed");

        let h1 = stable_sha256_hex(&out1).expect("hash should succeed");
        let h2 = stable_sha256_hex(&out2).expect("hash should succeed");
        assert_eq!(h1, h2);
    }

    #[test]
    fn rwif_conformance_bridge_edge_shape_matches_validator_contract() {
        let trace = RwifGateTrace {
            schema_version: "rwif_gate_trace_v1".to_string(),
            tick: 3,
            gate_id: "g3".to_string(),
            rotor_id: "rotor:g3".to_string(),
            plane_label: "phase_rotation_q0".to_string(),
            blade_mask: Some(0b11),
            blade_label: Some("γ0∧γ1".to_string()),
            blade_grade: Some(2),
            grade_classification: Some("bivector".to_string()),
            coupling_blade_mask: Some(0b111),
            coupling_manifold: Some("γ0∧γ1∧γ2".to_string()),
            coupling_blade_grade: Some(3),
            coupling_grade_classification: Some("trivector".to_string()),
            local_bivector_amplitude_before: 0.0,
            local_bivector_amplitude_after: 0.6,
            coupling_trivector_amplitude_before: 0.0,
            coupling_trivector_amplitude_after: 0.15,
            coupling_transfer_scalar: 0.15,
            normalized_coupling_intensity: 0.25,
            geometric_convergence_metric: 0.2,
            entanglement_coupling_active: true,
            torsion_scalar: 0.15,
            phase_alignment_index: 0.75,
        };

        let edge = trace.to_rwif_edge_envelope();
        assert_eq!(edge.get("schema_version"), Some(&json!("RWIF_EDGE_V2")));
        assert_eq!(edge.get("integer_wrap_mode"), Some(&json!("clamp")));
        assert_eq!(edge.get("state_encoding"), Some(&json!("phase_scalar_v1")));

        let events = edge
            .get("phase_trajectory")
            .and_then(Value::as_array)
            .expect("phase_trajectory should be array");
        assert_eq!(events.len(), 1);
        let event = events.first().expect("one event expected");
        assert_eq!(event.get("schema_version"), Some(&json!("RWIF_EVENT_V2")));
        assert_eq!(event.get("state_encoding"), Some(&json!("signed_i8_plus_intent_v2")));
        assert_eq!(event.get("quantization_step"), Some(&json!(1)));
        assert_eq!(event.get("plane_label"), Some(&json!("phase_rotation_q0")));
        assert_eq!(event.get("blade_label"), Some(&json!("γ0∧γ1")));
        assert_eq!(event.get("blade_grade"), Some(&json!(2)));
        assert_eq!(event.get("grade_classification"), Some(&json!("bivector")));
        assert_eq!(event.get("coupling_manifold"), Some(&json!("γ0∧γ1∧γ2")));
        assert_eq!(event.get("coupling_blade_grade"), Some(&json!(3)));
        assert_eq!(
            event.get("coupling_grade_classification"),
            Some(&json!("trivector"))
        );
        assert_eq!(
            event.get("entanglement_coupling_active"),
            Some(&json!(true))
        );
        assert_eq!(event.get("coupling_transfer_scalar"), Some(&json!(0.15)));
        assert_eq!(event.get("normalized_coupling_intensity"), Some(&json!(0.25)));
        assert_eq!(event.get("geometric_convergence_metric"), Some(&json!(0.2)));
        assert!(event.get("monotonic_index").is_some());
    }

    #[test]
    fn rwif_event_envelope_matches_canonical_required_fields() {
        let trace = RwifGateTrace {
            schema_version: "rwif_gate_trace_v1".to_string(),
            tick: 9,
            gate_id: "g9".to_string(),
            rotor_id: "rotor:g9".to_string(),
            plane_label: "phase_rotation_q0".to_string(),
            blade_mask: Some(0b11),
            blade_label: Some("γ0∧γ1".to_string()),
            blade_grade: Some(2),
            grade_classification: Some("bivector".to_string()),
            coupling_blade_mask: None,
            coupling_manifold: None,
            coupling_blade_grade: None,
            coupling_grade_classification: None,
            local_bivector_amplitude_before: 0.2,
            local_bivector_amplitude_after: 0.3,
            coupling_trivector_amplitude_before: 0.0,
            coupling_trivector_amplitude_after: 0.0,
            coupling_transfer_scalar: 0.0,
            normalized_coupling_intensity: 0.0,
            geometric_convergence_metric: 0.0,
            entanglement_coupling_active: false,
            torsion_scalar: 0.12,
            phase_alignment_index: 0.8,
        };
        let event = trace.to_rwif_event_envelope();
        assert_eq!(event.get("schema_version"), Some(&json!("RWIF_EVENT_V2")));
        assert_eq!(event.get("state_encoding"), Some(&json!("signed_i8_plus_intent_v2")));
        assert_eq!(event.get("quantization_step"), Some(&json!(1)));
        assert_eq!(event.get("monotonic_index"), Some(&json!(9)));
        assert_eq!(event.get("plane_label"), Some(&json!("phase_rotation_q0")));
        assert_eq!(event.get("blade_label"), Some(&json!("γ0∧γ1")));
        assert_eq!(event.get("blade_grade"), Some(&json!(2)));
        assert_eq!(event.get("grade_classification"), Some(&json!("bivector")));
        assert!(event.get("torsion_scalar").and_then(Value::as_f64).is_some());
        assert!(event.get("phase_alignment_index").and_then(Value::as_f64).is_some());
    }

    #[test]
    fn scaffold_contract_snapshot_has_stable_keys_and_types() {
        let payload = scaffold_report(3).expect("scaffold report should build");

        let mut keys = payload
            .as_object()
            .expect("payload must be object")
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        keys.sort();

        assert_eq!(
            keys,
            vec![
                "algebra_rank".to_string(),
                "deterministic".to_string(),
                "example_measurement".to_string(),
                "global_torsion".to_string(),
                "last_rwif_edge_envelope".to_string(),
                "last_rwif_event_envelope".to_string(),
                "last_rwif_gate_trace".to_string(),
                "measurement_policy".to_string(),
                "n_qubits".to_string(),
                "object".to_string(),
                "rwif_trace_len".to_string(),
                "schema_version".to_string(),
                "semantic_blade_notation".to_string(),
                "semantic_grade_classification".to_string(),
                "stable_envelope_sha256".to_string(),
                "state_dimension".to_string(),
                "status".to_string(),
                "tick".to_string(),
                "trace_len".to_string(),
                "traits".to_string(),
            ]
        );

        assert!(payload.get("n_qubits").and_then(Value::as_u64).is_some());
        assert!(payload.get("state_dimension").and_then(Value::as_u64).is_some());
        assert!(payload.get("deterministic").and_then(Value::as_bool).is_some());
        assert!(payload.get("traits").and_then(Value::as_array).is_some());
        assert_eq!(
            payload.get("semantic_blade_notation"),
            Some(&json!("clifford_gamma_wedge_v1"))
        );
        assert_eq!(
            payload.get("semantic_grade_classification"),
            Some(&json!("blade_grade_count_ones_v1"))
        );
        assert!(
            payload
                .get("stable_envelope_sha256")
                .and_then(Value::as_str)
                .is_some()
        );

        let trace = payload
            .get("last_rwif_gate_trace")
            .and_then(Value::as_object)
            .expect("last_rwif_gate_trace should be object");
        assert!(trace.get("plane_label").and_then(Value::as_str).is_some());
        assert!(trace.get("blade_label").and_then(Value::as_str).is_some());
        assert!(trace.get("blade_grade").and_then(Value::as_u64).is_some());
        assert!(
            trace
                .get("grade_classification")
                .and_then(Value::as_str)
                .is_some()
        );
        assert!(
            trace
                .get("normalized_coupling_intensity")
                .and_then(Value::as_f64)
                .is_some()
        );
        assert!(
            trace
                .get("geometric_convergence_metric")
                .and_then(Value::as_f64)
                .is_some()
        );

        let event = payload
            .get("last_rwif_event_envelope")
            .and_then(Value::as_object)
            .expect("last_rwif_event_envelope should be object");
        assert!(event.get("plane_label").and_then(Value::as_str).is_some());
        assert!(event.get("blade_label").and_then(Value::as_str).is_some());
        assert!(event.get("blade_grade").and_then(Value::as_u64).is_some());
        assert!(
            event
                .get("grade_classification")
                .and_then(Value::as_str)
                .is_some()
        );
        assert!(
            event
                .get("normalized_coupling_intensity")
                .and_then(Value::as_f64)
                .is_some()
        );
        assert!(
            event
                .get("geometric_convergence_metric")
                .and_then(Value::as_f64)
                .is_some()
        );

        let measurement = payload
            .get("example_measurement")
            .and_then(Value::as_object)
            .expect("example_measurement should be object");
        assert!(measurement.get("tie_break_rule").and_then(Value::as_str).is_some());
        assert!(measurement.get("projection_basis").and_then(Value::as_str).is_some());
        assert!(measurement.get("geometric_certainty").and_then(Value::as_f64).is_some());
        assert!(measurement.get("geometric_weight").and_then(Value::as_f64).is_some());
    }
}
