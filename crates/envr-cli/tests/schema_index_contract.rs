//! P12: schema index must stay aligned with source literals and schema files.

use regex::Regex;
use serde_json::Value;
use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else {
        return;
    };
    for ent in rd.flatten() {
        let p = ent.path();
        if p.is_dir() {
            collect_rs_files(&p, out);
        } else if p.extension() == Some(OsStr::new("rs")) {
            out.push(p);
        }
    }
}

fn success_messages_from_source(src: &str, emit_ok: &Regex, write_env: &Regex) -> BTreeSet<String> {
    let mut s = BTreeSet::new();
    for c in emit_ok.captures_iter(src) {
        s.insert(c[1].to_string());
    }
    for c in write_env.captures_iter(src) {
        s.insert(c[1].to_string());
    }
    s
}

fn failure_codes_from_source(src: &str, emit_fail: &Regex) -> BTreeSet<String> {
    let mut s = BTreeSet::new();
    for c in emit_fail.captures_iter(src) {
        s.insert(c[1].to_string());
    }
    s
}

#[test]
fn schema_index_matches_source_literals_and_schema_files() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo = manifest.join("../..");
    let data_dir = repo.join("schemas/cli/data");
    let index_path = repo.join("schemas/cli/index.json");

    let index_raw = fs::read_to_string(&index_path).expect("read schemas/cli/index.json");
    let index: Value = serde_json::from_str(&index_raw).expect("parse index");

    let idx_messages: BTreeSet<String> = index["data_messages"]
        .as_array()
        .expect("data_messages array")
        .iter()
        .filter_map(|v| v.as_str().map(ToOwned::to_owned))
        .collect();
    let idx_failures: BTreeSet<String> = index["failure_codes"]
        .as_array()
        .expect("failure_codes array")
        .iter()
        .filter_map(|v| v.as_str().map(ToOwned::to_owned))
        .collect();

    let emit_ok = Regex::new(r#"emit_ok\([^,]+,\s*"([a-zA-Z0-9_]+)""#).expect("regex");
    let write_env =
        Regex::new(r#"write_envelope\(\s*true\s*,\s*None\s*,\s*"([a-zA-Z0-9_]+)""#).expect("regex");
    let emit_fail =
        Regex::new(r#"emit_failure_envelope\([^"]*"([a-zA-Z0-9_]+)""#).expect("regex");

    let mut files = Vec::new();
    collect_rs_files(&manifest.join("src"), &mut files);

    let mut src_messages = BTreeSet::new();
    let mut src_failures = BTreeSet::new();
    for f in &files {
        let src = fs::read_to_string(f).unwrap_or_else(|e| panic!("read {}: {e}", f.display()));
        src_messages.extend(success_messages_from_source(&src, &emit_ok, &write_env));
        src_failures.extend(failure_codes_from_source(&src, &emit_fail));
    }

    assert_eq!(
        idx_messages,
        src_messages,
        "index data_messages mismatch; regenerate via `python scripts/generate_cli_schema_index.py`"
    );
    assert_eq!(
        idx_failures,
        src_failures,
        "index failure_codes mismatch; regenerate via `python scripts/generate_cli_schema_index.py`"
    );

    for m in &idx_messages {
        let p = data_dir.join(format!("{m}.json"));
        assert!(p.is_file(), "missing schema file: {}", p.display());
    }
    for code in &idx_failures {
        let p = data_dir.join(format!("failure_{code}.json"));
        assert!(p.is_file(), "missing failure schema file: {}", p.display());
    }
}

