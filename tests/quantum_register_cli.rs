use serde_json::Value;
use std::process::Command;

fn run_scaffold(bin: &str, n_qubits: u8) -> Value {
    let output = Command::new(bin)
        .arg("quantum-register-scaffold")
        .arg("--n-qubits")
        .arg(n_qubits.to_string())
        .output()
        .expect("quantum-register-scaffold should run");

    assert!(
        output.status.success(),
        "command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    serde_json::from_slice(&output.stdout).expect("command should emit JSON output")
}

#[test]
fn quantum_register_scaffold_emits_contract_payload() {
    let bin = env!("CARGO_BIN_EXE_csif_agent_v2_rust");

    let output = Command::new(bin)
        .arg("quantum-register-scaffold")
        .arg("--n-qubits")
        .arg("6")
        .output()
        .expect("quantum-register-scaffold should run");

    assert!(
        output.status.success(),
        "command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let payload: Value =
        serde_json::from_slice(&output.stdout).expect("command should emit JSON output");

    assert_eq!(
        payload.get("object"),
        Some(&Value::String("csif.quantum.register.scaffold".to_string()))
    );
    assert_eq!(payload.get("status"), Some(&Value::String("scaffold_only".to_string())));
    assert_eq!(payload.get("n_qubits"), Some(&Value::from(6)));
    assert!(payload.get("traits").and_then(Value::as_array).is_some());

    let measurement = payload
        .get("example_measurement")
        .and_then(Value::as_object)
        .expect("example_measurement should be object");
    assert!(measurement
        .get("tie_break_rule")
        .and_then(Value::as_str)
        .is_some());
    assert!(measurement
        .get("projection_basis")
        .and_then(Value::as_str)
        .is_some());
    assert!(measurement
        .get("geometric_certainty")
        .and_then(Value::as_f64)
        .is_some());
    assert!(measurement
        .get("geometric_weight")
        .and_then(Value::as_f64)
        .is_some());

    let rwif_event = payload
        .get("last_rwif_event_envelope")
        .and_then(Value::as_object)
        .expect("last_rwif_event_envelope should be object");
    assert_eq!(
        rwif_event.get("schema_version"),
        Some(&Value::String("RWIF_EVENT_V2".to_string()))
    );
    assert!(rwif_event.get("state_encoding").and_then(Value::as_str).is_some());
    assert!(rwif_event.get("quantization_step").and_then(Value::as_u64).is_some());
    assert!(rwif_event.get("monotonic_index").is_some());
    assert!(rwif_event.get("torsion_scalar").and_then(Value::as_f64).is_some());
    assert!(
        rwif_event
            .get("phase_alignment_index")
            .and_then(Value::as_f64)
            .is_some()
    );
}

#[test]
fn quantum_register_scaffold_rejects_unknown_option() {
    let bin = env!("CARGO_BIN_EXE_csif_agent_v2_rust");

    let output = Command::new(bin)
        .arg("quantum-register-scaffold")
        .arg("--unknown")
        .output()
        .expect("quantum-register-scaffold should run");

    assert!(
        !output.status.success(),
        "command unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("unknown quantum-register-scaffold option"),
        "stderr should explain unknown option"
    );
}

#[test]
fn quantum_register_scaffold_stable_hash_is_identical_across_processes() {
    let bin = env!("CARGO_BIN_EXE_csif_agent_v2_rust");

    let run1 = run_scaffold(bin, 6);
    let run2 = run_scaffold(bin, 6);

    let h1 = run1
        .get("stable_envelope_sha256")
        .and_then(Value::as_str)
        .expect("stable_envelope_sha256 must be present as a string");
    let h2 = run2
        .get("stable_envelope_sha256")
        .and_then(Value::as_str)
        .expect("stable_envelope_sha256 must be present as a string");

    assert_eq!(h1, h2, "stable_envelope_sha256 should match across runs");
}
