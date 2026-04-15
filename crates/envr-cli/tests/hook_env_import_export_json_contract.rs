//! Fill Phase A blind spots for hook/env/import/export JSON + offline-safe behavior.

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
fn hook_bash_json_emits_shell_hook_envelope_with_script() {
    let root = tempfile::tempdir().expect("tmp");
    write_settings(root.path());

    let out = Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_ROOT", root.path())
        .args(["--format", "json", "hook", "bash"])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_line(&out.stdout);
    assert_eq!(v["success"], true, "{v}");
    assert_eq!(v["code"], "shell_hook", "{v}");
    assert_eq!(v["data"]["shell"], "bash", "{v}");
    assert!(
        v["data"]["script"]
            .as_str()
            .is_some_and(|s| s.contains("envr")),
        "expected hook script payload: {v}"
    );
}

#[test]
fn hook_zsh_json_emits_shell_hook_envelope_with_script() {
    let root = tempfile::tempdir().expect("tmp");
    write_settings(root.path());

    let out = Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_ROOT", root.path())
        .args(["--format", "json", "hook", "zsh"])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_line(&out.stdout);
    assert_eq!(v["success"], true, "{v}");
    assert_eq!(v["code"], "shell_hook", "{v}");
    assert_eq!(v["data"]["shell"], "zsh", "{v}");
    assert!(
        v["data"]["script"]
            .as_str()
            .is_some_and(|s| s.contains("envr")),
        "expected hook script payload: {v}"
    );
}

#[test]
fn hook_keys_json_emits_key_list_offline() {
    let root = tempfile::tempdir().expect("tmp");
    write_settings(root.path());
    let project = root.path().join("proj");
    fs::create_dir_all(&project).expect("project");
    fs::write(project.join(".envr.toml"), "[env]\nFOO = \"bar\"\n").expect("envr.toml");

    let out = Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_ROOT", root.path())
        .current_dir(&project)
        .args(["--format", "json", "hook", "keys"])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_line(&out.stdout);
    assert_eq!(v["success"], true, "{v}");
    assert_eq!(v["code"], "hook_keys", "{v}");
    assert!(v["data"]["path"].is_string(), "{v}");
    assert!(v["data"]["keys"].is_array(), "{v}");
}

#[test]
fn env_json_emits_vars_for_posix_shell() {
    let root = tempfile::tempdir().expect("tmp");
    write_settings(root.path());
    let project = root.path().join("proj");
    fs::create_dir_all(&project).expect("project");
    fs::write(project.join(".envr.toml"), "[env]\nFOO = \"bar\"\n").expect("envr.toml");

    let out = Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_ROOT", root.path())
        .current_dir(&project)
        .args(["--format", "json", "env", "--shell", "posix"])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_line(&out.stdout);
    assert_eq!(v["success"], true, "{v}");
    assert_eq!(v["code"], "project_env", "{v}");
    assert_eq!(v["data"]["shell"], "posix", "{v}");
    assert!(v["data"]["vars"].is_object(), "{v}");
    assert_eq!(v["data"]["vars"]["FOO"], "bar", "{v}");
}

#[test]
fn import_then_export_json_roundtrip_offline() {
    let root = tempfile::tempdir().expect("tmp");
    write_settings(root.path());
    let project = root.path().join("proj");
    fs::create_dir_all(&project).expect("project");

    let import_src = root.path().join("import.toml");
    fs::write(
        &import_src,
        r#"
[runtimes.node]
version = "20"
[env]
FOO = "bar"
"#,
    )
    .expect("import.toml");

    let import_out = Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_ROOT", root.path())
        .current_dir(&project)
        .args([
            "--format",
            "json",
            "import",
            import_src.to_string_lossy().as_ref(),
            "--path",
            ".",
        ])
        .output()
        .expect("run import");
    assert!(
        import_out.status.success(),
        "import stderr={}",
        String::from_utf8_lossy(&import_out.stderr)
    );
    let iv = parse_json_line(&import_out.stdout);
    assert_eq!(iv["success"], true, "{iv}");
    assert_eq!(iv["code"], "config_imported", "{iv}");
    assert!(
        project.join(".envr.toml").is_file(),
        "import should write .envr.toml"
    );

    let export_out = Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_ROOT", root.path())
        .current_dir(&project)
        .args(["--format", "json", "export", "--path", "."])
        .output()
        .expect("run export");
    assert!(
        export_out.status.success(),
        "export stderr={}",
        String::from_utf8_lossy(&export_out.stderr)
    );
    let ev = parse_json_line(&export_out.stdout);
    assert_eq!(ev["success"], true, "{ev}");
    assert_eq!(ev["code"], "config_exported", "{ev}");
    assert!(ev["data"]["toml"].is_string(), "{ev}");
    let toml = ev["data"]["toml"].as_str().expect("export toml string");
    assert!(toml.contains("version = \"20\""), "{ev}");
    assert!(toml.contains("FOO = \"bar\""), "{ev}");
}
