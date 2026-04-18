//! Integration: `--env`, `--env-file`, `exec --output`, and `template`.

use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::process::Output;

/// Keep PATH minimal so `envr run` / `template` do not pick up a user `rustup` while probing an
/// empty envr rust dir (slow in parallel integration tests).
fn narrow_path_for_envr_process() -> String {
    #[cfg(windows)]
    {
        let root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_string());
        format!("{root}\\System32;{root}")
    }
    #[cfg(not(windows))]
    {
        "/usr/bin:/bin".to_string()
    }
}

fn run_envr(args: &[&str], runtime_root: &Path, cwd: &Path) -> Output {
    Command::cargo_bin("envr")
        .expect("envr binary")
        .env("ENVR_RUNTIME_ROOT", runtime_root.as_os_str())
        .env("PATH", narrow_path_for_envr_process())
        .current_dir(cwd)
        .args(args)
        .output()
        .expect("run envr")
}

fn run_envr_with_envr_root(
    args: &[&str],
    envr_root: &Path,
    runtime_root: &Path,
    cwd: &Path,
) -> Output {
    Command::cargo_bin("envr")
        .expect("envr binary")
        .env("ENVR_ROOT", envr_root.as_os_str())
        .env("ENVR_RUNTIME_ROOT", runtime_root.as_os_str())
        .env("PATH", narrow_path_for_envr_process())
        .current_dir(cwd)
        .args(args)
        .output()
        .expect("run envr")
}

fn run_envr_with_envr_root_and_path(
    args: &[&str],
    envr_root: &Path,
    runtime_root: &Path,
    cwd: &Path,
    path: &str,
) -> Output {
    Command::cargo_bin("envr")
        .expect("envr binary")
        .env("ENVR_ROOT", envr_root.as_os_str())
        .env("ENVR_RUNTIME_ROOT", runtime_root.as_os_str())
        .env("PATH", path)
        .current_dir(cwd)
        .args(args)
        .output()
        .expect("run envr")
}

