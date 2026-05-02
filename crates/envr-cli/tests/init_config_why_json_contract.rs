//! Additional Phase A coverage: init/config-validate/why JSON + offline-safe behavior.

use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use std::path::Path;

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

fn write_settings(root: &Path) {
    let cfg = root.join("config");
    fs::create_dir_all(&cfg).expect("mkdir config");
    fs::write(cfg.join("settings.toml"), "[i18n]\nlocale = \"en_us\"\n").expect("settings");
}

fn write_node_layout(runtime_root: &Path, version: &str) {
    let ver = runtime_root.join("runtimes/node/versions").join(version);
    let bin = ver.join("bin");
    fs::create_dir_all(&bin).expect("create node bin");
    #[cfg(windows)]
    fs::write(bin.join("node.exe"), []).expect("touch node.exe");
    #[cfg(not(windows))]
    fs::write(bin.join("node"), []).expect("touch node");
}

#[test]
fn init_json_emits_project_config_init_and_writes_file() {
    let root = tempfile::tempdir().expect("tmp");
    write_settings(root.path());
    let project = root.path().join("proj");
    fs::create_dir_all(&project).expect("project");

    let out = Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_ROOT", root.path())
        .args([
            "--format",
            "json",
            "init",
            "--path",
            &project.to_string_lossy(),
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
    assert_eq!(v["code"], "project_config_init", "{v}");
    assert_eq!(v["data"]["interactive"], false, "{v}");
    assert!(v["data"]["path"].is_string(), "{v}");
    assert!(
        project.join(".envr.toml").is_file(),
        "init should write .envr.toml"
    );
}

#[test]
fn config_validate_json_emits_config_validate_ok_offline() {
    let root = tempfile::tempdir().expect("tmp");
    write_settings(root.path());

    let out = Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_ROOT", root.path())
        .args(["--format", "json", "config", "validate"])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let v = parse_json_line(&out.stdout);
    assert_eq!(v["success"], true, "{v}");
    assert_eq!(v["code"], "config_validate_ok", "{v}");
    assert_eq!(v["data"]["valid"], true, "{v}");
    assert!(v["data"]["path"].is_string(), "{v}");
}

#[test]
fn why_json_reports_project_pin_resolution() {
    let root = tempfile::tempdir().expect("tmp");
    write_settings(root.path());
    let runtime_root = root.path().join("runtime-root");
    let project = root.path().join("project");
    fs::create_dir_all(&project).expect("project");
    write_node_layout(&runtime_root, "20.10.0");
    fs::write(
        project.join(".envr.toml"),
        "[runtimes.node]\nversion = \"20.10.0\"\n",
    )
    .expect("envr.toml");

    let out = Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_ROOT", root.path())
        .env("ENVR_RUNTIME_ROOT", runtime_root.as_os_str())
        .current_dir(&project)
        .args(["--format", "json", "why", "node"])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let v = parse_json_line(&out.stdout);
    assert_eq!(v["success"], true, "{v}");
    assert_eq!(v["code"], "why_runtime", "{v}");
    assert_eq!(v["data"]["lang"], "node", "{v}");
    assert_eq!(v["data"]["resolution"], "project_pin", "{v}");
    assert_eq!(v["data"]["project"]["pin"], "20.10.0", "{v}");
    assert_eq!(v["data"]["request_source"], "project", "{v}");
    assert_eq!(v["data"]["request_kind"], "exact", "{v}");
    assert!(
        v["data"]["resolved_home"]
            .as_str()
            .is_some_and(|s| s.contains("20.10.0")),
        "resolved_home should point to pinned version: {v}"
    );
}

#[test]
fn why_json_reports_tool_versions_compat_resolution() {
    let root = tempfile::tempdir().expect("tmp");
    write_settings(root.path());
    let runtime_root = root.path().join("runtime-root");
    let project = root.path().join("project");
    fs::create_dir_all(&project).expect("project");
    write_node_layout(&runtime_root, "22.11.0");
    fs::write(project.join(".tool-versions"), "nodejs 22.11.0\n").expect("tool-versions");

    let out = Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_ROOT", root.path())
        .env("ENVR_RUNTIME_ROOT", runtime_root.as_os_str())
        .current_dir(&project)
        .args(["--format", "json", "why", "node"])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let v = parse_json_line(&out.stdout);
    assert_eq!(v["success"], true, "{v}");
    assert_eq!(v["code"], "why_runtime", "{v}");
    assert_eq!(v["data"]["compat_source"], "nodejs", "{v}");
    assert_eq!(v["data"]["request_source"], "tool_versions_compat", "{v}");
    assert_eq!(v["data"]["resolution"], "tool_versions_compat", "{v}");
    assert_eq!(v["data"]["request_kind"], "exact", "{v}");
    assert_eq!(v["data"]["request_normalized"], Value::Null, "{v}");
    assert!(
        v["data"]["project"]["compat_asdf_names"]
            .as_array()
            .is_some_and(|a| !a.is_empty()),
        "compat mapping should be present: {v}"
    );
}

#[test]
fn why_json_reports_version_request_normalization() {
    let root = tempfile::tempdir().expect("tmp");
    write_settings(root.path());
    let runtime_root = root.path().join("runtime-root");
    let project = root.path().join("project");
    fs::create_dir_all(&project).expect("project");
    write_node_layout(&runtime_root, "22.11.0");
    fs::write(project.join(".envr.toml"), "[runtimes.node]\nversion = \"v22.11.0\"\n").expect("envr.toml");

    let out = Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_ROOT", root.path())
        .env("ENVR_RUNTIME_ROOT", runtime_root.as_os_str())
        .current_dir(&project)
        .args(["--format", "json", "why", "node"])
        .output()
        .expect("run");
    assert!(out.status.success(), "stderr={}", String::from_utf8_lossy(&out.stderr));

    let v = parse_json_line(&out.stdout);
    assert_eq!(v["success"], true, "{v}");
    assert_eq!(v["data"]["request_source"], "project", "{v}");
    assert_eq!(v["data"]["request_kind"], "exact", "{v}");
    assert_eq!(v["data"]["request_normalized"], "22.11.0", "{v}");
    assert_eq!(v["data"]["resolved_version"], "22.11.0", "{v}");
}

#[test]
fn why_human_reports_request_alias() {
    let root = tempfile::tempdir().expect("tmp");
    write_settings(root.path());
    let runtime_root = root.path().join("runtime-root");
    let project = root.path().join("project");
    fs::create_dir_all(&project).expect("project");
    write_node_layout(&runtime_root, "22.11.0");
    fs::write(project.join(".envr.toml"), "[runtimes.node]\nversion = \"latest\"\n").expect("envr.toml");

    let out = Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_ROOT", root.path())
        .env("ENVR_RUNTIME_ROOT", runtime_root.as_os_str())
        .current_dir(&project)
        .arg("why")
        .arg("node")
        .output()
        .expect("run");
    assert!(out.status.success(), "stderr={}", String::from_utf8_lossy(&out.stderr));

    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("请求类型： alias") || text.contains("Request kind: alias"), "{text}");
    assert!(text.contains("请求别名： latest") || text.contains("Request alias: latest"), "{text}");
}
*** End Patch