//! T914: static checks for `locales/*` vs `tr_key` / `cli_help` usage.

use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn workspace_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .expect("crates/envr-i18n-lint")
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

fn flatten_messages(raw: &str) -> HashMap<String, String> {
    let parsed: toml::Value = match raw.parse() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("envr-i18n-lint: failed to parse locale TOML: {e}");
            return HashMap::new();
        }
    };
    let Some(tbl) = parsed.get("messages").and_then(|v| v.as_table()) else {
        return HashMap::new();
    };
    let mut out = HashMap::new();
    flatten_table("", tbl, &mut out);
    out
}

fn flatten_table(
    prefix: &str,
    tbl: &toml::map::Map<String, toml::Value>,
    out: &mut HashMap<String, String>,
) {
    for (k, v) in tbl {
        let full = if prefix.is_empty() {
            k.clone()
        } else {
            format!("{prefix}.{k}")
        };
        match v {
            toml::Value::String(s) => {
                out.insert(full, s.clone());
            }
            toml::Value::Table(t) => flatten_table(&full, t, out),
            _ => {}
        }
    }
}

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else {
        return;
    };
    for ent in rd.flatten() {
        let p = ent.path();
        if p.is_dir() {
            collect_rs_files(&p, out);
        } else if p.extension().is_some_and(|e| e == "rs") {
            out.push(p);
        }
    }
}

fn main() -> ExitCode {
    let root = workspace_root();
    let zh_path = root.join("locales/zh-CN.toml");
    let en_path = root.join("locales/en-US.toml");
    if !zh_path.is_file() || !en_path.is_file() {
        eprintln!("envr-i18n-lint: missing locales/zh-CN.toml or locales/en-US.toml");
        return ExitCode::from(1);
    }

    let zh = flatten_messages(&fs::read_to_string(&zh_path).unwrap_or_default());
    let en = flatten_messages(&fs::read_to_string(&en_path).unwrap_or_default());

    let mut errors: Vec<String> = Vec::new();

    let zh_keys: HashSet<_> = zh.keys().cloned().collect();
    let en_keys: HashSet<_> = en.keys().cloned().collect();
    for k in &zh_keys {
        if !en_keys.contains(k) {
            errors.push(format!("key `{k}` in zh-CN.toml but missing in en-US.toml"));
        }
    }
    for k in &en_keys {
        if !zh_keys.contains(k) {
            errors.push(format!("key `{k}` in en-US.toml but missing in zh-CN.toml"));
        }
    }

    let tr_key_re = Regex::new(r#"tr_key\s*\(\s*"([^"]+)""#).expect("regex");
    let cli_help_tr_re = Regex::new(r#"\btr\s*\(\s*"([^"]+)""#).expect("regex");
    let legacy_tr_re = Regex::new(r"envr_core::i18n::tr\s*\(").expect("regex");
    // `tr_key(key, zh, en)` tuples still declare static keys as string literals.
    let i18n_key_literal_re =
        Regex::new(r#""((?:cli|gui)\.[a-zA-Z0-9_]+(?:\.[a-zA-Z0-9_]+)*)""#).expect("regex");

    let scan_roots = [
        root.join("crates/envr-gui/src"),
        root.join("crates/envr-cli/src"),
        root.join("crates/envr-core/src"),
    ];

    let mut code_keys: HashSet<String> = HashSet::new();
    let mut files: Vec<PathBuf> = Vec::new();
    for d in &scan_roots {
        if d.is_dir() {
            collect_rs_files(d, &mut files);
        }
    }

    for path in &files {
        let Ok(text) = fs::read_to_string(path) else {
            continue;
        };
        let rel = path.strip_prefix(&root).unwrap_or(path);

        let under_gui_or_cli = rel
            .to_string_lossy()
            .replace('\\', "/")
            .contains("/envr-gui/")
            || rel
                .to_string_lossy()
                .replace('\\', "/")
                .contains("/envr-cli/");
        if under_gui_or_cli && legacy_tr_re.is_match(&text) {
            errors.push(format!(
                "legacy `envr_core::i18n::tr(` in {} (use tr_key in GUI/CLI)",
                rel.display()
            ));
        }

        for cap in tr_key_re.captures_iter(&text) {
            code_keys.insert(cap[1].to_string());
        }

        for cap in i18n_key_literal_re.captures_iter(&text) {
            code_keys.insert(cap[1].to_string());
        }

        if rel.as_os_str().to_string_lossy().ends_with("cli_help.rs") {
            for cap in cli_help_tr_re.captures_iter(&text) {
                code_keys.insert(cap[1].to_string());
            }
        }
    }

    for k in &code_keys {
        if k.starts_with("__") {
            continue;
        }
        if !zh.contains_key(k) {
            errors.push(format!(
                "tr_key/tr(`{k}`) used in code but missing zh-CN.toml"
            ));
        }
        if !en.contains_key(k) {
            errors.push(format!(
                "tr_key/tr(`{k}`) used in code but missing en-US.toml"
            ));
        }
    }

    let mut unused: Vec<_> = zh_keys
        .iter()
        .filter(|k| !code_keys.contains(*k))
        .cloned()
        .collect();
    unused.sort();
    if !unused.is_empty() {
        eprintln!(
            "envr-i18n-lint: note — {} keys in locales but not referenced by tr_key/tr in scanned crates (OK if reserved):",
            unused.len()
        );
        for k in unused.iter().take(40) {
            eprintln!("  - {k}");
        }
        if unused.len() > 40 {
            eprintln!("  ... and {} more", unused.len() - 40);
        }
    }

    if !errors.is_empty() {
        eprintln!("envr-i18n-lint: {} error(s)", errors.len());
        for e in &errors {
            eprintln!("  {e}");
        }
        return ExitCode::from(1);
    }

    println!(
        "envr-i18n-lint: OK ({} locale keys, {} keys referenced from code)",
        zh_keys.len(),
        code_keys.len()
    );
    ExitCode::SUCCESS
}