fn parse_json_line(stdout: &[u8]) -> Value {
    for line in stdout.split(|b| *b == b'\n') {
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

fn write_node_layout(runtime_root: &Path, version: &str) {
    let ver = runtime_root.join("runtimes/node/versions").join(version);
    let bin = ver.join("bin");
    fs::create_dir_all(&bin).expect("create node bin");
    #[cfg(windows)]
    fs::write(bin.join("node.exe"), []).expect("touch node.exe");
    #[cfg(not(windows))]
    fs::write(bin.join("node"), []).expect("touch node");
}

fn write_go_layout(runtime_root: &Path, version: &str) {
    let ver = runtime_root.join("runtimes/go/versions").join(version);
    let bin = ver.join("bin");
    fs::create_dir_all(&bin).expect("create go bin");
    #[cfg(windows)]
    fs::write(bin.join("go.exe"), []).expect("touch go.exe");
    #[cfg(not(windows))]
    fs::write(bin.join("go"), []).expect("touch go");
}

fn write_dotnet_layout(runtime_root: &Path, version: &str) {
    let ver = runtime_root.join("runtimes/dotnet/versions").join(version);
    fs::create_dir_all(&ver).expect("create dotnet dir");
    #[cfg(windows)]
    fs::write(ver.join("dotnet.exe"), []).expect("touch dotnet.exe");
    #[cfg(not(windows))]
    fs::write(ver.join("dotnet"), []).expect("touch dotnet");
}

fn write_zig_layout(runtime_root: &Path, version: &str) {
    let zig_home = runtime_root.join("runtimes/zig");
    let ver = zig_home.join("versions").join(version);
    fs::create_dir_all(&ver).expect("create zig version dir");
    #[cfg(windows)]
    fs::write(ver.join("zig.exe"), []).expect("touch zig.exe");
    #[cfg(not(windows))]
    fs::write(ver.join("zig"), []).expect("touch zig");
    let current = zig_home.join("current");
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let rel = format!("versions/{version}");
        symlink(rel, &current).expect("zig current symlink");
    }
    #[cfg(windows)]
    {
        let abs = fs::canonicalize(&ver).unwrap_or_else(|_| ver.clone());
        fs::write(&current, format!("{}\n", abs.display())).expect("zig current pointer");
    }
}

fn write_ruby_layout(runtime_root: &Path, version: &str) {
    let ver = runtime_root.join("runtimes/ruby/versions").join(version);
    let bin = ver.join("bin");
    fs::create_dir_all(&bin).expect("create ruby bin");
    #[cfg(windows)]
    {
        fs::write(bin.join("ruby.exe"), []).expect("touch ruby.exe");
        fs::write(bin.join("gem.exe"), []).expect("touch gem.exe");
        fs::write(bin.join("bundle.exe"), []).expect("touch bundle.exe");
        fs::write(bin.join("irb.exe"), []).expect("touch irb.exe");
    }
    #[cfg(not(windows))]
    {
        fs::write(bin.join("ruby"), []).expect("touch ruby");
        fs::write(bin.join("gem"), []).expect("touch gem");
        fs::write(bin.join("bundle"), []).expect("touch bundle");
        fs::write(bin.join("irb"), []).expect("touch irb");
    }
}

fn write_system_ruby_on_path(system_bin: &Path) {
    fs::create_dir_all(system_bin).expect("system bin dir");
    #[cfg(windows)]
    fs::write(system_bin.join("ruby.exe"), []).expect("touch system ruby.exe");
    #[cfg(not(windows))]
    fs::write(system_bin.join("ruby"), []).expect("touch system ruby");
}

fn project_with_node_pin(project: &Path, runtime_root: &Path, version: &str) {
    fs::create_dir_all(project).expect("project dir");
    write_node_layout(runtime_root, version);
    fs::write(
        project.join(".envr.toml"),
        format!(
            r#"
[runtimes.node]
version = "{version}"
"#
        ),
    )
    .expect("write .envr.toml");
}

fn project_with_runtime_pin(project: &Path, key: &str, version: &str) {
    fs::create_dir_all(project).expect("project dir");
    fs::write(
        project.join(".envr.toml"),
        format!(
            r#"
[runtimes.{key}]
version = "{version}"
"#
        ),
    )
    .expect("write .envr.toml");
}

fn write_settings_ruby_path_proxy(envr_root: &Path, path_proxy_enabled: bool) {
    let cfg_dir = envr_root.join("config");
    fs::create_dir_all(&cfg_dir).expect("config dir");
    let cfg = cfg_dir.join("settings.toml");
    fs::write(
        cfg,
        format!(
            "[runtime.ruby]\npath_proxy_enabled = {}\n",
            if path_proxy_enabled { "true" } else { "false" }
        ),
    )
    .expect("write settings.toml");
}

#[test]
fn exec_dry_run_includes_env_overrides() {
    let tmp = tempfile::tempdir().expect("tmp");
    let runtime_root = tmp.path().join("runtime-root");
    let project = tmp.path().join("project");
    project_with_node_pin(&project, &runtime_root, "20.10.0");

    let out = if cfg!(windows) {
        run_envr(
            &[
                "exec",
                "--lang",
                "node",
                "--dry-run",
                "--env",
                "ENVR_IT_EXEC=from_cli",
                "cmd",
                "/c",
                "echo",
                "noop",
            ],
            &runtime_root,
            &project,
        )
    } else {
        run_envr(
            &[
                "exec",
                "--lang",
                "node",
                "--dry-run",
                "--env",
                "ENVR_IT_EXEC=from_cli",
                "sh",
                "-c",
                "echo noop",
            ],
            &runtime_root,
            &project,
        )
    };
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("ENVR_IT_EXEC=from_cli"),
        "dry-run should list overridden env; stdout:\n{stdout}"
    );
}

#[test]
fn exec_dry_run_env_file_then_cli_override() {
    let tmp = tempfile::tempdir().expect("tmp");
    let runtime_root = tmp.path().join("runtime-root");
    let project = tmp.path().join("project");
    project_with_node_pin(&project, &runtime_root, "20.10.0");

    let env_path = project.join("ci.env");
    fs::write(&env_path, "ENVR_IT_LAYER=file\n").expect("write env file");
    let ep = env_path.to_string_lossy();

    let out = if cfg!(windows) {
        run_envr(
            &[
                "exec",
                "--lang",
                "node",
                "--dry-run",
                "--env-file",
                ep.as_ref(),
                "--env",
                "ENVR_IT_LAYER=from_cli",
                "cmd",
                "/c",
                "echo",
                "noop",
            ],
            &runtime_root,
            &project,
        )
    } else {
        run_envr(
            &[
                "exec",
                "--lang",
                "node",
                "--dry-run",
                "--env-file",
                ep.as_ref(),
                "--env",
                "ENVR_IT_LAYER=from_cli",
                "sh",
                "-c",
                "echo noop",
            ],
            &runtime_root,
            &project,
        )
    };
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("ENVR_IT_LAYER=from_cli"),
        "CLI --env must win over file; got:\n{stdout}"
    );
}

