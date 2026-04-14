use crate::cli::GlobalArgs;
use crate::commands::common::kind_label;
use crate::output::{self, fmt_template};

use envr_core::runtime::service::RuntimeService;
use envr_domain::runtime::{RemoteFilter, RuntimeKind, parse_runtime_kind};
use envr_error::EnvrResult;
use envr_platform::paths::{current_platform_paths, index_cache_dir_from_platform};
use envr_runtime_node::{list_node_remote_rows, parse_node_index, NodeRemoteRow};
use serde_json::{Value, json};

// Keep defaults limited to runtimes that support remote listing across platforms.
const ALL_KINDS: [RuntimeKind; 5] = [
    RuntimeKind::Node,
    RuntimeKind::Python,
    RuntimeKind::Java,
    RuntimeKind::Go,
    RuntimeKind::Bun,
];

fn node_index_cache_path() -> Option<std::path::PathBuf> {
    let platform = current_platform_paths().ok()?;
    Some(
        index_cache_dir_from_platform(&platform)
            .join("node")
            .join("index.json"),
    )
}

fn try_node_remote_rows(filter: &RemoteFilter) -> Option<Vec<NodeRemoteRow>> {
    let path = node_index_cache_path()?;
    let body = std::fs::read_to_string(&path).ok()?;
    let releases = parse_node_index(&body).ok()?;
    list_node_remote_rows(
        &releases,
        std::env::consts::OS,
        std::env::consts::ARCH,
        filter,
    )
    .ok()
}

fn node_version_json(row: &NodeRemoteRow) -> Value {
    let mut o = json!({
        "version": row.version,
        "lts": row.lts,
        "lts_codename": Value::Null,
        "date": Value::Null,
    });
    if let Some(cn) = &row.lts_codename {
        o["lts_codename"] = json!(cn);
    }
    if let Some(d) = &row.date {
        o["date"] = json!(d);
    }
    o
}

fn format_remote_node_line(g: &GlobalArgs, row: &NodeRemoteRow) -> String {
    let styles = output::use_terminal_styles(g);
    let lts_tag = envr_core::i18n::tr_key("cli.list.lts_tag", "(LTS)", "(LTS)");
    let mut tags = String::new();
    if row.lts {
        tags.push(' ');
        if styles {
            tags.push_str("\x1b[33m");
        }
        tags.push_str(&lts_tag);
        if let Some(c) = &row.lts_codename {
            tags.push_str(&format!(" {c}"));
        }
        if styles {
            tags.push_str("\x1b[0m");
        }
    }
    if let Some(d) = &row.date {
        tags.push_str(if styles { " \x1b[2m" } else { " " });
        tags.push_str(d);
        if styles {
            tags.push_str("\x1b[0m");
        }
    }
    format!("  {}{tags}", row.version)
}

/// Body for [`crate::commands::dispatch`]; errors are finished at the dispatch boundary.
pub(crate) fn run_inner(
    g: &GlobalArgs,
    service: &RuntimeService,
    runtime: Option<String>,
    prefix: Option<String>,
) -> EnvrResult<i32> {
    let filter = RemoteFilter { prefix };
    let kinds: Vec<RuntimeKind> = match runtime {
        None => ALL_KINDS.to_vec(),
        Some(l) => vec![parse_runtime_kind(l.trim())?],
    };

    enum RemoteRow {
        Plain(Vec<String>),
        Node(Vec<NodeRemoteRow>),
    }

    let mut rows: Vec<(RuntimeKind, RemoteRow)> = Vec::with_capacity(kinds.len());
    for kind in kinds {
        let vers = service.list_remote(kind, &filter)?;
        let payload = if kind == RuntimeKind::Node {
            if let Some(enriched) = try_node_remote_rows(&filter) {
                RemoteRow::Node(enriched)
            } else {
                RemoteRow::Plain(vers.into_iter().map(|v| v.0).collect())
            }
        } else {
            RemoteRow::Plain(vers.into_iter().map(|v| v.0).collect())
        };
        rows.push((kind, payload));
    }

    let runtimes: Vec<_> = rows
        .iter()
        .map(|(k, payload)| {
            let versions: Vec<Value> = match payload {
                RemoteRow::Plain(vers) => vers
                    .iter()
                    .map(|v| serde_json::json!({ "version": v }))
                    .collect(),
                RemoteRow::Node(node_rows) => node_rows.iter().map(node_version_json).collect(),
            };
            serde_json::json!({
                "kind": kind_label(*k),
                "versions": versions,
            })
        })
        .collect();
    let data = serde_json::json!({ "remote_runtimes": runtimes });

    Ok(output::emit_ok(g, "list_remote", data, || {
        if output::wants_porcelain(g) {
            let multi_kind = rows.len() > 1;
            for (kind, payload) in &rows {
                match payload {
                    RemoteRow::Plain(vers) => {
                        for v in vers {
                            if multi_kind {
                                println!("{}\t{}", kind_label(*kind), v);
                            } else {
                                println!("{v}");
                            }
                        }
                    }
                    RemoteRow::Node(node_rows) => {
                        for r in node_rows {
                            if multi_kind {
                                println!("{}\t{}", kind_label(*kind), r.version);
                            } else {
                                println!("{}", r.version);
                            }
                        }
                    }
                }
            }
            return;
        }

        let none_line = envr_core::i18n::tr_key("cli.list.indent_none", "  （无）", "  (none)");
        for (kind, payload) in rows {
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
            match payload {
                RemoteRow::Plain(versions) => {
                    if versions.is_empty() {
                        println!("{none_line}");
                    } else {
                        for v in versions {
                            println!("  {v}");
                        }
                    }
                }
                RemoteRow::Node(node_rows) => {
                    if node_rows.is_empty() {
                        println!("{none_line}");
                    } else {
                        for r in &node_rows {
                            println!("{}", format_remote_node_line(g, r));
                        }
                    }
                }
            }
        }
    }))
}
