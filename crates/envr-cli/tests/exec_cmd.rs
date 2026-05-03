use assert_cmd::Command;
use serde_json::Value;
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

#[test]
fn exec_json_reports_exec_run_envelope_fields() {
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
        .args([
            "--format", "json", "exec", "--lang", "node", "--", "cmd", "/C", "echo", "ok",
        ])
        .output()
        .expect("run");

    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: Value = serde_json::from_str(
        stdout
            .lines()
            .find(|l| l.trim_start().starts_with('{'))
            .expect("json line"),
    )
    .expect("parse json");
    assert_eq!(v["code"], "child_completed", "{v}");
    for key in [
        "exit_code",
        "command",
        "args",
        "lang",
        "install_if_missing",
        "dry_run",
        "verbose",
        "auto_installed",
        "env_files",
        "env_overrides",
        "output_file",
    ] {
        assert!(
            v["data"].get(key).is_some(),
            "expected exec json envelope to include {key}: {v}"
        );
    }
}

#[test]
fn run_json_reports_run_exec_envelope_fields() {
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
        .args(["--format", "json", "run", "cmd", "/C", "echo", "ok"])
        .output()
        .expect("run");

    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: Value = serde_json::from_str(
        stdout
            .lines()
            .find(|l| l.trim_start().starts_with('{'))
            .expect("json line"),
    )
    .expect("parse json");
    assert_eq!(v["code"], "child_completed", "{v}");
    for key in [
        "exit_code",
        "command",
        "args",
        "install_if_missing",
        "dry_run",
        "verbose",
        "auto_installed",
        "env_files",
        "env_overrides",
    ] {
        assert!(
            v["data"].get(key).is_some(),
            "expected run json envelope to include {key}: {v}"
        );
    }
}
