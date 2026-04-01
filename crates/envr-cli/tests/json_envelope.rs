//! JSON responses use a single envelope per `refactor docs/02-cli-设计.md`.

use assert_cmd::Command;
use serde_json::Value;

fn parse_json_line(stdout: &[u8]) -> Value {
    for line in stdout.split(|b| *b == b'\n') {
        if line.is_empty() {
            continue;
        }
        if line.first() == Some(&b'{') && let Ok(v) = serde_json::from_slice::<Value>(line) {
            return v;
        }
    }
    panic!("no json object in stdout: {}", String::from_utf8_lossy(stdout));
}

fn json_stdout(args: &[&str], root: &std::path::Path) -> Value {
    let out = Command::cargo_bin("envr")
        .expect("envr binary")
        .env("ENVR_RUNTIME_ROOT", root.as_os_str())
        .args(args)
        .output()
        .expect("envr output");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    parse_json_line(&out.stdout)
}

fn assert_envelope_shape(v: &Value) {
    assert!(v.get("success").is_some(), "missing success: {v}");
    assert!(v.get("code").is_some(), "missing code: {v}");
    assert!(v.get("message").is_some(), "missing message: {v}");
    assert!(v.get("data").is_some(), "missing data: {v}");
    assert!(v.get("diagnostics").is_some(), "missing diagnostics: {v}");
}

#[test]
fn list_json_contract() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let v = json_stdout(
        &["--format", "json", "list"],
        tmp.path(),
    );
    assert_envelope_shape(&v);
    assert_eq!(v["success"], true);
    assert!(v["code"].is_null());
    assert_eq!(v["message"], "list_installed");
    assert!(v["data"]["runtimes"].is_array());
}

#[test]
fn current_json_contract() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let v = json_stdout(
        &["--format", "json", "current"],
        tmp.path(),
    );
    assert_envelope_shape(&v);
    assert_eq!(v["success"], true);
    assert!(v["code"].is_null());
    assert_eq!(v["message"], "show_current");
    assert!(v["data"]["current"].is_array());
}

#[test]
fn doctor_json_contract() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let v = json_stdout(
        &["--format", "json", "doctor"],
        tmp.path(),
    );
    assert_envelope_shape(&v);
    assert_eq!(v["success"], true);
    assert!(v["code"].is_null());
    assert_eq!(v["message"], "doctor_ok");
    assert!(v["data"]["kinds"].is_array());
}

#[test]
fn validation_error_json_has_code() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = Command::cargo_bin("envr")
        .expect("envr binary")
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .args(["--format", "json", "install", "node"])
        .output()
        .expect("run");
    assert!(!out.status.success());
    let v = parse_json_line(&out.stdout);
    assert_envelope_shape(&v);
    assert_eq!(v["success"], false);
    assert_eq!(v["code"], "validation");
}
