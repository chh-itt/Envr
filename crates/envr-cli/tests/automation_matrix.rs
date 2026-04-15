//! P0 regression: porcelain + JSON paths documented in docs/cli/automation-matrix.md.

use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use std::path::Path;

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

fn assert_envelope_core(v: &Value) {
    assert!(
        v.get("schema_version").is_some(),
        "missing schema_version: {v}"
    );
    assert!(v.get("success").is_some(), "missing success: {v}");
    assert!(v.get("code").is_some(), "missing code: {v}");
    assert!(v.get("message").is_some(), "missing message: {v}");
    assert!(v.get("data").is_some(), "missing data: {v}");
    assert!(v.get("diagnostics").is_some(), "missing diagnostics: {v}");
}

fn write_node_layout(runtime_root: &Path, version: &str) {
    let ver = runtime_root.join("runtimes/node/versions").join(version);
    let bin = ver.join("bin");
    fs::create_dir_all(&bin).expect("bin");
    #[cfg(windows)]
    fs::write(bin.join("node.exe"), []).expect("node.exe");
    #[cfg(not(windows))]
    fs::write(bin.join("node"), []).expect("node");
}

#[test]
fn json_config_path_emits_envelope() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = Command::cargo_bin("envr")
        .expect("envr binary")
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .args(["--format", "json", "config", "path"])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_line(&out.stdout);
    assert_envelope_core(&v);
    assert_eq!(v["success"], true);
    assert_eq!(v["code"], "config_path");
    assert!(v["data"]["path"].is_string());
}

#[test]
fn porcelain_current_all_runtimes_tab_lines() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = Command::cargo_bin("envr")
        .expect("envr binary")
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .args(["--porcelain", "current"])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        if line.is_empty() {
            continue;
        }
        assert!(
            line.contains('\t'),
            "each non-empty line must be runtime\\tversion: {line:?}"
        );
    }
}

#[test]
fn porcelain_resolve_single_line_runtime_home() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let runtime_root = tmp.path().join("rr");
    let project = tmp.path().join("proj");
    fs::create_dir_all(&project).expect("proj");
    write_node_layout(&runtime_root, "20.10.0");
    fs::write(
        project.join(".envr.toml"),
        "[runtimes.node]\nversion = \"20.10.0\"\n",
    )
    .expect("envr.toml");
    let out = Command::cargo_bin("envr")
        .expect("envr binary")
        .current_dir(&project)
        .env("ENVR_RUNTIME_ROOT", runtime_root.as_os_str())
        .args(["--porcelain", "resolve", "node"])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    assert_eq!(lines.len(), 1, "porcelain resolve: one line, got: {text:?}");
    assert!(
        lines[0].contains("runtimes") && lines[0].contains("20.10.0"),
        "expected resolved home path: {:?}",
        lines[0]
    );
}

#[test]
fn porcelain_which_single_line_executable_path() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let runtime_root = tmp.path().join("rr");
    let project = tmp.path().join("proj");
    fs::create_dir_all(&project).expect("proj");
    write_node_layout(&runtime_root, "20.10.0");
    fs::write(
        project.join(".envr.toml"),
        "[runtimes.node]\nversion = \"20.10.0\"\n",
    )
    .expect("envr.toml");
    #[cfg(windows)]
    let path_env = format!(
        "{}\\System32;{}",
        std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_string()),
        std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_string())
    );
    #[cfg(not(windows))]
    let path_env = "/usr/bin:/bin";
    let out = Command::cargo_bin("envr")
        .expect("envr binary")
        .current_dir(&project)
        .env("ENVR_RUNTIME_ROOT", runtime_root.as_os_str())
        .env("PATH", path_env)
        .args(["--porcelain", "which", "node"])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    assert_eq!(
        lines.len(),
        1,
        "porcelain which: one stdout line, got: {text:?}"
    );
    let p = std::path::Path::new(lines[0]);
    assert!(p.is_absolute(), "expected absolute path: {:?}", lines[0]);
}

#[test]
fn porcelain_list_all_runtimes_tab_lines_when_no_runtime_filter() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let runtime_root = tmp.path().join("rr");
    let cwd = tmp.path().join("cwd");
    fs::create_dir_all(&cwd).expect("cwd");
    write_node_layout(&runtime_root, "21.0.0");
    write_node_layout(&runtime_root, "21.1.0");
    let out = Command::cargo_bin("envr")
        .expect("envr binary")
        .current_dir(&cwd)
        .env("ENVR_RUNTIME_ROOT", runtime_root.as_os_str())
        .args(["--porcelain", "list"])
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8_lossy(&out.stdout);
    let node_lines: Vec<&str> = text.lines().filter(|l| l.starts_with("node\t")).collect();
    assert!(
        node_lines.len() >= 2,
        "expected at least two node\\t lines, got:\n{text}"
    );
}

#[test]
fn exec_and_run_help_list_shared_run_flags() {
    for sub in ["exec", "run"] {
        let out = Command::cargo_bin("envr")
            .expect("envr binary")
            .args([sub, "--help"])
            .output()
            .expect("run");
        assert!(
            out.status.success(),
            "{} --help stderr={}",
            sub,
            String::from_utf8_lossy(&out.stderr)
        );
        let h = String::from_utf8_lossy(&out.stdout);
        assert!(
            h.contains("--dry-run") && h.contains("--profile"),
            "{sub} --help should document shared run flags; got:\n{h}"
        );
        assert!(
            h.contains("--install-if-missing") || h.contains("--install"),
            "{sub} --help should document install-if-missing; got:\n{h}"
        );
    }
}
