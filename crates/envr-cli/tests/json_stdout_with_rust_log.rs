//! With `RUST_LOG` enabled, tracing must not write to stdout (JSON / porcelain rely on a clean stdout).

use assert_cmd::Command;

#[test]
fn format_json_success_emits_only_one_stdout_line_with_rust_log_info() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = Command::cargo_bin("envr")
        .expect("envr binary")
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .env("RUST_LOG", "info")
        .args(["--format", "json", "list"])
        .output()
        .expect("run");

    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let non_empty_lines: Vec<_> = stdout
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    assert_eq!(
        non_empty_lines.len(),
        1,
        "expected exactly one JSON envelope line on stdout; got {non_empty_lines:?} full stdout={stdout:?} stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        non_empty_lines[0].starts_with('{'),
        "expected JSON object: {:?}",
        non_empty_lines[0]
    );
}
