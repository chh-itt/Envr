//! T915: CLI `--help` respects `settings.toml` locale under `ENVR_ROOT`.

use assert_cmd::Command;
use std::fs;
use tempfile::tempdir;

#[test]
fn envr_help_uses_zh_cn_when_configured() {
    let dir = tempdir().expect("tempdir");
    let cfg = dir.path().join("config");
    fs::create_dir_all(&cfg).expect("mkdir config");
    fs::write(cfg.join("settings.toml"), "[i18n]\nlocale = \"zh_cn\"\n").expect("write settings");

    let assert = Command::cargo_bin("envr")
        .expect("envr binary")
        .env("ENVR_ROOT", dir.path())
        .arg("--help")
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        out.contains("语言运行时版本管理器"),
        "expected localized root about line; got:\n{out}"
    );
}

#[test]
fn envr_help_uses_en_us_when_configured() {
    let dir = tempdir().expect("tempdir");
    let cfg = dir.path().join("config");
    fs::create_dir_all(&cfg).expect("mkdir config");
    fs::write(cfg.join("settings.toml"), "[i18n]\nlocale = \"en_us\"\n").expect("write settings");

    let assert = Command::cargo_bin("envr")
        .expect("envr binary")
        .env("ENVR_ROOT", dir.path())
        .arg("--help")
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        out.contains("Language runtime version manager"),
        "expected English root about line; got:\n{out}"
    );
}
