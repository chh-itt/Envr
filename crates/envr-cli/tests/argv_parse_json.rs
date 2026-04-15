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
fn clap_parse_errors_emit_json_envelope_when_format_json_requested() {
    let out = Command::cargo_bin("envr")
        .expect("envr binary")
        .args(["--format", "json", "--this-flag-does-not-exist"])
        .output()
        .expect("run");

    assert!(
        !out.status.success(),
        "expected non-zero; stdout={}\nstderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let v = parse_json_line(&out.stdout);
    assert_eq!(v["success"], false, "{v}");
    assert_eq!(v["code"], "argv_parse_error", "{v}");
    assert!(v.get("schema_version").is_some(), "{v}");
    assert_eq!(v["data"]["source"], "clap", "{v}");
    assert!(v["data"]["kind"].is_string(), "{v}");
    assert!(v["data"]["error"].is_string(), "{v}");
    assert!(v["data"]["exit_code"].is_number(), "{v}");
}

#[test]
fn clap_parse_errors_emit_json_envelope_when_legacy_doctor_json_shorthand_present() {
    let out = Command::cargo_bin("envr")
        .expect("envr binary")
        // `--fix-path-apply` requires `--fix-path` → clap parse error, before dispatch.
        .args(["doctor", "--json", "--fix-path-apply"])
        .output()
        .expect("run");

    assert!(
        !out.status.success(),
        "expected non-zero; stdout={}\nstderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let v = parse_json_line(&out.stdout);
    assert_eq!(v["success"], false, "{v}");
    assert_eq!(v["code"], "argv_parse_error", "{v}");
    assert_eq!(v["data"]["source"], "clap", "{v}");
    assert!(v["data"]["kind"].is_string(), "{v}");
}

#[test]
fn clap_parse_errors_print_to_stderr_in_text_mode_without_json_stdout() {
    let out = Command::cargo_bin("envr")
        .expect("envr binary")
        .args(["--this-flag-does-not-exist"])
        .output()
        .expect("run");

    assert!(
        !out.status.success(),
        "expected non-zero; stdout={}\nstderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.trim_start().starts_with('{'),
        "unexpected JSON envelope on stdout: {stdout:?}"
    );

    let stderr = String::from_utf8_lossy(&out.stderr);
    let stderr_lower = stderr.to_ascii_lowercase();
    assert!(
        stderr_lower.contains("error")
            || stderr_lower.contains("unknown")
            || stderr_lower.contains("unrecognized"),
        "expected clap-style error on stderr, got: {stderr:?}"
    );
}
