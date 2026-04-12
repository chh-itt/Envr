use assert_cmd::Command;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Output;
use std::time::{Duration, SystemTime};

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
    {
        fs::write(bin.join("node.exe"), []).expect("touch node.exe");
    }
    #[cfg(not(windows))]
    {
        fs::write(bin.join("node"), []).expect("touch node");
    }
}

#[test]
fn which_resolves_project_pinned_node() {
    let tmp = tempfile::tempdir().expect("tmp");
    let runtime_root = tmp.path().join("runtime-root");
    let project = tmp.path().join("project");
    fs::create_dir_all(&project).expect("project dir");
    write_node_layout(&runtime_root, "20.10.0");

    fs::write(
        project.join(".envr.toml"),
        r#"
[runtimes.node]
version = "20.10.0"
"#,
    )
    .expect("write project config");

    let out = run_envr(&["which", "node"], &runtime_root, &project);
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let lower = stdout.to_lowercase();
    #[cfg(windows)]
    assert!(lower.contains("node.exe"), "stdout={stdout}");
    #[cfg(not(windows))]
    assert!(lower.contains("/node"), "stdout={stdout}");
    assert!(
        stdout.contains("20.10.0"),
        "expected version in which output: {stdout}"
    );
    assert!(
        lower.contains("project") || lower.contains(".envr"),
        "expected project source hint: {stdout}"
    );
}

#[test]
fn uninstall_removes_installed_version_dir() {
    let tmp = tempfile::tempdir().expect("tmp");
    let runtime_root = tmp.path().join("runtime-root");
    let project = tmp.path().join("project");
    fs::create_dir_all(&project).expect("project dir");
    write_node_layout(&runtime_root, "20.10.0");

    let version_dir = runtime_root.join("runtimes/node/versions/20.10.0");
    assert!(version_dir.is_dir(), "version must exist before uninstall");

    let out = run_envr(
        &["uninstall", "--yes", "node", "20.10.0"],
        &runtime_root,
        &project,
    );
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!version_dir.exists(), "version dir should be removed");
}

#[test]
fn uninstall_dry_run_prints_target_path() {
    let tmp = tempfile::tempdir().expect("tmp");
    let runtime_root = tmp.path().join("runtime-root");
    let project = tmp.path().join("project");
    fs::create_dir_all(&project).expect("project dir");
    write_node_layout(&runtime_root, "20.10.0");

    let out = run_envr(
        &["uninstall", "--dry-run", "node", "20.10.0"],
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
        stdout.contains("20.10.0") || stdout.contains("versions"),
        "unexpected stdout: {stdout}"
    );
}

#[test]
fn cache_clean_removes_kind_subdir() {
    let tmp = tempfile::tempdir().expect("tmp");
    let runtime_root = tmp.path().join("runtime-root");
    let project = tmp.path().join("project");
    fs::create_dir_all(&project).expect("project dir");

    let cache_node = runtime_root.join("cache/node");
    fs::create_dir_all(&cache_node).expect("cache dir");
    fs::write(cache_node.join("dummy.bin"), b"x").expect("cache file");
    assert!(cache_node.exists(), "cache dir should exist before clean");

    let out = run_envr(&["cache", "clean", "node"], &runtime_root, &project);
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!cache_node.exists(), "node cache dir should be removed");
}

#[test]
fn cache_clean_older_than_keeps_recent_files() {
    let tmp = tempfile::tempdir().expect("tmp");
    let runtime_root = tmp.path().join("runtime-root");
    let project = tmp.path().join("project");
    fs::create_dir_all(&project).expect("project dir");

    let cache_node = runtime_root.join("cache/node");
    fs::create_dir_all(&cache_node).expect("cache dir");
    fs::write(cache_node.join("dummy.bin"), b"x").expect("cache file");
    assert!(cache_node.join("dummy.bin").is_file());

    let out = run_envr(
        &["cache", "clean", "node", "--older-than", "99999d"],
        &runtime_root,
        &project,
    );
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        cache_node.join("dummy.bin").exists(),
        "recent file should remain with extreme older-than"
    );
    assert!(
        cache_node.exists(),
        "cache kind dir should remain when files kept"
    );
}

#[test]
fn cache_clean_newer_than_requires_older_than() {
    let tmp = tempfile::tempdir().expect("tmp");
    let runtime_root = tmp.path().join("runtime-root");
    let project = tmp.path().join("project");
    fs::create_dir_all(&project).expect("project dir");

    let out = run_envr(
        &["cache", "clean", "node", "--newer-than", "90d"],
        &runtime_root,
        &project,
    );
    assert!(
        !out.status.success(),
        "expected validation failure, stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn cache_clean_age_window_invalid() {
    let tmp = tempfile::tempdir().expect("tmp");
    let runtime_root = tmp.path().join("runtime-root");
    let project = tmp.path().join("project");
    fs::create_dir_all(&project).expect("project dir");

    let out = run_envr(
        &[
            "cache",
            "clean",
            "node",
            "--older-than",
            "30d",
            "--newer-than",
            "10d",
        ],
        &runtime_root,
        &project,
    );
    assert!(
        !out.status.success(),
        "expected validation failure, stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn cache_clean_dry_run_full_keeps_tree() {
    let tmp = tempfile::tempdir().expect("tmp");
    let runtime_root = tmp.path().join("runtime-root");
    let project = tmp.path().join("project");
    fs::create_dir_all(&project).expect("project dir");

    let cache_node = runtime_root.join("cache/node");
    fs::create_dir_all(&cache_node).expect("cache dir");
    fs::write(cache_node.join("f.bin"), vec![0u8; 10]).expect("cache file");

    let out = run_envr(
        &["cache", "clean", "node", "--dry-run"],
        &runtime_root,
        &project,
    );
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        cache_node.join("f.bin").exists(),
        "dry-run must not delete cache files"
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("[dry-run]") || stdout.contains("dry-run"),
        "expected dry-run hint in stdout: {stdout}"
    );
}

#[test]
fn cache_clean_dry_run_prune_counts_old_file() {
    let tmp = tempfile::tempdir().expect("tmp");
    let runtime_root = tmp.path().join("runtime-root");
    let project = tmp.path().join("project");
    fs::create_dir_all(&project).expect("project dir");

    let cache_node = runtime_root.join("cache/node");
    fs::create_dir_all(&cache_node).expect("cache dir");
    let path = cache_node.join("old.bin");
    {
        let mut f = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)
            .expect("open");
        f.write_all(b"x").expect("write");
        let ancient = SystemTime::UNIX_EPOCH + Duration::from_secs(86_400 * 400);
        f.set_modified(ancient).expect("set mtime");
    }

    let out = run_envr(
        &[
            "cache",
            "clean",
            "node",
            "--older-than",
            "30d",
            "--dry-run",
        ],
        &runtime_root,
        &project,
    );
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(path.exists(), "dry-run prune must not remove files");

    let json_out = run_envr(
        &[
            "--format",
            "json",
            "cache",
            "clean",
            "node",
            "--older-than",
            "30d",
            "--dry-run",
        ],
        &runtime_root,
        &project,
    );
    assert!(json_out.status.success(), "json stderr");
    let stdout = String::from_utf8_lossy(&json_out.stdout);
    assert!(
        stdout.contains("\"dry_run\":true") && stdout.contains("\"files_would_remove\":1"),
        "expected dry-run prune json fields: {stdout}"
    );
}
