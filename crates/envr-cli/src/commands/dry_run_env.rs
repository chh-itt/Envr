//! Dry-run helpers: full env dump vs diff against the parent process.

use crate::cli::GlobalArgs;
use crate::output;

use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

pub fn parent_env_snapshot() -> HashMap<String, String> {
    std::env::vars().collect()
}

fn path_tokens(s: &str) -> Vec<String> {
    std::env::split_paths(s)
        .map(|p| p.display().to_string())
        .collect()
}

fn path_entries_added(parent: Option<&String>, child: Option<&String>) -> Vec<String> {
    let pa = parent.map(|s| path_tokens(s)).unwrap_or_default();
    let set: HashSet<_> = pa.into_iter().collect();
    let cb = child.map(|s| path_tokens(s)).unwrap_or_default();
    cb.into_iter().filter(|p| !set.contains(p)).collect()
}

fn path_entries_removed(parent: Option<&String>, child: Option<&String>) -> Vec<String> {
    path_entries_added(child, parent)
}

/// Build structured diff: added / changed / removed keys and PATH entry deltas.
pub fn env_diff(parent: &HashMap<String, String>, child: &HashMap<String, String>) -> Value {
    let mut added = serde_json::Map::new();
    let mut changed = serde_json::Map::new();
    let mut removed = serde_json::Map::new();

    let pk: HashSet<_> = parent.keys().cloned().collect();
    let ck: HashSet<_> = child.keys().cloned().collect();

    for k in ck.difference(&pk) {
        if let Some(v) = child.get(k) {
            added.insert(k.clone(), json!(v));
        }
    }
    for k in pk.difference(&ck) {
        if let Some(v) = parent.get(k) {
            removed.insert(k.clone(), json!(v));
        }
    }
    for k in pk.intersection(&ck) {
        let p = parent.get(k).unwrap();
        let c = child.get(k).unwrap();
        if p != c {
            changed.insert(
                k.clone(),
                json!({
                    "from": p,
                    "to": c,
                }),
            );
        }
    }

    let path_key = if child.contains_key("PATH") || parent.contains_key("PATH") {
        "PATH"
    } else if cfg!(windows) && (child.contains_key("Path") || parent.contains_key("Path")) {
        "Path"
    } else {
        "PATH"
    };
    let p_path = parent.get(path_key);
    let c_path = child.get(path_key);

    json!({
        "added": Value::Object(added),
        "changed": Value::Object(changed),
        "removed": Value::Object(removed),
        "path_entries_added": path_entries_added(p_path, c_path),
        "path_entries_removed": path_entries_removed(p_path, c_path),
    })
}

pub fn shell_words_join(args: &[String]) -> String {
    args.iter()
        .map(|a| {
            if a.contains(char::is_whitespace) {
                format!("{a:?}")
            } else {
                a.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn emit_dry_run_diff(
    g: &GlobalArgs,
    parent: &HashMap<String, String>,
    child: &HashMap<String, String>,
    command: &str,
    args: &[String],
) -> i32 {
    let diff = env_diff(parent, child);
    let data = json!({
        "command": command,
        "args": args,
        "env_diff": diff,
    });
    output::emit_ok(g, "dry_run_diff", data, || {
        if g.quiet {
            return;
        }
        println!(
            "{}",
            envr_core::i18n::tr_key(
                "cli.dry_run.would_run",
                "将执行：",
                "Would run:",
            )
        );
        println!("  {} {}", command, shell_words_join(args));
        println!();

        let styles = output::use_terminal_styles(g);
        let hl = |s: &str| {
            if styles {
                format!("\x1b[33;1m{s}\x1b[0m")
            } else {
                s.to_string()
            }
        };

        let pe_added: Vec<String> = serde_json::from_value(
            diff.get("path_entries_added").cloned().unwrap_or(json!([])),
        )
        .unwrap_or_default();
        if !pe_added.is_empty() {
            println!(
                "{}",
                envr_core::i18n::tr_key(
                    "cli.dry_run.path_added_heading",
                    "PATH 新增条目（前置顺序）：",
                    "PATH entries prepended (in order):",
                )
            );
            for p in &pe_added {
                println!("  + {}", hl(p));
            }
            println!();
        }

        let pe_removed: Vec<String> = serde_json::from_value(
            diff.get("path_entries_removed")
                .cloned()
                .unwrap_or(json!([])),
        )
        .unwrap_or_default();
        if !pe_removed.is_empty() {
            println!(
                "{}",
                envr_core::i18n::tr_key(
                    "cli.dry_run.path_removed_heading",
                    "PATH 移除条目（相对当前 shell）：",
                    "PATH entries no longer present (vs current shell):",
                )
            );
            for p in &pe_removed {
                println!("  - {p}");
            }
            println!();
        }

        if let Some(obj) = diff.get("added").and_then(|v| v.as_object()) {
            if !obj.is_empty() {
                println!(
                    "{}",
                    envr_core::i18n::tr_key(
                        "cli.dry_run.env_added_heading",
                        "新增环境变量：",
                        "New environment variables:",
                    )
                );
                let mut keys: Vec<_> = obj.keys().cloned().collect();
                keys.sort();
                for k in keys {
                    if k == "PATH" || k == "Path" {
                        continue;
                    }
                    if let Some(v) = obj.get(&k) {
                        println!("  + {k}={}", v.as_str().unwrap_or(""));
                    }
                }
                println!();
            }
        }

        if let Some(obj) = diff.get("changed").and_then(|v| v.as_object()) {
            if !obj.is_empty() {
                println!(
                    "{}",
                    envr_core::i18n::tr_key(
                        "cli.dry_run.env_changed_heading",
                        "变更的环境变量：",
                        "Changed environment variables:",
                    )
                );
                let mut keys: Vec<_> = obj.keys().cloned().collect();
                keys.sort();
                for k in keys {
                    if k == "PATH" || k == "Path" {
                        continue;
                    }
                    if let Some(entry) = obj.get(&k).and_then(|v| v.as_object()) {
                        let from = entry.get("from").and_then(|v| v.as_str()).unwrap_or("");
                        let to = entry.get("to").and_then(|v| v.as_str()).unwrap_or("");
                        println!("  ~ {k}: {from} -> {to}");
                    }
                }
            }
        }
    })
}
