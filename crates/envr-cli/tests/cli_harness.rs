//! Deeper integration tests: mock runtime tree under `ENVR_RUNTIME_ROOT` + project dir.

use assert_cmd::Command;
use std::fs;
use std::path::Path;
use std::process::Output;

fn run_envr(args: &[&str], runtime_root: &Path, cwd: &Path) -> Output {
    Command::cargo_bin("envr")
        .expect("envr binary")
        .env("ENVR_RUNTIME_ROOT", runtime_root.as_os_str())
        .current_dir(cwd)
        .args(args)
        .output()
        .expect("run envr")
}

fn write_node_layout(runtime_root: &Path, version: &str) {
    let ver = runtime_root.join("runtimes/node/versions").join(version);
    let bin = ver.join("bin");
    fs::create_dir_all(&bin).expect("create node bin");
    #[cfg(windows)]
    fs::write(bin.join("node.exe"), []).expect("touch node.exe");
    #[cfg(not(windows))]
    fs::write(bin.join("node"), []).expect("touch node");
}

#[test]
fn list_node_text_lists_mock_version() {
    let tmp = tempfile::tempdir().expect("tmp");
    let runtime_root = tmp.path().join("runtime-root");
    let cwd = tmp.path().join("cwd");
    fs::create_dir_all(&cwd).expect("cwd");
    write_node_layout(&runtime_root, "18.99.0");

    let out = run_envr(&["list", "node"], &runtime_root, &cwd);
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("18.99.0"),
        "expected installed version in list output:\n{stdout}"
    );
}

#[test]
fn list_node_porcelain_one_line_per_version() {
    let tmp = tempfile::tempdir().expect("tmp");
    let runtime_root = tmp.path().join("runtime-root");
    let cwd = tmp.path().join("cwd");
    fs::create_dir_all(&cwd).expect("cwd");
    write_node_layout(&runtime_root, "20.0.0");
    write_node_layout(&runtime_root, "20.1.0");

    let out = run_envr(&["--porcelain", "list", "node"], &runtime_root, &cwd);
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = stdout
        .lines()
        .map(str::trim)
        .filter(|l| {
            !l.is_empty()
                && l.chars().next().is_some_and(|c| c.is_ascii_digit())
                && l.chars().all(|c| c.is_ascii_digit() || c == '.')
        })
        .collect();
    assert_eq!(lines.len(), 2, "expected two version lines, got:\n{stdout}");
    assert!(lines.iter().any(|l| l.contains("20.0.0")), "{stdout}");
    assert!(lines.iter().any(|l| l.contains("20.1.0")), "{stdout}");
}

#[test]
fn which_node_resolves_with_project_pin() {
    let tmp = tempfile::tempdir().expect("tmp");
    let runtime_root = tmp.path().join("runtime-root");
    let project = tmp.path().join("project");
    fs::create_dir_all(&project).expect("project");
    write_node_layout(&runtime_root, "20.10.0");
    fs::write(
        project.join(".envr.toml"),
        "[runtimes.node]\nversion = \"20.10.0\"\n",
    )
    .expect("envr.toml");

    let out = run_envr(&["which", "node"], &runtime_root, &project);
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.to_lowercase().contains("20.10.0"), "stdout={stdout}");
}

#[test]
fn uninstall_dry_run_succeeds_for_mock_node() {
    let tmp = tempfile::tempdir().expect("tmp");
    let runtime_root = tmp.path().join("runtime-root");
    let cwd = tmp.path().join("cwd");
    fs::create_dir_all(&cwd).expect("cwd");
    write_node_layout(&runtime_root, "18.99.0");

    let out = run_envr(
        &["uninstall", "--dry-run", "node", "18.99.0"],
        &runtime_root,
        &cwd,
    );
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn why_node_shows_pin_and_home() {
    let tmp = tempfile::tempdir().expect("tmp");
    let runtime_root = tmp.path().join("runtime-root");
    let project = tmp.path().join("project");
    fs::create_dir_all(&project).expect("project");
    write_node_layout(&runtime_root, "20.1.0");
    fs::write(
        project.join(".envr.toml"),
        "[runtimes.node]\nversion = \"20.1.0\"\n",
    )
    .expect("envr.toml");

    let out = run_envr(&["why", "node"], &runtime_root, &project);
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("20.1.0") && (stdout.contains("versions") || stdout.contains("runtimes")),
        "expected pin and resolved path in why output:\n{stdout}"
    );
}

#[test]
fn why_node_spec_overrides_project_pin() {
    let tmp = tempfile::tempdir().expect("tmp");
    let runtime_root = tmp.path().join("runtime-root");
    let project = tmp.path().join("project");
    fs::create_dir_all(&project).expect("project");
    write_node_layout(&runtime_root, "20.0.0");
    write_node_layout(&runtime_root, "20.1.0");
    fs::write(
        project.join(".envr.toml"),
        "[runtimes.node]\nversion = \"20.0.0\"\n",
    )
    .expect("envr.toml");

    let out = run_envr(
        &["why", "node", "--spec", "20.1.0"],
        &runtime_root,
        &project,
    );
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("20.1.0") && stdout.contains("versions"),
        "expected --spec to resolve 20.1.0 tree:\n{stdout}"
    );
}

#[test]
fn run_emits_script_miss_hint_when_scripts_exist_and_token_is_not_script() {
    let tmp = tempfile::tempdir().expect("tmp");
    let runtime_root = tmp.path().join("runtime-root");
    let project = tmp.path().join("project");
    fs::create_dir_all(&project).expect("project");
    fs::write(
        project.join(".envr.toml"),
        r#"
[scripts]
build = "echo ok"
"#,
    )
    .expect("envr.toml");

    let out = run_envr(
        &["run", "envr_probe_missing_script"],
        &runtime_root,
        &project,
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("exec") && stderr.contains("--lang"),
        "expected script-miss hint on stderr; got:\n{stderr}"
    );
}

#[test]
fn run_does_not_emit_script_miss_hint_for_common_binaries_when_scripts_exist() {
    let tmp = tempfile::tempdir().expect("tmp");
    let runtime_root = tmp.path().join("runtime-root");
    let project = tmp.path().join("project");
    fs::create_dir_all(&project).expect("project");
    fs::write(
        project.join(".envr.toml"),
        r#"
[scripts]
build = "echo ok"
"#,
    )
    .expect("envr.toml");

    let out = run_envr(&["run", "node", "--version"], &runtime_root, &project);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.contains("not a script name"),
        "did not expect script-miss hint for `node`; stderr:\n{stderr}"
    );
}

#[test]
fn use_prints_global_current_note_after_success() {
    let tmp = tempfile::tempdir().expect("tmp");
    let runtime_root = tmp.path().join("runtime-root");
    let cwd = tmp.path().join("cwd");
    fs::create_dir_all(&cwd).expect("cwd");
    write_node_layout(&runtime_root, "20.10.0");

    let out = run_envr(&["use", "node", "20.10.0"], &runtime_root, &cwd);
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("ENVR_RUNTIME_ROOT")
            && (stdout.contains("global") || stdout.contains("全局")),
        "expected global current note on stdout; got:\n{stdout}"
    );
}

#[test]
fn status_without_project_prints_next_step_hints() {
    let tmp = tempfile::tempdir().expect("tmp");
    let runtime_root = tmp.path().join("runtime-root");
    let cwd = tmp.path().join("cwd");
    fs::create_dir_all(&cwd).expect("cwd");

    let out = run_envr(&["status"], &runtime_root, &cwd);
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("envr init") && stdout.contains("envr doctor"),
        "expected onboarding hints when no project; got:\n{stdout}"
    );
}
