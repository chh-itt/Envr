//! `envr update` — CLI version info (self-update TBD).

use crate::cli::GlobalArgs;
use crate::CommandOutcome;
use crate::output::{self, fmt_template};

use envr_error::EnvrResult;

pub fn run(g: &GlobalArgs, check: bool) -> i32 {
    CommandOutcome::from_result(run_inner(g, check)).finish(g)
}

fn run_inner(g: &GlobalArgs, check: bool) -> EnvrResult<i32> {
    let version = env!("CARGO_PKG_VERSION");
    let data = serde_json::json!({
        "version": version,
        "check_requested": check,
        "self_update": "not_implemented",
    });
    Ok(output::emit_ok(g, "update_info", data, || {
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