#[test]
fn exec_child_sees_injected_env() {
    let tmp = tempfile::tempdir().expect("tmp");
    let runtime_root = tmp.path().join("runtime-root");
    let project = tmp.path().join("project");
    project_with_node_pin(&project, &runtime_root, "20.10.0");

    let out = if cfg!(windows) {
        run_envr(
            &[
                "exec",
                "--lang",
                "node",
                "--env",
                "ENVR_IT_CHILD=child_ok",
                "cmd",
                "/c",
                "echo",
                "%ENVR_IT_CHILD%",
            ],
            &runtime_root,
            &project,
        )
    } else {
        run_envr(
            &[
                "exec",
                "--lang",
                "node",
                "--env",
                "ENVR_IT_CHILD=child_ok",
                "sh",
                "-c",
                "echo \"$ENVR_IT_CHILD\"",
            ],
            &runtime_root,
            &project,
        )
    };
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("child_ok"),
        "child stdout should contain value; got:\n{stdout}"
    );
}

#[test]
fn exec_output_redirects_child_streams() {
    let tmp = tempfile::tempdir().expect("tmp");
    let runtime_root = tmp.path().join("runtime-root");
    let project = tmp.path().join("project");
    project_with_node_pin(&project, &runtime_root, "20.10.0");

    let log = project.join("child.log");
    let lp = log.to_string_lossy();

    let out = if cfg!(windows) {
        run_envr(
            &[
                "exec",
                "--lang",
                "node",
                "--output",
                lp.as_ref(),
                "cmd",
                "/c",
                "echo OUT_LINE && echo ERR_LINE 1>&2",
            ],
            &runtime_root,
            &project,
        )
    } else {
        run_envr(
            &[
                "exec",
                "--lang",
                "node",
                "--output",
                lp.as_ref(),
                "sh",
                "-c",
                "echo OUT_LINE; echo ERR_LINE >&2",
            ],
            &runtime_root,
            &project,
        )
    };
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let body = fs::read_to_string(&log).expect("read log");
    assert!(
        body.contains("OUT_LINE"),
        "log should contain stdout line; body={body:?}"
    );
    assert!(
        body.contains("ERR_LINE"),
        "log should contain stderr line; body={body:?}"
    );
}

#[test]
fn exec_dry_run_dotnet_includes_runtime_home_env() {
    let tmp = tempfile::tempdir().expect("tmp");
    let runtime_root = tmp.path().join("runtime-root");
    let project = tmp.path().join("project");
    write_dotnet_layout(&runtime_root, "8.0.420");
    project_with_runtime_pin(&project, "dotnet", "8.0.420");

    let out = if cfg!(windows) {
        run_envr(
            &[
                "exec",
                "--lang",
                "dotnet",
                "--dry-run",
                "cmd",
                "/c",
                "echo",
                "noop",
            ],
            &runtime_root,
            &project,
        )
    } else {
        run_envr(
            &[
                "exec",
                "--lang",
                "dotnet",
                "--dry-run",
                "sh",
                "-c",
                "echo noop",
            ],
            &runtime_root,
            &project,
        )
    };
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("DOTNET_ROOT="),
        "dry-run should include DOTNET_ROOT; stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("DOTNET_MULTILEVEL_LOOKUP=0"),
        "dry-run should disable multilevel lookup; stdout:\n{stdout}"
    );
}

