use assert_cmd::Command;
use std::process::Output;

fn run_envr(args: &[&str], root: &std::path::Path) -> Output {
    Command::cargo_bin("envr")
        .expect("envr binary")
        .env("ENVR_RUNTIME_ROOT", root.as_os_str())
        .args(args)
        .output()
        .expect("run envr")
}

#[test]
fn list_and_current_succeed_with_isolated_runtime_root() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = run_envr(&["list"], tmp.path());
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out = run_envr(&["current"], tmp.path());
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn install_requires_two_positionals() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = run_envr(&["install", "node"], tmp.path());
    assert!(!out.status.success());
}

#[test]
fn unknown_lang_is_error() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = run_envr(&["list", "not-a-lang"], tmp.path());
    assert!(!out.status.success());
}

#[test]
fn doctor_succeeds_with_isolated_runtime_root() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = run_envr(&["doctor"], tmp.path());
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn uninstall_requires_version() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = run_envr(&["uninstall", "node"], tmp.path());
    assert!(!out.status.success());
}

#[test]
fn which_requires_name() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = run_envr(&["which"], tmp.path());
    assert!(!out.status.success());
}

#[test]
fn which_unknown_tool_errors() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = run_envr(&["which", "not-a-core-tool"], tmp.path());
    assert!(!out.status.success());
}
