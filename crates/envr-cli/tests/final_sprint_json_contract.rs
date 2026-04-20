//! Final sprint contracts: shell success/failure, diagnostics export JSON, config validate failure.

use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

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

fn write_settings(root: &Path, settings_body: &str) {
    let cfg = root.join("config");
    fs::create_dir_all(&cfg).expect("mkdir config");
    fs::write(cfg.join("settings.toml"), settings_body).expect("settings");
}

fn write_shell_script(dir: &Path, exit_code: i32) -> PathBuf {
    #[cfg(windows)]
    {
        let p = dir.join(format!("exit_{exit_code}.cmd"));
        let body = format!("@echo off\r\nexit /b {exit_code}\r\n");
        fs::write(&p, body).expect("write cmd script");
        p
    }
    #[cfg(not(windows))]
    {
        let p = dir.join(format!("exit_{exit_code}.sh"));
        let body = format!("#!/usr/bin/env sh\nexit {exit_code}\n");
        fs::write(&p, body).expect("write sh script");
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&p).expect("meta").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&p, perms).expect("chmod");
        p
    }
}

#[test]
fn shell_json_success_emits_shell_exited_envelope() {
    let root = tempfile::tempdir().expect("tmp");
    write_settings(root.path(), "[i18n]\nlocale = \"en_us\"\n");
    let project = root.path().join("proj");
    fs::create_dir_all(&project).expect("project");
    fs::write(project.join(".envr.toml"), "[env]\nFOO = \"bar\"\n").expect("envr.toml");
    let shell = write_shell_script(root.path(), 0);

    let out = Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_ROOT", root.path())
        .current_dir(&project)
        .args([
            "--format",
            "json",
            "shell",
            "--shell",
            shell.to_string_lossy().as_ref(),
        ])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_line(&out.stdout);
    assert_eq!(v["success"], true, "{v}");
    assert_eq!(v["code"], "shell_exited", "{v}");
    assert!(v["data"]["shell"].is_string(), "{v}");
    assert!(v["data"]["cwd"].is_string(), "{v}");
}

#[test]
fn shell_json_failure_emits_shell_exit_failure_code() {
    let root = tempfile::tempdir().expect("tmp");
    write_settings(root.path(), "[i18n]\nlocale = \"en_us\"\n");
    let project = root.path().join("proj");
    fs::create_dir_all(&project).expect("project");
    fs::write(project.join(".envr.toml"), "[env]\nFOO = \"bar\"\n").expect("envr.toml");
    let shell = write_shell_script(root.path(), 7);

    let out = Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_ROOT", root.path())
        .current_dir(&project)
        .args([
            "--format",
            "json",
            "shell",
            "--shell",
            shell.to_string_lossy().as_ref(),
        ])
        .output()
        .expect("run");
    assert!(
        !out.status.success(),
        "expected non-zero shell exit; stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_line(&out.stdout);
    assert_eq!(v["success"], false, "{v}");
    assert_eq!(v["code"], "shell_exit", "{v}");
    assert_eq!(v["data"]["exit_code"], 7, "{v}");
}

#[test]
fn diagnostics_export_json_emits_diagnostics_export_ok_with_path() {
    let root = tempfile::tempdir().expect("tmp");
    let runtime_root = root.path().join("rr");
    fs::create_dir_all(&runtime_root).expect("runtime root");
    let zip_path = root.path().join("diag.zip");

    let out = Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_RUNTIME_ROOT", runtime_root.as_os_str())
        .args([
            "--format",
            "json",
            "diagnostics",
            "export",
            "--output",
            zip_path.to_string_lossy().as_ref(),
        ])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_line(&out.stdout);
    assert_eq!(v["success"], true, "{v}");
    assert_eq!(v["code"], "diagnostics_export_ok", "{v}");
    assert!(v["data"]["path"].is_string(), "{v}");
    assert!(zip_path.is_file(), "diagnostics export should write zip");
}

#[test]
fn config_validate_json_invalid_settings_emits_config_failure() {
    let root = tempfile::tempdir().expect("tmp");
    write_settings(root.path(), "this-is-not-valid-toml = [");

    let out = Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_ROOT", root.path())
        .args(["--format", "json", "config", "validate"])
        .output()
        .expect("run");
    assert!(
        !out.status.success(),
        "expected invalid settings to fail; stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_line(&out.stdout);
    assert_eq!(v["success"], false, "{v}");
    assert_eq!(v["code"], "validation", "{v}");
    assert!(v["message"].is_string(), "{v}");
}
