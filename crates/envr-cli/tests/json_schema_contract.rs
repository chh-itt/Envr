//! Validate `--format json` envelopes and selected `data` blobs against checked-in JSON Schemas.

use assert_cmd::Command;
use serde_json::Value;
use std::ffi::OsStr;
use std::fs;
use std::path::Path;

const ENVELOPE_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schemas/cli/envelope.json"
));
const LIST_DATA_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schemas/cli/data/list_installed.json"
));
const CURRENT_DATA_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schemas/cli/data/show_current.json"
));
const REMOTE_DATA_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schemas/cli/data/list_remote.json"
));
const CONFIG_PATH_DATA_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schemas/cli/data/config_path.json"
));
const PROJECT_STATUS_DATA_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schemas/cli/data/project_status.json"
));
const DOCTOR_OK_DATA_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schemas/cli/data/doctor_ok.json"
));
const DOCTOR_ISSUES_DATA_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schemas/cli/data/doctor_issues.json"
));
const DEACTIVATE_HINT_DATA_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schemas/cli/data/deactivate_hint.json"
));
const TEMPLATE_RENDERED_DATA_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schemas/cli/data/template_rendered.json"
));
const FAILURE_PROJECT_CHECK_FAILED_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schemas/cli/data/failure_project_check_failed.json"
));
const FAILURE_CHILD_EXIT_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schemas/cli/data/failure_child_exit.json"
));
const FAILURE_PROJECT_VALIDATE_FAILED_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schemas/cli/data/failure_project_validate_failed.json"
));
const FAILURE_DIAGNOSTICS_EXPORT_FAILED_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schemas/cli/data/failure_diagnostics_export_failed.json"
));
const RUNTIME_RESOLVED_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schemas/cli/data/runtime_resolved.json"
));
const RESOLVED_EXECUTABLE_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schemas/cli/data/resolved_executable.json"
));
const UPDATE_INFO_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schemas/cli/data/update_info.json"
));
const HOOK_PROMPT_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schemas/cli/data/hook_prompt.json"
));
const CONFIG_KEYS_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schemas/cli/data/config_keys.json"
));
const CONFIG_GET_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schemas/cli/data/config_get.json"
));
const CONFIG_SHOW_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schemas/cli/data/config_show.json"
));
const CONFIG_SET_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schemas/cli/data/config_set.json"
));
const DRY_RUN_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schemas/cli/data/dry_run.json"
));
const CACHE_CLEANED_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schemas/cli/data/cache_cleaned.json"
));
const CACHE_INDEX_STATUS_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schemas/cli/data/cache_index_status.json"
));
const CACHE_INDEX_SYNCED_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schemas/cli/data/cache_index_synced.json"
));
const BUNDLE_CREATED_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schemas/cli/data/bundle_created.json"
));
const BUNDLE_APPLIED_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schemas/cli/data/bundle_applied.json"
));
const PROJECT_VALIDATED_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schemas/cli/data/project_validated.json"
));
const PRUNE_DRY_RUN_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schemas/cli/data/prune_dry_run.json"
));
const CURRENT_RUNTIME_SET_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schemas/cli/data/current_runtime_set.json"
));
const CHILD_COMPLETED_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schemas/cli/data/child_completed.json"
));
const PROJECT_PIN_ADDED_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../schemas/cli/data/project_pin_added.json"
));

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

fn assert_valid(schema_src: &str, instance: &Value) {
    let schema_src = schema_src.trim_start_matches('\u{feff}');
    let schema: Value = serde_json::from_str(schema_src).expect("schema JSON");
    if let Err(e) = jsonschema::validate(&schema, instance) {
        panic!("schema validation failed: {e}");
    }
}

fn json_stdout(args: &[&str], root: &std::path::Path) -> Value {
    let out = Command::cargo_bin("envr")
        .expect("envr binary")
        .env("ENVR_RUNTIME_ROOT", root.as_os_str())
        .args(args)
        .output()
        .expect("envr output");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    parse_json_line(&out.stdout)
}

