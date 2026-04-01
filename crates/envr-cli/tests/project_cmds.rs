use assert_cmd::Command;
use std::fs;

const DOT_ENVR_TOML: &str = ".envr.toml";

#[test]
fn init_creates_envr_toml() {
    let tmp = tempfile::tempdir().expect("tempdir");
    Command::cargo_bin("envr")
        .expect("envr")
        .current_dir(tmp.path())
        .args(["init"])
        .assert()
        .success();
    let p = tmp.path().join(DOT_ENVR_TOML);
    assert!(p.is_file());
    let text = fs::read_to_string(&p).expect("read");
    assert!(text.contains("[runtimes.node]"));
}

#[test]
fn init_refuses_existing_without_force() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let p = tmp.path().join(DOT_ENVR_TOML);
    fs::write(&p, "[env]\n").expect("write");
    Command::cargo_bin("envr")
        .expect("envr")
        .current_dir(tmp.path())
        .args(["init"])
        .assert()
        .failure();
}

#[test]
fn check_passes_after_init() {
    let tmp = tempfile::tempdir().expect("tempdir");
    Command::cargo_bin("envr")
        .expect("envr")
        .current_dir(tmp.path())
        .arg("init")
        .assert()
        .success();
    Command::cargo_bin("envr")
        .expect("envr")
        .current_dir(tmp.path())
        .arg("check")
        .assert()
        .success();
}

#[test]
fn check_fails_when_no_config() {
    let tmp = tempfile::tempdir().expect("tempdir");
    Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .current_dir(tmp.path())
        .arg("check")
        .assert()
        .failure();
}