#[test]
fn exec_dry_run_zig_resolves_project_pin() {
    let tmp = tempfile::tempdir().expect("tmp");
    let runtime_root = tmp.path().join("runtime-root");
    let project = tmp.path().join("project");
    write_zig_layout(&runtime_root, "0.14.1");
    project_with_runtime_pin(&project, "zig", "0.14.1");

    let out = if cfg!(windows) {
        run_envr(
            &[
                "exec",
                "--lang",
                "zig",
                "--dry-run",
                "cmd",
                "/c",
                "echo",
                "noop",
            ],
            &runtime_root,
            &project,
        )
    } else {
        run_envr(
            &[
                "exec",
                "--lang",
                "zig",
                "--dry-run",
                "sh",
                "-c",
                "echo noop",
            ],
            &runtime_root,
            &project,
        )
    };
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let path_line = stdout
        .lines()
        .find(|l| l.starts_with("PATH=") || l.starts_with("Path="))
        .unwrap_or_else(|| panic!("expected PATH in dry-run stdout:\n{stdout}"));
    assert!(
        path_line.contains("0.14.1") && path_line.to_lowercase().contains("zig"),
        "PATH should include zig version home; line={path_line:?} full stdout:\n{stdout}"
    );
}

#[test]
fn run_dry_run_includes_env_overrides() {
    let tmp = tempfile::tempdir().expect("tmp");
    let runtime_root = tmp.path().join("runtime-root");
    let project = tmp.path().join("project");
    project_with_node_pin(&project, &runtime_root, "20.10.0");

    let out = if cfg!(windows) {
        run_envr(
            &[
                "run",
                "--dry-run",
                "--env",
                "ENVR_IT_RUN=run_layer",
                "cmd",
                "/c",
                "echo",
                "noop",
            ],
            &runtime_root,
            &project,
        )
    } else {
        run_envr(
            &[
                "run",
                "--dry-run",
                "--env",
                "ENVR_IT_RUN=run_layer",
                "sh",
                "-c",
                "echo noop",
            ],
            &runtime_root,
            &project,
        )
    };
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("ENVR_IT_RUN=run_layer"),
        "run dry-run should list overridden env; stdout:\n{stdout}"
    );
}

#[test]
fn run_dry_run_env_file_applies_before_command() {
    let tmp = tempfile::tempdir().expect("tmp");
    let runtime_root = tmp.path().join("runtime-root");
    let project = tmp.path().join("project");
    project_with_node_pin(&project, &runtime_root, "20.10.0");

    let env_path = project.join("run.env");
    fs::write(&env_path, "ENVR_IT_RUNFILE=from_file\n").expect("write run.env");
    let ep = env_path.to_string_lossy();

    let out = if cfg!(windows) {
        run_envr(
            &[
                "run",
                "--dry-run",
                "--env-file",
                ep.as_ref(),
                "cmd",
                "/c",
                "echo",
                "noop",
            ],
            &runtime_root,
            &project,
        )
    } else {
        run_envr(
            &[
                "run",
                "--dry-run",
                "--env-file",
                ep.as_ref(),
                "sh",
                "-c",
                "echo noop",
            ],
            &runtime_root,
            &project,
        )
    };
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("ENVR_IT_RUNFILE=from_file"),
        "run dry-run should list env from file; stdout:\n{stdout}"
    );
}

#[test]
fn run_dry_run_go_pin_includes_goroot() {
    let tmp = tempfile::tempdir().expect("tmp");
    let runtime_root = tmp.path().join("runtime-root");
    let project = tmp.path().join("project");
    write_go_layout(&runtime_root, "1.22.5");
    project_with_runtime_pin(&project, "go", "1.22.5");

    let out = if cfg!(windows) {
        run_envr(
            &["run", "--dry-run", "cmd", "/c", "echo", "noop"],
            &runtime_root,
            &project,
        )
    } else {
        run_envr(
            &["run", "--dry-run", "sh", "-c", "echo noop"],
            &runtime_root,
            &project,
        )
    };
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("GOROOT="),
        "run dry-run should include GOROOT for pinned go; stdout:\n{stdout}"
    );
}

#[test]
fn run_script_alias_executes_configured_command() {
    let tmp = tempfile::tempdir().expect("tmp");
    let runtime_root = tmp.path().join("runtime-root");
    let project = tmp.path().join("project");
    project_with_node_pin(&project, &runtime_root, "20.10.0");
    fs::write(
        project.join(".envr.toml"),
        r#"
[runtimes.node]
version = "20.10.0"

[scripts]
hi = "echo ENVR_SCRIPT_MARK"
"#,
    )
    .expect("write .envr.toml with scripts");

    let out = run_envr(&["run", "hi"], &runtime_root, &project);
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let out_s = String::from_utf8_lossy(&out.stdout);
    let err_s = String::from_utf8_lossy(&out.stderr);
    assert!(
        out_s.contains("ENVR_SCRIPT_MARK") || err_s.contains("ENVR_SCRIPT_MARK"),
        "expected script output; stdout={out_s} stderr={err_s}"
    );
}

