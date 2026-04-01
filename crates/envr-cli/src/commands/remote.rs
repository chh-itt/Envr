use crate::cli::{GlobalArgs, OutputFormat};
use crate::commands::common::{self, kind_label};

use envr_core::runtime::service::RuntimeService;
use envr_domain::runtime::{RemoteFilter, RuntimeKind, parse_runtime_kind};

const ALL_KINDS: [RuntimeKind; 3] = [RuntimeKind::Node, RuntimeKind::Python, RuntimeKind::Java];

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

    match g.output_format.unwrap_or(OutputFormat::Text) {
        OutputFormat::Json => {
            let runtimes: Vec<_> = rows
                .iter()
                .map(|(k, vers)| {
                    serde_json::json!({
                        "kind": kind_label(*k),
                        "versions": vers,
                    })
                })
                .collect();
            println!(
                "{}",
                serde_json::json!({
                    "success": true,
                    "data": { "runtimes": runtimes },
                    "diagnostics": [],
                })
            );
        }
        OutputFormat::Text => {
            for (kind, versions) in rows {
                println!("{} (remote):", kind_label(kind));
                if versions.is_empty() {
                    println!("  (none)");
                } else {
                    for v in versions {
                        println!("  {v}");
                    }
                }
            }
        }
    }
    0
}
