//! `envr prune` — uninstall installed runtime versions except the active `current` one.
use crate::CliExit;

use crate::cli::GlobalArgs;
use crate::commands::common::kind_label;
use crate::output::{self, fmt_template};

use envr_core::runtime::service::RuntimeService;
use envr_domain::runtime::{RuntimeKind, RuntimeVersion, parse_runtime_kind, runtime_kinds_all};
use envr_error::EnvrResult;

/// Body for [`crate::commands::dispatch`]; errors are finished at the dispatch boundary.
pub(crate) fn run_inner(
    g: &GlobalArgs,
    service: &RuntimeService,
    lang: Option<String>,
    execute: bool,
) -> EnvrResult<CliExit> {
    let kinds: Vec<RuntimeKind> = match lang {
        None => runtime_kinds_all().collect(),
        Some(l) => vec![parse_runtime_kind(l.trim())?],
    };

    let mut plan: Vec<(RuntimeKind, Vec<String>)> = Vec::new();
    for kind in kinds {
        let index = service.index_port(kind)?;
        let current = index.current()?;
        let Some(ref cur) = current else {
            plan.push((kind, vec![]));
            continue;
        };
        let installed = index.list_installed()?;
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
        return Ok(output::emit_ok(
            g,
            crate::codes::ok::PRUNE_DRY_RUN,
            data,
            || {
                println!(
                    "{}",
                    envr_core::i18n::tr_key(
                        "cli.prune.dry_run_hint",
                        "试运行（未卸载任何版本）。使用 --execute 生效。",
                        "Dry run (no versions uninstalled). Use --execute to apply.",
                    )
                );
                for (kind, vers) in &plan {
                    let k = kind_label(*kind);
                    if vers.is_empty() {
                        println!(
                            "{}",
                            fmt_template(
                                &envr_core::i18n::tr_key(
                                    "cli.prune.nothing_to_prune_kind",
                                    "{kind}：无可清理项（无 current，或仅安装了 current）",
                                    "{kind}: nothing to prune (no current, or only current installed)",
                                ),
                                &[("kind", k)],
                            )
                        );
                    } else {
                        println!(
                            "{}",
                            fmt_template(
                                &envr_core::i18n::tr_key(
                                    "cli.prune.would_remove_header",
                                    "{kind}：将移除：",
                                    "{kind}: would remove:",
                                ),
                                &[("kind", k)],
                            )
                        );
                        for v in vers {
                            println!("  {v}");
                        }
                    }
                }
            },
        ));
    }

    let mut removed: Vec<serde_json::Value> = Vec::new();
    for (kind, vers) in plan {
        let installer = service.installer_port(kind)?;
        for v in vers {
            let rv = RuntimeVersion(v.clone());
            installer.uninstall(&rv)?;
            removed.push(serde_json::json!({
                "kind": kind_label(kind),
                "version": v,
            }));
        }
    }

    let data = serde_json::json!({ "removed": removed });
    Ok(output::emit_ok(
        g,
        crate::codes::ok::PRUNE_EXECUTED,
        data,
        || {
            if removed.is_empty() {
                println!(
                    "{}",
                    envr_core::i18n::tr_key(
                        "cli.prune.nothing_pruned",
                        "未清理任何版本",
                        "nothing pruned",
                    )
                );
            } else {
                for r in &removed {
                    let kind = r["kind"].as_str().unwrap_or("");
                    let version = r["version"].as_str().unwrap_or("");
                    println!(
                        "{}",
                        fmt_template(
                            &envr_core::i18n::tr_key(
                                "cli.prune.uninstalled_line",
                                "已卸载 {kind} {version}",
                                "{kind} {version} uninstalled",
                            ),
                            &[("kind", kind), ("version", version)],
                        )
                    );
                }
            }
        },
    ))
}
