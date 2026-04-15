//! Fill Phase A blind spots for config/debug/profile JSON and offline-safe paths.

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

#[test]
fn config_schema_json_emits_template_payload_offline() {
    let root = tempfile::tempdir().expect("tmp");
    write_settings(root.path());

    let out = Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_ROOT", root.path())
        .args(["--format", "json", "config", "schema"])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_line(&out.stdout);
    assert_eq!(v["success"], true, "{v}");
    assert_eq!(v["code"], "config_schema", "{v}");
    assert!(v["data"]["path"].is_string(), "{v}");
    let template = v["data"]["template"]
        .as_str()
        .expect("config_schema data.template should be string");
    assert!(
        template.contains("[i18n]"),
        "expected schema template content in JSON data.template: {v}"
    );
}

#[test]
fn debug_info_json_emits_runtime_and_env_snapshot_fields() {
    let root = tempfile::tempdir().expect("tmp");
    let runtime_root = root.path().join("rt");
    fs::create_dir_all(runtime_root.join("runtimes")).expect("runtime tree");

    let out = Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_ROOT", root.path())
        .env("ENVR_RUNTIME_ROOT", runtime_root.as_os_str())
        .env("ENVR_PROFILE", "dev")
        .args(["--format", "json", "debug", "info"])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_line(&out.stdout);
    assert_eq!(v["success"], true, "{v}");
    assert_eq!(v["code"], "debug_info", "{v}");
    assert!(v["data"]["cwd"].is_string(), "{v}");
    assert!(v["data"]["runtime_root"].is_string(), "{v}");
    assert!(v["data"]["runtime_root_children_sample"].is_array(), "{v}");
    assert!(v["data"]["envr_env"].is_array(), "{v}");
}

#[test]
fn profile_list_json_reports_profiles_from_project_config() {
    let tmp = tempfile::tempdir().expect("tmp");
    let project = tmp.path().join("proj");
    fs::create_dir_all(&project).expect("project");
    fs::write(
        project.join(".envr.toml"),
        r#"
[profiles.dev.runtimes.node]
version = "20"
[profiles.ci.env]
CI = "1"
"#,
    )
    .expect("envr.toml");

    let out = Command::cargo_bin("envr")
        .expect("envr")
        .current_dir(&project)
        .args(["--format", "json", "profile", "list"])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_line(&out.stdout);
    assert_eq!(v["success"], true, "{v}");
    assert_eq!(v["code"], "profiles_list", "{v}");
    let profiles = v["data"]["profiles"]
        .as_array()
        .expect("profiles_list data.profiles should be array");
    assert!(
        profiles.iter().any(|p| p == "dev"),
        "expected profile `dev` in list: {v}"
    );
    assert!(
        profiles.iter().any(|p| p == "ci"),
        "expected profile `ci` in list: {v}"
    );
}

#[test]
fn profile_show_json_includes_profile_name_runtimes_and_env() {
    let tmp = tempfile::tempdir().expect("tmp");
    let project = tmp.path().join("proj");
    fs::create_dir_all(&project).expect("project");
    fs::write(
        project.join(".envr.toml"),
        r#"
[profiles.dev.runtimes.node]
version = "20"
[profiles.dev.runtimes.python]
version = "3.12"
[profiles.dev.env]
FOO = "bar"
"#,
    )
    .expect("envr.toml");

    let out = Command::cargo_bin("envr")
        .expect("envr")
        .current_dir(&project)
        .args(["--format", "json", "profile", "show", "dev"])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_line(&out.stdout);
    assert_eq!(v["success"], true, "{v}");
    assert_eq!(v["code"], "profile_show", "{v}");
    assert_eq!(v["data"]["name"], "dev", "{v}");
    assert_eq!(v["data"]["runtimes"]["node"]["version"], "20", "{v}");
    assert_eq!(v["data"]["env"]["FOO"], "bar", "{v}");
}
