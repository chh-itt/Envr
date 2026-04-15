//! Contract guardrails for failure-envelope code tokens and schema stubs.

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

fn failure_codes_from_source(src: &str, emit_fail: &Regex) -> BTreeSet<String> {
    let mut s = BTreeSet::new();
    for c in emit_fail.captures_iter(src) {
        s.insert(c[1].to_string());
    }
    s
}

#[test]
fn every_emit_failure_envelope_code_has_failure_schema_stub() {
    let emit_fail = Regex::new(r#"emit_failure_envelope\([^"]*"([a-zA-Z0-9_]+)""#).expect("regex");
    let snake = Regex::new(r"^[a-z][a-z0-9_]*$").expect("snake_case regex");

    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    collect_rs_files(&manifest.join("src/commands"), &mut files);
    collect_rs_files(&manifest.join("src/cli"), &mut files);

    let mut codes = BTreeSet::new();
    for f in &files {
        let src = fs::read_to_string(f).unwrap_or_else(|e| panic!("read {}: {e}", f.display()));
        codes.extend(failure_codes_from_source(&src, &emit_fail));
    }

    for code in &codes {
        assert!(
            snake.is_match(code),
            "failure code must be snake_case: {code}"
        );
    }

    let data_dir = manifest.join("../../schemas/cli/data");
    assert!(
        data_dir.is_dir(),
        "missing schemas dir {}",
        data_dir.display()
    );

    let mut missing = Vec::new();
    for code in &codes {
        let p = data_dir.join(format!("failure_{code}.json"));
        if !p.is_file() {
            missing.push(code.clone());
        }
    }
    assert!(
        missing.is_empty(),
        "failure codes without schemas/cli/data/failure_<code>.json:\n  {}",
        missing.join("\n  ")
    );
}
