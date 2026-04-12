use crate::cli::GlobalArgs;
use crate::commands::common::{self, kind_label};
use crate::output::{self, fmt_template};

use envr_core::runtime::service::RuntimeService;
use envr_domain::runtime::{RuntimeKind, parse_runtime_kind};
use serde_json::Value;

const ALL_KINDS: [RuntimeKind; 8] = [
    RuntimeKind::Node,
    RuntimeKind::Python,
    RuntimeKind::Java,
    RuntimeKind::Go,
    RuntimeKind::Rust,
    RuntimeKind::Php,
    RuntimeKind::Deno,
    RuntimeKind::Bun,
];

pub fn run(g: &GlobalArgs, service: &RuntimeService, runtime: Option<String>) -> i32 {
    let kinds: Vec<RuntimeKind> = match runtime {
        None => ALL_KINDS.to_vec(),
        Some(l) => match parse_runtime_kind(l.trim()) {
            Ok(k) => vec![k],
            Err(e) => return common::print_envr_error(g, e),
        },
    };

    let mut rows: Vec<(RuntimeKind, Option<String>)> = Vec::with_capacity(kinds.len());
    for kind in kinds {
        match service.current(kind) {
            Ok(cur) => rows.push((kind, cur.map(|v| v.0))),
            Err(e) => return common::print_envr_error(g, e),
        }
    }

    let runtimes: Vec<_> = rows
        .iter()
        .map(|(k, ver)| {
            let hint: Value = if ver.is_none() {
                serde_json::json!(fmt_template(
                    &envr_core::i18n::tr_key(
                        "cli.current.none_hint",
                        "使用 `envr use {kind} <version>` 设置全局当前版本。",
                        "None selected. Run `envr use {kind} <version>` to set a global current.",
                    ),
                    &[("kind", kind_label(*k))],
                ))
            } else {
                Value::Null
            };
            serde_json::json!({
                "kind": kind_label(*k),
                "version": ver,
                "hint": hint,
            })
        })
        .collect();
    // JSON `data.active_versions`: one row per runtime kind (see `schemas/cli/data/show_current.json`).
    let data = serde_json::json!({ "active_versions": runtimes });

    output::emit_ok(g, "show_current", data, || {
        if output::wants_porcelain(g) {
            if rows.len() == 1 {
                if let Some(v) = rows[0].1.as_deref() {
                    println!("{v}");
                }
            } else {
                for (kind, version) in &rows {
                    if let Some(v) = version.as_deref() {
                        println!("{}\t{}", kind_label(*kind), v);
                    } else {
                        println!("{}\t", kind_label(*kind));
                    }
                }
            }
            return;
        }
        let none = envr_core::i18n::tr_key("cli.common.none", "（无）", "(none)");
        for (kind, version) in rows {
            let k = kind_label(kind);
            match version {
                Some(v) => {
                    let line = fmt_template(
                        &envr_core::i18n::tr_key(
                            "cli.current.line",
                            "{kind}：{version}",
                            "{kind}: {version}",
                        ),
                        &[("kind", k), ("version", v.as_str())],
                    );
                    if output::use_terminal_styles(g) {
                        println!("\x1b[2m{k}\x1b[0m: \x1b[32;1m{v}\x1b[0m");
                    } else {
                        println!("{line}");
                    }
                }
                None => {
                    println!(
                        "{}",
                        fmt_template(
                            &envr_core::i18n::tr_key(
                                "cli.current.none_line",
                                "{kind}：{none}",
                                "{kind}: {none}",
                            ),
                            &[("kind", k), ("none", &none)],
                        )
                    );
                    let hint = fmt_template(
                        &envr_core::i18n::tr_key(
                            "cli.current.none_hint",
                            "使用 `envr use {kind} <version>` 设置全局当前版本。",
                            "None selected. Run `envr use {kind} <version>` to set a global current.",
                        ),
                        &[("kind", k)],
                    );
                    if output::use_terminal_styles(g) {
                        println!("\x1b[2m  {hint}\x1b[0m");
                    } else {
                        println!("  {hint}");
                    }
                }
            }
        }
    })
}
