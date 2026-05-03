use assert_cmd::Command;
use serde_json::Value;
use std::fs;

const DOT_ENVR_TOML: &str = ".envr.toml";

fn parse_json(stdout: &str) -> Value {
    let line = stdout.lines().find(|l| l.trim_start().starts_with('{')).expect("json line");
    serde_json::from_str(line).expect("parse json")
}

#[test]
fn run_install_if_missing_auto_installs_missing_pins() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join(DOT_ENVR_TOML), r#"
[runtimes.node]
version = "22.11.0"
"#).expect("write envr toml");

    let out = Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .current_dir(tmp.path())
        .args(["run", "--install-if-missing", "echo", "ok"])
        .output()
        .expect("run");

    assert!(out.status.success(), "stderr={}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("ok") || stdout.contains("Installed missing pinned runtimes."), "expected run to execute or report install: {stdout}");
}

#[test]
fn run_install_if_missing_json_reports_auto_installed() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join(DOT_ENVR_TOML), r#"
[runtimes.node]
version = "22.11.0"
"#).expect("write envr toml");

    let out = Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .current_dir(tmp.path())
        .args(["--format", "json", "run", "--install-if-missing", "echo", "ok"])
        .output()
        .expect("run");

    assert!(out.status.success(), "stderr={}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v = parse_json(&stdout);
    assert!(v["data"]["auto_installed"].is_boolean(), "expected auto_installed boolean: {v}");
    assert_eq!(v["data"]["install_if_missing"], true, "{v}");
    assert_eq!(v["data"]["dry_run"], false, "{v}");
    assert_eq!(v["data"]["verbose"], false, "{v}");
    assert!(v["data"]["env_files"].is_array(), "{v}");
    assert!(v["data"]["env_overrides"].is_array(), "{v}");
}

#[test]
fn run_install_if_missing_human_reports_missing_pins() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join(DOT_ENVR_TOML), r#"
[runtimes.node]
version = "99.99.99"
"#).expect("write envr toml");

    let out = Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .current_dir(tmp.path())
        .args(["run", "--install-if-missing", "echo", "ok"])
        .output()
        .expect("run");

    assert!(!out.status.success(), "expected missing runtime to fail when install-if-missing is set");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("node") || stderr.contains("99.99.99"), "expected missing pin details in human output: {stderr}");
}