fn write_node_layout(runtime_root: &Path, version: &str) {
    let ver = runtime_root.join("runtimes/node/versions").join(version);
    let bin = ver.join("bin");
    fs::create_dir_all(&bin).expect("node bin dir");
    #[cfg(windows)]
    fs::write(bin.join("node.exe"), []).expect("touch node.exe");
    #[cfg(not(windows))]
    fs::write(bin.join("node"), []).expect("touch node");
}

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

#[test]
fn list_json_matches_schemas() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let v = json_stdout(&["--format", "json", "list"], tmp.path());
    assert_valid(ENVELOPE_SCHEMA, &v);
    assert_valid(LIST_DATA_SCHEMA, v.get("data").expect("data"));
}

#[test]
fn current_json_matches_schemas() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let v = json_stdout(&["--format", "json", "current"], tmp.path());
    assert_valid(ENVELOPE_SCHEMA, &v);
    assert_valid(CURRENT_DATA_SCHEMA, v.get("data").expect("data"));
}

#[test]
fn config_path_json_matches_schemas() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let v = json_stdout(&["--format", "json", "config", "path"], tmp.path());
    assert_valid(ENVELOPE_SCHEMA, &v);
    assert_valid(CONFIG_PATH_DATA_SCHEMA, v.get("data").expect("data"));
}

#[test]
fn project_status_json_matches_schemas() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let v = json_stdout(&["--format", "json", "status"], tmp.path());
    assert_valid(ENVELOPE_SCHEMA, &v);
    assert_valid(PROJECT_STATUS_DATA_SCHEMA, v.get("data").expect("data"));
}

#[test]
fn doctor_json_data_matches_schema() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let v = json_stdout(&["--format", "json", "doctor"], tmp.path());
    assert_valid(ENVELOPE_SCHEMA, &v);
    assert_valid(DOCTOR_OK_DATA_SCHEMA, v.get("data").expect("data"));
}

#[test]
fn doctor_issues_json_matches_schema_when_runtime_root_missing() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let missing_root = tmp.path().join("no_such_runtime_root");
    assert!(
        !missing_root.exists(),
        "precondition: missing ENVR_RUNTIME_ROOT dir"
    );
    let out = Command::cargo_bin("envr")
        .expect("envr binary")
        .env("ENVR_RUNTIME_ROOT", missing_root.as_os_str())
        .args(["--format", "json", "doctor"])
        .output()
        .expect("envr output");
    assert!(
        !out.status.success(),
        "expected non-zero exit, stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_line(&out.stdout);
    assert_valid(ENVELOPE_SCHEMA, &v);
    assert_eq!(v.get("success"), Some(&serde_json::json!(false)));
    assert_eq!(v.get("code"), Some(&serde_json::json!("doctor_issues")));
    assert!(
        v.get("message")
            .and_then(|m| m.as_str())
            .is_some_and(|s| !s.is_empty()),
        "failure message is localized text, not a stable token: {:?}",
        v.get("message")
    );
    let data = v.get("data").expect("data");
    let issues = data
        .get("issues")
        .and_then(|x| x.as_array())
        .expect("issues array");
    assert!(
        !issues.is_empty(),
        "expected at least one issue when runtime root is missing"
    );
    assert_valid(DOCTOR_ISSUES_DATA_SCHEMA, data);
}

#[test]
fn deactivate_hint_json_matches_schemas() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let v = json_stdout(&["--format", "json", "deactivate"], tmp.path());
    assert_valid(ENVELOPE_SCHEMA, &v);
    assert_valid(DEACTIVATE_HINT_DATA_SCHEMA, v.get("data").expect("data"));
}

