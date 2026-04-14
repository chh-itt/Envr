use crate::cli::GlobalArgs;
use crate::commands::common;
use crate::output::{self, fmt_template};
use crate::CliPathProfile;

use envr_error::{EnvrError, EnvrResult};
use envr_shim_core::{
    WhichRuntimeDetail, WhichRuntimeSource, normalize_invoked_basename, parse_core_command,
    resolve_core_shim_command, which_runtime_detail,
};

/// Body for [`crate::commands::dispatch`]; errors are finished at the dispatch boundary.
pub(crate) fn run_inner(g: &GlobalArgs, name: Option<String>) -> EnvrResult<i32> {
    let Some(name) = name else {
        return Ok(common::missing_positional(g, "which", "envr which node"));
    };

    let base = normalize_invoked_basename(name.trim());
    let Some(cmd) = parse_core_command(&base) else {
        return Err(EnvrError::Validation(crate::output::fmt_template(
            &envr_core::i18n::tr_key(
                "cli.err.unknown_tool",
                "未知工具 `{name}`（可试 node、npm、npx、python、pip、java、javac、bun、bunx）",
                "unknown tool `{name}` (try node, npm, npx, python, pip, java, javac, bun, bunx)",
            ),
            &[("name", name.trim())],
        )));
    };

    let cwd = std::env::current_dir().map_err(EnvrError::from)?;
    let session = CliPathProfile::new(cwd, None).load_project()?;

    let shim = resolve_core_shim_command(cmd, &session.ctx)?;
    let detail = which_runtime_detail(cmd, &session.ctx, &shim.executable)?;
    let selection_source = which_selection_json(&detail.source);
    let data = serde_json::json!({
        "executable": shim.executable.to_string_lossy(),
        "version": detail.version,
        "selection_source": selection_source,
        "extra_env": shim.extra_env.iter().map(|(k, v)| {
            serde_json::json!({ "key": k, "value": v })
        }).collect::<Vec<_>>(),
    });
    Ok(output::emit_ok(g, "resolved_executable", data, || {
        println!("{}", shim.executable.display());
        if output::wants_porcelain(g) {
            return;
        }
        let meta = format_which_meta_line(&detail);
        if output::use_terminal_styles(g) {
            println!("\x1b[2m{meta}\x1b[0m");
        } else {
            println!("{meta}");
        }
        for (k, v) in &shim.extra_env {
            eprintln!("{k}={v}");
        }
    }))
}

fn which_selection_json(src: &WhichRuntimeSource) -> &'static str {
    match src {
        WhichRuntimeSource::ProjectPin => "project_pin",
        WhichRuntimeSource::GlobalCurrent => "global_current",
        WhichRuntimeSource::PathProxyBypass => "path_proxy_bypass",
    }
}

fn format_which_meta_line(d: &WhichRuntimeDetail) -> String {
    match (d.source, d.version.as_str()) {
        (WhichRuntimeSource::PathProxyBypass, "system") => envr_core::i18n::tr_key(
            "cli.which.only_bypass",
            "system PATH（已在设置中关闭路径代理）",
            "system PATH (path proxy disabled in settings)",
        ),
        _ => {
            let source_phrase = match d.source {
                WhichRuntimeSource::ProjectPin => envr_core::i18n::tr_key(
                    "cli.which.source.project",
                    "来自项目 .envr.toml",
                    "from project .envr.toml",
                ),
                WhichRuntimeSource::GlobalCurrent => envr_core::i18n::tr_key(
                    "cli.which.source.global",
                    "来自全局 current",
                    "from global current",
                ),
                WhichRuntimeSource::PathProxyBypass => envr_core::i18n::tr_key(
                    "cli.which.source.bypass",
                    "system PATH（路径代理已关闭）",
                    "system PATH (path proxy disabled)",
                ),
            };
            fmt_template(
                &envr_core::i18n::tr_key(
                    "cli.which.meta",
                    "version: {version}（{source}）",
                    "version: {version} ({source})",
                ),
                &[
                    ("version", d.version.as_str()),
                    ("source", source_phrase.as_str()),
                ],
            )
        }
    }
}
