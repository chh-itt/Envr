//! JSON responses use a single envelope per `refactor docs/02-cli-设计.md`.

use assert_cmd::Command;
use serde_json::Value;
use std::fs;

fn parse_json_line(stdout: &[u8]) -> Value {
    for line in stdout.split(|b| *b == b'\n') {
        if line.is_empty() {
            continue;
        }
        if line.first() == Some(&b'{')
            && let Ok(v) = serde_json::from_slice::<Value>(line)
        {
            return v;
        }
    }
    panic!(
        "no json object in stdout: {}",
        String::from_utf8_lossy(stdout)
    );
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

fn write_node_layout(runtime_root: &std::path::Path, version: &str) {
    use std::fs;
    let ver = runtime_root.join("runtimes/node/versions").join(version);
    let bin = ver.join("bin");
    fs::create_dir_all(&bin).expect("bin");
    #[cfg(windows)]
    fs::write(bin.join("node.exe"), []).expect("node.exe");
    #[cfg(not(windows))]
    fs::write(bin.join("node"), []).expect("node");
}

fn narrow_path_for_envr_process() -> String {
    #[cfg(windows)]
    {
        let root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_string());
        format!("{root}\\System32;{root}")
    }
    #[cfg(not(windows))]
    {
        "/usr/bin:/bin".to_string()
    }
}

fn assert_envelope_shape(v: &Value) {
    assert_eq!(
        v.get("schema_version"),
        Some(&serde_json::json!(3)),
        "missing or wrong schema_version: {v}"
    );
    assert!(v.get("success").is_some(), "missing success: {v}");
    assert!(v.get("code").is_some(), "missing code: {v}");
    assert!(v.get("message").is_some(), "missing message: {v}");
    assert!(v.get("data").is_some(), "missing data: {v}");
    assert!(v.get("diagnostics").is_some(), "missing diagnostics: {v}");
}

#[test]
fn list_json_contract() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let v = json_stdout(&["--format", "json", "list"], tmp.path());
    assert_envelope_shape(&v);
    assert_eq!(v["success"], true);
    assert_eq!(v["code"], "list_installed");
    let runtimes = v["data"]["installed_runtimes"]
        .as_array()
        .expect("installed_runtimes array");
    assert!(!runtimes.is_empty(), "expected at least one runtime row");
    let row = &runtimes[0];
    assert!(row["kind"].is_string());
    let vers = row["versions"].as_array().expect("versions");
    if let Some(v0) = vers.first() {
        assert!(
            v0["version"].is_string(),
            "list versions[] entries are objects with version: {v0}"
        );
    }
}

#[test]
fn current_json_contract() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let v = json_stdout(&["--format", "json", "current"], tmp.path());
    assert_envelope_shape(&v);
    assert_eq!(v["success"], true);
    assert_eq!(v["code"], "show_current");
    let runtimes = v["data"]["active_versions"]
        .as_array()
        .expect("active_versions");
    assert!(!runtimes.is_empty());
    let row = &runtimes[0];
    assert!(row["kind"].is_string());
    assert!(
        row["version"].is_string() || row["version"].is_null(),
        "version string or null: {:?}",
        row["version"]
    );
    assert!(
        row["hint"].is_string() || row["hint"].is_null(),
        "hint string or null: {:?}",
        row["hint"]
    );
}

#[test]
fn doctor_json_contract() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let v = json_stdout(&["--format", "json", "doctor"], tmp.path());
    assert_envelope_shape(&v);
    assert_eq!(v["success"], true);
    assert_eq!(v["code"], "doctor_ok");
    let kinds = v["data"]["kinds"].as_array().expect("kinds");
    assert!(!kinds.is_empty());
    let row = &kinds[0];
    assert!(row["kind"].is_string());
    assert!(
        row.get("current").is_none(),
        "use current_version, not current"
    );
    assert!(
        row["current_version"].is_string() || row["current_version"].is_null(),
        "current_version: {:?}",
        row.get("current_version")
    );
    let d = &v["data"];
    assert!(
        d["warnings"].is_array(),
        "warnings: {:?}",
        d.get("warnings")
    );
    assert!(d["notes"].is_array(), "notes: {:?}", d.get("notes"));
    assert!(d["recommendations"].is_array());
    assert!(
        d["path_shadowing"].is_null() || d["path_shadowing"].is_object(),
        "path_shadowing: {:?}",
        d.get("path_shadowing")
    );
    assert!(
        d["path_conflicts"].is_array(),
        "path_conflicts: {:?}",
        d.get("path_conflicts")
    );
    assert!(
        d["findings"].is_array(),
        "findings: {:?}",
        d.get("findings")
    );
    assert!(
        d["path_analysis"].is_null() || d["path_analysis"].is_object(),
        "path_analysis: {:?}",
        d.get("path_analysis")
    );
    assert!(
        d["shims_dir_writable"].is_boolean() || d["shims_dir_writable"].is_null(),
        "shims_dir_writable: {:?}",
        d.get("shims_dir_writable")
    );
    let oc = d["onboarding_checklist"]
        .as_array()
        .expect("onboarding_checklist");
    assert_eq!(oc.len(), 4, "onboarding_checklist: {oc:?}");
    assert!(oc[0].is_string());
}

