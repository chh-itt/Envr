//! Integration: `--env`, `--env-file`, `exec --output`, and `template`.

use assert_cmd::Command;
use serde_json::Value;
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
        &[
            "template",
            "--env",
            "ENVR_IT_TMPL=replaced",
            tp.as_ref(),
        ],
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
    assert_eq!(v["message"], "child_completed");
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
