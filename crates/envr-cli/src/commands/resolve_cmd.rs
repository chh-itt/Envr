use crate::cli::{GlobalArgs, ProjectPathProfileArgs};
use crate::CommandOutcome;
use crate::output::{self, fmt_template};
use crate::CliPathProfile;

use envr_domain::runtime::parse_runtime_kind;
use envr_error::{EnvrError, EnvrResult};
use envr_shim_core::resolve_runtime_home_for_lang_with_project;
pub fn run(
    g: &GlobalArgs,
    lang: String,
    spec: Option<String>,
    project: ProjectPathProfileArgs,
) -> i32 {
    CommandOutcome::from_result(run_inner(g, lang, spec, project)).finish(g)
}

fn run_inner(
    g: &GlobalArgs,
    lang: String,
    spec: Option<String>,
    project: ProjectPathProfileArgs,
) -> EnvrResult<i32> {
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
    let home = resolve_runtime_home_for_lang_with_project(&session.ctx, &lang, trimmed, cfg)?;
    let home = std::fs::canonicalize(&home).map_err(EnvrError::from)?;

    let version_label = home
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();

    let data = serde_json::json!({
        "kind": lang,
        "resolution_source": source,
        "home": home.to_string_lossy(),
        "version_dir": version_label,
    });
    Ok(output::emit_ok(g, "runtime_resolved", data, || {
        if output::wants_porcelain(g) {
            println!("{}", home.display());
            return;
        }
        if !g.quiet {
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
        }
    }))
}
