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
    assert_eq!(rwif_event.get("blade_grade"), Some(&Value::from(2)));
    assert_eq!(
        rwif_event.get("grade_classification"),
        Some(&Value::String("bivector".to_string()))
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

#[test]
fn bv_cli_structural_mode_recovers_hidden_string() {
    let bin = env!("CARGO_BIN_EXE_csif_agent_v2_rust");

    let output = Command::new(bin)
        .arg("bv")
        .arg("--hidden")
        .arg("1011")
        .arg("--mode")
        .arg("structural")
        .output()
        .expect("bv command should run");

    assert!(
        output.status.success(),
        "command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let payload: Value = serde_json::from_slice(&output.stdout).expect("bv command should emit JSON output");
    assert_eq!(payload.get("algorithm"), Some(&Value::String("bernstein_vazirani".to_string())));
    assert_eq!(payload.get("execution_mode"), Some(&Value::String("structural".to_string())));
    assert_eq!(payload.get("hidden_string"), Some(&Value::String("1011".to_string())));
    assert_eq!(payload.get("recovered_hidden_string"), Some(&Value::String("1011".to_string())));
}

#[test]
fn bv_cli_black_box_mode_accepts_shot_override() {
    let bin = env!("CARGO_BIN_EXE_csif_agent_v2_rust");

    let output = Command::new(bin)
        .arg("bv")
        .arg("--hidden")
        .arg("1011")
        .arg("--mode")
        .arg("black-box")
        .arg("--shots")
        .arg("1024")
        .output()
        .expect("bv command should run");

    assert!(
        output.status.success(),
        "command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let payload: Value = serde_json::from_slice(&output.stdout).expect("bv command should emit JSON output");
    assert_eq!(payload.get("execution_mode"), Some(&Value::String("black_box".to_string())));
    assert_eq!(payload.get("measurement_shots"), Some(&Value::from(1024)));
}

#[test]
fn bell_state_cli_emits_correlation_payload() {
    let bin = env!("CARGO_BIN_EXE_csif_agent_v2_rust");

    let output = Command::new(bin)
        .arg("bell-state")
        .output()
        .expect("bell-state command should run");

    assert!(
        output.status.success(),
        "command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let payload: Value = serde_json::from_slice(&output.stdout).expect("command should emit JSON output");
    assert_eq!(
        payload.get("algorithm"),
        Some(&Value::String("bell_state_preparation".to_string()))
    );
    assert!(payload.get("correlation").and_then(Value::as_object).is_some());
    assert!(payload
        .get("entanglement_coupling_active")
        .and_then(Value::as_bool)
        .is_some());
    assert_eq!(payload.get("error_model"), Some(&Value::Null));
}

#[test]
fn bell_state_cli_accepts_noise_parameters() {
    let bin = env!("CARGO_BIN_EXE_csif_agent_v2_rust");

    let output = Command::new(bin)
        .arg("bell-state")
        .arg("--noise-factor")
        .arg("0.2")
        .arg("--noise-seed")
        .arg("777")
        .output()
        .expect("bell-state command should run");

    assert!(
        output.status.success(),
        "command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let payload: Value = serde_json::from_slice(&output.stdout).expect("command should emit JSON output");
    let model = payload
        .get("error_model")
        .and_then(Value::as_object)
        .expect("error_model should be object when noise parameters are provided");
    assert_eq!(model.get("torsion_noise_factor"), Some(&Value::from(0.2)));
    assert_eq!(model.get("seed"), Some(&Value::from(777u64)));
}

#[test]
fn bell_state_cli_sweep_export_json_emits_compact_rows() {
    let bin = env!("CARGO_BIN_EXE_csif_agent_v2_rust");

    let output = Command::new(bin)
        .arg("bell-state")
        .arg("--sweep-export")
        .arg("json")
        .arg("--sweep-noise-factors")
        .arg("0.0,0.2")
        .arg("--sweep-seeds")
        .arg("42,777")
        .output()
        .expect("bell-state sweep command should run");

    assert!(
        output.status.success(),
        "command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let rows: Vec<Value> = serde_json::from_slice(&output.stdout).expect("sweep json should parse");
    assert_eq!(rows.len(), 4);
    assert_eq!(rows[0].get("noise_factor"), Some(&Value::from(0.0)));
    assert!(rows[0].get("stable_envelope_sha256").and_then(Value::as_str).is_some());
}

#[test]
fn bell_state_cli_sweep_export_csv_emits_header_and_rows() {
    let bin = env!("CARGO_BIN_EXE_csif_agent_v2_rust");

    let output = Command::new(bin)
        .arg("bell-state")
        .arg("--sweep-export")
        .arg("csv")
        .arg("--sweep-noise-factors")
        .arg("0.0,0.2")
        .arg("--sweep-seeds")
        .arg("42")
        .output()
        .expect("bell-state sweep command should run");

    assert!(
        output.status.success(),
        "command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let csv = String::from_utf8(output.stdout).expect("csv should be valid utf-8");
    let lines = csv
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    assert_eq!(
        lines.first().copied(),
        Some("noise_factor,seed,correlation_score,coupling_trivector_amplitude_after,entanglement_coupling_active,stable_envelope_sha256")
    );
    assert_eq!(lines.len(), 3);
}

#[test]
fn dirac_mode_cli_sweep_export_json_emits_compact_rows() {
    let bin = env!("CARGO_BIN_EXE_csif_agent_v2_rust");

    let output = Command::new(bin)
        .arg("dirac-mode")
        .arg("--n-qubits")
        .arg("6")
        .arg("--state-model")
        .arg("low-grade-bias")
        .arg("--sweep-export")
        .arg("json")
        .arg("--sweep-coupling-densities")
        .arg("0.02,0.12,0.2")
        .arg("--sweep-seeds")
        .arg("42,777")
        .output()
        .expect("dirac-mode sweep command should run");

    assert!(
        output.status.success(),
        "command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let rows: Vec<Value> = serde_json::from_slice(&output.stdout).expect("dirac sweep json should parse");
    assert_eq!(rows.len(), 6);
    assert_eq!(rows[0].get("state_model"), Some(&Value::String("low-grade-bias".to_string())));
    assert!(rows[0].get("low_grade_blade_concentration").and_then(Value::as_f64).is_some());
    assert!(rows[0].get("dense_fallback_active").and_then(Value::as_bool).is_some());
    assert!(rows[0].get("stable_envelope_sha256").and_then(Value::as_str).is_some());
}

#[test]
fn dirac_mode_cli_sweep_export_csv_emits_header_and_rows() {
    let bin = env!("CARGO_BIN_EXE_csif_agent_v2_rust");

    let output = Command::new(bin)
        .arg("dirac-mode")
        .arg("--n-qubits")
        .arg("6")
        .arg("--rotor-profile")
        .arg("harmonic-stride")
        .arg("--sweep-export")
        .arg("csv")
        .arg("--sweep-coupling-densities")
        .arg("0.02,0.12")
        .arg("--sweep-seeds")
        .arg("42")
        .output()
        .expect("dirac-mode sweep command should run");

    assert!(
        output.status.success(),
        "command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let csv = String::from_utf8(output.stdout).expect("csv should be valid utf-8");
    let lines = csv
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    assert_eq!(
        lines.first().copied(),
        Some("state_model,coupling_density,seed,perturbation_amplitude,perturbation_frequency,n_qubits,state_dimension,support_density,low_grade_blade_concentration,observed_density,perturbed_observed_density,density_threshold,threshold_distance,perturbed_threshold_distance,dense_fallback_active,perturbed_dense_fallback_active,phase_relaxation_steps,torsion_hysteresis,perturbation_volatility_index,stable_envelope_sha256")
    );
    assert_eq!(lines.len(), 3);
}

#[test]
fn dirac_mode_cli_summary_json_emits_report_object() {
    let bin = env!("CARGO_BIN_EXE_csif_agent_v2_rust");

    let output = Command::new(bin)
        .arg("dirac-mode")
        .arg("--n-qubits")
        .arg("6")
        .arg("--state-model")
        .arg("low-grade-bias")
        .arg("--summary")
        .arg("--profile-report")
        .arg("--sweep-export")
        .arg("json")
        .arg("--sweep-coupling-densities")
        .arg("0.02,0.12,0.2")
        .arg("--sweep-seeds")
        .arg("42,777")
        .output()
        .expect("dirac-mode summary command should run");

    assert!(
        output.status.success(),
        "command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let report: Value = serde_json::from_slice(&output.stdout).expect("summary json should parse");
    assert_eq!(
        report.get("object"),
        Some(&Value::String("csif.quantum.dirac_mode.threshold_report".to_string()))
    );
    let summary = report
        .get("summary")
        .and_then(Value::as_object)
        .expect("summary should be object");
    assert_eq!(
        summary.get("state_model"),
        Some(&Value::String("low-grade-bias".to_string()))
    );
    assert_eq!(
        summary.get("baseline_state_model"),
        Some(&Value::String("uniform-random".to_string()))
    );
    assert!(summary
        .get("first_crossing_density_delta_from_baseline")
        .and_then(Value::as_f64)
        .is_some());
    assert!(summary
        .get("crossing_density_ratio_to_uniform")
        .and_then(Value::as_f64)
        .is_some());
    assert!(summary
        .get("first_crossing_density")
        .and_then(Value::as_f64)
        .is_some());
    assert!(summary
        .get("per_seed_crossing_spread")
        .and_then(Value::as_f64)
        .is_some());
    assert!(summary
        .get("volatility_index_mean")
        .and_then(Value::as_f64)
        .is_some());
}

#[test]
fn dirac_mode_cli_summary_csv_emits_summary_header_metadata() {
    let bin = env!("CARGO_BIN_EXE_csif_agent_v2_rust");

    let output = Command::new(bin)
        .arg("dirac-mode")
        .arg("--n-qubits")
        .arg("6")
        .arg("--rotor-profile")
        .arg("harmonic-stride")
        .arg("--summary")
        .arg("--sweep-export")
        .arg("csv")
        .arg("--sweep-coupling-densities")
        .arg("0.02,0.12,0.2")
        .arg("--sweep-seeds")
        .arg("42")
        .output()
        .expect("dirac-mode summary command should run");

    assert!(
        output.status.success(),
        "command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let csv = String::from_utf8(output.stdout).expect("csv should be valid utf-8");
    let lines = csv
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    assert_eq!(lines.first().copied(), Some("# object=csif.quantum.dirac_mode.threshold_report"));
    assert!(lines.get(1).copied().unwrap_or_default().starts_with("# summary={"));
    assert!(lines.get(1).copied().unwrap_or_default().contains("harmonic-stride"));
    assert_eq!(
        lines.get(2).copied(),
        Some("state_model,coupling_density,seed,perturbation_amplitude,perturbation_frequency,n_qubits,state_dimension,support_density,low_grade_blade_concentration,observed_density,perturbed_observed_density,density_threshold,threshold_distance,perturbed_threshold_distance,dense_fallback_active,perturbed_dense_fallback_active,phase_relaxation_steps,torsion_hysteresis,perturbation_volatility_index,stable_envelope_sha256")
    );
}

#[test]
fn dirac_mode_cli_perturbation_summary_emits_volatility_metrics() {
    let bin = env!("CARGO_BIN_EXE_csif_agent_v2_rust");

    let output = Command::new(bin)
        .arg("dirac-mode")
        .arg("--n-qubits")
        .arg("6")
        .arg("--state-model")
        .arg("high-grade-bias")
        .arg("--summary")
        .arg("--profile-report")
        .arg("--sweep-export")
        .arg("json")
        .arg("--sweep-coupling-densities")
        .arg("0.12,0.20,0.40")
        .arg("--sweep-seeds")
        .arg("42,777")
        .arg("--perturbation-amplitudes")
        .arg("0.0,0.2,0.6")
        .arg("--perturbation-frequency")
        .arg("24")
        .output()
        .expect("dirac-mode perturbation summary should run");

    assert!(
        output.status.success(),
        "command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let report: Value = serde_json::from_slice(&output.stdout).expect("summary json should parse");
    let summary = report
        .get("summary")
        .and_then(Value::as_object)
        .expect("summary should be object");
    assert_eq!(summary.get("perturbation_frequency"), Some(&Value::from(24.0)));
    assert!(summary
        .get("perturbation_amplitudes")
        .and_then(Value::as_array)
        .map(|arr| arr.len() == 3)
        .unwrap_or(false));
    assert!(summary
        .get("phase_relaxation_steps_mean")
        .and_then(Value::as_f64)
        .is_some());
    assert!(summary
        .get("catastrophic_unraveling_amplitude")
        .is_some());
}
