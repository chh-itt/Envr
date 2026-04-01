#![cfg(unix)]

use assert_cmd::Command;
use std::fs;
use std::io::Write;
use std::os::unix::fs::{PermissionsExt, symlink};
use tempfile::TempDir;

#[test]
fn subcommand_python_forwards_exit_code() {
    let tmp = TempDir::new().expect("tmp");
    let root = tmp.path();

    fs::create_dir_all(root.join("runtimes/python")).expect("mkdir");
    let home = root.join("runtimes/python/versions/3.12.0");
    fs::create_dir_all(home.join("bin")).expect("bin");
    let py = home.join("bin/python3");
    let mut f = fs::File::create(&py).expect("script");
    writeln!(f, "#!/bin/sh\nexit 77").expect("write");
    let mut perms = fs::metadata(&py).expect("meta").permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&py, perms).expect("chmod");

    symlink(&home, root.join("runtimes/python/current")).expect("symlink");

    Command::cargo_bin("envr-shim")
        .expect("bin")
        .current_dir(root)
        .env("ENVR_RUNTIME_ROOT", root.as_os_str())
        .arg("python3")
        .assert()
        .code(77);
}
