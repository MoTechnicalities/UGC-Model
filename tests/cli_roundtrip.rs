use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate should live under repo root")
        .to_path_buf()
}

fn fixture_path(name: &str) -> PathBuf {
    repo_root()
        .join("tests")
        .join("conformance")
        .join("rwif_v2")
        .join("fixtures")
        .join(name)
}

fn unique_output_path(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should be valid")
        .as_nanos();
    std::env::temp_dir().join(format!("{}_{}_{}.json", prefix, std::process::id(), nanos))
}

#[test]
fn migrate_then_validate_fixture_roundtrip() {
    let bin = env!("CARGO_BIN_EXE_csif_agent_v2_rust");
    let input = fixture_path("RWIF-C-001-v1-bank.json");
    let output = unique_output_path("rwif_v2_migrated_bank");

    let migrate = Command::new(bin)
        .arg("migrate-bank")
        .arg(&input)
        .arg(&output)
        .output()
        .expect("migrate command should run");

    assert!(
        migrate.status.success(),
        "migrate failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&migrate.stdout),
        String::from_utf8_lossy(&migrate.stderr)
    );

    let migrate_out: Value =
        serde_json::from_slice(&migrate.stdout).expect("migrate stdout should be valid JSON");
    assert_eq!(migrate_out.get("migrated"), Some(&Value::Bool(true)));

    let migrated_data: Value = serde_json::from_str(
        &fs::read_to_string(&output).expect("migrated file should be present"),
    )
    .expect("migrated file should be valid JSON");

    assert_eq!(
        migrated_data
            .get("rwif_schema_version")
            .and_then(Value::as_str),
        Some("RWIF_V2")
    );
    assert_eq!(
        migrated_data
            .get("unknown_bank_field")
            .and_then(Value::as_str),
        Some("must_stay")
    );

    let validate = Command::new(bin)
        .arg("validate-bank")
        .arg(&output)
        .output()
        .expect("validate command should run");

    assert!(
        validate.status.success(),
        "validate failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&validate.stdout),
        String::from_utf8_lossy(&validate.stderr)
    );

    let validate_out: Value =
        serde_json::from_slice(&validate.stdout).expect("validate stdout should be valid JSON");
    assert_eq!(validate_out.get("errors"), Some(&Value::Array(vec![])));

    let _ = fs::remove_file(&output);
}

#[test]
fn validate_fails_for_invalid_fixture() {
    let bin = env!("CARGO_BIN_EXE_csif_agent_v2_rust");
    let input = fixture_path("RWIF-C-002-v2-invalid-missing-integer-wrap.json");

    let validate = Command::new(bin)
        .arg("validate-bank")
        .arg(&input)
        .output()
        .expect("validate command should run");

    assert!(
        !validate.status.success(),
        "validate unexpectedly succeeded\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&validate.stdout),
        String::from_utf8_lossy(&validate.stderr)
    );

    let validate_out: Value =
        serde_json::from_slice(&validate.stdout).expect("validate stdout should be valid JSON");

    let errors = validate_out
        .get("errors")
        .and_then(Value::as_array)
        .expect("errors should be an array");
    assert!(
        errors.iter().any(|e| {
            e.as_str()
                .map(|s| s.contains("missing integer_wrap_mode"))
                .unwrap_or(false)
        }),
        "expected missing integer_wrap_mode error, got {:?}",
        errors
    );
}

#[test]
fn migrate_is_byte_identical_on_second_run() {
    let bin = env!("CARGO_BIN_EXE_csif_agent_v2_rust");
    let input = fixture_path("RWIF-C-001-v1-bank.json");
    let output1 = unique_output_path("rwif_v2_migrated_once");
    let output2 = unique_output_path("rwif_v2_migrated_twice");

    let first = Command::new(bin)
        .arg("migrate-bank")
        .arg(&input)
        .arg(&output1)
        .output()
        .expect("first migrate command should run");
    assert!(
        first.status.success(),
        "first migrate failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&first.stdout),
        String::from_utf8_lossy(&first.stderr)
    );

    let second = Command::new(bin)
        .arg("migrate-bank")
        .arg(&output1)
        .arg(&output2)
        .output()
        .expect("second migrate command should run");
    assert!(
        second.status.success(),
        "second migrate failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&second.stdout),
        String::from_utf8_lossy(&second.stderr)
    );

    let first_text = fs::read_to_string(&output1).expect("first migrated output should be readable");
    let second_text = fs::read_to_string(&output2).expect("second migrated output should be readable");
    assert_eq!(
        first_text, second_text,
        "expected byte-identical outputs for repeated migration"
    );

    let _ = fs::remove_file(&output1);
    let _ = fs::remove_file(&output2);
}
