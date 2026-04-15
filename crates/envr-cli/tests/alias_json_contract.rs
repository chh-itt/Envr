//! Fill Phase A blind spots for `alias` commands: JSON envelope + offline-safe behavior.

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

fn write_config(root: &Path, aliases_toml: &str) {
    let cfg = root.join("config");
    fs::create_dir_all(&cfg).expect("mkdir config");
    fs::write(cfg.join("settings.toml"), "[i18n]\nlocale = \"en_us\"\n").expect("settings");
    fs::write(cfg.join("aliases.toml"), aliases_toml).expect("aliases");
}

#[test]
fn alias_add_json_emits_ok_envelope_and_persists_to_aliases_file() {
    let root = tempfile::tempdir().expect("tmp");
    write_config(root.path(), "");

    let out = Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_ROOT", root.path())
        .args(["--format", "json", "alias", "add", "n", "node"])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_line(&out.stdout);
    assert_eq!(v["success"], true, "{v}");
    assert_eq!(v["code"], "alias_added", "{v}");
    assert_eq!(v["data"]["name"], "n", "{v}");
    assert_eq!(v["data"]["target"], "node", "{v}");

    let aliases = fs::read_to_string(root.path().join("config/aliases.toml")).expect("aliases");
    assert!(
        aliases.contains("n = \"node\""),
        "alias add must persist to aliases.toml, got:\n{aliases}"
    );
}

#[test]
fn alias_list_json_includes_alias_entries_offline() {
    let root = tempfile::tempdir().expect("tmp");
    write_config(
        root.path(),
        r#"
[aliases]
n = "node"
dx = "diagnostics export"
"#,
    );

    let out = Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_ROOT", root.path())
        .args(["--format", "json", "alias", "list"])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_line(&out.stdout);
    assert_eq!(v["success"], true, "{v}");
    assert_eq!(v["code"], "alias_list", "{v}");
    let aliases = v["data"]["aliases"]
        .as_array()
        .expect("alias_list data.aliases should be array");
    assert!(
        aliases
            .iter()
            .any(|e| e["name"] == "n" && e["target"] == "node"),
        "expected alias n->node in JSON data: {v}"
    );
    assert!(
        aliases
            .iter()
            .any(|e| e["name"] == "dx" && e["target"] == "diagnostics export"),
        "expected alias dx->diagnostics export in JSON data: {v}"
    );
}

#[test]
fn alias_remove_json_reports_removed_and_idempotent_absence() {
    let root = tempfile::tempdir().expect("tmp");
    write_config(
        root.path(),
        r#"
[aliases]
n = "node"
"#,
    );

    let first = Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_ROOT", root.path())
        .args(["--format", "json", "alias", "remove", "n"])
        .output()
        .expect("run");
    assert!(
        first.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&first.stderr)
    );
    let v1 = parse_json_line(&first.stdout);
    assert_eq!(v1["success"], true, "{v1}");
    assert_eq!(v1["code"], "alias_removed", "{v1}");
    assert_eq!(v1["data"]["name"], "n", "{v1}");
    assert_eq!(v1["data"]["removed"], true, "{v1}");

    let second = Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_ROOT", root.path())
        .args(["--format", "json", "alias", "remove", "n"])
        .output()
        .expect("run");
    assert!(
        second.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&second.stderr)
    );
    let v2 = parse_json_line(&second.stdout);
    assert_eq!(v2["success"], true, "{v2}");
    assert_eq!(v2["code"], "alias_removed", "{v2}");
    assert_eq!(v2["data"]["name"], "n", "{v2}");
    assert_eq!(v2["data"]["removed"], false, "{v2}");
}
