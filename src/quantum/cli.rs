#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuantumRegisterScaffoldArgs {
    pub n_qubits: u8,
    pub pretty: bool,
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
}
