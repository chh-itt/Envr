//! T914: static checks for `locales/*` vs `tr_key` / `cli_help` usage.
//! Pass `--write-locales` to merge missing keys from Rust source fallbacks (keeps existing TOML values).

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

/// Unescape contents of a Rust `"..."` string literal (tr_key second/third args).
fn unescape_rust_string(inner: &str) -> String {
    let mut out = String::with_capacity(inner.len());
    let mut it = inner.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some('\\') => out.push('\\'),
            Some('"') => out.push('"'),
            Some('0') => out.push('\0'),
            Some(x) => {
                out.push('\\');
                out.push(x);
            }
            None => out.push('\\'),
        }
    }
    out
}

fn escape_toml_basic(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '\\' => o.push_str("\\\\"),
            '"' => o.push_str("\\\""),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if c < ' ' => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o
}

fn format_message_line(key: &str, value: &str) -> String {
    format!("{key} = \"{}\"\n", escape_toml_basic(value))
}

/// TOML dotted keys cannot assign a string to `a.b` and also define `a.b.c` (parent becomes non-table).
fn dotted_key_conflicts(keys: &HashSet<String>) -> Vec<String> {
    let mut v: Vec<_> = keys.iter().cloned().collect();
    v.sort_by_key(|k| std::cmp::Reverse(k.len()));
    let mut errs = Vec::new();
    for long in &v {
        let mut p = long.as_str();
        while let Some(dot) = p.rfind('.') {
            p = &p[..dot];
            if p.is_empty() {
                break;
            }
            if keys.contains(p) {
                errs.push(format!(
                    "dotted-key conflict: `{p}` is both a leaf and a prefix of `{long}` (invalid TOML); rename the shorter key (e.g. add `_label` suffix)"
                ));
            }
        }
    }
    errs.sort();
    errs.dedup();
    errs
}

fn write_locale_file(path: &Path, header_lines: &[&str], messages: &HashMap<String, String>) -> std::io::Result<()> {
    let mut keys: Vec<_> = messages.keys().cloned().collect();
    keys.sort();
    let mut buf = String::new();
    for line in header_lines {
        buf.push_str(line);
        buf.push('\n');
    }
    buf.push_str("[messages]\n");
    let mut prev_top = String::new();
    for k in keys {
        let top = k.split('.').next().unwrap_or("").to_string();
        if top != prev_top {
            if !prev_top.is_empty() {
                buf.push('\n');
            }
            buf.push_str(&format!("# --- {top}.* ---\n"));
            prev_top = top;
        }
        let v = messages.get(&k).expect("key present");
        buf.push_str(&format_message_line(&k, v));
    }
    fs::write(path, buf)
}

/// `tr_key("k","zh","en")` / `envr_core::i18n::tr_key( ... )` with whitespace between args.
fn tr_key_extract_re() -> Regex {
    // Allow optional trailing comma before `)` (rustfmt / multi-line calls).
    Regex::new(
        r#"(?s)(?:Some\s*\(\s*)?&?\s*(?:envr_core::i18n::)?tr_key\s*\(\s*"([^"]+)"\s*,\s*"((?:\\.|[^"\\])*)"\s*,\s*"((?:\\.|[^"\\])*)"\s*,?\s*\)"#,
    )
    .expect("tr_key regex")
}

/// `tr("k","zh","en")` in cli_help.rs only.
fn cli_tr_extract_re() -> Regex {
    Regex::new(
        r#"(?s)\btr\s*\(\s*"([^"]+)"\s*,\s*"((?:\\.|[^"\\])*)"\s*,\s*"((?:\\.|[^"\\])*)"\s*,?\s*\)"#,
    )
    .expect("cli tr regex")
}

fn extract_tr_pairs(text: &str, path: &Path, out: &mut HashMap<String, (String, String)>) {
    let tr_key_re = tr_key_extract_re();
    for cap in tr_key_re.captures_iter(text) {
        let key = cap[1].to_string();
        let zh = unescape_rust_string(&cap[2]);
        let en = unescape_rust_string(&cap[3]);
        out.insert(key, (zh, en));
    }
    if path.file_name().is_some_and(|n| n == "cli_help.rs") {
        let tr_re = cli_tr_extract_re();
        for cap in tr_re.captures_iter(text) {
            let key = cap[1].to_string();
            let zh = unescape_rust_string(&cap[2]);
            let en = unescape_rust_string(&cap[3]);
            out.insert(key, (zh, en));
        }
    }
}

