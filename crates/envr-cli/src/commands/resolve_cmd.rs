use crate::cli::GlobalArgs;
use crate::commands::common;
use crate::output::{self, fmt_template};

use envr_config::project_config::load_project_config_profile;
use envr_domain::runtime::parse_runtime_kind;
use envr_shim_core::resolve_runtime_home_for_lang;
use std::path::PathBuf;

pub fn run(
    g: &GlobalArgs,
    lang: String,
    spec: Option<String>,
    path: PathBuf,
    profile: Option<String>,
) -> i32 {
    let lang = lang.trim().to_ascii_lowercase();
    if let Err(e) = parse_runtime_kind(&lang) {
        return common::print_envr_error(g, e);
    }

    let ctx = match common::shim_context_for(path, profile) {
        Ok(c) => c,
        Err(e) => return common::print_envr_error(g, e),
    };

    let cfg = match load_project_config_profile(&ctx.working_dir, ctx.profile.as_deref()) {
        Ok(l) => l.map(|(c, _)| c),
        Err(e) => return common::print_envr_error(g, e),
    };
    let has_pin = cfg
        .as_ref()
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
    let home = match resolve_runtime_home_for_lang(&ctx, &lang, trimmed) {
        Ok(h) => h,
        Err(e) => return common::print_envr_error(g, e),
    };
    let home = match std::fs::canonicalize(&home) {
        Ok(h) => h,
        Err(e) => return common::print_envr_error(g, e.into()),
    };
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
    output::emit_ok(g, "runtime_resolved", data, || {
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
    })
}
