use serde_json::{json, Value};

#[allow(dead_code)]
const DEFAULT_SHOR_MAX_BASE_RETRIES: u32 = 4;

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

fn mod_pow(mut base: u64, mut exponent: u64, modulus: u64) -> u64 {
    if modulus == 1 {
        return 0;
    }
    let mut result = 1u64;
    base %= modulus;
    while exponent > 0 {
        if exponent & 1 == 1 {
            result = result.saturating_mul(base) % modulus;
        }
        exponent >>= 1;
        base = base.saturating_mul(base) % modulus;
    }
    result
}

fn execute_geometric_qft(period: u64, sample_span: u64) -> Vec<Value> {
    if period == 0 || sample_span == 0 {
        return vec![];
    }

    let span = sample_span.max(8).min(64);
    (0..span)
        .map(|k| {
            let theta = 2.0 * std::f64::consts::PI * (k as f64) / (period as f64);
            let coherence = ((theta.cos() + 1.0) * 0.5).clamp(0.0, 1.0);
            json!({
                "k": k,
                "blade_grade": 2,
                "phase_alignment": (coherence * 1_000_000.0).round() / 1_000_000.0,
            })
        })
        .collect::<Vec<_>>()
}

#[derive(Clone, Debug)]
struct ShorAttemptResult {
    base_a: u64,
    status: String,
    period: Option<u64>,
    factors: Vec<u64>,
    oracle_trace: Vec<Value>,
    qft_projection: Vec<Value>,
    diagnostics: Value,
}

fn run_shor_single_attempt(factoring_target: u64, base_a: u64) -> ShorAttemptResult {
    let initial_gcd = gcd(base_a, factoring_target);
    if initial_gcd > 1 {
        return ShorAttemptResult {
            base_a,
            status: "trivial_factor_found".to_string(),
            period: None,
            factors: vec![initial_gcd, factoring_target / initial_gcd],
            oracle_trace: vec![],
            qft_projection: vec![],
            diagnostics: json!({
                "crossing_density_ratio_to_uniform": 0.0,
                "dense_fallback_active": false,
                "retry_reason": "base_shared_nontrivial_gcd",
            }),
        };
    }

    let mut oracle_trace = Vec::<Value>::new();
    let mut period: Option<u64> = None;
    for x in 1..=(factoring_target * 2) {
        let residue = mod_pow(base_a, x, factoring_target);
        oracle_trace.push(json!({
            "x": x,
            "residue": residue,
            "support_density": ((residue as f64 / factoring_target as f64) * 1_000_000.0).round() / 1_000_000.0,
            "blade_grade": 2,
        }));
        if residue == 1 {
            period = Some(x);
            break;
        }
    }

    let Some(period) = period else {
        return ShorAttemptResult {
            base_a,
            status: "period_not_found".to_string(),
            period: None,
            factors: vec![],
            oracle_trace,
            qft_projection: vec![],
            diagnostics: json!({
                "crossing_density_ratio_to_uniform": 0.0,
                "dense_fallback_active": true,
                "retry_reason": "period_not_found",
            }),
        };
    };

    let mut factors = Vec::<u64>::new();
    if period % 2 == 0 {
        let half_period = mod_pow(base_a, period / 2, factoring_target);
        if half_period != factoring_target - 1 {
            let f1 = gcd(half_period.saturating_sub(1), factoring_target);
            let f2 = gcd(half_period.saturating_add(1), factoring_target);
            if f1 > 1 && f1 < factoring_target {
                factors.push(f1);
            }
            if f2 > 1 && f2 < factoring_target && !factors.contains(&f2) {
                factors.push(f2);
            }
        }
    }
    factors.sort_unstable();

    let qft_projection = execute_geometric_qft(period, factoring_target);
    let factored = factors.len() >= 2;
    ShorAttemptResult {
        base_a,
        status: if factored {
            "factored".to_string()
        } else {
            "period_found_no_nontrivial_factor".to_string()
        },
        period: Some(period),
        factors,
        oracle_trace,
        qft_projection,
        diagnostics: json!({
            "crossing_density_ratio_to_uniform": ((period as f64 / factoring_target as f64) * 1_000_000.0).round() / 1_000_000.0,
            "dense_fallback_active": period > factoring_target,
            "retry_reason": if factored { Value::Null } else { json!("period_found_no_nontrivial_factor") },
        }),
    }
}

