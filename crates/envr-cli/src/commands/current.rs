use crate::CliExit;
use crate::CliUxPolicy;
use crate::app;
use crate::cli::GlobalArgs;
use crate::commands::common::kind_label;
use crate::output::{self, fmt_template};

use envr_core::runtime::service::RuntimeService;
use envr_domain::runtime::{RuntimeKind, runtime_kinds_all};
use envr_error::EnvrResult;
use serde_json::Value;

fn next_steps_for_current() -> Vec<(&'static str, String)> {
    vec![(
        "set_or_update_current",
        envr_core::i18n::tr_key(
            "cli.next_step.current.set_or_update",
            "可执行 `envr use <runtime> <version>` 设置或更新全局 current。",
            "Run `envr use <runtime> <version>` to set or update global current.",
        ),
    )]
}

/// Body for [`crate::commands::dispatch`]; errors are finished at the dispatch boundary.
pub(crate) fn run_inner(
    g: &GlobalArgs,
    service: &RuntimeService,
    runtime: Option<String>,
) -> EnvrResult<CliExit> {
    let kinds: Vec<RuntimeKind> = match runtime {
        None => runtime_kinds_all().collect(),
        Some(l) => vec![app::runtime_installation::parse_kind(&l)?],
    };

    let mut rows: Vec<(RuntimeKind, Option<String>)> = Vec::with_capacity(kinds.len());
    for kind in kinds {
        let cur = service.current(kind)?;
        rows.push((kind, cur.map(|v| v.0)));
    }

    let runtimes: Vec<_> = rows
        .iter()
        .map(|(k, ver)| {
            let hint: Value = if ver.is_none() {
                serde_json::json!(fmt_template(
                    &envr_core::i18n::tr_key(
                        "cli.current.none_hint",
                        "使用 `envr use {kind} <version>` 设置全局当前版本。",
                        "None selected. Run `envr use {kind} <version>` to set a global current.",
                    ),
                    &[("kind", kind_label(*k))],
                ))
            } else {
                Value::Null
            };
            serde_json::json!({
                "kind": kind_label(*k),
                "version": ver,
                "hint": hint,
            })
        })
        .collect();
    // JSON `data.active_versions`: one row per runtime kind (see `schemas/cli/data/show_current.json`).
    let mut data = serde_json::json!({ "active_versions": runtimes });
    data = output::with_next_steps(data, next_steps_for_current());

    Ok(output::emit_ok(
        g,
        crate::codes::ok::SHOW_CURRENT,
        data,
        || {
            let ux = CliUxPolicy::from_global(g);
            if ux.wants_porcelain_lines() {
                if rows.len() == 1 {
                    if let Some(v) = rows[0].1.as_deref() {
                        println!("{v}");
                    }
                } else {
                    for (kind, version) in &rows {
                        if let Some(v) = version.as_deref() {
                            println!("{}\t{}", kind_label(*kind), v);
                        } else {
                            println!("{}\t", kind_label(*kind));
                        }
                    }
                }
                return;
            }
            let none = envr_core::i18n::tr_key("cli.common.none", "（无）", "(none)");
            for (kind, version) in rows {
                let k = kind_label(kind);
                match version {
                    Some(v) => {
                        let line = fmt_template(
                            &envr_core::i18n::tr_key(
                                "cli.current.line",
                                "{kind}：{version}",
                                "{kind}: {version}",
                            ),
                            &[("kind", k), ("version", v.as_str())],
                        );
                        if ux.use_rich_text_styles() {
                            println!("\x1b[2m{k}\x1b[0m: \x1b[32;1m{v}\x1b[0m");
                        } else {
                            println!("{line}");
                        }
                    }
                    None => {
                        println!(
                            "{}",
                            fmt_template(
                                &envr_core::i18n::tr_key(
                                    "cli.current.none_line",
                                    "{kind}：{none}",
                                    "{kind}: {none}",
                                ),
                                &[("kind", k), ("none", &none)],
                            )
                        );
                        let hint = fmt_template(
                            &envr_core::i18n::tr_key(
                                "cli.current.none_hint",
                                "使用 `envr use {kind} <version>` 设置全局当前版本。",
                                "None selected. Run `envr use {kind} <version>` to set a global current.",
                            ),
                            &[("kind", k)],
                        );
                        if ux.use_rich_text_styles() {
                            println!("\x1b[2m  {hint}\x1b[0m");
                        } else {
                            println!("  {hint}");
                        }
                    }
                }
            }
        },
    ))
}
