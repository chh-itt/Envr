use crate::CliExit;
use crate::CliUxPolicy;
use crate::app;
use crate::cli::GlobalArgs;
use crate::commands::common::{emit_verbose_step, kind_label, runtime_service};
use crate::output::{self, fmt_template};

use envr_core::runtime::service::RuntimeService;
use envr_domain::runtime::{RemoteFilter, RuntimeKind, runtime_descriptor, runtime_kinds_all};
use envr_error::EnvrResult;
use envr_platform::paths::{current_platform_paths, index_cache_dir_from_platform};
use envr_runtime_node::{NodeRemoteRow, list_node_remote_rows, parse_node_index};
use serde_json::{Value, json};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

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
    let styles = CliUxPolicy::from_global(g).use_rich_text_styles();
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

fn next_steps_for_remote(refreshing: bool, prefix_fallback: bool) -> Vec<(&'static str, String)> {
    let mut steps = Vec::new();
    if refreshing {
        steps.push((
            "re_run_after_refresh",
            envr_core::i18n::tr_key(
                "cli.next_step.remote.re_run_after_refresh",
                "稍后重试 `envr remote`，获取最新远程索引结果。",
                "Re-run `envr remote` shortly to get refreshed index results.",
            ),
        ));
    }
    if prefix_fallback {
        steps.push((
            "retry_prefix_query",
            envr_core::i18n::tr_key(
                "cli.next_step.remote.retry_prefix_query",
                "当前为本地快照前缀过滤结果；稍后重试可获取更完整远程匹配。",
                "Current result is from local snapshot prefix filter; retry shortly for fuller remote matches.",
            ),
        ));
    }
    if steps.is_empty() {
        steps.push((
            "install_from_remote_list",
            envr_core::i18n::tr_key(
                "cli.next_step.remote.install_from_list",
                "从列表中选择版本后，使用 `envr install <运行时> <版本>` 安装。",
                "Pick a version from the list, then run `envr install <runtime> <version>`.",
            ),
        ));
    }
    steps
}

fn filtered_cached_snapshot(
    service: &RuntimeService,
    kind: RuntimeKind,
    filter: &RemoteFilter,
) -> Vec<String> {
    let mut versions: Vec<String> = service
        .try_load_full_remote_installable_from_disk(kind)
        .into_iter()
        .map(|v| v.0)
        .collect();
    if versions.is_empty() {
        versions = service
            .list_major_rows_cached(kind)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|r| r.latest_installable.map(|v| v.0))
            .collect();
    }
    if versions.is_empty() {
        versions = service
            .try_load_remote_latest_per_major_from_disk(kind)
            .into_iter()
            .map(|v| v.0)
            .collect();
    }
    if let Some(prefix) = filter.prefix.as_ref() {
        let p = prefix.trim().trim_start_matches('v').to_ascii_lowercase();
        if !p.is_empty() {
            versions.retain(|v| v.to_ascii_lowercase().starts_with(&p));
        }
    }
    versions
}

fn try_fetch_remote_with_timeout(
    kind: RuntimeKind,
    filter: RemoteFilter,
    timeout: Duration,
) -> Option<Vec<String>> {
    let (tx, rx) = mpsc::channel();
    let _ = thread::Builder::new()
        .name(format!("envr-remote-fast-{kind:?}").to_ascii_lowercase())
        .spawn(move || {
            let res = (|| -> EnvrResult<Vec<envr_domain::runtime::RuntimeVersion>> {
                let svc = runtime_service()?;
                let idx = svc.index_port(kind)?;
                idx.list_remote_installable(&filter)
            })();
            let _ = tx.send(res);
        });
    match rx.recv_timeout(timeout) {
        Ok(Ok(list)) => Some(list.into_iter().map(|v| v.0).collect()),
        _ => None,
    }
}

