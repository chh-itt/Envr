//! P0: every static `emit_ok` / success `write_envelope` message id has a matching
//! `schemas/cli/data/<message>.json` schema file (see docs/schemas/README.md).

use regex::Regex;
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

fn message_ids_from_source(
    src: &str,
    emit_ok: &Regex,
    write_env: &Regex,
    emit_doctor_ok: &Regex,
) -> BTreeSet<String> {
    let mut s = BTreeSet::new();
    for c in emit_ok.captures_iter(src) {
        s.insert(c[1].to_string());
    }
    for c in write_env.captures_iter(src) {
        s.insert(c[1].to_string());
    }
    for c in emit_doctor_ok.captures_iter(src) {
        s.insert(c[1].to_string());
    }
    s
}

#[test]
fn every_emit_ok_message_has_schema_stub_under_schemas_cli_data() {
    let emit_ok = Regex::new(r#"emit_ok\([^,]+,\s*"([a-zA-Z0-9_]+)""#).expect("regex");
    let write_env =
        Regex::new(r#"write_envelope\(\s*true\s*,\s*None\s*,\s*"([a-zA-Z0-9_]+)""#).expect("regex");
    let emit_doctor_ok =
        Regex::new(r#"emit_doctor\(\s*g\s*,\s*ok\s*,\s*"([a-zA-Z0-9_]+)""#).expect("regex");

    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    collect_rs_files(&manifest.join("src/commands"), &mut files);
    files.push(manifest.join("src/lib.rs"));
    files.push(manifest.join("src/main.rs"));

    let mut messages = BTreeSet::new();
    for f in &files {
        let src = fs::read_to_string(f).unwrap_or_else(|e| panic!("read {}: {e}", f.display()));
        messages.extend(message_ids_from_source(
            &src,
            &emit_ok,
            &write_env,
            &emit_doctor_ok,
        ));
    }

    let data_dir = manifest.join("../../schemas/cli/data");
    assert!(
        data_dir.is_dir(),
        "missing schemas dir {}",
        data_dir.display()
    );

    let mut missing = Vec::new();
    for m in &messages {
        let p = data_dir.join(format!("{m}.json"));
        if !p.is_file() {
            missing.push(m.clone());
        }
    }
    assert!(
        missing.is_empty(),
        "emit_ok / write_envelope messages without schemas/cli/data/<id>.json:\n  {}",
        missing.join("\n  ")
    );
}