#[test]
fn template_rendered_json_matches_schemas() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let tpl = tmp.path().join("ct.tpl");
    std::fs::write(&tpl, "x${PATH}").expect("write tpl");
    let p = tpl.to_string_lossy();
    let v = json_stdout(&["--format", "json", "template", p.as_ref()], tmp.path());
    assert_valid(ENVELOPE_SCHEMA, &v);
    assert_valid(TEMPLATE_RENDERED_DATA_SCHEMA, v.get("data").expect("data"));
}

#[test]
fn check_failure_project_check_failed_matches_schema() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let proj = tmp.path().join("proj");
    std::fs::create_dir_all(&proj).expect("mkdir");
    let toml = r#"
[runtimes.node]
version = "0.0.0-envr-schema-contract-nonexistent"
"#;
    std::fs::write(proj.join(".envr.toml"), toml).expect("write envr.toml");
    let out = Command::cargo_bin("envr")
        .expect("envr binary")
        .current_dir(&proj)
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .args(["--format", "json", "check"])
        .output()
        .expect("envr output");
    assert!(!out.status.success());
    let v = parse_json_line(&out.stdout);
    assert_valid(ENVELOPE_SCHEMA, &v);
    assert_eq!(v.get("success"), Some(&serde_json::json!(false)));
    assert_eq!(
        v.get("code"),
        Some(&serde_json::json!("project_check_failed"))
    );
    assert_valid(
        FAILURE_PROJECT_CHECK_FAILED_SCHEMA,
        v.get("data").expect("data"),
    );
}

#[test]
fn run_child_exit_failure_matches_schema() {
    let tmp = tempfile::tempdir().expect("tmp");
    let runtime_root = tmp.path().join("rr");
    let cwd = tmp.path().join("cwd");
    fs::create_dir_all(&cwd).expect("cwd");

    let args: &[&str] = if cfg!(windows) {
        &["--format", "json", "run", "cmd", "/c", "exit", "7"]
    } else {
        &["--format", "json", "run", "sh", "-c", "exit 7"]
    };

    let out = Command::cargo_bin("envr")
        .expect("envr binary")
        .current_dir(&cwd)
        .env("ENVR_RUNTIME_ROOT", runtime_root.as_os_str())
        .args(args)
        .output()
        .expect("run child_exit");
    assert!(!out.status.success(), "expected non-zero child exit");

    let v = parse_json_line(&out.stdout);
    assert_valid(ENVELOPE_SCHEMA, &v);
    assert_eq!(v.get("code"), Some(&serde_json::json!("child_exit")));
    assert_valid(FAILURE_CHILD_EXIT_SCHEMA, v.get("data").expect("data"));
}

#[test]
fn project_validate_failure_matches_schema() {
    let tmp = tempfile::tempdir().expect("tmp");
    let runtime_root = tmp.path().join("rr");
    let project = tmp.path().join("proj");
    fs::create_dir_all(&project).expect("proj");
    fs::write(
        project.join(".envr.toml"),
        "[runtimes.node]\nversion = \"0.0.0-nonexistent-envr\"\n",
    )
    .expect("envr.toml");

    let out = Command::cargo_bin("envr")
        .expect("envr binary")
        .current_dir(&project)
        .env("ENVR_RUNTIME_ROOT", runtime_root.as_os_str())
        .args(["--format", "json", "project", "validate"])
        .output()
        .expect("project validate failure");
    assert!(!out.status.success(), "expected validation failure");

    let v = parse_json_line(&out.stdout);
    assert_valid(ENVELOPE_SCHEMA, &v);
    assert_eq!(
        v.get("code"),
        Some(&serde_json::json!("project_validate_failed"))
    );
    assert_valid(
        FAILURE_PROJECT_VALIDATE_FAILED_SCHEMA,
        v.get("data").expect("data"),
    );
}

