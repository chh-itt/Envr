use assert_cmd::Command;

#[test]
fn install_visible_alias_i_parses() {
    let mut cmd = Command::cargo_bin("envr").expect("envr binary");
    cmd.env("ENVR_ROOT", tempfile::tempdir().expect("tmp").path())
        .args(["i", "--help"])
        .assert()
        .success();
}

#[test]
fn status_visible_alias_st_parses() {
    let mut cmd = Command::cargo_bin("envr").expect("envr binary");
    cmd.env("ENVR_ROOT", tempfile::tempdir().expect("tmp").path())
        .args(["st", "--help"])
        .assert()
        .success();
}

#[test]
fn diag_expands_to_diagnostics_export_help() {
    let mut cmd = Command::cargo_bin("envr").expect("envr binary");
    cmd.args(["diag", "--help"])
        .assert()
        .success();
}