#[allow(clippy::type_complexity)]
fn scan_sources(root: &Path) -> (HashSet<String>, HashMap<String, (String, String)>, Vec<String>) {
    let tr_key_re = Regex::new(r#"tr_key\s*\(\s*"([^"]+)""#).expect("regex");
    let cli_help_tr_re = Regex::new(r#"\btr\s*\(\s*"([^"]+)""#).expect("regex");
    let legacy_tr_re = Regex::new(r"envr_core::i18n::tr\s*\(").expect("regex");
    let i18n_key_literal_re =
        Regex::new(r#""((?:cli|gui)\.[a-zA-Z0-9_]+(?:\.[a-zA-Z0-9_]+)*)""#).expect("regex");

    let scan_roots = [
        root.join("crates/envr-gui/src"),
        root.join("crates/envr-cli/src"),
        root.join("crates/envr-core/src"),
        root.join("crates/envr-shim/src"),
    ];

    let mut code_keys: HashSet<String> = HashSet::new();
    let mut extracts: HashMap<String, (String, String)> = HashMap::new();
    let mut legacy_errors: Vec<String> = Vec::new();
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
        let rel = path.strip_prefix(root).unwrap_or(path);

        let under_gui_or_cli = rel
            .to_string_lossy()
            .replace('\\', "/")
            .contains("/envr-gui/")
            || rel
                .to_string_lossy()
                .replace('\\', "/")
                .contains("/envr-cli/");
        if under_gui_or_cli && legacy_tr_re.is_match(&text) {
            legacy_errors.push(format!(
                "legacy `envr_core::i18n::tr(` in {} (use tr_key in GUI/CLI)",
                rel.display()
            ));
        }

        extract_tr_pairs(&text, path, &mut extracts);

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

    (code_keys, extracts, legacy_errors)
}

fn is_bad_zh_placeholder(s: &str) -> bool {
    s.starts_with("[i18n missing zh]")
}

fn is_bad_en_placeholder(s: &str) -> bool {
    s.starts_with("[i18n missing en]")
}

fn is_placeholder_value(s: &str) -> bool {
    is_bad_zh_placeholder(s) || is_bad_en_placeholder(s)
}

fn merge_locales(
    mut zh: HashMap<String, String>,
    mut en: HashMap<String, String>,
    code_keys: &HashSet<String>,
    extracts: &HashMap<String, (String, String)>,
) -> (HashMap<String, String>, HashMap<String, String>) {
    // Source `tr_key` / `tr` literals win over stale `[i18n missing *]` rows from a bad prior sync.
    for (k, (z, e)) in extracts.iter() {
        let z_ok = zh.get(k).is_none_or(|v| is_placeholder_value(v));
        let e_ok = en.get(k).is_none_or(|v| is_placeholder_value(v));
        if z_ok {
            zh.insert(k.clone(), z.clone());
        }
        if e_ok {
            en.insert(k.clone(), e.clone());
        }
    }

    for k in code_keys {
        if k.starts_with("__") {
            continue;
        }
        if !zh.contains_key(k) {
            let z = extracts
                .get(k)
                .map(|(z, _)| z.clone())
                .or_else(|| {
                    en.get(k)
                        .filter(|e| !is_bad_en_placeholder(e))
                        .cloned()
                })
                .unwrap_or_else(|| format!("[i18n missing zh] {k}"));
            zh.insert(k.clone(), z);
        }
        if !en.contains_key(k) {
            let e = extracts
                .get(k)
                .map(|(_, e)| e.clone())
                .or_else(|| {
                    zh.get(k)
                        .filter(|z| !is_bad_zh_placeholder(z))
                        .cloned()
                })
                .unwrap_or_else(|| format!("[i18n missing en] {k}"));
            en.insert(k.clone(), e);
        }
    }

    let all: HashSet<String> = zh.keys().chain(en.keys()).cloned().collect();
    for k in all {
        if !zh.contains_key(&k) {
            let v = en[&k].clone();
            zh.insert(
                k.clone(),
                if is_bad_en_placeholder(&v) {
                    format!("[i18n missing zh] {k}")
                } else {
                    v
                },
            );
        }
        if !en.contains_key(&k) {
            let v = zh[&k].clone();
            en.insert(
                k.clone(),
                if is_bad_zh_placeholder(&v) {
                    format!("[i18n missing en] {k}")
                } else {
                    v
                },
            );
        }
    }

    (zh, en)
}

fn run_lint(root: &Path) -> Result<(), Vec<String>> {
    let zh_path = root.join("locales/zh-CN.toml");
    let en_path = root.join("locales/en-US.toml");
    if !zh_path.is_file() || !en_path.is_file() {
        return Err(vec!["missing locales/zh-CN.toml or locales/en-US.toml".into()]);
    }

    let zh = flatten_messages(&fs::read_to_string(&zh_path).unwrap_or_default());
    let en = flatten_messages(&fs::read_to_string(&en_path).unwrap_or_default());
    let (code_keys, _, legacy_errors) = scan_sources(root);

    let mut errors: Vec<String> = Vec::new();
    errors.extend(legacy_errors);

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

    for k in &code_keys {
        if k.starts_with("__") {
            continue;
        }
        if !zh.contains_key(k) {
            errors.push(format!("tr_key/tr(`{k}`) used in code but missing zh-CN.toml"));
        }
        if !en.contains_key(k) {
            errors.push(format!("tr_key/tr(`{k}`) used in code but missing en-US.toml"));
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

    if errors.is_empty() {
        println!(
            "envr-i18n-lint: OK ({} locale keys, {} keys referenced from code)",
            zh_keys.len(),
            code_keys.len()
        );
        Ok(())
    } else {
        Err(errors)
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let write_locales = args.iter().any(|a| a == "--write-locales" || a == "--sync-locales");
    let root = workspace_root();

    if write_locales {
        let zh_path = root.join("locales/zh-CN.toml");
        let en_path = root.join("locales/en-US.toml");
        let zh_raw = fs::read_to_string(&zh_path).expect("read zh-CN.toml");
        let en_raw = fs::read_to_string(&en_path).expect("read en-US.toml");
        let zh_flat = flatten_messages(&zh_raw);
        let en_flat = flatten_messages(&en_raw);
        let (code_keys, extracts, legacy_errors) = scan_sources(&root);
        if !legacy_errors.is_empty() {
            eprintln!("envr-i18n-lint: fix legacy tr() before --write-locales:");
            for e in &legacy_errors {
                eprintln!("  {e}");
            }
            return ExitCode::from(1);
        }
        let (merged_zh, merged_en) = merge_locales(zh_flat, en_flat, &code_keys, &extracts);

        let zh_k: HashSet<_> = merged_zh.keys().cloned().collect();
        let en_k: HashSet<_> = merged_en.keys().cloned().collect();
        let mut conflicts = dotted_key_conflicts(&zh_k);
        conflicts.extend(dotted_key_conflicts(&en_k));
        conflicts.sort();
        conflicts.dedup();
        if !conflicts.is_empty() {
            eprintln!("envr-i18n-lint: fix dotted-key conflicts before writing locales:");
            for c in &conflicts {
                eprintln!("  {c}");
            }
            return ExitCode::from(1);
        }

        let zh_header = [
            "# zh-CN locale for envr (embedded by envr-core).",
            "# Keep keys in sync with en-US.toml. CI: cargo run -p envr-i18n-lint --locked",
            "# Regenerate from source fallbacks: cargo run -p envr-i18n-lint --locked -- --write-locales",
        ];
        let en_header = [
            "# en-US locale for envr (embedded by envr-core).",
            "# Keep keys in sync with zh-CN.toml. CI: cargo run -p envr-i18n-lint --locked",
            "# Regenerate from source fallbacks: cargo run -p envr-i18n-lint --locked -- --write-locales",
        ];
        write_locale_file(&zh_path, &zh_header, &merged_zh).expect("write zh-CN.toml");
        write_locale_file(&en_path, &en_header, &merged_en).expect("write en-US.toml");
        eprintln!(
            "envr-i18n-lint: wrote {} + {} keys to locales (sorted)",
            merged_zh.len(),
            merged_en.len()
        );
        match run_lint(&root) {
            Ok(()) => ExitCode::SUCCESS,
            Err(errs) => {
                for e in errs {
                    eprintln!("envr-i18n-lint: {e}");
                }
                ExitCode::from(1)
            }
        }
    } else {
        match run_lint(&root) {
            Ok(()) => ExitCode::SUCCESS,
            Err(errs) => {
                eprintln!("envr-i18n-lint: {} error(s)", errs.len());
                for e in &errs {
                    eprintln!("  {e}");
                }
                ExitCode::from(1)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_tr_regex_extracts_patch_subcommand_about() {
        let snippet = r#"
        "why" => {
            *cmd = cmd
                .clone()
                .about(tr(
                    "cli.help.cmd.why",
                    "说明某运行时如何解析到当前安装目录（项目 pin / 全局 current）",
                    "Explain how a runtime resolves to its install directory (project pin vs global current)",
                ))
        }
        "#;
        let re = cli_tr_extract_re();
        let caps: Vec<_> = re.captures_iter(snippet).collect();
        assert_eq!(caps.len(), 1, "expected one tr() match");
        assert_eq!(caps[0].get(1).unwrap().as_str(), "cli.help.cmd.why");
    }

    #[test]
    fn extract_cli_help_rs_includes_why() {
        let root = workspace_root();
        let p = root.join("crates/envr-cli/src/cli_help.rs");
        let text = fs::read_to_string(&p).expect("cli_help.rs");
        let mut out = HashMap::new();
        extract_tr_pairs(&text, &p, &mut out);
        assert!(
            out.contains_key("cli.help.cmd.why"),
            "missing why; sample keys: {:?}",
            out.keys().filter(|k| k.contains("why")).collect::<Vec<_>>()
        );
    }
}
