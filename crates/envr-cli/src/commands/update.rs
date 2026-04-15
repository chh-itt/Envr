//! `envr update` — CLI version info (self-update TBD).
use crate::CliExit;

use crate::cli::GlobalArgs;
use crate::output::{self, fmt_template};

use envr_error::EnvrResult;

/// Body for [`crate::commands::dispatch`]; errors are finished at the dispatch boundary.
pub(crate) fn run_inner(g: &GlobalArgs, check: bool) -> EnvrResult<CliExit> {
    let version = env!("CARGO_PKG_VERSION");
    let data = serde_json::json!({
        "version": version,
        "check_requested": check,
        "self_update": "not_implemented",
    });
    Ok(output::emit_ok(g, crate::codes::ok::UPDATE_INFO, data, || {
        println!(
            "{}",
            fmt_template(
                &envr_core::i18n::tr_key(
                    "cli.update.version_line",
                    "envr {version}",
                    "envr {version}",
                ),
                &[("version", version)],
            )
        );
        if check {
            println!(
                "{}",
                envr_core::i18n::tr_key(
                    "cli.update.release_check_pending",
                    "（版本检查尚未实现）",
                    "(release check is not implemented yet)",
                )
            );
        }
        println!(
            "{}",
            envr_core::i18n::tr_key(
                "cli.update.self_update_hint",
                "暂不支持自更新；有升级时请从软件包来源重新安装。",
                "Self-update is not implemented; reinstall from your package source when upgrades are available.",
            )
        );
    }))
}
