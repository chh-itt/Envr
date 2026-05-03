use assert_cmd::Command;
use std::fs;

const DOT_ENVR_TOML: &str = ".envr.toml";

fn prepare_node_version(root: &std::path::Path, version: &str) {
    let bin = root.join("runtimes").join("node").join("versions").join(version).join("bin");
    fs::create_dir_all(&bin).expect("mkdir node bin");
    #[cfg(windows)]
    fs::write(bin.join("node.exe"), []).expect("touch node.exe");
    #[cfg(not(windows))]
    fs::write(bin.join("node"), []).expect("touch node");
}

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
    assert!(text.contains("22.1.0") && text.contains("[runtimes.node]"), "unexpected toml:\n{text}");
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
    fs::write(tmp.path().join(".tool-versions"), "nodejs 22.11.0\npython 3.12.7\ngolang 1.23.2\n").expect("write tool versions");

    Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .current_dir(tmp.path())
        .args(["import", "--config-format", "tool-versions"])
        .assert()
        .success();

    let text = fs::read_to_string(tmp.path().join(DOT_ENVR_TOML)).expect("read envr toml");
    assert!(text.contains("[runtimes.node]"), "unexpected toml:\n{text}");
    assert!(text.contains("version = \"22.11.0\""), "unexpected toml:\n{text}");
    assert!(text.contains("[runtimes.python]"), "unexpected toml:\n{text}");
    assert!(text.contains("[runtimes.go]"), "unexpected toml:\n{text}");
}

#[test]
fn export_tool_versions_uses_asdf_names() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join(DOT_ENVR_TOML), "\n[runtimes.node]\nversion = \"22.11.0\"\n\n[runtimes.go]\nversion = \"1.23.2\"\n").expect("write envr toml");

    Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .current_dir(tmp.path())
        .args(["export", "--config-format", "tool-versions", "--output", ".tool-versions"])
        .assert()
        .success();

    let text = fs::read_to_string(tmp.path().join(".tool-versions")).expect("read tool versions");
    assert!(text.contains("nodejs 22.11.0\n"), "unexpected export:\n{text}");
    assert!(text.contains("golang 1.23.2\n"), "unexpected export:\n{text}");
}

#[test]
fn import_tool_versions_records_compat_names() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join(".tool-versions"), "nodejs 22.11.0\ncustom-tool 1.2.3\n").expect("write tool versions");

    Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .current_dir(tmp.path())
        .args(["import", "--config-format", "tool-versions"])
        .assert()
        .success();

    let text = fs::read_to_string(tmp.path().join(DOT_ENVR_TOML)).expect("read envr toml");
    assert!(text.contains("[compat.asdf.names]"), "unexpected toml:\n{text}");
    assert!(text.contains("nodejs = \"node\""), "unexpected toml:\n{text}");
    assert!(!text.contains("custom-tool ="), "unexpected toml:\n{text}");
}

#[test]
fn project_lock_creates_locked_file_and_sync_accepts_it() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join(DOT_ENVR_TOML), "\n[runtimes.node]\nversion = \"22.11.0\"\n").expect("write envr toml");
    prepare_node_version(tmp.path(), "22.11.0");

    Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .current_dir(tmp.path())
        .args(["project", "lock"])
        .assert()
        .success();

    let lock_path = tmp.path().join(".envr.lock");
    assert!(lock_path.is_file(), "lockfile should exist");
    let lock_text = fs::read_to_string(&lock_path).expect("read lockfile");
    assert!(lock_text.contains("[[runtime]]"), "unexpected lockfile:\n{lock_text}");
    assert!(lock_text.contains("name = \"node\""), "unexpected lockfile:\n{lock_text}");
    assert!(lock_text.contains("request = \"22.11.0\""), "unexpected lockfile:\n{lock_text}");
    assert!(lock_text.contains("resolved = \"22.11.0\""), "unexpected lockfile:\n{lock_text}");
    assert!(lock_text.contains("source = \"resolved\""), "unexpected lockfile:\n{lock_text}");
    assert!(lock_text.contains("resolved_home"), "unexpected lockfile:\n{lock_text}");

    Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .current_dir(tmp.path())
        .args(["project", "sync", "--locked"])
        .assert()
        .success();
}

#[test]
fn project_sync_locked_rejects_stale_lock() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join(DOT_ENVR_TOML), "\n[runtimes.node]\nversion = \"22.11.0\"\n").expect("write envr toml");
    prepare_node_version(tmp.path(), "22.11.0");
    fs::write(tmp.path().join(".envr.lock"), "\nversion = 1\n\n[[runtime]]\nname = \"node\"\nrequest = \"22.11.0\"\nresolved = \"22.10.0\"\nsource = \"resolved\"\ncandidate_count = 1\n").expect("write stale lock");

    Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .current_dir(tmp.path())
        .args(["project", "sync", "--locked"])
        .assert()
        .failure();
}

#[test]
fn project_lock_alt_file_is_accepted() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join(DOT_ENVR_TOML), "\n[runtimes.node]\nversion = \"22.11.0\"\n").expect("write envr toml");
    prepare_node_version(tmp.path(), "22.11.0");
    fs::write(tmp.path().join(".envr.lock.toml"), "\nversion = 1\n\n[[runtime]]\nname = \"node\"\nrequest = \"22.11.0\"\nresolved = \"22.11.0\"\nsource = \"resolved\"\ncandidate_count = 1\n").expect("write alt lock");

    Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .current_dir(tmp.path())
        .args(["project", "sync", "--locked"])
        .assert()
        .success();
}