#[test]
fn diagnostics_export_failure_matches_schema() {
    let tmp = tempfile::tempdir().expect("tmp");
    let runtime_root = tmp.path().join("rr");
    let output_dir = tmp.path().join("already-dir");
    fs::create_dir_all(&output_dir).expect("output dir");

    let out = Command::cargo_bin("envr")
        .expect("envr binary")
        .env("ENVR_RUNTIME_ROOT", runtime_root.as_os_str())
        .args([
            "--format",
            "json",
            "diagnostics",
            "export",
            "--output",
            output_dir.to_str().expect("utf8"),
        ])
        .output()
        .expect("diagnostics export failure");
    assert!(!out.status.success(), "expected diagnostics export failure");

    let v = parse_json_line(&out.stdout);
    assert_valid(ENVELOPE_SCHEMA, &v);
    assert_eq!(
        v.get("code"),
        Some(&serde_json::json!("diagnostics_export_failed"))
    );
    assert_valid(
        FAILURE_DIAGNOSTICS_EXPORT_FAILED_SCHEMA,
        v.get("data").expect("data"),
    );
}

#[test]
fn remote_json_matches_schemas_when_success() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = assert_cmd::Command::cargo_bin("envr")
        .expect("envr binary")
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .args(["--format", "json", "remote", "node"])
        .output()
        .expect("envr output");
    if !out.status.success() {
        eprintln!(
            "skip remote_json_matches_schemas_when_success: remote node failed (network?): {}",
            String::from_utf8_lossy(&out.stderr)
        );
        return;
    }
    let v = parse_json_line(&out.stdout);
    assert_valid(ENVELOPE_SCHEMA, &v);
    assert_valid(REMOTE_DATA_SCHEMA, v.get("data").expect("data"));
}

#[test]
fn resolve_json_matches_schemas_with_project_pin() {
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
        .args([
            "--format",
            "json",
            "resolve",
            "node",
            "--path",
            project.to_str().expect("utf8 path"),
        ])
        .output()
        .expect("envr output");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_line(&out.stdout);
    assert_valid(ENVELOPE_SCHEMA, &v);
    assert_eq!(
        v.get("code"),
        Some(&serde_json::json!("runtime_resolved"))
    );
    assert_valid(RUNTIME_RESOLVED_SCHEMA, v.get("data").expect("data"));
}

#[test]
fn which_json_matches_schemas_with_project_pin() {
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
        .env("PATH", narrow_path_for_envr_process())
        .args(["--format", "json", "which", "node"])
        .output()
        .expect("envr output");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_line(&out.stdout);
    assert_valid(ENVELOPE_SCHEMA, &v);
    assert_eq!(
        v.get("code"),
        Some(&serde_json::json!("resolved_executable"))
    );
    assert_valid(RESOLVED_EXECUTABLE_SCHEMA, v.get("data").expect("data"));
}

#[test]
fn list_unknown_runtime_json_failure_matches_envelope_schema() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = Command::cargo_bin("envr")
        .expect("envr binary")
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .args(["--format", "json", "list", "not-a-lang"])
        .output()
        .expect("envr output");
    assert!(!out.status.success());
    let v = parse_json_line(&out.stdout);
    assert_valid(ENVELOPE_SCHEMA, &v);
    assert_eq!(v.get("success"), Some(&serde_json::json!(false)));
    assert_eq!(v.get("code"), Some(&serde_json::json!("validation")));
}

#[test]
fn current_unknown_runtime_json_failure_matches_envelope_schema() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let out = Command::cargo_bin("envr")
        .expect("envr binary")
        .env("ENVR_RUNTIME_ROOT", tmp.path().as_os_str())
        .args(["--format", "json", "current", "not-a-lang"])
        .output()
        .expect("envr output");
    assert!(!out.status.success());
    let v = parse_json_line(&out.stdout);
    assert_valid(ENVELOPE_SCHEMA, &v);
    assert_eq!(v.get("success"), Some(&serde_json::json!(false)));
    assert_eq!(v.get("code"), Some(&serde_json::json!("validation")));
}

#[test]
fn update_info_json_matches_schemas() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let v = json_stdout(&["--format", "json", "update"], tmp.path());
    assert_valid(ENVELOPE_SCHEMA, &v);
    assert_valid(UPDATE_INFO_SCHEMA, v.get("data").expect("data"));
}