#[test]
fn doctor_json_flag_matches_doctor_format_json() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let v = json_stdout(&["doctor", "--json"], tmp.path());
    assert_envelope_shape(&v);
    assert_eq!(v["success"], true);
    assert_eq!(v["code"], "doctor_ok");
    assert!(v["data"]["kinds"].is_array());
}

#[test]
fn deactivate_json_contract() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let v = json_stdout(&["--format", "json", "deactivate"], tmp.path());
    assert_envelope_shape(&v);
    assert_eq!(v["success"], true);
    assert_eq!(v["code"], "deactivate_hint");
    let d = &v["data"];
    assert_eq!(d["hint"], "hook_shell_only");
    assert!(d["docs"].is_string());
}

#[test]
fn status_json_contract() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let v = json_stdout(&["--format", "json", "status"], tmp.path());
    assert_envelope_shape(&v);
    assert_eq!(v["success"], true);
    assert_eq!(v["code"], "project_status");
    let d = &v["data"];
    assert!(d["working_dir"].is_string());
    assert!(d["runtimes"].is_array());
}

#[test]
fn hook_prompt_json_contract() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let v = json_stdout(&["--format", "json", "hook", "prompt"], tmp.path());
    assert_envelope_shape(&v);
    assert_eq!(v["success"], true);
    assert_eq!(v["code"], "hook_prompt");
    assert!(v["data"]["segment"].is_string());
}

#[test]
fn validation_error_json_has_code() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = Command::cargo_bin("envr")
        .expect("envr binary")
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .args(["--format", "json", "list", "not-a-lang"])
        .output()
        .expect("run");
    assert!(!out.status.success());
    let v = parse_json_line(&out.stdout);
    assert_envelope_shape(&v);
    assert_eq!(v["success"], false);
    assert_eq!(v["code"], "validation");
}

#[test]
fn current_invalid_runtime_json_validation_envelope() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = Command::cargo_bin("envr")
        .expect("envr binary")
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .args(["--format", "json", "current", "not-a-lang"])
        .output()
        .expect("run");
    assert!(!out.status.success());
    let v = parse_json_line(&out.stdout);
    assert_envelope_shape(&v);
    assert_eq!(v["success"], false);
    assert_eq!(v["code"], "validation");
}

#[test]
fn quiet_validation_json_message_is_bracket_tag_only() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = Command::cargo_bin("envr")
        .expect("envr binary")
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .args(["--quiet", "--format", "json", "which"])
        .output()
        .expect("run");
    assert!(!out.status.success());
    let v = parse_json_line(&out.stdout);
    assert_envelope_shape(&v);
    assert_eq!(v["success"], false);
    assert_eq!(v["code"], "validation");
    assert_eq!(v["message"], "[E_VALIDATION]");
    assert!(v["diagnostics"].as_array().is_some_and(|a| a.is_empty()));
}

#[test]
fn exec_dry_run_json_envelope_message_dry_run() {
    let tmp = tempfile::tempdir().expect("tmp");
    let runtime_root = tmp.path().join("runtime-root");
    let project = tmp.path().join("project");
    std::fs::create_dir_all(&project).expect("project");
    write_node_layout(&runtime_root, "20.10.0");
    std::fs::write(
        project.join(".envr.toml"),
        "[runtimes.node]\nversion = \"20.10.0\"\n",
    )
    .expect("envr.toml");

    let args: &[&str] = if cfg!(windows) {
        &[
            "--format",
            "json",
            "exec",
            "--lang",
            "node",
            "--dry-run",
            "cmd",
            "/c",
            "echo",
            "x",
        ]
    } else {
        &[
            "--format",
            "json",
            "exec",
            "--lang",
            "node",
            "--dry-run",
            "true",
        ]
    };

    let out = Command::cargo_bin("envr")
        .expect("envr binary")
        .current_dir(&project)
        .env("ENVR_RUNTIME_ROOT", runtime_root.as_os_str())
        .env("PATH", narrow_path_for_envr_process())
        .args(args)
        .output()
        .expect("exec dry-run");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_line(&out.stdout);
    assert_envelope_shape(&v);
    assert_eq!(v["code"], "dry_run");
    assert!(v["data"]["env"].is_object());
    assert!(v["data"]["command"].is_string());
}

#[test]
fn template_json_contract() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let tpl = tmp.path().join("x.tpl");
    fs::write(&tpl, r#"{"k":"${PATH}"}"#).expect("write tpl");
    let p = tpl.to_string_lossy();
    let v = json_stdout(&["--format", "json", "template", p.as_ref()], tmp.path());
    assert_envelope_shape(&v);
    assert_eq!(v["code"], "template_rendered");
    assert!(v["data"]["file"].is_string());
    assert!(v["data"]["rendered"].is_string());
}

