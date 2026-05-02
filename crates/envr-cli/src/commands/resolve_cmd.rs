use crate::CliExit;
use crate::CliPathProfile;
use crate::CliUxPolicy;
use crate::cli::{GlobalArgs, ProjectPathProfileArgs};
use crate::output::{self, fmt_template};
use serde_json::json;

use envr_domain::runtime::parse_runtime_kind;
use envr_config::project_config::project_lock_is_fresh;
use envr_error::{EnvrError, EnvrResult};
use envr_shim_core::{resolve_runtime_home_for_lang_with_project, resolve_version_home};

use super::version_request::{classify_request, explain_request};

fn next_steps_for_resolve(lang: &str, source: &str) -> Vec<(&'static str, String)> {
    let mut steps = Vec::new();
    steps.push((
        "check_resolved_executable",
        crate::output::fmt_template(
            &envr_core::i18n::tr_key(
                "cli.next_step.resolve.check_executable",
                "可执行 `envr which {lang}` 查看最终可执行文件路径。",
                "Run `envr which {lang}` to inspect the final executable path.",
            ),
            &[("lang", lang)],
        ),
    ));
    if source != "global_current" {
        steps.push((
            "set_global_current",
            envr_core::i18n::tr_key(
                "cli.next_step.resolve.set_global_current",
                "如需全局默认版本，可执行 `envr use <runtime> <version>`。",
                "If you want a global default version, run `envr use <runtime> <version>`.",
            ),
        ));
    }
    steps
}

/// Body for [`crate::commands::dispatch`]; errors are finished at the dispatch boundary.
pub(crate) fn run_inner(
    g: &GlobalArgs,
    lang: String,
    spec: Option<String>,
    project: ProjectPathProfileArgs,
) -> EnvrResult<CliExit> {
    let ProjectPathProfileArgs { path, profile } = project;
    let lang = lang.trim().to_ascii_lowercase();
    parse_runtime_kind(&lang)?;

    let session = CliPathProfile::new(path, profile).load_project()?;
    let cfg = session.project_config();
    let lock_state = session.project.as_ref().and_then(|(_, loc)| {
        let lock_path = loc.lock_file.as_ref()?;
        let fresh = project_lock_is_fresh(session.project_config(), lock_path).ok()?;
        Some((lock_path.to_string_lossy().to_string(), fresh))
    });

    let has_pin = cfg
        .and_then(|c| c.runtimes.get(&lang))
        .and_then(|r| r.version.as_deref())
        .is_some();
    let compat_name = cfg
        .and_then(|c| c.compat.asdf.names.iter().find_map(|(asdf, envr)| (envr == &lang).then_some(asdf.clone())));
    let override_nonempty = spec.as_ref().is_some_and(|s| !s.trim().is_empty());
    let source = if override_nonempty {
        "cli_override"
    } else if has_pin {
        "project"
    } else if compat_name.is_some() {
        "tool_versions_compat"
    } else {
        "global_current"
    };

    let trimmed = spec.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let request = classify_request(trimmed, has_pin);
    let resolution = if let Some(spec) = trimmed {
        let versions_dir = session
            .ctx
            .runtime_root
            .join("runtimes")
            .join(&lang)
            .join("versions");
        resolve_version_home(&versions_dir, spec).ok()
    } else {
        None
    };
    let home = resolve_runtime_home_for_lang_with_project(&session.ctx, &lang, trimmed, cfg)?;
    let home = std::fs::canonicalize(&home).map_err(EnvrError::from)?;

    let resolved_version = resolution
        .as_ref()
        .and_then(|r| r.resolved_version.clone())
        .or_else(|| home.file_name().and_then(|s| s.to_str()).map(|s| s.to_string()))
        .unwrap_or_default();

    let mut data = serde_json::json!({
        "kind": lang,
        "resolution_source": source,
        "compat_source": compat_name,
        "request_kind": request.kind_str(),
        "request_value": request.raw,
        "request_normalized": request.normalized,
        "resolved_version": resolved_version,
        "resolution_reason": if override_nonempty {
            explain_request(&request)
        } else if has_pin {
            "resolved from project runtime pin"
        } else if compat_name.is_some() {
            "resolved via .tool-versions compatibility mapping"
        } else {
            "resolved from global current runtime"
        },
        "candidate_count": resolution.as_ref().map(|r| r.candidate_count),
        "selection_reason": resolution.as_ref().map(|r| r.selection_reason()),
        "home": home.to_string_lossy(),
        "lock": lock_state.as_ref().map(|(path, fresh)| json!({
            "path": path,
            "fresh": fresh,
        })),
    });
    data = output::with_next_steps(data, next_steps_for_resolve(&lang, source));
    Ok(output::emit_ok(
        g,
        crate::codes::ok::RUNTIME_RESOLVED,
        data,
        || {
            let ux = CliUxPolicy::from_global(g);
            if ux.wants_porcelain_lines() {
                println!("{}", home.display());
                return;
            }
            if ux.human_text_primary() {
                let source_label = match source {
                    "cli_override" => envr_core::i18n::tr_key(
                        "cli.resolve.source.cli_override",
                        "命令行覆盖",
                        "CLI override",
                    ),
                    "project" => {
                        envr_core::i18n::tr_key("cli.resolve.source.project", "项目", "project")
                    }
                    "global_current" => envr_core::i18n::tr_key(
                        "cli.resolve.source.global_current",
                        "全局 current",
                        "global current",
                    ),
                    other => other.to_string(),
                };
                println!(
                    "{}",
                    fmt_template(
                        &envr_core::i18n::tr_key(
                            "cli.resolve.line",
                            "{lang}：{path}（来源：{source}）",
                            "{lang}: {path} (from {source})",
                        ),
                        &[
                            ("lang", &lang),
                            ("path", &home.display().to_string()),
                            ("source", &source_label),
                        ],
                    )
                );
                println!(
                    "{} {}",
                    envr_core::i18n::tr_key(
                        "cli.resolve.request_kind",
                        "请求类型：",
                        "Request kind:",
                    ),
                    request.kind_str()
                );
                if let Some(count) = resolution.as_ref().map(|r| r.candidate_count) {
                    println!(
                        "{} {}",
                        envr_core::i18n::tr_key(
                            "cli.resolve.candidate_count",
                            "候选数量：",
                            "Candidate count:"
                        ),
                        count
                    );
                }
                if let Some(reason) = resolution.as_ref().map(|r| r.selection_reason()) {
                    println!(
                        "{} {}",
                        envr_core::i18n::tr_key(
                            "cli.resolve.selection_reason",
                            "选择理由：",
                            "Selection reason:"
                        ),
                        reason
                    );
                }
            }
        },
    ))
}