#[test]
fn project_lock_dry_run_does_not_write_files() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join(DOT_ENVR_TOML), "\n[runtimes.node]\nversion = \"22.11.0\"\n").expect("write envr toml");
    prepare_node_version(tmp.path(), "22.11.0");

    Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .current_dir(tmp.path())
        .args(["project", "lock", "--dry-run"])
        .assert()
        .success();

    assert!(!tmp.path().join(".envr.lock").exists(), "dry run should not create lockfile");
    assert!(!tmp.path().join(".envr.lock.toml").exists(), "dry run should not create alt lockfile");
}

#[test]
fn project_sync_locked_json_reports_fresh_lock() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join(DOT_ENVR_TOML), "\n[runtimes.node]\nversion = \"22.11.0\"\n").expect("write envr toml");
    prepare_node_version(tmp.path(), "22.11.0");
    Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .current_dir(tmp.path())
        .args(["project", "lock"])
        .assert()
        .success();

    let out = Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .current_dir(tmp.path())
        .args(["--format", "json", "project", "sync", "--locked"])
        .output()
        .expect("run");
    assert!(out.status.success(), "stderr={}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value = serde_json::from_str(stdout.lines().find(|l| l.trim_start().starts_with('{')).expect("json line")).expect("parse json");
    assert_eq!(v["data"]["lock_status"]["fresh"], true, "expected fresh lock status in json: {stdout}");
    assert!(v["data"]["lock_status"]["path"].is_string(), "expected lock path in json: {stdout}");
}

#[test]
fn project_validate_locked_json_reports_fresh_lock() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join(DOT_ENVR_TOML), "\n[runtimes.node]\nversion = \"22.11.0\"\n").expect("write envr toml");
    prepare_node_version(tmp.path(), "22.11.0");
    Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .current_dir(tmp.path())
        .args(["project", "lock"])
        .assert()
        .success();

    let out = Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .current_dir(tmp.path())
        .args(["--format", "json", "project", "validate", "--locked"])
        .output()
        .expect("run");
    assert!(out.status.success(), "stderr={}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value = serde_json::from_str(stdout.lines().find(|l| l.trim_start().starts_with('{')).expect("json line")).expect("parse json");
    assert_eq!(v["data"]["lock_status"]["fresh"], true, "expected fresh lock status in json: {stdout}");
    assert!(v["data"]["lock_status"]["path"].is_string(), "expected lock path in json: {stdout}");
}

#[test]
fn project_validate_locked_human_reports_lock_status() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join(DOT_ENVR_TOML), "\n[runtimes.node]\nversion = \"22.11.0\"\n").expect("write envr toml");
    prepare_node_version(tmp.path(), "22.11.0");
    Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .current_dir(tmp.path())
        .args(["project", "lock"])
        .assert()
        .success();

    let out = Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .current_dir(tmp.path())
        .args(["project", "validate", "--locked"])
        .output()
        .expect("run");
    assert!(out.status.success(), "stderr={}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("项目校验通过") || stdout.contains("project validation ok"), "expected human validate output to mention success: {stdout}");
}

#[test]
fn project_validate_json_reports_absent_or_stale_lock() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join(DOT_ENVR_TOML), "\n[runtimes.node]\nversion = \"22.11.0\"\n").expect("write envr toml");
    prepare_node_version(tmp.path(), "22.11.0");

    let out = Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .current_dir(tmp.path())
        .args(["--format", "json", "project", "validate"])
        .output()
        .expect("run");
    assert!(out.status.success(), "stderr={}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value = serde_json::from_str(stdout.lines().find(|l| l.trim_start().starts_with('{')).expect("json line")).expect("parse json");
    assert!(v["data"]["lock"].is_null() || v["data"]["lock"]["fresh"] == false, "expected unlocked lock status in json: {stdout}");
}

#[test]
fn project_validate_check_remote_reports_warnings() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join(DOT_ENVR_TOML), "\n[runtimes.node]\nversion = \"22.11.0\"\n").expect("write envr toml");
    prepare_node_version(tmp.path(), "22.11.0");

    let out = Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .current_dir(tmp.path())
        .args(["--format", "json", "project", "validate", "--check-remote"])
        .output()
        .expect("run");
    assert!(out.status.success(), "stderr={}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value = serde_json::from_str(stdout.lines().find(|l| l.trim_start().starts_with('{')).expect("json line")).expect("parse json");
    assert!(v["data"]["remote_warnings"].is_array(), "expected remote warnings in json: {stdout}");
}

#[test]
fn project_validate_check_remote_human_reports_warning_hint() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(tmp.path().join(DOT_ENVR_TOML), "\n[runtimes.node]\nversion = \"22.11.0\"\n").expect("write envr toml");
    prepare_node_version(tmp.path(), "22.11.0");

    let out = Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .current_dir(tmp.path())
        .args(["project", "validate", "--check-remote"])
        .output()
        .expect("run");
    assert!(out.status.success(), "stderr={}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("项目校验通过") || stdout.contains("project validation ok"), "expected human validation success output: {stdout}");
}
