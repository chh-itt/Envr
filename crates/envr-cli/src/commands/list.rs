use crate::CliExit;
use crate::CliUxPolicy;
use crate::app;
use crate::cli::GlobalArgs;
use crate::commands::common::{
    emit_verbose_step, kind_label, runtime_service, session_runtime_root,
};
use crate::output::{self, fmt_template};

use envr_core::runtime::service::RuntimeService;
use envr_domain::runtime::{
    RuntimeKind, RuntimeVersion, runtime_kinds_all, version_line_key_for_kind,
};
use envr_error::EnvrResult;
use envr_platform::paths::{current_platform_paths, index_cache_dir_from_platform};
use envr_runtime_node::{NodePaths, normalize_node_version, parse_node_index};
use serde_json::{Value, json};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::Path;
use std::thread;

fn cmp_version_labels(a: &str, b: &str) -> Ordering {
    fn tokens(s: &str) -> Vec<&str> {
        s.split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|t| !t.is_empty())
            .collect()
    }
    let ta = tokens(a);
    let tb = tokens(b);
    let n = ta.len().max(tb.len());
    for i in 0..n {
        let va = ta.get(i).copied().unwrap_or("");
        let vb = tb.get(i).copied().unwrap_or("");
        let ord = match (va.parse::<u64>(), vb.parse::<u64>()) {
            (Ok(na), Ok(nb)) => na.cmp(&nb),
            _ => va.cmp(vb),
        };
        if ord != Ordering::Equal {
            return ord;
        }
    }
    Ordering::Equal
}

/// Grouping key for `list_remote_latest_per_major` rows vs an installed version.
fn major_line_key(kind: RuntimeKind, v: &str) -> String {
    let t = v.trim().trim_start_matches('v').trim_start_matches('V');
    match kind {
        RuntimeKind::Go => {
            let parts: Vec<&str> = t.split('.').collect();
            if parts.len() >= 2 && parts[0] == "1" {
                format!("{}.{}", parts[0], parts[1])
            } else if let Some(f) = parts.first() {
                (*f).to_string()
            } else {
                t.to_string()
            }
        }
        RuntimeKind::Zig
        | RuntimeKind::Julia
        | RuntimeKind::Lua
        | RuntimeKind::Kotlin
        | RuntimeKind::Scala
        | RuntimeKind::Clojure
        | RuntimeKind::Groovy
        | RuntimeKind::Terraform
        | RuntimeKind::Nim
        | RuntimeKind::Crystal
        | RuntimeKind::RLang => { version_line_key_for_kind(kind, t) }
            .unwrap_or_else(|| t.split(['.', '-', '+']).next().unwrap_or(t).to_string()),
        _ => t.split(['.', '-', '+']).next().unwrap_or(t).to_string(),
    }
}

fn remote_latest_by_line(kind: RuntimeKind, remote: &[RuntimeVersion]) -> HashMap<String, String> {
    let mut m: HashMap<String, String> = HashMap::new();
    for rv in remote {
        let key = major_line_key(kind, &rv.0);
        m.entry(key)
            .and_modify(|cur| {
                if cmp_version_labels(cur, &rv.0) == Ordering::Less {
                    *cur = rv.0.clone();
                }
            })
            .or_insert_with(|| rv.0.clone());
    }
    m
}

/// Normalized semver (no leading `v`) → optional LTS codename; empty string = LTS without codename.
fn try_node_lts_map() -> Option<HashMap<String, String>> {
    let platform = current_platform_paths().ok()?;
    let p = index_cache_dir_from_platform(&platform)
        .join("node")
        .join("index.json");
    let body = std::fs::read_to_string(&p).ok()?;
    let releases = parse_node_index(&body).ok()?;
    let mut m = HashMap::new();
    for r in releases {
        if let Some(cn) = &r.lts_codename {
            let k = normalize_node_version(&r.version);
            let v = if cn == "true" {
                String::new()
            } else {
                cn.clone()
            };
            m.insert(k, v);
        }
    }
    Some(m)
}

fn npm_package_version(path: &Path) -> Option<String> {
    let s = std::fs::read_to_string(path).ok()?;
    let v: Value = serde_json::from_str(&s).ok()?;
    v.get("version")?.as_str().map(|s| s.to_string())
}

fn try_read_bundled_npm_version(node_home: &Path) -> Option<String> {
    [
        node_home.join("lib/node_modules/npm/package.json"),
        node_home.join("node_modules/npm/package.json"),
    ]
    .into_iter()
    .find_map(|p| npm_package_version(&p))
}

