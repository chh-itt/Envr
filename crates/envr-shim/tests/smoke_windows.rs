#![cfg(windows)]

use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

#[test]
fn subcommand_npm_forwards_args_and_exit_code_with_current_pointer_file() {
    let tmp = TempDir::new().expect("tmp");
    let root = tmp.path();

    let home = root.join("runtimes/node/versions/20.10.0");
    fs::create_dir_all(&home).expect("home");
    fs::create_dir_all(root.join("runtimes/node")).expect("node root");

    let npm = home.join("npm.cmd");
    fs::write(
        &npm,
        "@echo off\r\nif \"%1\"==\"--ping\" exit /b 73\r\nexit /b 9\r\n",
    )
    .expect("write npm.cmd");

    // Windows fallback mode: `current` can be a pointer file when symlink/junction is unavailable.
    fs::write(
        root.join("runtimes/node/current"),
        home.display().to_string(),
    )
    .expect("current");

    Command::cargo_bin("envr-shim")
        .expect("bin")
        .current_dir(root)
        .env("ENVR_RUNTIME_ROOT", root.as_os_str())
        .arg("npm")
        .arg("--ping")
        .assert()
        .code(73);
}