#[test]
fn run_script_alias_dry_run_shows_resolved_shell() {
    let tmp = tempfile::tempdir().expect("tmp");
    let runtime_root = tmp.path().join("runtime-root");
    let project = tmp.path().join("project");
    project_with_node_pin(&project, &runtime_root, "20.10.0");
    fs::write(
        project.join(".envr.toml"),
        r#"
[runtimes.node]
version = "20.10.0"

[scripts]
hi = "echo ENVR_SCRIPT_MARK"
"#,
    )
    .expect("write .envr.toml with scripts");

    let out = run_envr(&["run", "--dry-run", "hi"], &runtime_root, &project);
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("ENVR_SCRIPT_MARK"),
        "dry-run should show resolved command; stdout:\n{stdout}"
    );
    #[cfg(windows)]
    assert!(
        stdout.to_ascii_lowercase().contains("cmd.exe"),
        "windows dry-run should use cmd; stdout:\n{stdout}"
    );
    #[cfg(not(windows))]
    assert!(
        stdout.contains("sh") || stdout.contains("echo ENVR_SCRIPT_MARK"),
        "unix dry-run should show shell; stdout:\n{stdout}"
    );
}

#[test]
fn template_substitutes_placeholder() {
    let tmp = tempfile::tempdir().expect("tmp");
    let runtime_root = tmp.path().join("runtime-root");
    let project = tmp.path().join("project");
    project_with_node_pin(&project, &runtime_root, "20.10.0");

    let tpl = project.join("app.tpl");
    fs::write(&tpl, r#"{"marker":"${ENVR_IT_TMPL}"}"#).expect("write tpl");
    let tp = tpl.to_string_lossy();

    let out = run_envr(
        &["template", "--env", "ENVR_IT_TMPL=replaced", tp.as_ref()],
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
        stdout.contains("replaced") && stdout.contains("marker"),
        "expected substituted JSON line; stdout:\n{stdout}"
    );
}

#[test]
fn exec_json_child_completed_lists_output_and_env_fields() {
    let tmp = tempfile::tempdir().expect("tmp");
    let runtime_root = tmp.path().join("runtime-root");
    let project = tmp.path().join("project");
    project_with_node_pin(&project, &runtime_root, "20.10.0");

    let log = project.join("out.json.log");
    let lp = log.to_string_lossy();

    let out = if cfg!(windows) {
        run_envr(
            &[
                "--format",
                "json",
                "exec",
                "--lang",
                "node",
                "--env",
                "ENVR_IT_JSON=1",
                "--output",
                lp.as_ref(),
                "cmd",
                "/c",
                "echo",
                "json_child",
            ],
            &runtime_root,
            &project,
        )
    } else {
        run_envr(
            &[
                "--format",
                "json",
                "exec",
                "--lang",
                "node",
                "--env",
                "ENVR_IT_JSON=1",
                "--output",
                lp.as_ref(),
                "sh",
                "-c",
                "echo json_child",
            ],
            &runtime_root,
            &project,
        )
    };
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_line(&out.stdout);
    assert_eq!(v["success"], true);
    assert_eq!(v["code"], "child_completed");
    let d = &v["data"];
    assert_eq!(d["env_overrides"], serde_json::json!(["ENVR_IT_JSON=1"]));
    let files = d["env_files"].as_array().expect("env_files");
    assert!(files.is_empty());
    assert_eq!(
        d["output_file"],
        serde_json::Value::String(log.display().to_string())
    );
    let body = fs::read_to_string(&log).expect("read log");
    assert!(
        body.contains("json_child"),
        "redirected child output missing: {body:?}"
    );
}

#[test]
fn ruby_exec_dry_run_path_prefers_which_project_pin_home() {
    let tmp = tempfile::tempdir().expect("tmp");
    let envr_root = tmp.path().join("envr-root");
    let runtime_root = tmp.path().join("runtime-root");
    let project = tmp.path().join("project");
    fs::create_dir_all(&project).expect("project dir");

    write_settings_ruby_path_proxy(&envr_root, true);
    write_ruby_layout(&runtime_root, "3.3.11");
    project_with_runtime_pin(&project, "ruby", "3.3.11");

    let which_out = run_envr_with_envr_root(
        &["--format", "json", "which", "ruby"],
        &envr_root,
        &runtime_root,
        &project,
    );
    assert!(
        which_out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&which_out.stderr)
    );
    let which_v = parse_json_line(&which_out.stdout);
    assert_eq!(which_v["code"], serde_json::json!("resolved_executable"));
    assert_eq!(which_v["data"]["version"], serde_json::json!("3.3.11"));
    let which_exe = which_v["data"]["executable"]
        .as_str()
        .expect("which executable string");

    let ps = if cfg!(windows) { '\\' } else { '/' };
    let home_sub = format!("runtimes{}ruby{}versions{}3.3.11", ps, ps, ps);
    let home_bin_sub = format!("runtimes{}ruby{}versions{}3.3.11{}bin", ps, ps, ps, ps);
    assert!(
        which_exe.contains(&home_bin_sub) || which_exe.contains(&home_sub),
        "expected which executable under runtimes/ruby/versions/3.3.11; got {which_exe}"
    );

    let exec_out = run_envr_with_envr_root(
        &[
            "--format",
            "json",
            "exec",
            "--lang",
            "ruby",
            "--dry-run",
            "ruby",
            "-v",
        ],
        &envr_root,
        &runtime_root,
        &project,
    );
    assert!(
        exec_out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&exec_out.stderr)
    );
    let exec_v = parse_json_line(&exec_out.stdout);
    assert_eq!(exec_v["code"], serde_json::json!("dry_run"));

    let env_path = exec_v["data"]["env"]["PATH"]
        .as_str()
        .expect("dry-run PATH string");

    let sep = if cfg!(windows) { ';' } else { ':' };
    let parts: Vec<&str> = env_path.split(sep).collect();
    assert!(parts.len() >= 2, "unexpected PATH format: {env_path}");
    assert!(
        parts[0].contains(&home_bin_sub),
        "expected PATH[0] to be ruby bin dir; got {env_path}",
    );
    assert!(
        parts[1].contains(&home_sub),
        "expected PATH[1] to be ruby home dir; got {env_path}",
    );
}

