#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuantumRegisterScaffoldArgs {
    pub n_qubits: u8,
    pub pretty: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DistributedWaveHarnessArgs {
    pub epochs: usize,
    pub replay_runs: usize,
    pub pretty: bool,
    pub sweep_export: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BvCommandArgs {
    pub hidden: String,
    pub mode: String,
    pub shots: u32,
    pub pretty: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BellStateCommandArgs {
    pub noise_factor: Option<f64>,
    pub noise_seed: u64,
    pub sweep_export: Option<String>,
    pub sweep_noise_factors: Vec<f64>,
    pub sweep_seeds: Vec<u64>,
    pub pretty: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DiracModeCommandArgs {
    pub n_qubits: u8,
    pub state_model: String,
    pub sweep_export: Option<String>,
    pub sweep_coupling_densities: Vec<f64>,
    pub sweep_seeds: Vec<u64>,
    pub perturbation_amplitudes: Vec<f64>,
    pub perturbation_frequency: f64,
    pub summary: bool,
    pub profile_report: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DiracAnnihilationCommandArgs {
    pub n_qubits: u8,
    pub profiles: Vec<String>,
    pub unwinding_steps: u32,
    pub flux_coupling_density: f64,
    pub sweep_export: Option<String>,
    pub output_prefix: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShorCommandArgs {
    pub factoring_target: u64,
    pub base_a: u64,
    pub max_base_retries: u32,
    pub pretty: bool,
}

fn parse_noise_factor_list_csv(value: &str) -> Result<Vec<f64>, String> {
    let mut values = Vec::<f64>::new();
    for item in value.split(',') {
        let token = item.trim();
        if token.is_empty() {
            continue;
        }
        let parsed = match token.parse::<f64>() {
            Ok(v) => v,
            Err(_) => return Err(format!("invalid --sweep-noise-factors value: {}", value)),
        };
        if !(0.0..=1.0).contains(&parsed) {
            return Err(format!("invalid --sweep-noise-factors value: {}", value));
        }
        values.push(parsed);
    }
    if values.is_empty() {
        return Err(format!("invalid --sweep-noise-factors value: {}", value));
    }
    Ok(values)
}

fn parse_seed_list_csv(value: &str) -> Result<Vec<u64>, String> {
    let mut values = Vec::<u64>::new();
    for item in value.split(',') {
        let token = item.trim();
        if token.is_empty() {
            continue;
        }
        let parsed = match token.parse::<u64>() {
            Ok(v) => v,
            Err(_) => return Err(format!("invalid --sweep-seeds value: {}", value)),
        };
        values.push(parsed);
    }
    if values.is_empty() {
        return Err(format!("invalid --sweep-seeds value: {}", value));
    }
    Ok(values)
}

fn parse_density_list_csv(value: &str) -> Result<Vec<f64>, String> {
    let mut values = Vec::<f64>::new();
    for item in value.split(',') {
        let token = item.trim();
        if token.is_empty() {
            continue;
        }
        let parsed = match token.parse::<f64>() {
            Ok(v) => v,
            Err(_) => return Err(format!("invalid --sweep-coupling-densities value: {}", value)),
        };
        if !(0.0..=1.0).contains(&parsed) {
            return Err(format!("invalid --sweep-coupling-densities value: {}", value));
        }
        values.push(parsed);
    }
    if values.is_empty() {
        return Err(format!("invalid --sweep-coupling-densities value: {}", value));
    }
    Ok(values)
}

fn parse_perturbation_amplitude_list_csv(value: &str) -> Result<Vec<f64>, String> {
    let mut values = Vec::<f64>::new();
    for item in value.split(',') {
        let token = item.trim();
        if token.is_empty() {
            continue;
        }
        let parsed = match token.parse::<f64>() {
            Ok(v) => v,
            Err(_) => return Err(format!("invalid --perturbation-amplitudes value: {}", value)),
        };
        if !(0.0..=1.0).contains(&parsed) {
            return Err(format!("invalid --perturbation-amplitudes value: {}", value));
        }
        values.push(parsed);
    }
    if values.is_empty() {
        return Err(format!("invalid --perturbation-amplitudes value: {}", value));
    }
    Ok(values)
}

fn parse_perturbation_frequency(value: &str) -> Result<f64, String> {
    let parsed = match value.parse::<f64>() {
        Ok(v) => v,
        Err(_) => return Err(format!("invalid --perturbation-frequency value: {}", value)),
    };
    if !(0.0..=1024.0).contains(&parsed) {
        return Err(format!("invalid --perturbation-frequency value: {}", value));
    }
    Ok(parsed)
}

fn parse_dirac_profile_list_csv(value: &str) -> Result<Vec<String>, String> {
    let mut values = Vec::<String>::new();
    for item in value.split(',') {
        let token = item.trim();
        if token.is_empty() {
            continue;
        }
        let normalized = token.to_ascii_lowercase();
        if !super::register::is_supported_dirac_state_model(&normalized) {
            return Err(format!("invalid --profiles value: {}", value));
        }
        if !values.iter().any(|existing| existing == &normalized) {
            values.push(normalized);
        }
    }
    if values.is_empty() {
        return Err(format!("invalid --profiles value: {}", value));
    }
    Ok(values)
}

pub fn parse_quantum_register_scaffold_args(args: &[String]) -> Result<QuantumRegisterScaffoldArgs, String> {
    let mut pretty = false;
    let mut n_qubits = 4u8;
    let mut i = 0usize;

    while i < args.len() {
        match args[i].as_str() {
            "--pretty" => {
                pretty = true;
                i += 1;
            }
            "--n-qubits" => {
                let Some(value) = args.get(i + 1) else {
                    return Err("--n-qubits requires a value".to_string());
                };
                n_qubits = match value.parse::<u8>() {
                    Ok(v) if v >= 1 => v,
                    _ => return Err(format!("invalid --n-qubits value: {}", value)),
                };
                i += 2;
            }
            other => {
                return Err(format!("unknown quantum-register-scaffold option: {}", other));
            }
        }
    }

    Ok(QuantumRegisterScaffoldArgs { n_qubits, pretty })
}

pub fn parse_distributed_wave_harness_args(args: &[String]) -> Result<DistributedWaveHarnessArgs, String> {
    let mut pretty = false;
    let mut epochs = 64usize;
    let mut replay_runs = 5usize;
    let mut sweep_export: Option<String> = None;
    let mut i = 0usize;

    while i < args.len() {
        match args[i].as_str() {
            "--pretty" => {
                pretty = true;
                i += 1;
            }
            "--epochs" => {
                let Some(value) = args.get(i + 1) else {
                    return Err("--epochs requires a value".to_string());
                };
                epochs = match value.parse::<usize>() {
                    Ok(v) if v >= 8 => v,
                    _ => return Err(format!("invalid --epochs value: {}", value)),
                };
                i += 2;
            }
            "--replay-runs" => {
                let Some(value) = args.get(i + 1) else {
                    return Err("--replay-runs requires a value".to_string());
                };
                replay_runs = match value.parse::<usize>() {
                    Ok(v) if v >= 2 => v,
                    _ => return Err(format!("invalid --replay-runs value: {}", value)),
                };
                i += 2;
            }
            "--sweep-export" => {
                let Some(value) = args.get(i + 1) else {
                    return Err("--sweep-export requires a value".to_string());
                };
                let normalized = value.to_ascii_lowercase();
                if normalized != "json" && normalized != "csv" {
                    return Err(format!("invalid --sweep-export value: {}", value));
                }
                sweep_export = Some(normalized);
                i += 2;
            }
            other => {
                return Err(format!("unknown distributed-wave-harness option: {}", other));
            }
        }
    }

    Ok(DistributedWaveHarnessArgs {
        epochs,
        replay_runs,
        pretty,
        sweep_export,
    })
}

pub fn parse_bv_command_args(args: &[String]) -> Result<BvCommandArgs, String> {
    let mut hidden: Option<String> = None;
    let mut mode = "structural".to_string();
    let mut shots: u32 = super::register::DEFAULT_BV_BLACK_BOX_SHOTS;
    let mut pretty = false;
    let mut i = 0usize;

    while i < args.len() {
        match args[i].as_str() {
            "--hidden" => {
                let Some(value) = args.get(i + 1) else {
                    return Err("--hidden requires a value".to_string());
                };
                hidden = Some(value.clone());
                i += 2;
            }
            "--mode" => {
                let Some(value) = args.get(i + 1) else {
                    return Err("--mode requires a value".to_string());
                };
                let normalized = value.to_ascii_lowercase();
                if normalized != "structural" && normalized != "black-box" {
                    return Err(format!("invalid --mode value: {}", value));
                }
                mode = normalized;
                i += 2;
            }
            "--shots" => {
                let Some(value) = args.get(i + 1) else {
                    return Err("--shots requires a value".to_string());
                };
                shots = match value.parse::<u32>() {
                    Ok(v) if v >= 1 => v,
                    _ => return Err(format!("invalid --shots value: {}", value)),
                };
                i += 2;
            }
            "--pretty" => {
                pretty = true;
                i += 1;
            }
            other => {
                return Err(format!("unknown bv option: {}", other));
            }
        }
    }

    let Some(hidden) = hidden else {
        return Err("--hidden is required".to_string());
    };

    Ok(BvCommandArgs {
        hidden,
        mode,
        shots,
        pretty,
    })
}

pub fn parse_bell_state_command_args(args: &[String]) -> Result<BellStateCommandArgs, String> {
    let mut noise_factor: Option<f64> = None;
    let mut noise_seed: u64 = 20260609;
    let mut sweep_export: Option<String> = None;
    let mut sweep_noise_factors = Vec::<f64>::new();
    let mut sweep_seeds = Vec::<u64>::new();
    let mut pretty = false;
    let mut i = 0usize;

    while i < args.len() {
        match args[i].as_str() {
            "--noise-factor" => {
                let Some(value) = args.get(i + 1) else {
                    return Err("--noise-factor requires a value".to_string());
                };
                let parsed = match value.parse::<f64>() {
                    Ok(v) => v,
                    Err(_) => return Err(format!("invalid --noise-factor value: {}", value)),
                };
                if !(0.0..=1.0).contains(&parsed) {
                    return Err(format!("invalid --noise-factor value: {}", value));
                }
                noise_factor = Some(parsed);
                i += 2;
            }
            "--noise-seed" => {
                let Some(value) = args.get(i + 1) else {
                    return Err("--noise-seed requires a value".to_string());
                };
                noise_seed = match value.parse::<u64>() {
                    Ok(v) => v,
                    Err(_) => return Err(format!("invalid --noise-seed value: {}", value)),
                };
                i += 2;
            }
            "--sweep-export" => {
                let Some(value) = args.get(i + 1) else {
                    return Err("--sweep-export requires a value".to_string());
                };
                let normalized = value.to_ascii_lowercase();
                if normalized != "json" && normalized != "csv" {
                    return Err(format!("invalid --sweep-export value: {}", value));
                }
                sweep_export = Some(normalized);
                i += 2;
            }
            "--sweep-noise-factors" => {
                let Some(value) = args.get(i + 1) else {
                    return Err("--sweep-noise-factors requires a value".to_string());
                };
                sweep_noise_factors = parse_noise_factor_list_csv(value)?;
                i += 2;
            }
            "--sweep-seeds" => {
                let Some(value) = args.get(i + 1) else {
                    return Err("--sweep-seeds requires a value".to_string());
                };
                sweep_seeds = parse_seed_list_csv(value)?;
                i += 2;
            }
            "--pretty" => {
                pretty = true;
                i += 1;
            }
            other => {
                return Err(format!("unknown bell-state option: {}", other));
            }
        }
    }

    Ok(BellStateCommandArgs {
        noise_factor,
        noise_seed,
        sweep_export,
        sweep_noise_factors,
        sweep_seeds,
        pretty,
    })
}

pub fn parse_dirac_mode_command_args(args: &[String]) -> Result<DiracModeCommandArgs, String> {
    let mut n_qubits = 6u8;
    let mut state_model = "uniform-random".to_string();
    let mut sweep_export: Option<String> = None;
    let mut sweep_coupling_densities = Vec::<f64>::new();
    let mut sweep_seeds = Vec::<u64>::new();
    let mut perturbation_amplitudes = Vec::<f64>::new();
    let mut perturbation_frequency = 8.0f64;
    let mut summary = false;
    let mut profile_report = false;
    let mut i = 0usize;

    while i < args.len() {
        match args[i].as_str() {
            "--n-qubits" => {
                let Some(value) = args.get(i + 1) else {
                    return Err("--n-qubits requires a value".to_string());
                };
                n_qubits = match value.parse::<u8>() {
                    Ok(v) if v >= 1 => v,
                    _ => return Err(format!("invalid --n-qubits value: {}", value)),
                };
                i += 2;
            }
            "--state-model" | "--rotor-profile" => {
                let Some(value) = args.get(i + 1) else {
                    return Err(format!("{} requires a value", args[i]));
                };
                let normalized = value.to_ascii_lowercase();
                if !super::register::is_supported_dirac_state_model(&normalized) {
                    return Err(format!("invalid --state-model value: {}", value));
                }
                state_model = normalized;
                i += 2;
            }
            "--sweep-export" => {
                let Some(value) = args.get(i + 1) else {
                    return Err("--sweep-export requires a value".to_string());
                };
                let normalized = value.to_ascii_lowercase();
                if normalized != "json" && normalized != "csv" {
                    return Err(format!("invalid --sweep-export value: {}", value));
                }
                sweep_export = Some(normalized);
                i += 2;
            }
            "--sweep-coupling-densities" => {
                let Some(value) = args.get(i + 1) else {
                    return Err("--sweep-coupling-densities requires a value".to_string());
                };
                sweep_coupling_densities = parse_density_list_csv(value)?;
                i += 2;
            }
            "--sweep-seeds" => {
                let Some(value) = args.get(i + 1) else {
                    return Err("--sweep-seeds requires a value".to_string());
                };
                sweep_seeds = parse_seed_list_csv(value)?;
                i += 2;
            }
            "--perturbation-amplitude" => {
                let Some(value) = args.get(i + 1) else {
                    return Err("--perturbation-amplitude requires a value".to_string());
                };
                let parsed = parse_perturbation_amplitude_list_csv(value)?;
                perturbation_amplitudes = vec![parsed[0]];
                i += 2;
            }
            "--perturbation-amplitudes" => {
                let Some(value) = args.get(i + 1) else {
                    return Err("--perturbation-amplitudes requires a value".to_string());
                };
                perturbation_amplitudes = parse_perturbation_amplitude_list_csv(value)?;
                i += 2;
            }
            "--perturbation-frequency" => {
                let Some(value) = args.get(i + 1) else {
                    return Err("--perturbation-frequency requires a value".to_string());
                };
                perturbation_frequency = parse_perturbation_frequency(value)?;
                i += 2;
            }
            "--summary" => {
                summary = true;
                i += 1;
            }
            "--profile-report" => {
                profile_report = true;
                i += 1;
            }
            other => {
                return Err(format!("unknown dirac-mode option: {}", other));
            }
        }
    }

    Ok(DiracModeCommandArgs {
        n_qubits,
        state_model,
        sweep_export,
        sweep_coupling_densities,
        sweep_seeds,
        perturbation_amplitudes,
        perturbation_frequency,
        summary,
        profile_report,
    })
}

pub fn parse_dirac_annihilation_command_args(args: &[String]) -> Result<DiracAnnihilationCommandArgs, String> {
    let mut n_qubits = 6u8;
    let mut profiles = vec!["uniform-random".to_string()];
    let mut unwinding_steps = 128u32;
    let mut flux_coupling_density = 0.40f64;
    let mut sweep_export: Option<String> = None;
    let mut output_prefix: Option<String> = None;
    let mut i = 0usize;

    while i < args.len() {
        match args[i].as_str() {
            "--n-qubits" => {
                let Some(value) = args.get(i + 1) else {
                    return Err("--n-qubits requires a value".to_string());
                };
                n_qubits = match value.parse::<u8>() {
                    Ok(v) if v >= 1 => v,
                    _ => return Err(format!("invalid --n-qubits value: {}", value)),
                };
                i += 2;
            }
            "--profiles" => {
                let Some(value) = args.get(i + 1) else {
                    return Err("--profiles requires a value".to_string());
                };
                profiles = parse_dirac_profile_list_csv(value)?;
                i += 2;
            }
            "--unwinding-steps" => {
                let Some(value) = args.get(i + 1) else {
                    return Err("--unwinding-steps requires a value".to_string());
                };
                unwinding_steps = match value.parse::<u32>() {
                    Ok(v) if v >= 4 => v,
                    _ => return Err(format!("invalid --unwinding-steps value: {}", value)),
                };
                i += 2;
            }
            "--flux-coupling-density" => {
                let Some(value) = args.get(i + 1) else {
                    return Err("--flux-coupling-density requires a value".to_string());
                };
                flux_coupling_density = match value.parse::<f64>() {
                    Ok(v) if (0.0..=1.0).contains(&v) => v,
                    _ => return Err(format!("invalid --flux-coupling-density value: {}", value)),
                };
                i += 2;
            }
            "--sweep-export" => {
                let Some(value) = args.get(i + 1) else {
                    return Err("--sweep-export requires a value".to_string());
                };
                let normalized = value.to_ascii_lowercase();
                if normalized != "json" && normalized != "csv" {
                    return Err(format!("invalid --sweep-export value: {}", value));
                }
                sweep_export = Some(normalized);
                i += 2;
            }
            "--output-prefix" => {
                let Some(value) = args.get(i + 1) else {
                    return Err("--output-prefix requires a value".to_string());
                };
                if value.trim().is_empty() {
                    return Err("invalid --output-prefix value: empty".to_string());
                }
                output_prefix = Some(value.clone());
                i += 2;
            }
            other => {
                return Err(format!("unknown dirac-annihilation option: {}", other));
            }
        }
    }

    Ok(DiracAnnihilationCommandArgs {
        n_qubits,
        profiles,
        unwinding_steps,
        flux_coupling_density,
        sweep_export,
        output_prefix,
    })
}

pub fn parse_shor_command_args(args: &[String]) -> Result<ShorCommandArgs, String> {
    let mut factoring_target = 15u64;
    let mut base_a = 2u64;
    let mut max_base_retries = 4u32;
    let mut pretty = false;
    let mut i = 0usize;

    while i < args.len() {
        match args[i].as_str() {
            "--factoring-target" => {
                let Some(value) = args.get(i + 1) else {
                    return Err("--factoring-target requires a value".to_string());
                };
                factoring_target = match value.parse::<u64>() {
                    Ok(v) if v >= 3 => v,
                    _ => return Err(format!("invalid --factoring-target value: {}", value)),
                };
                i += 2;
            }
            "--base-a" => {
                let Some(value) = args.get(i + 1) else {
                    return Err("--base-a requires a value".to_string());
                };
                base_a = match value.parse::<u64>() {
                    Ok(v) if v >= 2 => v,
                    _ => return Err(format!("invalid --base-a value: {}", value)),
                };
                i += 2;
            }
            "--max-base-retries" => {
                let Some(value) = args.get(i + 1) else {
                    return Err("--max-base-retries requires a value".to_string());
                };
                max_base_retries = match value.parse::<u32>() {
                    Ok(v) if v <= 32 => v,
                    _ => return Err(format!("invalid --max-base-retries value: {}", value)),
                };
                i += 2;
            }
            "--pretty" => {
                pretty = true;
                i += 1;
            }
            other => {
                return Err(format!("unknown shor option: {}", other));
            }
        }
    }

    Ok(ShorCommandArgs {
        factoring_target,
        base_a,
        max_base_retries,
        pretty,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vec_args(items: &[&str]) -> Vec<String> {
        items.iter().map(|v| (*v).to_string()).collect::<Vec<_>>()
    }

    #[test]
    fn parser_accepts_pretty_and_qubit_count() {
        let args = vec_args(&["--pretty", "--n-qubits", "7"]);
        let parsed = parse_quantum_register_scaffold_args(&args).expect("parse should succeed");
        assert_eq!(parsed.n_qubits, 7);
        assert!(parsed.pretty);
    }

    #[test]
    fn parser_rejects_missing_n_qubits_value() {
        let args = vec_args(&["--n-qubits"]);
        let err = parse_quantum_register_scaffold_args(&args).expect_err("parse should fail");
        assert_eq!(err, "--n-qubits requires a value");
    }

    #[test]
    fn parser_rejects_unknown_option() {
        let args = vec_args(&["--mystery"]);
        let err = parse_quantum_register_scaffold_args(&args).expect_err("parse should fail");
        assert_eq!(err, "unknown quantum-register-scaffold option: --mystery");
    }

    #[test]
    fn distributed_wave_parser_accepts_all_options() {
        let args = vec_args(&["--epochs", "96", "--replay-runs", "7", "--sweep-export", "csv", "--pretty"]);
        let parsed = parse_distributed_wave_harness_args(&args).expect("parse should succeed");
        assert_eq!(parsed.epochs, 96);
        assert_eq!(parsed.replay_runs, 7);
        assert_eq!(parsed.sweep_export.as_deref(), Some("csv"));
        assert!(parsed.pretty);
    }

    #[test]
    fn distributed_wave_parser_rejects_missing_value() {
        let args = vec_args(&["--replay-runs"]);
        let err = parse_distributed_wave_harness_args(&args).expect_err("parse should fail");
        assert_eq!(err, "--replay-runs requires a value");
    }

    #[test]
    fn distributed_wave_parser_rejects_unknown_option() {
        let args = vec_args(&["--bogus"]);
        let err = parse_distributed_wave_harness_args(&args).expect_err("parse should fail");
        assert_eq!(err, "unknown distributed-wave-harness option: --bogus");
    }

    #[test]
    fn distributed_wave_parser_rejects_bad_sweep_export_value() {
        let args = vec_args(&["--sweep-export", "yaml"]);
        let err = parse_distributed_wave_harness_args(&args).expect_err("parse should fail");
        assert_eq!(err, "invalid --sweep-export value: yaml");
    }

    #[test]
    fn bv_parser_accepts_black_box_with_shots() {
        let args = vec_args(&[
            "--hidden",
            "1011",
            "--mode",
            "black-box",
            "--shots",
            "1024",
            "--pretty",
        ]);
        let parsed = parse_bv_command_args(&args).expect("parse should succeed");
        assert_eq!(parsed.hidden, "1011");
        assert_eq!(parsed.mode, "black-box");
        assert_eq!(parsed.shots, 1024);
        assert!(parsed.pretty);
    }

    #[test]
    fn bv_parser_rejects_missing_hidden() {
        let args = vec_args(&["--mode", "structural"]);
        let err = parse_bv_command_args(&args).expect_err("parse should fail");
        assert_eq!(err, "--hidden is required");
    }

    #[test]
    fn bv_parser_rejects_bad_mode() {
        let args = vec_args(&["--hidden", "101", "--mode", "opaque"]);
        let err = parse_bv_command_args(&args).expect_err("parse should fail");
        assert_eq!(err, "invalid --mode value: opaque");
    }

    #[test]
    fn bell_state_parser_accepts_noise_and_pretty() {
        let args = vec_args(&["--noise-factor", "0.15", "--noise-seed", "42", "--pretty"]);
        let parsed = parse_bell_state_command_args(&args).expect("parse should succeed");
        assert_eq!(parsed.noise_factor, Some(0.15));
        assert_eq!(parsed.noise_seed, 42);
        assert_eq!(parsed.sweep_export.as_deref(), None);
        assert!(parsed.sweep_noise_factors.is_empty());
        assert!(parsed.sweep_seeds.is_empty());
        assert!(parsed.pretty);
    }

    #[test]
    fn bell_state_parser_accepts_sweep_export_options() {
        let args = vec_args(&[
            "--sweep-export",
            "csv",
            "--sweep-noise-factors",
            "0.0,0.1,0.2",
            "--sweep-seeds",
            "42,777",
        ]);
        let parsed = parse_bell_state_command_args(&args).expect("parse should succeed");
        assert_eq!(parsed.sweep_export.as_deref(), Some("csv"));
        assert_eq!(parsed.sweep_noise_factors, vec![0.0, 0.1, 0.2]);
        assert_eq!(parsed.sweep_seeds, vec![42, 777]);
    }

    #[test]
    fn bell_state_parser_rejects_noise_out_of_range() {
        let args = vec_args(&["--noise-factor", "1.2"]);
        let err = parse_bell_state_command_args(&args).expect_err("parse should fail");
        assert_eq!(err, "invalid --noise-factor value: 1.2");
    }

    #[test]
    fn bell_state_parser_rejects_bad_sweep_export_value() {
        let args = vec_args(&["--sweep-export", "yaml"]);
        let err = parse_bell_state_command_args(&args).expect_err("parse should fail");
        assert_eq!(err, "invalid --sweep-export value: yaml");
    }

    #[test]
    fn bell_state_parser_rejects_invalid_sweep_noise_factor_list() {
        let args = vec_args(&["--sweep-noise-factors", "0.1,2.0"]);
        let err = parse_bell_state_command_args(&args).expect_err("parse should fail");
        assert_eq!(err, "invalid --sweep-noise-factors value: 0.1,2.0");
    }

    #[test]
    fn bell_state_parser_rejects_invalid_sweep_seed_list() {
        let args = vec_args(&["--sweep-seeds", "42,abc"]);
        let err = parse_bell_state_command_args(&args).expect_err("parse should fail");
        assert_eq!(err, "invalid --sweep-seeds value: 42,abc");
    }

    #[test]
    fn bell_state_parser_rejects_unknown_option() {
        let args = vec_args(&["--mystery"]);
        let err = parse_bell_state_command_args(&args).expect_err("parse should fail");
        assert_eq!(err, "unknown bell-state option: --mystery");
    }

    #[test]
    fn dirac_mode_parser_accepts_sweep_options() {
        let args = vec_args(&[
            "--n-qubits",
            "8",
            "--state-model",
            "contiguous-band",
            "--sweep-export",
            "csv",
            "--sweep-coupling-densities",
            "0.02,0.12,0.35",
            "--sweep-seeds",
            "42,777",
            "--perturbation-amplitudes",
            "0.0,0.2",
            "--perturbation-frequency",
            "32",
            "--summary",
            "--profile-report",
        ]);
        let parsed = parse_dirac_mode_command_args(&args).expect("parse should succeed");
        assert_eq!(parsed.n_qubits, 8);
        assert_eq!(parsed.state_model, "contiguous-band");
        assert_eq!(parsed.sweep_export.as_deref(), Some("csv"));
        assert_eq!(parsed.sweep_coupling_densities, vec![0.02, 0.12, 0.35]);
        assert_eq!(parsed.sweep_seeds, vec![42, 777]);
        assert_eq!(parsed.perturbation_amplitudes, vec![0.0, 0.2]);
        assert_eq!(parsed.perturbation_frequency, 32.0);
        assert!(parsed.summary);
        assert!(parsed.profile_report);
    }

    #[test]
    fn dirac_mode_parser_accepts_rotor_profile_alias() {
        let args = vec_args(&["--rotor-profile", "high-grade-bias"]);
        let parsed = parse_dirac_mode_command_args(&args).expect("parse should succeed");
        assert_eq!(parsed.state_model, "high-grade-bias");
    }

    #[test]
    fn dirac_mode_parser_rejects_bad_density_list() {
        let args = vec_args(&["--sweep-coupling-densities", "0.1,1.2"]);
        let err = parse_dirac_mode_command_args(&args).expect_err("parse should fail");
        assert_eq!(err, "invalid --sweep-coupling-densities value: 0.1,1.2");
    }

    #[test]
    fn dirac_mode_parser_rejects_bad_state_model() {
        let args = vec_args(&["--state-model", "mystery"]);
        let err = parse_dirac_mode_command_args(&args).expect_err("parse should fail");
        assert_eq!(err, "invalid --state-model value: mystery");
    }

    #[test]
    fn dirac_mode_parser_rejects_bad_perturbation_amplitudes() {
        let args = vec_args(&["--perturbation-amplitudes", "0.2,2.0"]);
        let err = parse_dirac_mode_command_args(&args).expect_err("parse should fail");
        assert_eq!(err, "invalid --perturbation-amplitudes value: 0.2,2.0");
    }

    #[test]
    fn dirac_mode_parser_rejects_bad_perturbation_frequency() {
        let args = vec_args(&["--perturbation-frequency", "-1.0"]);
        let err = parse_dirac_mode_command_args(&args).expect_err("parse should fail");
        assert_eq!(err, "invalid --perturbation-frequency value: -1.0");
    }

    #[test]
    fn dirac_mode_parser_rejects_unknown_option() {
        let args = vec_args(&["--pretty"]);
        let err = parse_dirac_mode_command_args(&args).expect_err("parse should fail");
        assert_eq!(err, "unknown dirac-mode option: --pretty");
    }

    #[test]
    fn dirac_annihilation_parser_accepts_profile_sweep() {
        let args = vec_args(&[
            "--n-qubits",
            "6",
            "--profiles",
            "uniform-random,low-grade-bias,high-grade-bias,harmonic-stride",
            "--unwinding-steps",
            "128",
            "--flux-coupling-density",
            "0.40",
            "--sweep-export",
            "json",
            "--output-prefix",
            "docs/demo/dirac-annihilation-dynamics",
        ]);
        let parsed = parse_dirac_annihilation_command_args(&args).expect("parse should succeed");
        assert_eq!(parsed.n_qubits, 6);
        assert_eq!(
            parsed.profiles,
            vec![
                "uniform-random".to_string(),
                "low-grade-bias".to_string(),
                "high-grade-bias".to_string(),
                "harmonic-stride".to_string()
            ]
        );
        assert_eq!(parsed.unwinding_steps, 128);
        assert_eq!(parsed.flux_coupling_density, 0.40);
        assert_eq!(parsed.sweep_export.as_deref(), Some("json"));
        assert_eq!(
            parsed.output_prefix.as_deref(),
            Some("docs/demo/dirac-annihilation-dynamics")
        );
    }

    #[test]
    fn dirac_annihilation_parser_rejects_bad_profiles() {
        let args = vec_args(&["--profiles", "uniform-random,mystery"]);
        let err = parse_dirac_annihilation_command_args(&args).expect_err("parse should fail");
        assert_eq!(err, "invalid --profiles value: uniform-random,mystery");
    }

    #[test]
    fn dirac_annihilation_parser_rejects_unknown_option() {
        let args = vec_args(&["--pretty"]);
        let err = parse_dirac_annihilation_command_args(&args).expect_err("parse should fail");
        assert_eq!(err, "unknown dirac-annihilation option: --pretty");
    }

    #[test]
    fn shor_parser_accepts_target_and_base() {
        let args = vec_args(&[
            "--factoring-target",
            "15",
            "--base-a",
            "2",
            "--max-base-retries",
            "6",
            "--pretty",
        ]);
        let parsed = parse_shor_command_args(&args).expect("parse should succeed");
        assert_eq!(parsed.factoring_target, 15);
        assert_eq!(parsed.base_a, 2);
        assert_eq!(parsed.max_base_retries, 6);
        assert!(parsed.pretty);
    }

    #[test]
    fn shor_parser_rejects_bad_target() {
        let args = vec_args(&["--factoring-target", "2"]);
        let err = parse_shor_command_args(&args).expect_err("parse should fail");
        assert_eq!(err, "invalid --factoring-target value: 2");
    }

    #[test]
    fn shor_parser_rejects_unknown_option() {
        let args = vec_args(&["--mystery"]);
        let err = parse_shor_command_args(&args).expect_err("parse should fail");
        assert_eq!(err, "unknown shor option: --mystery");
    }

    #[test]
    fn shor_parser_rejects_bad_retry_count() {
        let args = vec_args(&["--max-base-retries", "999"]);
        let err = parse_shor_command_args(&args).expect_err("parse should fail");
        assert_eq!(err, "invalid --max-base-retries value: 999");
    }
}
