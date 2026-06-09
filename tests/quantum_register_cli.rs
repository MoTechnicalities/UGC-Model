use serde_json::Value;
use std::process::Command;
use std::{env, fs};

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
fn shor_cli_factors_15_in_geometric_scaffold() {
    let bin = env!("CARGO_BIN_EXE_csif_agent_v2_rust");

    let output = Command::new(bin)
        .arg("shor")
        .arg("--factoring-target")
        .arg("15")
        .arg("--base-a")
        .arg("2")
        .output()
        .expect("shor command should run");

    assert!(
        output.status.success(),
        "command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let payload: Value = serde_json::from_slice(&output.stdout).expect("shor command should emit JSON output");
    assert_eq!(
        payload.get("object"),
        Some(&Value::String("csif.quantum.shor.geometric_report".to_string()))
    );
    assert_eq!(payload.get("factoring_target"), Some(&Value::from(15)));

    let factors = payload
        .get("factors")
        .and_then(Value::as_array)
        .expect("factors should be array");
    assert!(factors.iter().any(|v| v == &Value::from(3)));
    assert!(factors.iter().any(|v| v == &Value::from(5)));
}

#[test]
fn shor_cli_rejects_bad_target_value() {
    let bin = env!("CARGO_BIN_EXE_csif_agent_v2_rust");

    let output = Command::new(bin)
        .arg("shor")
        .arg("--factoring-target")
        .arg("2")
        .output()
        .expect("shor command should run");

    assert!(
        !output.status.success(),
        "command unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("invalid --factoring-target value"),
        "stderr should explain bad target"
    );
}

#[test]
fn shor_cli_retries_when_first_base_has_no_nontrivial_factor() {
    let bin = env!("CARGO_BIN_EXE_csif_agent_v2_rust");

    let output = Command::new(bin)
        .arg("shor")
        .arg("--factoring-target")
        .arg("21")
        .arg("--base-a")
        .arg("4")
        .arg("--max-base-retries")
        .arg("3")
        .output()
        .expect("shor command should run");

    assert!(
        output.status.success(),
        "command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let payload: Value = serde_json::from_slice(&output.stdout).expect("shor command should emit JSON output");
    assert_eq!(payload.get("status"), Some(&Value::String("factored".to_string())));
    assert_eq!(payload.get("base_a"), Some(&Value::from(4)));

    let attempts = payload
        .get("attempts")
        .and_then(Value::as_array)
        .expect("attempts should be array");
    assert!(attempts.len() >= 2);
    assert_eq!(attempts[0].get("base_a"), Some(&Value::from(4)));
    assert_eq!(
        attempts[0].get("status"),
        Some(&Value::String("period_found_no_nontrivial_factor".to_string()))
    );
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

#[test]
fn dirac_annihilation_cli_json_emits_report_object() {
    let bin = env!("CARGO_BIN_EXE_csif_agent_v2_rust");

    let output = Command::new(bin)
        .arg("dirac-annihilation")
        .arg("--n-qubits")
        .arg("6")
        .arg("--profiles")
        .arg("uniform-random,low-grade-bias,high-grade-bias,harmonic-stride")
        .arg("--unwinding-steps")
        .arg("64")
        .arg("--flux-coupling-density")
        .arg("0.40")
        .arg("--sweep-export")
        .arg("json")
        .output()
        .expect("dirac-annihilation command should run");

    assert!(
        output.status.success(),
        "command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let report: Value = serde_json::from_slice(&output.stdout).expect("dirac-annihilation json should parse");
    assert_eq!(
        report.get("object"),
        Some(&Value::String("csif.quantum.dirac_annihilation.report".to_string()))
    );
    let rows = report
        .get("annihilation_report")
        .and_then(Value::as_array)
        .expect("annihilation_report should be array");
    assert_eq!(rows.len(), 4);
    let first = rows[0].as_object().expect("row should be object");
    assert!(first.get("unwinding_efficiency_index").and_then(Value::as_f64).is_some());
    assert!(first
        .get("peak_anticrystal_contradiction_count")
        .and_then(Value::as_u64)
        .is_some());
    assert!(first
        .get("impedance_matching_efficiency")
        .and_then(Value::as_f64)
        .is_some());
    assert!(first
        .get("residual_torsion_hysteresis")
        .and_then(Value::as_f64)
        .is_some());
    let conservation = first
        .get("conservation")
        .and_then(Value::as_object)
        .expect("conservation should be object");
    assert!(conservation.get("delta_q_topo").and_then(Value::as_f64).is_some());
    assert!(conservation
        .get("energy_tensor_normalization_coefficient")
        .and_then(Value::as_f64)
        .map(|value| (0.0..=1.0).contains(&value))
        .unwrap_or(false));
    assert!(conservation
        .get("phase_relaxation_gradient")
        .and_then(Value::as_f64)
        .is_some());

    let comparison = report
        .get("profile_comparison")
        .and_then(Value::as_object)
        .expect("profile_comparison should be object");
    let by_unwinding = comparison
        .get("ranking_by_unwinding_efficiency")
        .and_then(Value::as_array)
        .expect("ranking_by_unwinding_efficiency should be array");
    let by_impedance = comparison
        .get("ranking_by_impedance_matching")
        .and_then(Value::as_array)
        .expect("ranking_by_impedance_matching should be array");
    let by_pressure = comparison
        .get("ranking_by_pressure_compliance")
        .and_then(Value::as_array)
        .expect("ranking_by_pressure_compliance should be array");
    let by_combined = comparison
        .get("ranking_by_combined_score")
        .and_then(Value::as_array)
        .expect("ranking_by_combined_score should be array");
    assert_eq!(by_unwinding.len(), rows.len());
    assert_eq!(by_impedance.len(), rows.len());
    assert_eq!(by_pressure.len(), rows.len());
    assert_eq!(by_combined.len(), rows.len());

    for window in by_unwinding.windows(2) {
        let left = window[0]
            .get("unwinding_efficiency_index")
            .and_then(Value::as_f64)
            .expect("unwinding score should exist");
        let right = window[1]
            .get("unwinding_efficiency_index")
            .and_then(Value::as_f64)
            .expect("unwinding score should exist");
        assert!(left >= right);
    }
    for window in by_impedance.windows(2) {
        let left = window[0]
            .get("impedance_matching_efficiency")
            .and_then(Value::as_f64)
            .expect("impedance score should exist");
        let right = window[1]
            .get("impedance_matching_efficiency")
            .and_then(Value::as_f64)
            .expect("impedance score should exist");
        assert!(left >= right);
    }
    for window in by_pressure.windows(2) {
        let left = window[0]
            .get("pressure_compliance_margin")
            .and_then(Value::as_f64)
            .expect("pressure compliance margin should exist");
        let right = window[1]
            .get("pressure_compliance_margin")
            .and_then(Value::as_f64)
            .expect("pressure compliance margin should exist");
        assert!(left >= right);
    }
    for window in by_combined.windows(2) {
        let left = window[0]
            .get("combined_leaderboard_score")
            .and_then(Value::as_f64)
            .expect("combined score should exist");
        let right = window[1]
            .get("combined_leaderboard_score")
            .and_then(Value::as_f64)
            .expect("combined score should exist");
        assert!(left <= right);
    }

    let markdown_table = comparison
        .get("comparison_markdown_table")
        .and_then(Value::as_str)
        .expect("comparison_markdown_table should be string");
    assert!(markdown_table.contains("| state_model | unwinding_efficiency_rank | impedance_matching_rank | pressure_compliance_rank | combined_leaderboard_score |"));

    let rank_by_profile = comparison
        .get("rank_by_profile")
        .and_then(Value::as_object)
        .expect("rank_by_profile should be object");
    let profile_unwinding = rank_by_profile
        .get("unwinding_efficiency")
        .and_then(Value::as_object)
        .expect("unwinding_efficiency rank map should be object");
    let profile_impedance = rank_by_profile
        .get("impedance_matching")
        .and_then(Value::as_object)
        .expect("impedance_matching rank map should be object");
    let profile_pressure = rank_by_profile
        .get("pressure_compliance")
        .and_then(Value::as_object)
        .expect("pressure_compliance rank map should be object");
    let profile_combined = rank_by_profile
        .get("combined_score")
        .and_then(Value::as_object)
        .expect("combined_score rank map should be object");
    assert_eq!(profile_unwinding.len(), rows.len());
    assert_eq!(profile_impedance.len(), rows.len());
    assert_eq!(profile_pressure.len(), rows.len());
    assert_eq!(profile_combined.len(), rows.len());
}

#[test]
fn dirac_annihilation_cli_csv_emits_header_and_rows() {
    let bin = env!("CARGO_BIN_EXE_csif_agent_v2_rust");

    let output = Command::new(bin)
        .arg("dirac-annihilation")
        .arg("--profiles")
        .arg("uniform-random,high-grade-bias")
        .arg("--unwinding-steps")
        .arg("32")
        .arg("--flux-coupling-density")
        .arg("0.40")
        .arg("--sweep-export")
        .arg("csv")
        .output()
        .expect("dirac-annihilation csv command should run");

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
        Some("state_model,n_qubits,unwinding_steps,flux_coupling_density,crossing_density_ratio_to_uniform,frame_transition_step,unwinding_efficiency_index,peak_anticrystal_contradiction_count,residual_torsion_hysteresis,impedance_matching_efficiency,vorticity_dissipation_rate,delta_q_topo,torsion_leak_detected,pressure_equivalence_error,pressure_equivalence_compliant,phase_relaxation_gradient,invariant_violation_count")
    );
    assert_eq!(lines.len(), 3);
}

#[test]
fn dirac_annihilation_cli_output_prefix_writes_json_artifact() {
    let bin = env!("CARGO_BIN_EXE_csif_agent_v2_rust");
    let base = env::temp_dir().join("ugcmodel_dirac_annihilation_test");
    let _ = fs::remove_file(base.with_extension("json"));

    let output = Command::new(bin)
        .arg("dirac-annihilation")
        .arg("--profiles")
        .arg("uniform-random,high-grade-bias")
        .arg("--sweep-export")
        .arg("json")
        .arg("--output-prefix")
        .arg(base.to_string_lossy().to_string())
        .output()
        .expect("dirac-annihilation output-prefix command should run");

    assert!(
        output.status.success(),
        "command failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let artifact = base.with_extension("json");
    assert!(artifact.exists(), "expected artifact {} to exist", artifact.display());
    let raw = fs::read_to_string(&artifact).expect("artifact should be readable");
    assert!(raw.contains("csif.quantum.dirac_annihilation.report"));

    let _ = fs::remove_file(artifact);
}
