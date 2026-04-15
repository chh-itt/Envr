//! P2: zh-CN and en-US root `--help` must share the same discoverable subcommand surface
//! (ASCII names + localized section headers).

use assert_cmd::Command;
use std::fs;
use tempfile::tempdir;

fn root_help_with_locale(locale: &str) -> String {
    let dir = tempdir().expect("tempdir");
    let cfg = dir.path().join("config");
    fs::create_dir_all(&cfg).expect("mkdir config");
    fs::write(
        cfg.join("settings.toml"),
        format!("[i18n]\nlocale = \"{locale}\"\n"),
    )
    .expect("write settings");

    let assert = Command::cargo_bin("envr")
        .expect("envr binary")
        .env("ENVR_ROOT", dir.path())
        .arg("--help")
        .assert()
        .success();
    String::from_utf8_lossy(&assert.get_output().stdout).into_owned()
}

/// Subcommands that must appear in both locales (names stay ASCII in clap output).
const SHARED_SUBCOMMAND_TOKENS: &[&str] = &[
    "install",
    "use",
    "list",
    "current",
    "uninstall",
    "which",
    "remote",
    "doctor",
    "config",
    "exec",
    "run",
    "cache",
    "bundle",
    "project",
    "status",
    "help",
];

#[test]
fn zh_and_en_help_share_subcommand_tokens() {
    let zh = root_help_with_locale("zh_cn");
    let en = root_help_with_locale("en_us");

    assert!(
        zh.contains("命令分组"),
        "zh help expected 命令分组 section; got:\n{zh}"
    );
    assert!(
        en.contains("Command groups"),
        "en help expected Command groups section; got:\n{en}"
    );
    assert!(
        zh.contains("L1"),
        "zh help expected L1 tier line; got:\n{zh}"
    );
    assert!(
        en.contains("L1"),
        "en help expected L1 tier line; got:\n{en}"
    );

    for token in SHARED_SUBCOMMAND_TOKENS {
        assert!(
            zh.contains(*token),
            "zh help should mention `{token}` for structural parity"
        );
        assert!(
            en.contains(*token),
            "en help should mention `{token}` for structural parity"
        );
    }
}
