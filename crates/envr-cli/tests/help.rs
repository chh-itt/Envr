use assert_cmd::Command;
use std::fs;
use tempfile::tempdir;

#[test]
fn long_help_includes_command_group_index() {
    let dir = tempdir().expect("tempdir");
    let cfg = dir.path().join("config");
    fs::create_dir_all(&cfg).expect("mkdir config");
    fs::write(cfg.join("settings.toml"), "[i18n]\nlocale = \"en_us\"\n").expect("write settings");

    let mut cmd = Command::cargo_bin("envr").expect("envr binary");
    let assert = cmd
        .env("ENVR_ROOT", dir.path())
        .arg("--help")
        .assert()
        .success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        out.contains("Command groups") && out.contains("Runtime management"),
        "expected grouped command index in long --help; got:\n{out}"
    );
    assert!(
        out.contains("L1 essential") && out.contains("L3 automation"),
        "expected L1/L3 tier legend in long --help; got:\n{out}"
    );
}

#[test]
fn help_is_readable_and_lists_l1_commands() {
    let mut cmd = Command::cargo_bin("envr").expect("envr binary");
    let assert = cmd.arg("--help").assert().success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    for sub in [
        "install",
        "use",
        "list",
        "current",
        "uninstall",
        "which",
        "remote",
        "rust",
        "doctor",
        "diagnostics",
        "init",
        "check",
        "status",
        "project",
        "completion",
        "help",
        "resolve",
        "why",
        "config",
        "alias",
        "prune",
        "update",
        "exec",
        "run",
        "env",
        "template",
        "shell",
        "hook",
        "deactivate",
        "import",
        "export",
        "profile",
        "shim",
        "cache",
        "bundle",
        "hook status",
        "hook doctor",
        "hook powershell",
    ] {
        assert!(
            out.contains(sub),
            "help output should mention `{sub}`:\n{out}"
        );
    }
}

#[test]
fn global_flags_appear_in_help() {
    let mut cmd = Command::cargo_bin("envr").expect("envr binary");
    let assert = cmd.arg("--help").assert().success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(out.contains("--format") || out.contains("format"));
    assert!(out.contains("--quiet"));
    assert!(out.contains("--no-color"));
    assert!(out.contains("--runtime-root"));
}

#[test]
fn completion_bash_writes_script() {
    let mut cmd = Command::cargo_bin("envr").expect("envr binary");
    let assert = cmd.args(["completion", "bash"]).assert().success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        out.contains("envr"),
        "bash completion should reference envr:\n{out}"
    );
    assert!(
        out.contains("help shortcuts"),
        "completion script should point at argv shorthands:\n{out}"
    );
}

#[test]
fn help_shortcuts_lists_preprocess_tokens() {
    let mut cmd = Command::cargo_bin("envr").expect("envr binary");
    let assert = cmd.args(["help", "shortcuts"]).assert().success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        out.contains("add") && out.contains("project add"),
        "expected shorthand table:\n{out}"
    );
}

#[test]
fn exec_and_run_help_mention_install_if_missing() {
    for sub in ["exec", "run"] {
        let mut cmd = Command::cargo_bin("envr").expect("envr binary");
        let assert = cmd.args([sub, "--help"]).assert().success();
        let out = String::from_utf8_lossy(&assert.get_output().stdout);
        assert!(
            out.contains("--install-if-missing") && out.contains("--install"),
            "{sub} help should list install flags:\n{out}"
        );
    }
}