fn version_json_entries(
    kind: RuntimeKind,
    versions: &[RuntimeVersion],
    current: Option<&RuntimeVersion>,
    node_lts: Option<&HashMap<String, String>>,
    runtime_root: Option<&std::path::PathBuf>,
    remote_by_line: Option<&HashMap<String, String>>,
) -> Vec<Value> {
    let node_paths = runtime_root.map(|r| NodePaths::new(r.clone()));
    versions
        .iter()
        .map(|ver| {
            let is_cur = current.is_some_and(|c| c.0 == ver.0);
            let mut row = json!({
                "version": ver.0.clone(),
                "current": is_cur,
                "lts": false,
                "lts_codename": Value::Null,
                "npm": Value::Null,
            });
            if let Some(map) = remote_by_line {
                let key = major_line_key(kind, &ver.0);
                let remote_latest = map.get(&key).cloned();
                let outdated = remote_latest
                    .as_ref()
                    .is_some_and(|r| cmp_version_labels(&ver.0, r) == Ordering::Less);
                row["remote_latest"] = json!(remote_latest);
                row["outdated"] = json!(outdated);
            }
            if kind == RuntimeKind::Node {
                let norm = normalize_node_version(&ver.0);
                if let Some(map) = node_lts
                    && let Some(cn) = map.get(&norm)
                {
                    row["lts"] = json!(true);
                    if !cn.is_empty() {
                        row["lts_codename"] = json!(cn);
                    }
                }
                if let Some(np) = &node_paths {
                    let home = np.version_dir(&ver.0);
                    if let Some(npm) = try_read_bundled_npm_version(&home) {
                        row["npm"] = json!(npm);
                    }
                }
            }
            row
        })
        .collect()
}

fn format_version_text_line(
    g: &GlobalArgs,
    version: &str,
    is_current: bool,
    is_lts: bool,
    lts_codename: Option<&str>,
    npm: Option<&str>,
    outdated_hint: Option<&str>,
) -> String {
    let styles = CliUxPolicy::from_global(g).use_rich_text_styles();
    let mark = if is_current {
        if styles {
            "\x1b[32;1m*\x1b[0m".to_string()
        } else {
            "*".to_string()
        }
    } else {
        " ".to_string()
    };
    let lts_tag = envr_core::i18n::tr_key("cli.list.lts_tag", "(LTS)", "(LTS)");
    let mut tags = String::new();
    if is_lts {
        tags.push(' ');
        if styles {
            tags.push_str("\x1b[33m");
        }
        tags.push_str(&lts_tag);
        if let Some(c) = lts_codename
            && !c.is_empty()
        {
            tags.push_str(&format!(" {c}"));
        }
        if styles {
            tags.push_str("\x1b[0m");
        }
    }
    if let Some(npm_v) = npm {
        tags.push_str(&format!(
            "{}npm {npm_v}",
            if styles { " \x1b[2m" } else { " " },
        ));
        if styles {
            tags.push_str("\x1b[0m");
        }
    }
    if let Some(h) = outdated_hint {
        tags.push_str(if styles { " \x1b[36m" } else { " " });
        tags.push_str(h);
        if styles {
            tags.push_str("\x1b[0m");
        }
    }
    let ver_display = if styles {
        if is_current {
            format!("\x1b[32;1m{version}\x1b[0m")
        } else {
            format!("\x1b[2m{version}\x1b[0m")
        }
    } else {
        version.to_string()
    };
    format!("  {mark} {ver_display}{tags}")
}

fn next_steps_for_list(outdated: bool, stale_remote_index: bool) -> Vec<(&'static str, String)> {
    let mut steps = Vec::new();
    if outdated {
        steps.push((
            "sync_remote_index",
            envr_core::i18n::tr_key(
                "cli.next_step.list.sync_remote_index",
                "可执行 `envr cache index sync` 预热远程索引，再次查看可升级提示更稳定。",
                "Run `envr cache index sync` to warm remote indexes for more stable upgrade hints.",
            ),
        ));
        if stale_remote_index {
            steps.push((
                "re_run_list_outdated",
                envr_core::i18n::tr_key(
                    "cli.next_step.list.re_run_outdated",
                    "远程索引后台刷新后，重试 `envr list --outdated` 获取更完整结果。",
                    "After background refresh, re-run `envr list --outdated` for more complete results.",
                ),
            ));
        }
    }
    steps
}