#[test]
fn hook_prompt_json_matches_schemas() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let v = json_stdout(&["--format", "json", "hook", "prompt"], tmp.path());
    assert_valid(ENVELOPE_SCHEMA, &v);
    assert_valid(HOOK_PROMPT_SCHEMA, v.get("data").expect("data"));
}

#[test]
fn config_keys_get_show_set_json_match_schemas_under_envr_root() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let envr_home = tmp.path().join("envr-home");
    fs::create_dir_all(&envr_home).expect("envr home");

    let rr_empty = tmp.path().join("rr-empty");
    fs::create_dir_all(&rr_empty).expect("rr-empty");
    let mut cmd = Command::cargo_bin("envr").expect("envr binary");
    cmd.env("ENVR_ROOT", envr_home.as_os_str())
        .env("ENVR_RUNTIME_ROOT", rr_empty.as_os_str());
    let out = cmd
        .args(["--format", "json", "config", "keys"])
        .output()
        .expect("config keys");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_line(&out.stdout);
    assert_valid(ENVELOPE_SCHEMA, &v);
    assert_valid(CONFIG_KEYS_SCHEMA, v.get("data").expect("data"));

    let mut cmd = Command::cargo_bin("envr").expect("envr binary");
    let out = cmd
        .env("ENVR_ROOT", envr_home.as_os_str())
        .args(["--format", "json", "config", "get", "mirror.mode"])
        .output()
        .expect("config get");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_line(&out.stdout);
    assert_valid(ENVELOPE_SCHEMA, &v);
    assert_valid(CONFIG_GET_SCHEMA, v.get("data").expect("data"));

    let mut cmd = Command::cargo_bin("envr").expect("envr binary");
    let out = cmd
        .env("ENVR_ROOT", envr_home.as_os_str())
        .args(["--format", "json", "config", "show"])
        .output()
        .expect("config show");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_line(&out.stdout);
    assert_valid(ENVELOPE_SCHEMA, &v);
    assert_valid(CONFIG_SHOW_SCHEMA, v.get("data").expect("data"));

    let mut cmd = Command::cargo_bin("envr").expect("envr binary");
    let out = cmd
        .env("ENVR_ROOT", envr_home.as_os_str())
        .args(["--format", "json", "config", "set", "mirror.mode", "auto"])
        .output()
        .expect("config set");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_line(&out.stdout);
    assert_valid(ENVELOPE_SCHEMA, &v);
    assert_valid(CONFIG_SET_SCHEMA, v.get("data").expect("data"));
}

#[test]
fn exec_dry_run_json_matches_schemas() {
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

    let args: &[&str] = if cfg!(windows) {
        &[
            "--format",
            "json",
            "exec",
            "--lang",
            "node",
            "--dry-run",
            "cmd",
            "/c",
            "echo",
            "x",
        ]
    } else {
        &[
            "--format",
            "json",
            "exec",
            "--lang",
            "node",
            "--dry-run",
            "true",
        ]
    };

    let out = Command::cargo_bin("envr")
        .expect("envr binary")
        .current_dir(&project)
        .env("ENVR_RUNTIME_ROOT", runtime_root.as_os_str())
        .env("PATH", narrow_path_for_envr_process())
        .args(args)
        .output()
        .expect("exec dry-run");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_line(&out.stdout);
    assert_valid(ENVELOPE_SCHEMA, &v);
    assert_eq!(v.get("code"), Some(&serde_json::json!("dry_run")));
    assert_valid(DRY_RUN_SCHEMA, v.get("data").expect("data"));
}

