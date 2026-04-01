use crate::cli::GlobalArgs;
use crate::commands::common::{self, kind_label};
use crate::output;

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
        for (kind, version) in rows {
            match version {
                Some(v) => println!("{}: {v}", kind_label(kind)),
                None => println!("{}: (none)", kind_label(kind)),
            }
        }
    })
}