#[test]
fn ruby_exec_dry_run_path_prefers_which_ruby_version_home_when_no_envr_pin() {
    let tmp = tempfile::tempdir().expect("tmp");
    let envr_root = tmp.path().join("envr-root");
    let runtime_root = tmp.path().join("runtime-root");
    let project = tmp.path().join("project");
    fs::create_dir_all(&project).expect("project dir");

    write_settings_ruby_path_proxy(&envr_root, true);
    write_ruby_layout(&runtime_root, "3.3.11");
    fs::write(project.join(".ruby-version"), "3.3.11\n").expect("write .ruby-version");

    let which_out = run_envr_with_envr_root(
        &["--format", "json", "which", "ruby"],
        &envr_root,
        &runtime_root,
        &project,
    );
    assert!(
        which_out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&which_out.stderr)
    );
    let which_v = parse_json_line(&which_out.stdout);
    assert_eq!(which_v["code"], serde_json::json!("resolved_executable"));
    assert_eq!(which_v["data"]["version"], serde_json::json!("3.3.11"));

    let ps = if cfg!(windows) { '\\' } else { '/' };
    let home_sub = format!("runtimes{}ruby{}versions{}3.3.11", ps, ps, ps);
    let home_bin_sub = format!("runtimes{}ruby{}versions{}3.3.11{}bin", ps, ps, ps, ps);

    let exec_out = run_envr_with_envr_root(
        &[
            "--format",
            "json",
            "exec",
            "--lang",
            "ruby",
            "--dry-run",
            "ruby",
            "-v",
        ],
        &envr_root,
        &runtime_root,
        &project,
    );
    assert!(
        exec_out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&exec_out.stderr)
    );
    let exec_v = parse_json_line(&exec_out.stdout);
    assert_eq!(exec_v["code"], serde_json::json!("dry_run"));

    let env_path = exec_v["data"]["env"]["PATH"]
        .as_str()
        .expect("dry-run PATH string");

    let sep = if cfg!(windows) { ';' } else { ':' };
    let parts: Vec<&str> = env_path.split(sep).collect();
    assert!(parts.len() >= 2, "unexpected PATH format: {env_path}");
    assert!(
        parts[0].contains(&home_bin_sub),
        "expected PATH[0] to be ruby bin dir; got {env_path}"
    );
    assert!(
        parts[1].contains(&home_sub),
        "expected PATH[1] to be ruby home dir; got {env_path}"
    );
}

