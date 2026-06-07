use sha2::{Digest, Sha256};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const RWIF_EVENT_SCHEMA_VERSION: &str = "RWIF_EVENT_V2";
const RWIF_EDGE_SCHEMA_VERSION: &str = "RWIF_EDGE_V2";

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
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RotorSpec {
    pub rotor_id: String,
    pub plane_label: String,
    pub angle_radians: f64,
    pub targets: Vec<usize>,
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
        let (plane_label, fallback_angle) = match &self.kind {
            GateKind::NoOp => ("identity".to_string(), 0.0),
            GateKind::Hadamard => ("hadamard_reflection".to_string(), std::f64::consts::PI),
            GateKind::PauliX => ("x_flip".to_string(), std::f64::consts::PI),
            GateKind::Phase => ("phase_rotation".to_string(), std::f64::consts::FRAC_PI_2),
            GateKind::Cnot => ("controlled_reflection".to_string(), std::f64::consts::FRAC_PI_2),
            GateKind::Custom(name) => (format!("custom:{}", name), 0.1),
        };

        RotorSpec {
            rotor_id: format!("rotor:{}", self.gate_id),
            plane_label,
            angle_radians: self.angle_radians.unwrap_or(fallback_angle),
            targets: self.targets.clone(),
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
        let dimension = usize::from(n_qubits) * 2;
        Ok(Self {
            n_qubits,
            state: MultiVectorND::zero(dimension),
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
            }
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

        let base = match gate.kind {
            GateKind::NoOp => 0.0,
            GateKind::Hadamard => 0.05,
            GateKind::PauliX => 0.03,
            GateKind::Phase => 0.02,
            GateKind::Cnot => 0.08,
            GateKind::Custom(_) => 0.01,
        };
        let angle_component = rotor.angle_radians.abs() * 0.001;
        let torsion_delta = base + angle_component;
        let phase_alignment_index = (1.0 - (rotor.angle_radians.sin()).abs()).clamp(0.0, 1.0);

        self.tick += 1;
        self.global_torsion += torsion_delta;

        if let Some(first) = self.state.components.first_mut() {
            *first += torsion_delta;
        }

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
            torsion_scalar: event.torsion_scalar,
            phase_alignment_index: event.phase_alignment_index,
        };

        self.rwif_trace.push(rwif);
        self.trace.push(event.clone());
        Ok(event)
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
        assert_eq!(register.state.dimension, 6);
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
        assert_eq!(rotor.plane_label, "hadamard_reflection");
        assert!(rotor.angle_radians > 0.0);
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
        assert!(event.get("monotonic_index").is_some());
    }

    #[test]
    fn rwif_event_envelope_matches_canonical_required_fields() {
        let trace = RwifGateTrace {
            schema_version: "rwif_gate_trace_v1".to_string(),
            tick: 9,
            gate_id: "g9".to_string(),
            rotor_id: "rotor:g9".to_string(),
            torsion_scalar: 0.12,
            phase_alignment_index: 0.8,
        };
        let event = trace.to_rwif_event_envelope();
        assert_eq!(event.get("schema_version"), Some(&json!("RWIF_EVENT_V2")));
        assert_eq!(event.get("state_encoding"), Some(&json!("signed_i8_plus_intent_v2")));
        assert_eq!(event.get("quantization_step"), Some(&json!(1)));
        assert_eq!(event.get("monotonic_index"), Some(&json!(9)));
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
        assert!(
            payload
                .get("stable_envelope_sha256")
                .and_then(Value::as_str)
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
