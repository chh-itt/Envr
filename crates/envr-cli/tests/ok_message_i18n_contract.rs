//! Contract: every JSON success `code` must have `cli.ok.<code>` entries in both locales.
//!
//! This prevents regressions where success `message` would silently fall back to a generic string.

use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest.join("../..")
}

fn read_success_codes() -> Vec<String> {
    let p = repo_root().join("schemas/cli/index.json");
    let raw = fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
    let v: JsonValue = serde_json::from_str(&raw).expect("parse schemas/cli/index.json");
    v["success_codes"]
        .as_array()
        .expect("success_codes array")
        .iter()
        .filter_map(|x| x.as_str().map(ToOwned::to_owned))
        .collect()
}

fn flatten_messages_toml(raw: &str) -> HashMap<String, String> {
    let v = raw.parse::<toml::Value>().expect("parse locales toml");
    let tbl = v
        .get("messages")
        .and_then(|x| x.as_table())
        .expect("[messages] table");
    let mut out = HashMap::new();
    fn walk(
        prefix: &str,
        t: &toml::map::Map<String, toml::Value>,
        out: &mut HashMap<String, String>,
    ) {
        for (k, v) in t {
            let full = if prefix.is_empty() {
                k.clone()
            } else {
                format!("{prefix}.{k}")
            };
            match v {
                toml::Value::String(s) => {
                    out.insert(full, s.clone());
                }
                toml::Value::Table(tt) => walk(&full, tt, out),
                _ => {}
            }
        }
    }
    walk("", tbl, &mut out);
    out
}

fn load_locale_map(file: &str) -> HashMap<String, String> {
    let p = repo_root().join("locales").join(file);
    let raw = fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()));
    flatten_messages_toml(&raw)
}

#[test]
fn every_success_code_has_cli_ok_message_in_both_locales() {
    let success = read_success_codes();
    let en = load_locale_map("en-US.toml");
    let zh = load_locale_map("zh-CN.toml");

    // Default fallback must exist too (used only when a specific key is missing).
    for (name, map) in [("en-US", &en), ("zh-CN", &zh)] {
        let v = map
            .get("cli.ok._default")
            .unwrap_or_else(|| panic!("{name} missing key cli.ok._default"));
        assert!(
            !v.trim().is_empty(),
            "{name} key cli.ok._default must be non-empty"
        );
    }

    for code in success {
        let key = format!("cli.ok.{code}");
        for (name, map) in [("en-US", &en), ("zh-CN", &zh)] {
            let v = map
                .get(&key)
                .unwrap_or_else(|| panic!("{name} missing key {key}"));
            assert!(!v.trim().is_empty(), "{name} key {key} must be non-empty");
        }
    }
}
