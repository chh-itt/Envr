use assert_cmd::Command;

#[test]
fn bundle_create_rejects_full_and_no_current() {
    let mut cmd = Command::cargo_bin("envr").expect("bin");
    cmd.env("ENVR_ROOT", tempfile::tempdir().expect("tmp").path())
        .args([
            "bundle",
            "create",
            "--full",
            "--no-current",
            "--output",
            "bundle.zip",
        ])
        .assert()
        .failure();
}

