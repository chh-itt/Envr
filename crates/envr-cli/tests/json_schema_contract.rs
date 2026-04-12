//! Validate `--format json` envelopes and selected `data` blobs against checked-in JSON Schemas.

use assert_cmd::Command;
use serde_json::Value;
use std::ffi::OsStr;
use std::fs;
use std::path::Path;

const ENVELOPE_SCHEMA: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../schemas/cli/envelope.json"));
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
