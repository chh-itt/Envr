//! `envr prune` 鈥?uninstall installed runtime versions except the active `current` one.

use crate::cli::GlobalArgs;
use crate::commands::common::{self, kind_label};
use crate::output;

use envr_core::runtime::service::RuntimeService;
use envr_domain::runtime::{RuntimeKind, RuntimeVersion, parse_runtime_kind};

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

pub fn run(g: &GlobalArgs, service: &RuntimeService, lang: Option<String>, execute: bool) -> i32 {
    let kinds: Vec<RuntimeKind> = match lang {
        None => ALL_KINDS.to_vec(),
        Some(l) => match parse_runtime_kind(l.trim()) {
            Ok(k) => vec![k],
            Err(e) => return common::print_envr_error(g, e),
        },
    };

    let mut plan: Vec<(RuntimeKind, Vec<String>)> = Vec::new();
    for kind in kinds {
        let current = match service.current(kind) {
            Ok(c) => c,
            Err(e) => return common::print_envr_error(g, e),
        };
        let Some(ref cur) = current else {
            plan.push((kind, vec![]));
            continue;
        };
        let installed = match service.list_installed(kind) {
            Ok(v) => v,
            Err(e) => return common::print_envr_error(g, e),
        };
        let to_remove: Vec<String> = installed
            .into_iter()
            .filter(|v| v.0 != cur.0)
            .map(|v| v.0)
            .collect();
        plan.push((kind, to_remove));
    }

    if !execute {
        let rows: Vec<_> = plan
            .iter()
            .map(|(k, vers)| {
                serde_json::json!({
                    "kind": kind_label(*k),
                    "would_remove": vers,
                })
            })
            .collect();
        let data = serde_json::json!({ "dry_run": true, "plan": rows });
        return output::emit_ok(g, "prune_dry_run", data, || {
            println!("Dry run (no versions uninstalled). Use --execute to apply.");
            for (kind, vers) in &plan {
                if vers.is_empty() {
                    println!(
                        "{}: nothing to prune (no current, or only current installed)",
                        kind_label(*kind)
                    );
                } else {
                    println!("{}: would remove:", kind_label(*kind));
                    for v in vers {
                        println!("  {v}");
                    }
                }
            }
        });
    }

    let mut removed: Vec<serde_json::Value> = Vec::new();
    for (kind, vers) in plan {
        for v in vers {
            let rv = RuntimeVersion(v.clone());
            match service.uninstall(kind, &rv) {
                Ok(()) => {
                    removed.push(serde_json::json!({
                        "kind": kind_label(kind),
                        "version": v,
                    }));
                }
                Err(e) => return common::print_envr_error(g, e),
            }
        }
    }

    let data = serde_json::json!({ "removed": removed });
    output::emit_ok(g, "prune_executed", data, || {
        if removed.is_empty() {
            println!("nothing pruned");
        } else {
            for r in &removed {
                println!(
                    "{} {} uninstalled",
                    r["kind"].as_str().unwrap_or(""),
                    r["version"].as_str().unwrap_or("")
                );
            }
        }
    })
}
