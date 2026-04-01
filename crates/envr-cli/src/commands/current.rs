use crate::cli::{GlobalArgs, OutputFormat};
use crate::commands::common::{self, kind_label};

use envr_core::runtime::service::RuntimeService;
use envr_domain::runtime::{RuntimeKind, parse_runtime_kind};

const ALL_KINDS: [RuntimeKind; 3] = [RuntimeKind::Node, RuntimeKind::Python, RuntimeKind::Java];

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

    match g.output_format.unwrap_or(OutputFormat::Text) {
        OutputFormat::Json => {
            let runtimes: Vec<_> = rows
                .iter()
                .map(|(k, ver)| {
                    serde_json::json!({
                        "kind": kind_label(*k),
                        "version": ver,
                    })
                })
                .collect();
            println!(
                "{}",
                serde_json::json!({
                    "success": true,
                    "data": { "current": runtimes },
                    "diagnostics": [],
                })
            );
        }
        OutputFormat::Text => {
            for (kind, version) in rows {
                match version {
                    Some(v) => println!("{}: {v}", kind_label(kind)),
                    None => println!("{}: (none)", kind_label(kind)),
                }
            }
        }
    }
    0
}
