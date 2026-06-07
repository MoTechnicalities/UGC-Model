use sha2::{Digest, Sha256};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const RWIF_EVENT_SCHEMA_VERSION: &str = "RWIF_EVENT_V2";
const RWIF_EDGE_SCHEMA_VERSION: &str = "RWIF_EDGE_V2";
const MAX_SCAFFOLD_QUBITS: u8 = 10;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MultiVectorND {
    pub dimension: usize,
    pub components: Vec<f64>,
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

pub trait RegisterGateApplier {
    fn apply_gate(&mut self, gate: GateSpec) -> Result<TorsionEvent, String>;
}

pub trait DeterministicProjector {
    fn measure_qubit(&self, qubit: usize) -> Result<MeasurementOutcome, String>;
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
        let evolved = if matches!(gate.kind, GateKind::NoOp) {
            prev_state.clone()
        } else {
            let rotor_mv = rotor_multivector(&rotor, self.state.dimension)?;
            // Spinor-style state update: left action evolves the state from vacuum.
            rotor_mv.geometric_product(&prev_state)?
        };

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