#[test]
fn update_json_contract() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let v = json_stdout(&["--format", "json", "update"], tmp.path());
    assert_envelope_shape(&v);
    assert_eq!(v["code"], "update_info");
    assert!(v["data"]["version"].is_string());
    assert!(v["data"]["check_requested"].is_boolean());
    assert!(v["data"]["self_update"].is_string());
}

#[test]
fn shim_sync_json_contract() {
    let tmp = tempfile::tempdir().expect("tmp");
    let runtime_root = tmp.path().join("runtime-root");
    std::fs::create_dir_all(&runtime_root).expect("runtime-root");
    let v = json_stdout(&["--format", "json", "shim", "sync"], &runtime_root);
    assert_envelope_shape(&v);
    assert_eq!(v["code"], "shims_synced");
    let d = &v["data"];
    assert!(d["runtime_root"].is_string());
    assert!(d["ensured_core_kinds"].is_array());
    assert!(d["globals_synced"].is_boolean());
}

#[test]
fn project_sync_json_contract() {
    let tmp = tempfile::tempdir().expect("tmp");
    let runtime_root = tmp.path().join("runtime-root");
    let project = tmp.path().join("project");
    std::fs::create_dir_all(&project).expect("project");
    write_node_layout(&runtime_root, "20.10.0");
    std::fs::write(
        project.join(".envr.toml"),
        "[runtimes.node]\nversion = \"20.10.0\"\n",
    )
    .expect("envr.toml");
    let out = Command::cargo_bin("envr")
        .expect("envr binary")
        .current_dir(&project)
        .env("ENVR_RUNTIME_ROOT", runtime_root.as_os_str())
        .args(["--format", "json", "project", "sync"])
        .output()
        .expect("project sync");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_line(&out.stdout);
    assert_envelope_shape(&v);
    assert_eq!(v["code"], "project_synced");
    assert!(v["data"]["missing"].is_array() || v["data"]["missing_before"].is_array());
    assert!(v["data"]["installed"].is_array());
}

#[test]
fn config_edit_json_contract() {
    let tmp = tempfile::tempdir().expect("tmp");
    let envr_root = tmp.path().join("envr-home");
    std::fs::create_dir_all(&envr_root).expect("envr-home");
    let out = Command::cargo_bin("envr")
        .expect("envr binary")
        .env("ENVR_ROOT", envr_root.as_os_str())
        .env("EDITOR", "echo")
        .args(["--format", "json", "config", "edit"])
        .output()
        .expect("config edit");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_line(&out.stdout);
    assert_envelope_shape(&v);
    assert_eq!(v["code"], "config_edit_ok");
    assert!(v["data"]["path"].is_string());
}

#[test]
fn rust_install_managed_json_contract() {
    let tmp = tempfile::tempdir().expect("tmp");
    let runtime_root = tmp.path().join("runtime-root");
    std::fs::create_dir_all(&runtime_root).expect("runtime-root");
    let out = Command::cargo_bin("envr")
        .expect("envr binary")
        .env("ENVR_RUNTIME_ROOT", runtime_root.as_os_str())
        .env("ENVR_CLI_TEST_MOCK_RUST_INSTALL_MANAGED", "1")
        .args(["--format", "json", "rust", "install-managed"])
        .output()
        .expect("rust install-managed");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_line(&out.stdout);
    assert_envelope_shape(&v);
    assert_eq!(v["code"], "rust_managed_installed");
    assert_eq!(v["data"]["channel"], "stable");
}

#[test]
fn run_json_child_includes_install_metadata() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let runtime_root = tmp.path().join("runtime-root");
    let project = tmp.path().join("project");
    std::fs::create_dir_all(&project).expect("project");
    write_node_layout(&runtime_root, "20.10.0");
    std::fs::write(
        project.join(".envr.toml"),
        "[runtimes.node]\nversion = \"20.10.0\"\n",
    )
    .expect("envr.toml");
    let args: Vec<&str> = if cfg!(windows) {
        vec!["--format", "json", "run", "cmd", "/c", "echo", "ok"]
    } else {
        vec!["--format", "json", "run", "sh", "-c", "echo ok"]
    };
    let out = Command::cargo_bin("envr")
        .expect("envr binary")
        .current_dir(&project)
        .env("ENVR_RUNTIME_ROOT", runtime_root.as_os_str())
        .env("PATH", narrow_path_for_envr_process())
        .args(&args)
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_line(&out.stdout);
    assert_envelope_shape(&v);
    assert_eq!(v["code"], "child_completed");
    let d = &v["data"];
    assert_eq!(d["install_if_missing"], false);
    assert_eq!(d["dry_run"], false);
    assert_eq!(d["verbose"], false);
    let auto = d["auto_installed"].as_array().expect("auto_installed");
    assert!(auto.is_empty());
    assert!(d.get("lang").is_none(), "run must not set data.lang");
    assert!(d["env_files"].is_array());
    assert!(d["env_overrides"].is_array());
}