#[test]
fn cache_clean_dry_run_prune_json_matches_schemas() {
    let tmp = tempfile::tempdir().expect("tmp");
    let runtime_root = tmp.path().join("runtime-root");
    let project = tmp.path().join("project");
    fs::create_dir_all(&project).expect("project");
    let cache_node = runtime_root.join("cache/node");
    fs::create_dir_all(&cache_node).expect("cache");
    let path = cache_node.join("old.bin");
    {
        use std::io::Write;
        use std::time::{Duration, SystemTime};
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)
            .expect("open");
        f.write_all(b"x").expect("write");
        let ancient = SystemTime::UNIX_EPOCH + Duration::from_secs(86_400 * 400);
        f.set_modified(ancient).expect("set mtime");
    }
    let out = Command::cargo_bin("envr")
        .expect("envr binary")
        .current_dir(&project)
        .env("ENVR_RUNTIME_ROOT", runtime_root.as_os_str())
        .args([
            "--format",
            "json",
            "cache",
            "clean",
            "node",
            "--older-than",
            "30d",
            "--dry-run",
        ])
        .output()
        .expect("cache clean");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_line(&out.stdout);
    assert_valid(ENVELOPE_SCHEMA, &v);
    assert_eq!(v.get("code"), Some(&serde_json::json!("cache_cleaned")));
    assert_valid(CACHE_CLEANED_SCHEMA, v.get("data").expect("data"));
}

#[test]
fn cache_index_status_json_matches_schemas() {
    let tmp = tempfile::tempdir().expect("tmp");
    let idx = tmp.path().join("idx-empty");
    fs::create_dir_all(&idx).expect("idx");
    let out = Command::cargo_bin("envr")
        .expect("envr binary")
        .env("ENVR_RUNTIME_ROOT", tmp.path().join("rr").as_os_str())
        .args([
            "--format",
            "json",
            "cache",
            "index",
            "status",
            "--dir",
            idx.to_str().expect("utf8"),
        ])
        .output()
        .expect("cache index status");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_line(&out.stdout);
    assert_valid(ENVELOPE_SCHEMA, &v);
    assert_eq!(
        v.get("code"),
        Some(&serde_json::json!("cache_index_status"))
    );
    assert_valid(CACHE_INDEX_STATUS_SCHEMA, v.get("data").expect("data"));
}

#[test]
fn bundle_created_and_applied_json_match_schemas() {
    let tmp = tempfile::tempdir().expect("tmp");
    let runtime_root = tmp.path().join("rr");
    fs::create_dir_all(&runtime_root).expect("rr");
    let cwd = tmp.path().join("work");
    fs::create_dir_all(&cwd).expect("work");
    let zip_path = cwd.join("phase-a-bundle.zip");

    let out = Command::cargo_bin("envr")
        .expect("envr binary")
        .current_dir(&cwd)
        .env("ENVR_RUNTIME_ROOT", runtime_root.as_os_str())
        .args([
            "--format",
            "json",
            "bundle",
            "create",
            "--no-current",
            "--output",
            zip_path.to_str().expect("zip utf8"),
        ])
        .output()
        .expect("bundle create");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_line(&out.stdout);
    assert_valid(ENVELOPE_SCHEMA, &v);
    assert_eq!(v.get("code"), Some(&serde_json::json!("bundle_created")));
    assert_valid(BUNDLE_CREATED_SCHEMA, v.get("data").expect("data"));
    assert!(zip_path.is_file(), "zip missing");

    let apply_root = tmp.path().join("apply-rr");
    fs::create_dir_all(&apply_root).expect("apply rr");
    let out = Command::cargo_bin("envr")
        .expect("envr binary")
        .current_dir(&cwd)
        .env("ENVR_RUNTIME_ROOT", apply_root.as_os_str())
        .args([
            "--format",
            "json",
            "bundle",
            "apply",
            zip_path.to_str().expect("zip"),
        ])
        .output()
        .expect("bundle apply");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_line(&out.stdout);
    assert_valid(ENVELOPE_SCHEMA, &v);
    assert_eq!(v.get("code"), Some(&serde_json::json!("bundle_applied")));
    assert_valid(BUNDLE_APPLIED_SCHEMA, v.get("data").expect("data"));
}

