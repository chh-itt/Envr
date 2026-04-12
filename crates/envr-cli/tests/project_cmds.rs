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
fn project_add_writes_pin() {
    let tmp = tempfile::tempdir().expect("tempdir");
    Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .current_dir(tmp.path())
        .args(["project", "add", "node@22.1.0"])
        .assert()
        .success();
    let p = tmp.path().join(DOT_ENVR_TOML);
    let text = fs::read_to_string(&p).expect("read");
    assert!(
        text.contains("22.1.0") && text.contains("[runtimes.node]"),
        "unexpected toml:\n{text}"
    );
}

#[test]
fn project_add_rejects_bad_spec() {
    let tmp = tempfile::tempdir().expect("tempdir");
    Command::cargo_bin("envr")
        .expect("envr")
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .current_dir(tmp.path())
        .args(["project", "add", "not-a-runtime@1"])
        .assert()
        .failure();
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
