use assert_cmd::Command;

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
        "doctor",
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
