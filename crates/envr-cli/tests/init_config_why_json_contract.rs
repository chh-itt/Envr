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
    assert!(out.status.success(), "stderr={}", String::from_utf8_lossy(&out.stderr));

    let v = parse_json_line(&out.stdout);
    assert_eq!(v["success"], true, "{v}");
    assert_eq!(v["code"], "project_config_init", "{v}");
    assert_eq!(v["data"]["interactive"], false, "{v}");
    assert!(v["data"]["path"].is_string(), "{v}");
    assert!(project.join(".envr.toml").is_file(), "init should write .envr.toml");
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
    assert!(out.status.success(), "stderr={}", String::from_utf8_lossy(&out.stderr));

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
    assert!(out.status.success(), "stderr={}", String::from_utf8_lossy(&out.stderr));

    let v = parse_json_line(&out.stdout);
    assert_eq!(v["success"], true, "{v}");
    assert_eq!(v["code"], "why_runtime", "{v}");
    assert_eq!(v["data"]["lang"], "node", "{v}");
    assert_eq!(v["data"]["resolution_source"], "project_pin", "{v}");
    assert_eq!(v["data"]["resolution_reason"], "resolved from project runtime pin", "{v}");
    assert_eq!(v["data"]["project"]["pin"], "20.10.0", "{v}");
    assert!(v["data"]["project"]["lock_status"].is_null(), "{v}");
    assert_eq!(v["data"]["request_source"], "project", "{v}");
    assert_eq!(v["data"]["request_kind"], "exact", "{v}");
    assert_eq!(v["data"]["request_normalized"], "20.10.0", "{v}");
    assert_eq!(v["data"]["request_explanation"], "exact version requested", "{v}");
    assert!(v["data"]["resolved_home"].as_str().is_some_and(|s| s.contains("20.10.0")), "resolved_home should point to pinned version: {v}");
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
    assert!(out.status.success(), "stderr={}", String::from_utf8_lossy(&out.stderr));

    let v = parse_json_line(&out.stdout);
    assert_eq!(v["success"], true, "{v}");
    assert_eq!(v["code"], "why_runtime", "{v}");
    assert_eq!(v["data"]["compat_source"], "nodejs", "{v}");
    assert_eq!(v["data"]["request_source"], "tool_versions_compat", "{v}");
    assert_eq!(v["data"]["resolution_source"], "tool_versions_compat", "{v}");
    assert_eq!(v["data"]["resolution_reason"], "resolved via .tool-versions compatibility mapping", "{v}");
    assert_eq!(v["data"]["request_kind"], "exact", "{v}");
    assert_eq!(v["data"]["request_normalized"], "22.11.0", "{v}");
    assert_eq!(v["data"]["request_alias"], Value::Null, "{v}");
    assert!(v["data"]["project"]["compat_asdf_names"].as_object().is_some_and(|o| !o.is_empty()), "compat mapping should be present: {v}");
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
    assert_eq!(v["data"]["request_alias"], Value::Null, "{v}");
    assert_eq!(v["data"]["request_explanation"], "exact version requested", "{v}");
}

#[test]
fn why_json_reports_alias_explanation() {
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
        .args(["--format", "json", "why", "node"])
        .output()
        .expect("run");
    assert!(out.status.success(), "stderr={}", String::from_utf8_lossy(&out.stderr));
    let v = parse_json_line(&out.stdout);
    assert_eq!(v["data"]["request_kind"], "alias", "{v}");
    assert_eq!(v["data"]["request_explanation"], "latest alias resolved by runtime policy", "{v}");
    assert_eq!(v["data"]["request_alias"], "latest", "{v}");
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
    assert!(text.contains("Request kind: alias") || text.contains("请求类型： alias"), "{text}");
    assert!(text.contains("Request alias: latest") || text.contains("请求别名： latest"), "{text}");
    assert!(text.contains("Request explanation: latest alias resolved by runtime policy") || text.contains("请求说明： latest alias resolved by runtime policy"), "{text}");
}

#[test]
fn why_human_reports_alias_explanation() {
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
    assert!(text.contains("Request kind: alias") || text.contains("请求类型： alias"), "{text}");
    assert!(text.contains("Request alias: latest") || text.contains("请求别名： latest"), "{text}");
    assert!(text.contains("Request explanation: latest alias resolved by runtime policy") || text.contains("请求说明： latest alias resolved by runtime policy"), "{text}");
}

#[test]
fn why_json_reports_cli_spec_override_over_project_pin() {
    let root = tempfile::tempdir().expect("tmp");
    write_settings(root.path());
    let runtime_root = root.path().join("runtime-root");
    let project = root.path().join("project");
    fs::create_dir_all(&project).expect("project");
    write_node_layout(&runtime_root, "22.11.0");
    write_node_layout(&runtime_root, "20.10.0");
    fs::write(project.join(".envr.toml"), "[runtimes.node]\nversion = \"20.10.0\"\n").expect("envr.toml");

    let out = Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_ROOT", root.path())
        .env("ENVR_RUNTIME_ROOT", runtime_root.as_os_str())
        .current_dir(&project)
        .args(["--format", "json", "why", "node", "--spec", "22.11.0"])
        .output()
        .expect("run");
    assert!(out.status.success(), "stderr={}", String::from_utf8_lossy(&out.stderr));

    let v = parse_json_line(&out.stdout);
    assert_eq!(v["success"], true, "{v}");
    assert_eq!(v["data"]["request_source"], "cli", "{v}");
    assert_eq!(v["data"]["resolution_source"], "spec_override", "{v}");
    assert_eq!(v["data"]["spec_override"], "22.11.0", "{v}");
    assert_eq!(v["data"]["project"]["pin"], "20.10.0", "{v}");
    assert!(v["data"]["resolved_home"].as_str().is_some_and(|s| s.contains("22.11.0")), "resolved_home should follow cli spec override: {v}");
    assert!(v["data"]["candidate_note"].as_str().is_some_and(|s| s.contains("runtime-specific resolver")), "{v}");
}

#[test]
fn why_human_reports_lockfile_notice_without_project_pin() {
    let root = tempfile::tempdir().expect("tmp");
    write_settings(root.path());
    let runtime_root = root.path().join("runtime-root");
    let project = root.path().join("project");
    fs::create_dir_all(&project).expect("project");
    write_node_layout(&runtime_root, "22.11.0");
    fs::create_dir_all(runtime_root.join("runtimes/node/current")).expect("create current dir");
    fs::write(
        runtime_root.join("runtimes/node/current").join("version"),
        "22.11.0",
    )
    .expect("write current marker");
    fs::write(project.join(".envr.toml"), "[runtimes.python]\nversion = \"3.12.7\"\n").expect("unrelated project pin");
    fs::write(project.join(".envr.lock"), "version = 1\n\n[[runtime]]\nname = \"node\"\nrequest = \"22.11.0\"\nresolved = \"22.11.0\"\nsource = \"resolved\"\ncandidate_count = 1\n").expect("lockfile");

    let out = Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_ROOT", root.path())
        .env("ENVR_RUNTIME_ROOT", runtime_root.as_os_str())
        .current_dir(&project)
        .args(["why", "node"])
        .output()
        .expect("run");
    assert!(out.status.success(), "stderr={}", String::from_utf8_lossy(&out.stderr));

    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("lockfile") || text.contains("lock 文件") || text.contains("lock present") || text.contains("fresh lockfile") || text.contains("No project pin:"), "expected lockfile notice in human output: {text}");
    assert!(text.contains("Resolved version:") || text.contains("解析版本："), "{text}");
}
