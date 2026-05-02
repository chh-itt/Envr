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

#[test]
fn project_lock_creates_locked_file_and_sync_accepts_it() {
    let tmp = tempfile::tempdir().expect("tempdir");
    fs::write(
        tmp.path().join(DOT_ENVR_TOML),
        r#"
[runtimes.node]
version = "22.11.0"
"#,
    )
    .expect("write envr toml");

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
    assert!(lock_text.contains("[project.runtimes.node]"), "unexpected lockfile:\n{lock_text}");
    assert!(lock_text.contains("version = \"22.11.0\""), "unexpected lockfile:\n{lock_text}");

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
    fs::write(
        tmp.path().join(DOT_ENVR_TOML),
        r#"
[runtimes.node]
version = "22.11.0"
"#,
    )
    .expect("write envr toml");
    fs::write(
        tmp.path().join(".envr.lock"),
        r#"
version = 1

[[runtime]]
name = "node"
request = "22.11.0"
resolved = "22.10.0"
source = "resolved"
candidate_count = 1
"#,
    )
    .expect("write stale lock");

    Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .current_dir(tmp.path())
        .args(["project", "sync", "--locked"])
        .assert()
        .failure();
}
+
+#[test]
+fn project_lock_alt_file_is_accepted() {
+    let tmp = tempfile::tempdir().expect("tempdir");
+    fs::write(
+        tmp.path().join(DOT_ENVR_TOML),
+        r#"
[runtimes.node]
version = "22.11.0"
"#,
+    )
+    .expect("write envr toml");
+    fs::write(
+        tmp.path().join(".envr.lock.toml"),
+        r#"
version = 1

[[runtime]]
name = "node"
request = "22.11.0"
resolved = "22.11.0"
source = "resolved"
candidate_count = 1
"#,
+    )
+    .expect("write alt lock");
+
+    Command::cargo_bin("envr")
+        .expect("envr")
+        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
+        .current_dir(tmp.path())
+        .args(["project", "sync", "--locked"])
+        .assert()
+        .success();
+}
