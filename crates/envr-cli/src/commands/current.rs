use crate::cli::GlobalArgs;
use crate::commands::common::{self, kind_label};
use crate::output::{self, fmt_template};

use envr_core::runtime::service::RuntimeService;
use envr_domain::runtime::{RuntimeKind, parse_runtime_kind};

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

pub fn run(g: &GlobalArgs, service: &RuntimeService, lang: Option<String>) -> i32 {
    let kinds: Vec<RuntimeKind> = match lang {
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
            serde_json::json!({
                "kind": kind_label(*k),
                "version": ver,
            })
        })
        .collect();
    let data = serde_json::json!({ "current": runtimes });

    output::emit_ok(g, "show_current", data, || {
        let none = envr_core::i18n::tr_key("cli.common.none", "（无）", "(none)");
        for (kind, version) in rows {
            let k = kind_label(kind);
            match version {
                Some(v) => println!(
                    "{}",
                    fmt_template(
                        &envr_core::i18n::tr_key(
                            "cli.current.line",
                            "{kind}：{version}",
                            "{kind}: {version}",
                        ),
                        &[("kind", k), ("version", v.as_str())],
                    )
                ),
                None => println!(
                    "{}",
                    fmt_template(
                        &envr_core::i18n::tr_key(
                            "cli.current.none_line",
                            "{kind}：{none}",
                            "{kind}: {none}",
                        ),
                        &[("kind", k), ("none", &none)],
                    )
                ),
            }
        }
    })
}
