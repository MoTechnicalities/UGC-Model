use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .to_path_buf()
}

fn fixture_path(name: &str) -> PathBuf {
    repo_root()
        .join("tests")
        .join("conformance")
        .join("layer0")
        .join("fixtures")
        .join(name)
}

#[test]
fn layer0_check_accepts_valid_temporal_order_fixture() {
    let bin = env!("CARGO_BIN_EXE_csif_agent_v2_rust");
    let input = fixture_path("LAYER0-C-001-temporal-valid.json");

    let output = Command::new(bin)
        .arg("layer0-check")
        .arg(&input)
        .output()
        .expect("layer0-check should run");

    assert!(
        output.status.success(),
        "layer0-check failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let payload: Value = serde_json::from_slice(&output.stdout).expect("json output expected");
    assert_eq!(payload.get("valid"), Some(&Value::Bool(true)));
    assert_eq!(payload.get("stop_reason"), Some(&Value::String("path_found".to_string())));
}

#[test]
fn layer0_check_rejects_temporal_cycle_fixture() {
    let bin = env!("CARGO_BIN_EXE_csif_agent_v2_rust");
    let input = fixture_path("LAYER0-C-002-temporal-cycle.json");

    let output = Command::new(bin)
        .arg("layer0-check")
        .arg(&input)
        .output()
        .expect("layer0-check should run");

    assert!(
        !output.status.success(),
        "layer0-check unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let payload: Value = serde_json::from_slice(&output.stdout).expect("json output expected");
    assert_eq!(payload.get("valid"), Some(&Value::Bool(false)));
    assert_eq!(
        payload.get("stop_reason"),
        Some(&Value::String("contradiction_detected".to_string()))
    );
    let contradictions = payload
        .get("contradictions")
        .and_then(Value::as_array)
        .expect("contradictions should be array");
    assert!(contradictions.iter().any(|item| {
        item.get("code") == Some(&Value::String("temporal_cycle".to_string()))
    }));
}

#[test]
fn layer0_conformance_single_case_matches_expected() {
    let bin = env!("CARGO_BIN_EXE_csif_agent_v2_rust");

    let output = Command::new(bin)
        .arg("layer0-conformance")
        .arg("--case")
        .arg("C-012")
        .output()
        .expect("layer0-conformance should run");

    assert!(
        output.status.success(),
        "layer0-conformance failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let payload: Value = serde_json::from_slice(&output.stdout).expect("json output expected");
    assert_eq!(payload.get("fail_count"), Some(&Value::from(0)));
    let results = payload
        .get("results")
        .and_then(Value::as_array)
        .expect("results should be array");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].get("case_id"), Some(&Value::String("C-012".to_string())));
    assert_eq!(results[0].get("passed"), Some(&Value::Bool(true)));

    let _ = fs::metadata(fixture_path("LAYER0-C-001-temporal-valid.json"));
}
