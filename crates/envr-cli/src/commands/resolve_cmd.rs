use crate::CliExit;
use crate::CliPathProfile;
use crate::CliUxPolicy;
use crate::cli::{GlobalArgs, ProjectPathProfileArgs};
use crate::output::{self, fmt_template};

use envr_domain::runtime::parse_runtime_kind;
use envr_error::{EnvrError, EnvrResult};
use envr_shim_core::resolve_runtime_home_for_lang_with_project;

use super::version_request::{classify_request, request_kind_str};

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

    let has_pin = cfg
        .and_then(|c| c.runtimes.get(&lang))
        .and_then(|r| r.version.as_deref())
        .is_some();
    let override_nonempty = spec.as_ref().is_some_and(|s| !s.trim().is_empty());
    let source = if override_nonempty {
        "cli_override"
    } else if has_pin {
        "project"
    } else {
        "global_current"
    };

    let trimmed = spec.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let request = classify_request(trimmed, has_pin);
    let home = resolve_runtime_home_for_lang_with_project(&session.ctx, &lang, trimmed, cfg)?;
    let home = std::fs::canonicalize(&home).map_err(EnvrError::from)?;

    let version_label = home
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();

    let mut data = serde_json::json!({
        "kind": lang,
        "resolution_source": source,
        "request_kind": request_kind_str(request.kind),
        "request_value": request.raw,
        "home": home.to_string_lossy(),
        "version_dir": version_label,
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
                    request_kind_str(request.kind)
                );
            }
        },
    ))
}