/// Body for [`crate::commands::dispatch`]; errors are finished at the dispatch boundary.
pub(crate) fn run_inner(
    g: &GlobalArgs,
    service: &RuntimeService,
    runtime: Option<String>,
    outdated: bool,
) -> EnvrResult<CliExit> {
    let kinds: Vec<RuntimeKind> = match runtime {
        None => runtime_kinds_all().collect(),
        Some(l) => vec![app::runtime_installation::parse_kind(&l)?],
    };

    let mut rows: Vec<(RuntimeKind, Vec<RuntimeVersion>)> = Vec::with_capacity(kinds.len());
    let mut currents: Vec<Option<RuntimeVersion>> = Vec::with_capacity(kinds.len());
    for kind in kinds {
        let vers = service.list_installed(kind)?;
        let cur = service.current(kind).ok().flatten();
        currents.push(cur);
        rows.push((kind, vers));
    }

    let mut stale_remote_index = false;
    let kinds_for_refresh: Vec<RuntimeKind> = rows.iter().map(|(k, _)| *k).collect();
    let remote_maps: Vec<Option<HashMap<String, String>>> = if outdated {
        rows.iter()
            .map(|(kind, _)| {
                let cached = service.try_load_remote_latest_per_major_from_disk(*kind);
                if cached.is_empty() {
                    stale_remote_index = true;
                }
                Some(remote_latest_by_line(*kind, &cached))
            })
            .collect()
    } else {
        (0..rows.len()).map(|_| None).collect()
    };
    if outdated {
        if stale_remote_index {
            emit_verbose_step(
                g,
                &envr_core::i18n::tr_key(
                    "cli.verbose.list.outdated_refresh",
                    "[verbose] 本地远程索引为空或过旧，已在后台刷新缓存",
                    "[verbose] remote index cache is empty/stale; refreshing in background",
                ),
            );
        }
        let _ = thread::Builder::new()
            .name("envr-list-outdated-refresh".to_string())
            .spawn(move || {
                if let Ok(svc) = runtime_service() {
                    for kind in kinds_for_refresh {
                        let _ = svc.list_remote_latest_per_major(kind);
                    }
                }
            });
    }

    let node_lts = try_node_lts_map();
    let runtime_root = session_runtime_root().ok();

    let runtimes: Vec<_> = rows
        .iter()
        .zip(currents.iter())
        .zip(remote_maps.iter())
        .map(|(((k, vers), cur), rmap)| {
            json!({
                "kind": kind_label(*k),
                "versions": version_json_entries(*k, vers, cur.as_ref(), node_lts.as_ref(), runtime_root.as_ref(), rmap.as_ref()),
            })
        })
        .collect();
    let mut data = json!({ "installed_runtimes": runtimes });
    data = output::with_next_steps(data, next_steps_for_list(outdated, stale_remote_index));

    Ok(output::emit_ok(
        g,
        crate::codes::ok::LIST_INSTALLED,
        data,
        || {
            if CliUxPolicy::from_global(g).wants_porcelain_lines() {
                if rows.len() == 1 {
                    for v in &rows[0].1 {
                        println!("{}", v.0);
                    }
                } else {
                    for (kind, versions) in &rows {
                        for v in versions {
                            println!("{}\t{}", kind_label(*kind), v.0);
                        }
                    }
                }
                return;
            }
            let none_line = envr_core::i18n::tr_key("cli.list.indent_none", "  （无）", "  (none)");
            let np = runtime_root.as_ref().map(|r| NodePaths::new(r.clone()));
            for (((kind, versions), cur), rmap) in
                rows.iter().zip(currents.iter()).zip(remote_maps.iter())
            {
                println!(
                    "{}",
                    fmt_template(
                        &envr_core::i18n::tr_key("cli.list.header", "{kind}：", "{kind}:",),
                        &[("kind", kind_label(*kind))],
                    )
                );
                if versions.is_empty() {
                    println!("{none_line}");
                } else {
                    for ver in versions {
                        let is_cur = cur.as_ref().is_some_and(|c| c.0 == ver.0);
                        let (is_lts, lts_cn, npm_str) = if *kind == RuntimeKind::Node {
                            let norm = normalize_node_version(&ver.0);
                            let cn = node_lts.as_ref().and_then(|m| m.get(&norm));
                            let is_lts = cn.is_some();
                            let lts_cn =
                                cn.and_then(|s| if s.is_empty() { None } else { Some(s.as_str()) });
                            let npm = np
                                .as_ref()
                                .and_then(|p| try_read_bundled_npm_version(&p.version_dir(&ver.0)));
                            (is_lts, lts_cn, npm)
                        } else {
                            (false, None, None)
                        };
                        let outdated_hint = rmap.as_ref().and_then(|map| {
                            let key = major_line_key(*kind, &ver.0);
                            map.get(&key).and_then(|latest| {
                                (cmp_version_labels(&ver.0, latest) == Ordering::Less).then(|| {
                                    fmt_template(
                                        &envr_core::i18n::tr_key(
                                            "cli.list.outdated_hint",
                                            "（可升级至 {latest}）",
                                            "(upgrade to {latest})",
                                        ),
                                        &[("latest", latest.as_str())],
                                    )
                                    .to_string()
                                })
                            })
                        });
                        let line = format_version_text_line(
                            g,
                            &ver.0,
                            is_cur,
                            is_lts,
                            lts_cn,
                            npm_str.as_deref(),
                            outdated_hint.as_deref(),
                        );
                        println!("{line}");
                    }
                }
                if outdated && stale_remote_index {
                    println!(
                        "{}",
                        envr_core::i18n::tr_key(
                            "cli.list.outdated_refresh_notice",
                            "  远程索引更新中，本次结果可能暂不含最新可升级信息（下次命令生效）。",
                            "  Remote index is refreshing; upgrade hints may be incomplete this run (available next command).",
                        )
                    );
                }
            }
        },
    ))
}
