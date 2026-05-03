use assert_cmd::Command;
use std::fs;

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

fn run_command_args() -> [&'static str; 6] {
    ["run", "--verbose", "cmd", "/C", "echo", "ok"]
}

fn exec_command_args() -> [&'static str; 11] {
    [
        "exec",
        "--lang",
        "node",
        "--spec",
        "22.11.0",
        "--verbose",
        "--",
        "cmd",
        "/C",
        "echo",
        "ok",
    ]
}

#[test]
fn run_env_verbose_lists_runtime_layers() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(
        tmp.path().join(".envr.toml"),
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
        .args(run_command_args())
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
        "expected verbose output to include node resolution: {stderr}"
    );
    assert!(
        stderr.contains("22.11.0"),
        "expected verbose output to include version: {stderr}"
    );
}

#[test]
fn exec_env_verbose_reports_spec_resolution() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(
        tmp.path().join(".envr.toml"),
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
        .args(exec_command_args())
        .output()
        .expect("run");

    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("22.11.0"),
        "expected verbose exec output to include spec: {stderr}"
    );
    assert!(
        stderr.contains("node"),
        "expected verbose exec output to include node: {stderr}"
    );
}
