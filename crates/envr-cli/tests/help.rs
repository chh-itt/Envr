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

#[test]
fn hook_help_lists_status_doctor_and_keys() {
    let mut cmd = Command::cargo_bin("envr").expect("envr binary");
    let assert = cmd.args(["hook", "--help"]).assert().success();
    let out = String::from_utf8_lossy(&assert.get_output().stdout);
    assert!(
        out.contains("status"),
        "hook help should mention status:\n{out}"
    );
    assert!(
        out.contains("doctor"),
        "hook help should mention doctor:\n{out}"
    );
    assert!(
        out.contains("keys"),
        "hook help should mention keys:\n{out}"
    );
}

#[test]
fn hook_status_and_doctor_report_profile_details() {
    let dir = tempdir().expect("tempdir");
    let cfg = dir.path().join("config");
    fs::create_dir_all(&cfg).expect("mkdir config");
    fs::write(cfg.join("settings.toml"), "[i18n]\nlocale = \"en_us\"\n").expect("write settings");
    fs::write(dir.path().join(".envr.toml"), "[env]\nFOO = \"bar\"\n").expect("write project");

    let mut status = Command::cargo_bin("envr").expect("envr binary");
    let status = status
        .env("ENVR_ROOT", dir.path())
        .args([
            "hook",
            "status",
            "--path",
            dir.path().to_string_lossy().as_ref(),
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&status.get_output().stdout);
    assert!(
        out.contains("selected profile root"),
        "expected hook status to mention selected profile root:\n{out}"
    );

    let mut doctor = Command::cargo_bin("envr").expect("envr binary");
    let doctor = doctor
        .env("ENVR_ROOT", dir.path())
        .env("PROFILE", dir.path().join("PowerShell_profile.ps1"))
        .args([
            "hook",
            "doctor",
            "powershell",
            "--path",
            dir.path().to_string_lossy().as_ref(),
        ])
        .assert()
        .success();
    let out = String::from_utf8_lossy(&doctor.get_output().stdout);
    assert!(
        out.contains("profile root") && out.contains("powershell profile"),
        "expected hook doctor to mention powershell profile:\n{out}"
    );
}
