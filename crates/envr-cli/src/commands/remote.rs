use crate::cli::GlobalArgs;
use crate::commands::common::{self, kind_label};
use crate::output::{self, fmt_template};

use envr_core::runtime::service::RuntimeService;
use envr_domain::runtime::{RemoteFilter, RuntimeKind, parse_runtime_kind};

// Keep defaults limited to runtimes that support remote listing across platforms.
const ALL_KINDS: [RuntimeKind; 5] = [
    RuntimeKind::Node,
    RuntimeKind::Python,
    RuntimeKind::Java,
    RuntimeKind::Go,
    RuntimeKind::Bun,
];

pub fn run(
    g: &GlobalArgs,
    service: &RuntimeService,
    lang: Option<String>,
    prefix: Option<String>,
) -> i32 {
    let filter = RemoteFilter { prefix };
    let kinds: Vec<RuntimeKind> = match lang {
        None => ALL_KINDS.to_vec(),
        Some(l) => match parse_runtime_kind(l.trim()) {
            Ok(k) => vec![k],
            Err(e) => return common::print_envr_error(g, e),
        },
    };

    let mut rows: Vec<(RuntimeKind, Vec<String>)> = Vec::with_capacity(kinds.len());
    for kind in kinds {
        match service.list_remote(kind, &filter) {
            Ok(vers) => rows.push((kind, vers.into_iter().map(|v| v.0).collect())),
            Err(e) => return common::print_envr_error(g, e),
        }
    }

    let runtimes: Vec<_> = rows
        .iter()
        .map(|(k, vers)| {
            serde_json::json!({
                "kind": kind_label(*k),
                "versions": vers,
            })
        })
        .collect();
    let data = serde_json::json!({ "runtimes": runtimes });

    output::emit_ok(g, "list_remote", data, || {
        let none_line = envr_core::i18n::tr_key(
            "cli.list.indent_none",
            "  （无）",
            "  (none)",
        );
        for (kind, versions) in rows {
            println!(
                "{}",
                fmt_template(
                    &envr_core::i18n::tr_key(
                        "cli.remote.header",
                        "{kind}（远程）：",
                        "{kind} (remote):",
                    ),
                    &[("kind", kind_label(kind))],
                )
            );
            if versions.is_empty() {
                println!("{none_line}");
            } else {
                for v in versions {
                    println!("  {v}");
                }
            }
        }
    })
}