/// Body for [`crate::commands::dispatch`]; errors are finished at the dispatch boundary.
pub(crate) fn run_inner(
    g: &GlobalArgs,
    service: &RuntimeService,
    runtime: Option<String>,
    prefix: Option<String>,
    update: bool,
) -> EnvrResult<CliExit> {
    let filter = RemoteFilter {
        prefix,
        force_index_refresh: update,
    };
    let single_runtime = runtime.is_some();
    let kinds: Vec<RuntimeKind> = match runtime {
        None => runtime_kinds_all()
            .filter(|k| runtime_descriptor(*k).supports_remote_latest)
            .collect(),
        Some(l) => vec![app::runtime_installation::parse_kind(&l)?],
    };
    let kinds_for_refresh = kinds.clone();

    enum RemoteRow {
        Plain(Vec<String>),
        Node(Vec<NodeRemoteRow>),
    }

    let mut rows: Vec<(RuntimeKind, RemoteRow)> = Vec::with_capacity(kinds.len());
    let mut used_cached_snapshot = false;
    let mut missing_cached_snapshot = false;
    let mut prefix_fallback = false;
    for kind in kinds {
        if update {
            let index = service.index_port(kind)?;
            let full = index.list_remote_installable(&filter)?;
            if filter.prefix.is_none() {
                let _ = service.persist_full_remote_installable_snapshot(kind, &full);
            }
            let vers = full.into_iter().map(|v| v.0).collect::<Vec<_>>();
            if filter.prefix.is_none() {
                let _ = index.list_remote_latest_installable_per_major();
            }
            let payload = if kind == RuntimeKind::Node {
                if let Some(enriched) = try_node_remote_rows(&filter) {
                    RemoteRow::Node(enriched)
                } else {
                    RemoteRow::Plain(vers)
                }
            } else {
                RemoteRow::Plain(vers)
            };
            rows.push((kind, payload));
            continue;
        }
        if filter.prefix.is_none() {
            let cached = filtered_cached_snapshot(service, kind, &RemoteFilter::default());
            if !cached.is_empty() {
                used_cached_snapshot = true;
                rows.push((kind, RemoteRow::Plain(cached)));
                continue;
            }
            // Single-runtime cold start: an empty on-disk snapshot should not print "(无)" —
            // block once on the same `list_remote` path as `-u` / update so the first run is useful.
            if single_runtime {
                let index = service.index_port(kind)?;
                let full = index.list_remote_installable(&RemoteFilter::default())?;
                let _ = service.persist_full_remote_installable_snapshot(kind, &full);
                let vers = full.into_iter().map(|v| v.0).collect::<Vec<_>>();
                let _ = index.list_remote_latest_installable_per_major();
                rows.push((kind, RemoteRow::Plain(vers)));
                continue;
            }
            missing_cached_snapshot = true;
            rows.push((kind, RemoteRow::Plain(Vec::new())));
            continue;
        }
        // Prefix query: prefer local unified snapshot first (stale OK), then refresh in background.
        let cached = filtered_cached_snapshot(service, kind, &filter);
        if !cached.is_empty() {
            used_cached_snapshot = true;
            let payload = if kind == RuntimeKind::Node {
                if let Some(enriched) = try_node_remote_rows(&filter) {
                    RemoteRow::Node(enriched)
                } else {
                    RemoteRow::Plain(cached)
                }
            } else {
                RemoteRow::Plain(cached)
            };
            rows.push((kind, payload));
            continue;
        }
        let timeout = Duration::from_millis(900);
        let vers =
            try_fetch_remote_with_timeout(kind, filter.clone(), timeout).unwrap_or_else(|| {
                prefix_fallback = true;
                filtered_cached_snapshot(service, kind, &filter)
            });
        let payload = if kind == RuntimeKind::Node {
            if let Some(enriched) = try_node_remote_rows(&filter) {
                RemoteRow::Node(enriched)
            } else {
                RemoteRow::Plain(vers)
            }
        } else {
            RemoteRow::Plain(vers)
        };
        rows.push((kind, payload));
    }
    let remote_refreshing = !update
        && ((filter.prefix.is_none() && (used_cached_snapshot || missing_cached_snapshot))
            || (filter.prefix.is_some() && (used_cached_snapshot || prefix_fallback)));
    if remote_refreshing {
        emit_verbose_step(
            g,
            &envr_core::i18n::tr_key(
                "cli.verbose.remote.refreshing",
                "[verbose] 正在后台刷新远程索引缓存",
                "[verbose] refreshing remote index cache in background",
            ),
        );
        let filter_bg = filter.clone();
        let _ = thread::Builder::new()
            .name("envr-remote-refresh".to_string())
            .spawn(move || {
                if let Ok(svc) = runtime_service() {
                    for kind in kinds_for_refresh {
                        if let Ok(index) = svc.index_port(kind) {
                            let _ = index.list_remote_latest_installable_per_major();
                            if let Ok(full) = index.list_remote_installable(&filter_bg) {
                                if filter_bg.prefix.is_none() {
                                    let _ =
                                        svc.persist_full_remote_installable_snapshot(kind, &full);
                                }
                            }
                        }
                    }
                }
            });
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
    let mut data = serde_json::json!({
        "remote_runtimes": runtimes,
        "cached_snapshot": used_cached_snapshot,
        "remote_refreshing": remote_refreshing,
        "prefix_fallback": prefix_fallback,
    });
    data = output::with_next_steps(
        data,
        next_steps_for_remote(remote_refreshing, prefix_fallback),
    );

    Ok(output::emit_ok(
        g,
        crate::codes::ok::LIST_REMOTE,
        data,
        || {
            if CliUxPolicy::from_global(g).wants_porcelain_lines() {
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
            if remote_refreshing && filter.prefix.is_none() {
                println!(
                    "{}",
                    envr_core::i18n::tr_key(
                        "cli.remote.refresh_notice",
                        "远程索引更新中，本次先展示本地快照（或空结果）；稍后再次执行可获得最新数据。",
                        "Remote index is refreshing; this run shows a local snapshot (or empty rows). Re-run shortly for latest results.",
                    )
                );
            }
            if prefix_fallback {
                println!(
                    "{}",
                    envr_core::i18n::tr_key(
                        "cli.remote.prefix_fallback_notice",
                        "前缀查询在短时间内未完成，已降级为本地快照过滤结果（可能不完整），稍后重试可获取最新。",
                        "Prefix query did not complete quickly; showing filtered local snapshot (may be incomplete). Re-run shortly for latest data.",
                    )
                );
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
        },
    ))
}