#[test]
fn which_ruby_bypass_uses_system_path_when_path_proxy_disabled() {
    let tmp = tempfile::tempdir().expect("tmp");
    let envr_root = tmp.path().join("envr-root");
    let runtime_root = tmp.path().join("runtime-root");
    let project = tmp.path().join("project");
    let system_bin = tmp.path().join("system-bin");
    fs::create_dir_all(&project).expect("project dir");

    write_settings_ruby_path_proxy(&envr_root, false);
    write_system_ruby_on_path(&system_bin);

    let narrow = narrow_path_for_envr_process();
    let sep = if cfg!(windows) { ';' } else { ':' };
    let path = format!("{}{}{}", system_bin.display(), sep, narrow);

    let which_out = run_envr_with_envr_root_and_path(
        &["--format", "json", "which", "ruby"],
        &envr_root,
        &runtime_root,
        &project,
        &path,
    );
    assert!(
        which_out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&which_out.stderr)
    );

    let which_v = parse_json_line(&which_out.stdout);
    assert_eq!(which_v["code"], serde_json::json!("resolved_executable"));
    assert_eq!(
        which_v["data"]["selection_source"],
        serde_json::json!("path_proxy_bypass")
    );
    assert_eq!(which_v["data"]["version"], serde_json::json!("system"));

    let exe = which_v["data"]["executable"]
        .as_str()
        .expect("executable string");
    assert!(
        exe.contains(&system_bin.display().to_string()),
        "expected system ruby under system-bin; exe={exe}"
    );
}

#[test]
fn which_ruby_prefers_envr_runtime_home_when_path_proxy_enabled() {
    let tmp = tempfile::tempdir().expect("tmp");
    let envr_root = tmp.path().join("envr-root");
    let runtime_root = tmp.path().join("runtime-root");
    let project = tmp.path().join("project");
    let system_bin = tmp.path().join("system-bin");
    fs::create_dir_all(&project).expect("project dir");

    write_settings_ruby_path_proxy(&envr_root, true);
    write_system_ruby_on_path(&system_bin);

    write_ruby_layout(&runtime_root, "3.3.11");
    fs::write(
        project.join(".envr.toml"),
        "[runtimes.ruby]\nversion = \"3.3.11\"\n",
    )
    .expect("envr.toml");

    let narrow = narrow_path_for_envr_process();
    let sep = if cfg!(windows) { ';' } else { ':' };
    let path = format!("{}{}{}", system_bin.display(), sep, narrow);

    let which_out = run_envr_with_envr_root_and_path(
        &["--format", "json", "which", "ruby"],
        &envr_root,
        &runtime_root,
        &project,
        &path,
    );
    assert!(
        which_out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&which_out.stderr)
    );

    let which_v = parse_json_line(&which_out.stdout);
    assert_eq!(which_v["code"], serde_json::json!("resolved_executable"));
    assert_eq!(
        which_v["data"]["selection_source"],
        serde_json::json!("project_pin")
    );
    assert_eq!(which_v["data"]["version"], serde_json::json!("3.3.11"));

    let exe = which_v["data"]["executable"]
        .as_str()
        .expect("executable string");
    let ps = if cfg!(windows) { '\\' } else { '/' };
    let expected_sub = format!("runtimes{}ruby{}versions{}3.3.11{}bin", ps, ps, ps, ps);
    assert!(
        exe.contains(&expected_sub),
        "expected runtime ruby home in executable; exe={exe}"
    );
    assert!(
        !exe.contains(&system_bin.display().to_string()),
        "system ruby should not be chosen when path proxy is enabled; exe={exe}"
    );
}
