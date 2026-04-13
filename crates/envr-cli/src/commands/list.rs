use crate::cli::GlobalArgs;
use crate::CommandOutcome;
use crate::commands::common::{kind_label, session_runtime_root};
use crate::output::{self, fmt_template};

use envr_core::runtime::service::RuntimeService;
use envr_error::EnvrResult;
use envr_domain::runtime::{RuntimeKind, RuntimeVersion, parse_runtime_kind};
use envr_platform::paths::{current_platform_paths, index_cache_dir_from_platform};
use envr_runtime_node::{NodePaths, normalize_node_version, parse_node_index};
use serde_json::{Value, json};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::Path;

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
        _ => t
            .split(['.', '-', '+'])
            .next()
            .unwrap_or(t)
            .to_string(),
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
            let v = if cn == "true" { String::new() } else { cn.clone() };
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
                    && let Some(cn) = map.get(&norm) {
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
    let styles = output::use_terminal_styles(g);
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
            && !c.is_empty() {
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

pub fn run(
    g: &GlobalArgs,
    service: &RuntimeService,
    runtime: Option<String>,
    outdated: bool,
) -> i32 {
    CommandOutcome::from_result(run_inner(g, service, runtime, outdated)).finish(g)
}

fn run_inner(
    g: &GlobalArgs,
    service: &RuntimeService,
    runtime: Option<String>,
    outdated: bool,
) -> EnvrResult<i32> {
    let kinds: Vec<RuntimeKind> = match runtime {
        None => ALL_KINDS.to_vec(),
        Some(l) => vec![parse_runtime_kind(l.trim())?],
    };

    let mut rows: Vec<(RuntimeKind, Vec<RuntimeVersion>)> = Vec::with_capacity(kinds.len());
    let mut currents: Vec<Option<RuntimeVersion>> = Vec::with_capacity(kinds.len());
    for kind in kinds {
        let vers = service.list_installed(kind)?;
        let cur = service.current(kind).ok().flatten();
        currents.push(cur);
        rows.push((kind, vers));
    }

    let remote_maps: Vec<Option<HashMap<String, String>>> = if outdated {
        rows
            .iter()
            .map(|(kind, _)| {
                match service.list_remote_latest_per_major(*kind) {
                    Ok(list) if !list.is_empty() => {
                        Some(remote_latest_by_line(*kind, &list))
                    }
                    _ => Some(HashMap::new()),
                }
            })
            .collect()
    } else {
        (0..rows.len()).map(|_| None).collect()
    };

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
    let data = json!({ "installed_runtimes": runtimes });

    Ok(output::emit_ok(g, "list_installed", data, || {
        if output::wants_porcelain(g) {
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
        let np = runtime_root
            .as_ref()
            .map(|r| NodePaths::new(r.clone()));
        for (((kind, versions), cur), rmap) in rows
            .iter()
            .zip(currents.iter())
            .zip(remote_maps.iter())
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
                        let lts_cn = cn.and_then(|s| {
                            if s.is_empty() {
                                None
                            } else {
                                Some(s.as_str())
                            }
                        });
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
                                    ).to_string()
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
        }
    }))
}
