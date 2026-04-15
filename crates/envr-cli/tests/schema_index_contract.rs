//! P12: schema index must stay aligned with source literals and schema files.

use regex::Regex;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
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

fn parse_code_registry(path: &Path) -> (BTreeMap<String, String>, BTreeMap<String, String>) {
    let raw = fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let const_re =
        Regex::new(r#"pub const ([A-Z0-9_]+): &str = "([a-zA-Z0-9_]+)";"#).expect("const regex");
    let mut ok = BTreeMap::new();
    let mut err = BTreeMap::new();
    let mut scope: Option<&str> = None;
    for line in raw.lines() {
        let t = line.trim();
        if t == "pub mod ok {" {
            scope = Some("ok");
            continue;
        }
        if t == "pub mod err {" {
            scope = Some("err");
            continue;
        }
        if t == "}" {
            scope = None;
        }
        if let Some(c) = const_re.captures(t) {
            let key = c[1].to_string();
            let val = c[2].to_string();
            match scope {
                Some("ok") => {
                    ok.insert(key, val);
                }
                Some("err") => {
                    err.insert(key, val);
                }
                _ => {}
            }
        }
    }
    (ok, err)
}

fn command_spec_success_messages(path: &Path) -> BTreeSet<String> {
    let raw = fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let spec_row_re = Regex::new(
        r#"(?s)CommandSpec::new\(\s*"[^"]+"\s*,.*?,\s*&\[[^\]]*\]\s*,\s*&\[(?P<messages>[^\]]*)\]\s*\)\s*\),"#,
    )
    .expect("command spec row regex");
    let msg_re = Regex::new(r#""([a-zA-Z0-9_]+)""#).expect("message regex");

    let mut out = BTreeSet::new();
    for row in spec_row_re.captures_iter(&raw) {
        let list = row
            .name("messages")
            .expect("messages capture")
            .as_str()
            .trim();
        if list.is_empty() {
            continue;
        }
        for m in msg_re.captures_iter(list) {
            out.insert(m[1].to_string());
        }
    }
    out
}

fn code_from_literal_or_const(
    src_value: &str,
    const_map: &BTreeMap<String, String>,
) -> Option<String> {
    let src_value = src_value.trim_matches('"');
    if src_value.starts_with("crate::codes::") || src_value.starts_with("codes::") {
        let name = src_value.rsplit("::").next()?;
        const_map.get(name).cloned()
    } else {
        Some(src_value.to_string())
    }
}

fn success_messages_from_source(
    src: &str,
    emit_ok: &Regex,
    write_env: &Regex,
    emit_doctor_ok: &Regex,
    ok_codes: &BTreeMap<String, String>,
) -> BTreeSet<String> {
    let mut s = BTreeSet::new();
    for c in emit_ok.captures_iter(src) {
        if let Some(code) = code_from_literal_or_const(&c[1], ok_codes) {
            s.insert(code);
        }
    }
    for c in write_env.captures_iter(src) {
        if let Some(code) = code_from_literal_or_const(&c[1], ok_codes) {
            s.insert(code);
        }
    }
    for c in emit_doctor_ok.captures_iter(src) {
        if let Some(code) = code_from_literal_or_const(&c[1], ok_codes) {
            s.insert(code);
        }
    }
    s
}

fn failure_codes_from_source(
    src: &str,
    emit_fail: &Regex,
    err_codes: &BTreeMap<String, String>,
) -> BTreeSet<String> {
    let mut s = BTreeSet::new();
    for c in emit_fail.captures_iter(src) {
        if let Some(code) = code_from_literal_or_const(&c[1], err_codes) {
            s.insert(code);
        }
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

    let idx_messages: BTreeSet<String> = index["success_codes"]
        .as_array()
        .expect("success_codes array")
        .iter()
        .filter_map(|v| v.as_str().map(ToOwned::to_owned))
        .collect();
    let idx_failures: BTreeSet<String> = index["failure_codes"]
        .as_array()
        .expect("failure_codes array")
        .iter()
        .filter_map(|v| v.as_str().map(ToOwned::to_owned))
        .collect();

    let (ok_codes, err_codes) = parse_code_registry(&manifest.join("src/codes.rs"));

    let emit_ok =
        Regex::new(r#"emit_ok\([^,]+,\s*("([a-zA-Z0-9_]+)"|(?:crate::)?codes::ok::[A-Z0-9_]+)"#)
            .expect("regex");
    let write_env = Regex::new(
        r#"write_envelope\(\s*true\s*,\s*("([a-zA-Z0-9_]+)"|(?:crate::)?codes::ok::[A-Z0-9_]+)"#,
    )
    .expect("regex");
    let emit_doctor_ok = Regex::new(
        r#"emit_doctor\(\s*g\s*,\s*ok\s*,\s*("([a-zA-Z0-9_]+)"|(?:crate::)?codes::ok::[A-Z0-9_]+)"#,
    )
    .expect("regex");
    let emit_fail = Regex::new(
        r#"emit_failure_envelope\([^,]+,\s*("([a-zA-Z0-9_]+)"|(?:crate::)?codes::err::[A-Z0-9_]+)"#,
    )
    .expect("regex");

    let mut files = Vec::new();
    collect_rs_files(&manifest.join("src"), &mut files);

    let mut src_messages = BTreeSet::new();
    let mut src_failures = BTreeSet::new();
    for f in &files {
        let src = fs::read_to_string(f).unwrap_or_else(|e| panic!("read {}: {e}", f.display()));
        src_messages.extend(success_messages_from_source(
            &src,
            &emit_ok,
            &write_env,
            &emit_doctor_ok,
            &ok_codes,
        ));
        src_failures.extend(failure_codes_from_source(&src, &emit_fail, &err_codes));
    }

    assert_eq!(
        idx_messages, src_messages,
        "index success_codes mismatch; regenerate via `python scripts/generate_cli_schema_index.py`"
    );
    assert_eq!(
        idx_failures, src_failures,
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

#[test]
fn failure_registry_constants_are_covered_by_schema_index_and_kind_map() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo = manifest.join("../..");
    let index_path = repo.join("schemas/cli/index.json");
    let kind_map_path = repo.join("schemas/cli/error-kind-map.json");

    let index_raw = fs::read_to_string(&index_path).expect("read schemas/cli/index.json");
    let index: Value = serde_json::from_str(&index_raw).expect("parse index");
    let idx_failures: BTreeSet<String> = index["failure_codes"]
        .as_array()
        .expect("failure_codes array")
        .iter()
        .filter_map(|v| v.as_str().map(ToOwned::to_owned))
        .collect();

    let kind_map_raw =
        fs::read_to_string(&kind_map_path).expect("read schemas/cli/error-kind-map.json");
    let kind_map: Value = serde_json::from_str(&kind_map_raw).expect("parse error-kind-map");
    let map_obj = kind_map["map"]
        .as_object()
        .expect("error-kind-map map must be object");

    let (_, err_codes) = parse_code_registry(&manifest.join("src/codes.rs"));
    let failure_data_dir = repo.join("schemas/cli/data");
    for code in err_codes.values() {
        assert!(
            idx_failures.contains(code),
            "codes::err constant `{code}` must be listed in schemas/cli/index.json failure_codes"
        );
        assert!(
            map_obj.contains_key(code),
            "codes::err constant `{code}` must exist in schemas/cli/error-kind-map.json map"
        );
        let schema_path = failure_data_dir.join(format!("failure_{code}.json"));
        assert!(
            schema_path.is_file(),
            "codes::err constant `{code}` missing schema file {}",
            schema_path.display()
        );
    }
}

#[test]
fn success_registry_constants_are_covered_by_schema_index_and_schema_files() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo = manifest.join("../..");
    let index_path = repo.join("schemas/cli/index.json");

    let index_raw = fs::read_to_string(&index_path).expect("read schemas/cli/index.json");
    let index: Value = serde_json::from_str(&index_raw).expect("parse index");
    let idx_success: BTreeSet<String> = index["success_codes"]
        .as_array()
        .expect("success_codes array")
        .iter()
        .filter_map(|v| v.as_str().map(ToOwned::to_owned))
        .collect();

    let (ok_codes, _) = parse_code_registry(&manifest.join("src/codes.rs"));
    let data_dir = repo.join("schemas/cli/data");
    for code in ok_codes.values() {
        assert!(
            idx_success.contains(code),
            "codes::ok constant `{code}` must be listed in schemas/cli/index.json success_codes"
        );
        let schema_path = data_dir.join(format!("{code}.json"));
        assert!(
            schema_path.is_file(),
            "codes::ok constant `{code}` missing schema file {}",
            schema_path.display()
        );
    }
}

#[test]
fn success_registry_constants_match_command_spec_success_messages() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let (ok_codes, _) = parse_code_registry(&manifest.join("src/codes.rs"));
    let ok_values: BTreeSet<String> = ok_codes.values().cloned().collect();
    let spec_values = command_spec_success_messages(&manifest.join("src/cli/command_spec.rs"));
    assert_eq!(
        ok_values, spec_values,
        "codes::ok constants must match CommandSpec.success_messages union"
    );
}
