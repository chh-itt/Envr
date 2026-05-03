use assert_cmd::Command;
use serde_json::Value;
use std::fs;

#[test]
fn hook_status_emits_hook_keys_envelope_and_profiles() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cfg = dir.path().join("config");
    fs::create_dir_all(&cfg).expect("mkdir config");
    fs::write(cfg.join("settings.toml"), "[i18n]\nlocale = \"en_us\"\n").expect("write settings");
    fs::write(dir.path().join(".envr.toml"), "[env]\nFOO = \"bar\"\n").expect("write project");

    let out = Command::cargo_bin("envr")
        .expect("envr binary")
        .env("ENVR_ROOT", dir.path())
        .args([
            "--format",
            "json",
            "hook",
            "status",
            "--path",
            dir.path().to_string_lossy().as_ref(),
        ])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: Value = serde_json::from_str(
        stdout
            .lines()
            .find(|l| l.trim_start().starts_with('{'))
            .expect("json line"),
    )
    .expect("parse json");
    assert_eq!(v["code"], "hook_keys", "{v}");
    assert!(v["data"]["path"].is_string(), "{v}");
    assert!(
        v["data"]["hooks"].as_array().is_some_and(|a| !a.is_empty()),
        "{v}"
    );
}

#[test]
fn hook_doctor_powershell_emits_hook_keys_envelope() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cfg = dir.path().join("config");
    fs::create_dir_all(&cfg).expect("mkdir config");
    fs::write(cfg.join("settings.toml"), "[i18n]\nlocale = \"en_us\"\n").expect("write settings");

    let out = Command::cargo_bin("envr")
        .expect("envr binary")
        .env("ENVR_ROOT", dir.path())
        .env("PROFILE", dir.path().join("PowerShell_profile.ps1"))
        .args([
            "--format",
            "json",
            "hook",
            "doctor",
            "powershell",
            "--path",
            dir.path().to_string_lossy().as_ref(),
        ])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: Value = serde_json::from_str(
        stdout
            .lines()
            .find(|l| l.trim_start().starts_with('{'))
            .expect("json line"),
    )
    .expect("parse json");
    assert_eq!(v["code"], "hook_keys", "{v}");
    assert_eq!(v["data"]["shell"], "powershell", "{v}");
    assert!(
        v["data"]["profile_state"]
            .as_str()
            .is_some_and(|s| s.contains("powershell profile")),
        "{v}"
    );
    assert!(
        v["data"]["recommendations"]
            .as_array()
            .is_some_and(|a| !a.is_empty()),
        "{v}"
    );
}

#[test]
fn hook_doctor_powershell_without_profile_reports_not_detected() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cfg = dir.path().join("config");
    fs::create_dir_all(&cfg).expect("mkdir config");
    fs::write(cfg.join("settings.toml"), "[i18n]\nlocale = \"en_us\"\n").expect("write settings");

    let out = Command::cargo_bin("envr")
        .expect("envr binary")
        .env("ENVR_ROOT", dir.path())
        .env_remove("PROFILE")
        .env_remove("PSPROFILE")
        .env_remove("USERPROFILE")
        .args([
            "--format",
            "json",
            "hook",
            "doctor",
            "powershell",
            "--path",
            dir.path().to_string_lossy().as_ref(),
        ])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: Value = serde_json::from_str(
        stdout
            .lines()
            .find(|l| l.trim_start().starts_with('{'))
            .expect("json line"),
    )
    .expect("parse json");
    assert_eq!(v["code"], "hook_keys", "{v}");
    assert_eq!(v["data"]["shell"], "powershell", "{v}");
    assert!(
        v["data"]["profile_state"]
            .as_str()
            .is_some_and(|s| s.contains("not detected")),
        "{v}"
    );
    assert!(
        v["data"]["recommendations"]
            .as_array()
            .is_some_and(|a| !a.is_empty()),
        "{v}"
    );
}