#[test]
fn project_validated_json_matches_schemas() {
    let tmp = tempfile::tempdir().expect("tmp");
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
        .args(["--format", "json", "project", "validate"])
        .output()
        .expect("project validate");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_line(&out.stdout);
    assert_valid(ENVELOPE_SCHEMA, &v);
    assert_eq!(
        v.get("code"),
        Some(&serde_json::json!("project_validated"))
    );
    assert_valid(PROJECT_VALIDATED_SCHEMA, v.get("data").expect("data"));
}

#[test]
fn prune_dry_run_json_matches_schemas() {
    let tmp = tempfile::tempdir().expect("tmp");
    // `prune` defaults to dry-run unless `--execute`.
    let v = json_stdout(&["--format", "json", "prune", "node"], tmp.path());
    assert_valid(ENVELOPE_SCHEMA, &v);
    assert_eq!(v.get("code"), Some(&serde_json::json!("prune_dry_run")));
    assert_valid(PRUNE_DRY_RUN_SCHEMA, v.get("data").expect("data"));
}

#[test]
fn use_sets_current_json_matches_schemas() {
    let tmp = tempfile::tempdir().expect("tmp");
    let runtime_root = tmp.path().join("rr");
    fs::create_dir_all(&runtime_root).expect("rr");
    write_node_layout(&runtime_root, "20.10.0");
    let cwd = tmp.path().join("cwd");
    fs::create_dir_all(&cwd).expect("cwd");
    let out = Command::cargo_bin("envr")
        .expect("envr binary")
        .current_dir(&cwd)
        .env("ENVR_RUNTIME_ROOT", runtime_root.as_os_str())
        .args(["--format", "json", "use", "node", "20.10.0"])
        .output()
        .expect("use");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_line(&out.stdout);
    assert_valid(ENVELOPE_SCHEMA, &v);
    assert_eq!(
        v.get("code"),
        Some(&serde_json::json!("current_runtime_set"))
    );
    assert_valid(CURRENT_RUNTIME_SET_SCHEMA, v.get("data").expect("data"));
}

#[test]
fn run_child_completed_json_matches_schemas() {
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

    let args: &[&str] = if cfg!(windows) {
        &["--format", "json", "run", "cmd", "/c", "echo", "ok"]
    } else {
        &["--format", "json", "run", "sh", "-c", "echo ok"]
    };

    let out = Command::cargo_bin("envr")
        .expect("envr binary")
        .current_dir(&project)
        .env("ENVR_RUNTIME_ROOT", runtime_root.as_os_str())
        .env("PATH", narrow_path_for_envr_process())
        .args(args)
        .output()
        .expect("run");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_line(&out.stdout);
    assert_valid(ENVELOPE_SCHEMA, &v);
    assert_eq!(
        v.get("code"),
        Some(&serde_json::json!("child_completed"))
    );
    assert_valid(CHILD_COMPLETED_SCHEMA, v.get("data").expect("data"));
}

#[test]
fn exec_child_completed_json_matches_schemas() {
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

    let args: &[&str] = if cfg!(windows) {
        &[
            "--format", "json", "exec", "--lang", "node", "cmd", "/c", "echo", "ok",
        ]
    } else {
        &[
            "--format", "json", "exec", "--lang", "node", "sh", "-c", "echo ok",
        ]
    };

    let out = Command::cargo_bin("envr")
        .expect("envr binary")
        .current_dir(&project)
        .env("ENVR_RUNTIME_ROOT", runtime_root.as_os_str())
        .env("PATH", narrow_path_for_envr_process())
        .args(args)
        .output()
        .expect("exec");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_line(&out.stdout);
    assert_valid(ENVELOPE_SCHEMA, &v);
    assert_eq!(
        v.get("code"),
        Some(&serde_json::json!("child_completed"))
    );
    assert_valid(CHILD_COMPLETED_SCHEMA, v.get("data").expect("data"));
}

