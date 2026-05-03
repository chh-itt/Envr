use assert_cmd::Command;
use std::fs;

const DOT_ENVR_TOML: &str = ".envr.toml";

fn prepare_version_marker(root: &std::path::Path, lang: &str) {
    let version_dir = root
        .join("runtimes")
        .join(lang)
        .join("versions")
        .join("22.11.0");
    fs::create_dir_all(version_dir.join("bin")).expect("mkdir version bin");
    fs::write(version_dir.join("bin").join("tool"), "installed").expect("write version marker");
    let current = root.join("runtimes").join(lang).join("current");
    fs::write(current, version_dir.as_os_str().to_string_lossy().as_ref())
        .expect("write current marker");
}

fn child_command_args() -> Vec<&'static str> {
    if cfg!(windows) {
        vec!["cmd", "/C", "echo", "ok"]
    } else {
        vec!["sh", "-c", "echo ok"]
    }
}

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
    prepare_version_marker(tmp.path(), "node");
    prepare_version_marker(tmp.path(), "python");

    let out = Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .current_dir(tmp.path())
        .args(["run", "--verbose", "--"])
        .args(child_command_args())
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
        stderr.contains("22.11.0"),
        "expected verbose output to include node version: {stderr}"
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
    prepare_version_marker(tmp.path(), "node");

    let out = Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .current_dir(tmp.path())
        .args(["exec", "--lang", "node", "--verbose", "--"])
        .args(child_command_args())
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
    prepare_version_marker(tmp.path(), "node");

    let out = Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .current_dir(tmp.path())
        .args(["exec", "--lang", "node", "--install-if-missing", "--"])
        .args(child_command_args())
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
        stderr.contains("正在安装缺失的运行时")
            || stderr.contains("installing missing runtime")
            || stderr.contains("missing runtime"),
        "expected install hint in output: {stderr}"
    );
}