#[test]
fn hook_status_human_mentions_selected_root_and_found_profiles() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cfg = dir.path().join("config");
    fs::create_dir_all(&cfg).expect("mkdir config");
    fs::write(cfg.join("settings.toml"), "[i18n]\nlocale = \"en_us\"\n").expect("write settings");
    fs::write(dir.path().join(".envr.toml"), "[env]\nFOO = \"bar\"\n").expect("write project");

    let out = Command::cargo_bin("envr")
        .expect("envr binary")
        .env("ENVR_ROOT", dir.path())
        .args([
            "hook",
            "status",
            "--path",
            dir.path().to_string_lossy().as_ref(),
        ])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("selected profile root") || stdout.contains("选择的配置根"),
        "{stdout}"
    );
    assert!(
        stdout.contains("found profile") || stdout.contains("发现配置"),
        "{stdout}"
    );
}

#[test]
fn hook_doctor_bash_emits_hook_keys_envelope() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cfg = dir.path().join("config");
    fs::create_dir_all(&cfg).expect("mkdir config");
    fs::write(cfg.join("settings.toml"), "[i18n]\nlocale = \"en_us\"\n").expect("write settings");

    let out = Command::cargo_bin("envr")
        .expect("envr binary")
        .env("ENVR_ROOT", dir.path())
        .env("BASH_VERSION", "5.2.0")
        .args([
            "--format",
            "json",
            "hook",
            "doctor",
            "bash",
            "--path",
            dir.path().to_string_lossy().as_ref(),
        ])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: Value = serde_json::from_str(
        stdout
            .lines()
            .find(|l| l.trim_start().starts_with('{'))
            .expect("json line"),
    )
    .expect("parse json");
    assert_eq!(v["code"], "hook_keys", "{v}");
    assert_eq!(v["data"]["shell"], "bash", "{v}");
    assert!(
        v["data"]["profile_state"]
            .as_str()
            .is_some_and(|s| s.contains("bash shell detected")),
        "{v}"
    );
    assert!(
        v["data"]["recommendations"]
            .as_array()
            .is_some_and(|a| !a.is_empty()),
        "{v}"
    );
}

#[test]
fn hook_doctor_zsh_emits_hook_keys_envelope() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cfg = dir.path().join("config");
    fs::create_dir_all(&cfg).expect("mkdir config");
    fs::write(cfg.join("settings.toml"), "[i18n]\nlocale = \"en_us\"\n").expect("write settings");

    let out = Command::cargo_bin("envr")
        .expect("envr binary")
        .env("ENVR_ROOT", dir.path())
        .env("ZSH_VERSION", "5.9")
        .args([
            "--format",
            "json",
            "hook",
            "doctor",
            "zsh",
            "--path",
            dir.path().to_string_lossy().as_ref(),
        ])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let v: Value = serde_json::from_str(
        stdout
            .lines()
            .find(|l| l.trim_start().starts_with('{'))
            .expect("json line"),
    )
    .expect("parse json");
    assert_eq!(v["code"], "hook_keys", "{v}");
    assert_eq!(v["data"]["shell"], "zsh", "{v}");
    assert!(
        v["data"]["profile_state"]
            .as_str()
            .is_some_and(|s| s.contains("zsh shell detected")),
        "{v}"
    );
    assert!(
        v["data"]["recommendations"]
            .as_array()
            .is_some_and(|a| !a.is_empty()),
        "{v}"
    );
}

#[test]
fn hook_keys_reports_restore_keys() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cfg = dir.path().join("config");
    fs::create_dir_all(&cfg).expect("mkdir config");
    fs::write(cfg.join("settings.toml"), "[i18n]\nlocale = \"en_us\"\n").expect("write settings");
    fs::write(
        dir.path().join(".envr.toml"),
        "[env]\nFOO = \"bar\"\nPATH = \"/tmp/bin\"\n",
    )
    .expect("write project");

    let out = Command::cargo_bin("envr")
        .expect("envr binary")
        .env("ENVR_ROOT", dir.path())
        .args([
            "hook",
            "keys",
            "--path",
            dir.path().to_string_lossy().as_ref(),
        ])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let keys: Vec<&str> = stdout.lines().filter(|l| !l.trim().is_empty()).collect();
    assert!(keys.contains(&"PATH"), "{stdout}");
    assert!(keys.contains(&"FOO"), "{stdout}");
    assert!(
        keys.iter()
            .any(|k| matches!(*k, "JAVA_HOME" | "ERLANG_HOME")),
        "expected common restore keys in output: {stdout}"
    );
}
