use assert_cmd::Command;
use std::fs::File;
use tempfile::tempdir;
use zip::ZipArchive;

#[test]
fn diagnostics_export_writes_zip_with_doctor_json() {
    let tmp = tempdir().expect("tempdir");
    let zip_path = tmp.path().join("bundle.zip");

    let mut cmd = Command::cargo_bin("envr").expect("envr binary");
    cmd.env("ENVR_RUNTIME_ROOT", tmp.path())
        .args(["diagnostics", "export", "--output"])
        .arg(&zip_path)
        .assert()
        .success();

    assert!(zip_path.is_file(), "zip should exist");

    let file = File::open(&zip_path).expect("open zip");
    let mut archive = ZipArchive::new(file).expect("zip archive");
    let mut names = Vec::new();
    for i in 0..archive.len() {
        let ent = archive.by_index(i).expect("entry");
        names.push(ent.name().to_string());
    }
    assert!(
        names.iter().any(|n| n == "doctor.json"),
        "expected doctor.json in zip, got {names:?}"
    );
    assert!(
        names.iter().any(|n| n == "system.txt"),
        "expected system.txt in zip, got {names:?}"
    );
    assert!(
        names.iter().any(|n| n == "provider-state.json"),
        "expected provider-state.json in zip, got {names:?}"
    );
}
