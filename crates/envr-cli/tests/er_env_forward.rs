//! `er` must forward the parent process environment to `envr` (e.g. `ENVR_RUNTIME_ROOT`).

use assert_cmd::Command;
use serde_json::Value;

fn parse_json_line(stdout: &[u8]) -> Value {
    for line in stdout.split(|b| *b == b'\n') {
        if line.is_empty() {
            continue;
        }
        if line.first() == Some(&b'{')
            && let Ok(v) = serde_json::from_slice::<Value>(line)
        {
            return v;
        }
    }
    panic!(
        "no json object in stdout: {}",
        String::from_utf8_lossy(stdout)
    );
}

#[test]
fn er_forwards_envr_runtime_root_to_envr() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = Command::cargo_bin("er")
        .expect("er binary")
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .args(["--format", "json", "doctor"])
        .output()
        .expect("run er");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_line(&out.stdout);
    let want = tmp.path().to_string_lossy();
    assert_eq!(
        v["data"]["envr_runtime_root_env"].as_str(),
        Some(want.as_ref()),
        "doctor data: {:?}",
        v["data"]
    );
}