pub fn run_shor_geometric_report_with_retry(
    factoring_target: u64,
    base_a: u64,
    max_base_retries: u32,
) -> Result<Value, String> {
    if factoring_target < 3 {
        return Err("factoring_target must be >= 3".to_string());
    }
    if factoring_target % 2 == 0 {
        return Err("factoring_target must be odd for this scaffold (try 15)".to_string());
    }
    if base_a <= 1 || base_a >= factoring_target {
        return Err("base_a must be in (1, factoring_target)".to_string());
    }

    let mut candidate_bases = Vec::<u64>::new();
    candidate_bases.push(base_a);
    for candidate in 2..factoring_target {
        if candidate != base_a {
            candidate_bases.push(candidate);
        }
    }

    let max_attempts = 1usize.saturating_add(max_base_retries as usize);
    let mut attempts = Vec::<ShorAttemptResult>::new();
    for candidate in candidate_bases.into_iter().take(max_attempts) {
        let attempt = run_shor_single_attempt(factoring_target, candidate);
        let terminal = attempt.status == "factored" || attempt.status == "trivial_factor_found";
        attempts.push(attempt);
        if terminal {
            break;
        }
    }

    let final_attempt = attempts
        .last()
        .cloned()
        .ok_or_else(|| "shor retry policy produced no attempts".to_string())?;

    Ok(json!({
        "object": "csif.quantum.shor.geometric_report",
        "algorithm": "shor_geometric",
        "status": final_attempt.status,
        "factoring_target": factoring_target,
        "base_a": base_a,
        "selected_base_a": final_attempt.base_a,
        "max_base_retries": max_base_retries,
        "retry_attempts_used": attempts.len().saturating_sub(1),
        "period": final_attempt.period,
        "factors": final_attempt.factors,
        "oracle_trace": final_attempt.oracle_trace,
        "qft_projection": {
            "blade_grade": 2,
            "phase_alignment_sweep": final_attempt.qft_projection,
        },
        "diagnostics": final_attempt.diagnostics,
        "attempts": attempts
            .iter()
            .enumerate()
            .map(|(idx, attempt)| json!({
                "attempt_index": idx,
                "base_a": attempt.base_a,
                "status": attempt.status,
                "period": attempt.period,
                "factors": attempt.factors,
                "diagnostics": attempt.diagnostics,
            }))
            .collect::<Vec<_>>(),
    }))
}

#[allow(dead_code)]
pub fn run_shor_geometric_report(factoring_target: u64, base_a: u64) -> Result<Value, String> {
    run_shor_geometric_report_with_retry(factoring_target, base_a, DEFAULT_SHOR_MAX_BASE_RETRIES)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shor_geometric_scaffold_factors_15() {
        let payload = run_shor_geometric_report(15, 2).expect("shor scaffold should run");
        assert_eq!(payload.get("algorithm"), Some(&json!("shor_geometric")));
        assert_eq!(payload.get("factoring_target"), Some(&json!(15)));
        let factors = payload
            .get("factors")
            .and_then(Value::as_array)
            .expect("factors should be array");
        assert!(factors.iter().any(|value| value == &json!(3)));
        assert!(factors.iter().any(|value| value == &json!(5)));
    }

    #[test]
    fn shor_geometric_retry_policy_uses_next_base_after_nontrivial_failure() {
        let payload = run_shor_geometric_report_with_retry(21, 4, 3)
            .expect("shor retry scaffold should run");
        assert_eq!(payload.get("status"), Some(&json!("factored")));
        assert_eq!(payload.get("base_a"), Some(&json!(4)));

        let attempts = payload
            .get("attempts")
            .and_then(Value::as_array)
            .expect("attempts should be array");
        assert!(attempts.len() >= 2);
        assert_eq!(attempts[0].get("base_a"), Some(&json!(4)));
        assert_eq!(
            attempts[0].get("status"),
            Some(&json!("period_found_no_nontrivial_factor"))
        );
    }
}
