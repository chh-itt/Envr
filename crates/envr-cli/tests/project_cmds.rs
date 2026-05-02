use assert_cmd::Command;
use std::fs;

const DOT_ENVR_TOML: &str = ".envr.toml";

#[test]
fn init_creates_envr_toml() {
    let tmp = tempfile::tempdir().expect("tempdir");
    Command::cargo_bin("envr")
        .expect("envr")
        .current_dir(tmp.path())
        .args(["init"])
        .assert()
        .success();
    let p = tmp.path().join(DOT_ENVR_TOML);
    assert!(p.is_file());
    let text = fs::read_to_string(&p).expect("read");
    assert!(text.contains("[runtimes.node]"));
}

#[test]
fn init_refuses_existing_without_force() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let p = tmp.path().join(DOT_ENVR_TOML);
    fs::write(&p, "[env]\n").expect("write");
    Command::cargo_bin("envr")
        .expect("envr")
        .current_dir(tmp.path())
        .args(["init"])
        .assert()
        .failure();
}

#[test]
fn check_passes_after_init() {
    let tmp = tempfile::tempdir().expect("tempdir");
    Command::cargo_bin("envr")
        .expect("envr")
        .current_dir(tmp.path())
        .arg("init")
        .assert()
        .success();
    Command::cargo_bin("envr")
        .expect("envr")
        .current_dir(tmp.path())
        .arg("check")
        .assert()
        .success();
}

#[test]
fn project_add_writes_pin() {
    let tmp = tempfile::tempdir().expect("tempdir");
    Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .current_dir(tmp.path())
        .args(["project", "add", "node@22.1.0"])
        .assert()
        .success();
    let p = tmp.path().join(DOT_ENVR_TOML);
    let text = fs::read_to_string(&p).expect("read");
    assert!(
        text.contains("22.1.0") && text.contains("[runtimes.node]"),
        "unexpected toml:\n{text}"
    );
}

#[test]
fn project_add_rejects_bad_spec() {
    let tmp = tempfile::tempdir().expect("tempdir");
    Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .current_dir(tmp.path())
        .args(["project", "add", "not-a-runtime@1"])
        .assert()
        .failure();
}

#[test]
fn check_fails_when_no_config() {
    let tmp = tempfile::tempdir().expect("tempdir");
    Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .current_dir(tmp.path())
        .arg("check")
        .assert()
        .failure();
}

#[test]
fn import_tool_versions_writes_envr_toml() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(
        tmp.path().join(".tool-versions"),
        "nodejs 22.11.0\npython 3.12.7\ngolang 1.23.2\n",
    )
    .expect("write tool versions");

    Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .current_dir(tmp.path())
        .args(["import", "--config-format", "tool-versions"])
        .assert()
        .success();

    let text = fs::read_to_string(tmp.path().join(DOT_ENVR_TOML)).expect("read envr toml");
    assert!(text.contains("[runtimes.node]"), "unexpected toml:\n{text}");
    assert!(
        text.contains("version = \"22.11.0\""),
        "unexpected toml:\n{text}"
    );
    assert!(
        text.contains("[runtimes.python]"),
        "unexpected toml:\n{text}"
    );
    assert!(text.contains("[runtimes.go]"), "unexpected toml:\n{text}");
}

#[test]
fn export_tool_versions_uses_asdf_names() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(
        tmp.path().join(DOT_ENVR_TOML),
        r#"
[runtimes.node]
version = "22.11.0"

[runtimes.go]
version = "1.23.2"
"#,
    )
    .expect("write envr toml");

    Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .current_dir(tmp.path())
        .args([
            "export",
            "--config-format",
            "tool-versions",
            "--output",
            ".tool-versions",
        ])
        .assert()
        .success();

    let text = fs::read_to_string(tmp.path().join(".tool-versions")).expect("read tool versions");
    assert!(
        text.contains("nodejs 22.11.0\n"),
        "unexpected export:\n{text}"
    );
    assert!(
        text.contains("golang 1.23.2\n"),
        "unexpected export:\n{text}"
    );
}

#[test]
fn import_tool_versions_records_compat_names() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(
        tmp.path().join(".tool-versions"),
        "nodejs 22.11.0\ncustom-tool 1.2.3\n",
    )
    .expect("write tool versions");

    Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .current_dir(tmp.path())
        .args(["import", "--config-format", "tool-versions"])
        .assert()
        .success();

    let text = fs::read_to_string(tmp.path().join(DOT_ENVR_TOML)).expect("read envr toml");
    assert!(
        text.contains("[compat.asdf.names]"),
        "unexpected toml:\n{text}"
    );
    assert!(
        text.contains("nodejs = \"node\""),
        "unexpected toml:\n{text}"
    );
    assert!(!text.contains("custom-tool ="), "unexpected toml:\n{text}");
}
