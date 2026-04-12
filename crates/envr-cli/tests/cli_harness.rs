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

    let out = run_envr(
        &["--porcelain", "list", "node"],
        &runtime_root,
        &cwd,
    );
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
    assert!(
        stdout.to_lowercase().contains("20.10.0"),
        "stdout={stdout}"
    );
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
        stdout.contains("20.1.0")
            && (stdout.contains("versions") || stdout.contains("runtimes")),
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