#[test]
fn project_add_json_matches_schemas() {
    let tmp = tempfile::tempdir().expect("tmp");
    let runtime_root = tmp.path().join("rr");
    fs::create_dir_all(&runtime_root).expect("rr");
    write_node_layout(&runtime_root, "22.1.0");
    let cwd = tmp.path().join("cwd");
    fs::create_dir_all(&cwd).expect("cwd");
    let out = Command::cargo_bin("envr")
        .expect("envr binary")
        .current_dir(&cwd)
        .env("ENVR_RUNTIME_ROOT", runtime_root.as_os_str())
        .args(["--format", "json", "project", "add", "node@22.1.0"])
        .output()
        .expect("project add");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_line(&out.stdout);
    assert_valid(ENVELOPE_SCHEMA, &v);
    assert_eq!(
        v.get("code"),
        Some(&serde_json::json!("project_pin_added"))
    );
    assert_valid(PROJECT_PIN_ADDED_SCHEMA, v.get("data").expect("data"));
}

#[test]
fn cache_index_sync_json_matches_schemas_when_success() {
    let tmp = tempfile::tempdir().expect("tmp");
    let idx = tmp.path().join("idx");
    fs::create_dir_all(&idx).expect("idx");
    let out = Command::cargo_bin("envr")
        .expect("envr binary")
        .env("ENVR_RUNTIME_ROOT", tmp.path().join("rr").as_os_str())
        .args([
            "--format",
            "json",
            "cache",
            "index",
            "sync",
            "node",
            "--dir",
            idx.to_str().expect("utf8"),
        ])
        .output()
        .expect("cache index sync");
    if !out.status.success() {
        eprintln!(
            "skip cache_index_sync_json_matches_schemas_when_success: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        return;
    }
    let v = parse_json_line(&out.stdout);
    assert_valid(ENVELOPE_SCHEMA, &v);
    assert_eq!(
        v.get("code"),
        Some(&serde_json::json!("cache_index_synced"))
    );
    assert_valid(CACHE_INDEX_SYNCED_SCHEMA, v.get("data").expect("data"));
}

#[test]
fn uninstall_dry_run_json_matches_envelope_and_data_shape() {
    let tmp = tempfile::tempdir().expect("tmp");
    let runtime_root = tmp.path().join("rr");
    let cwd = tmp.path().join("cwd");
    fs::create_dir_all(&cwd).expect("cwd");
    write_node_layout(&runtime_root, "18.99.0");
    let out = Command::cargo_bin("envr")
        .expect("envr binary")
        .current_dir(&cwd)
        .env("ENVR_RUNTIME_ROOT", runtime_root.as_os_str())
        .args([
            "--format",
            "json",
            "uninstall",
            "--dry-run",
            "node",
            "18.99.0",
        ])
        .output()
        .expect("uninstall dry-run");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v = parse_json_line(&out.stdout);
    assert_valid(ENVELOPE_SCHEMA, &v);
    assert_eq!(v.get("success"), Some(&serde_json::json!(true)));
    let data = v.get("data").expect("data");
    assert_eq!(data.get("kind"), Some(&serde_json::json!("node")));
    assert_eq!(data.get("version"), Some(&serde_json::json!("18.99.0")));
    assert!(data.get("paths").is_some());
}

#[test]
fn schemas_cli_data_dir_files_are_valid_json_schemas() {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schemas/cli/data");
    for ent in fs::read_dir(&base).expect("read schemas/cli/data") {
        let p = ent.expect("dir entry").path();
        if p.extension() != Some(OsStr::new("json")) {
            continue;
        }
        let raw = fs::read_to_string(&p).expect("read schema");
        let raw = raw.trim_start_matches('\u{feff}');
        let schema: Value = serde_json::from_str(raw).expect("schema JSON");
        jsonschema::validator_for(&schema).unwrap_or_else(|e| {
            panic!("{}: invalid JSON Schema: {e}", p.display());
        });
    }
}
