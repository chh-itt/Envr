use assert_cmd::Command;
use std::fs;

const DOT_ENVR_TOML: &str = ".envr.toml";

#[test]
fn run_verbose_reports_resolved_layers() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(
        tmp.path().join(DOT_ENVR_TOML),
        r#"
[runtimes.node]
version = "22.11.0"

[runtimes.python]
version = "3.12.7"
"#,
    )
    .expect("write envr toml");

    let out = Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .current_dir(tmp.path())
        .args(["run", "--verbose", "--", "cmd", "/C", "echo", "ok"])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("node"),
        "expected verbose output to mention node: {stderr}"
    );
    assert!(
        stderr.contains("python"),
        "expected verbose output to mention python: {stderr}"
    );
    assert!(
        stderr.contains("22.11.0") || stderr.contains("3.12.7"),
        "expected verbose output to include versions: {stderr}"
    );
}

#[test]
fn exec_verbose_reports_exec_resolution() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(
        tmp.path().join(DOT_ENVR_TOML),
        r#"
[runtimes.node]
version = "22.11.0"
"#,
    )
    .expect("write envr toml");

    let out = Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .current_dir(tmp.path())
        .args([
            "exec",
            "--lang",
            "node",
            "--verbose",
            "--",
            "cmd",
            "/C",
            "echo",
            "ok",
        ])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("node"),
        "expected exec verbose output to mention node: {stderr}"
    );
    assert!(
        stderr.contains("22.11.0"),
        "expected exec verbose output to mention version: {stderr}"
    );
}

#[test]
fn exec_install_if_missing_reports_missing_pin() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(
        tmp.path().join(DOT_ENVR_TOML),
        r#"
[runtimes.node]
version = "99.99.99"
"#,
    )
    .expect("write envr toml");

    let out = Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .current_dir(tmp.path())
        .args([
            "exec",
            "--lang",
            "node",
            "--install-if-missing",
            "--",
            "cmd",
            "/C",
            "echo",
            "ok",
        ])
        .output()
        .expect("run");
    assert!(
        !out.status.success(),
        "expected missing runtime to fail when install-if-missing is set"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("99.99.99") || stderr.contains("node"),
        "expected missing pin details: {stderr}"
    );
    assert!(
        stderr.contains("install-if-missing") || stderr.contains("missing"),
        "expected install hint in output: {stderr}"
    );
}
