//! User `config/aliases.toml` entries expand before built-in argv shorthands.

use assert_cmd::Command;
use std::fs;
use std::path::Path;

fn write_config(root: &Path, aliases_toml: &str) {
    let cfg = root.join("config");
    fs::create_dir_all(&cfg).expect("mkdir config");
    fs::write(cfg.join("settings.toml"), "[i18n]\nlocale = \"en_us\"\n").expect("settings");
    fs::write(cfg.join("aliases.toml"), aliases_toml).expect("aliases");
}

#[test]
fn user_alias_ci_overrides_builtin_before_preprocess() {
    let root = tempfile::tempdir().expect("tmp");
    write_config(
        root.path(),
        r#"
[aliases]
ci = "doctor"
"#,
    );

    let out = Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_ROOT", root.path())
        .args(["ci", "--help"])
        .output()
        .expect("run");

    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("environment checks"),
        "expected doctor help; got:\n{stdout}"
    );
    assert!(
        !stdout.contains("Download remote indexes"),
        "builtin `ci` should not apply when user alias wins:\n{stdout}"
    );
}

#[test]
fn user_alias_multi_word_target() {
    let root = tempfile::tempdir().expect("tmp");
    write_config(
        root.path(),
        r#"
[aliases]
mydx = "diagnostics export"
"#,
    );

    let out = Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_ROOT", root.path())
        .args(["mydx", "--help"])
        .output()
        .expect("run");

    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("doctor.json"),
        "expected diagnostics export help; got:\n{stdout}"
    );
}

#[test]
fn user_alias_chains_until_non_alias() {
    let root = tempfile::tempdir().expect("tmp");
    write_config(
        root.path(),
        r#"
[aliases]
a = "b"
b = "doctor"
"#,
    );

    let out = Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_ROOT", root.path())
        .args(["a", "--help"])
        .output()
        .expect("run");

    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("environment checks"),
        "expected chained expansion to doctor; got:\n{stdout}"
    );
}

#[test]
fn user_alias_resolves_after_global_flags() {
    let root = tempfile::tempdir().expect("tmp");
    write_config(
        root.path(),
        r#"
[aliases]
ci = "doctor"
"#,
    );

    let out = Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_ROOT", root.path())
        .args(["--format", "json", "ci", "--help"])
        .output()
        .expect("run");

    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("environment checks"),
        "expected doctor help after globals; got:\n{stdout}"
    );
}
