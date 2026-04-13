//! `envr rust …` — helpers that bypass the generic `RuntimeService` install path.

use crate::cli::{GlobalArgs, RustCmd};
use crate::commands::cli_install_progress;
use crate::CommandOutcome;
use crate::output;

use envr_config::settings::resolve_runtime_root;
use envr_domain::runtime::VersionSpec;
use envr_error::EnvrResult;
use envr_runtime_rust::{RustChannel, install_rustup_managed};

pub fn run(g: &GlobalArgs, sub: RustCmd) -> i32 {
    match sub {
        RustCmd::InstallManaged => CommandOutcome::from_result(install_managed_inner(g)).finish(g),
    }
}

fn install_managed_inner(g: &GlobalArgs) -> EnvrResult<i32> {
    let root = resolve_runtime_root()?;
    let headline = envr_core::i18n::tr_key(
        "cli.rust.install_managed.downloading",
        "正在下载 rustup-init（托管安装 stable）…",
        "Downloading rustup-init (managed stable)…",
    );
    let spec = VersionSpec("stable".into());
    let (request, guard) =
        cli_install_progress::install_request_with_progress(g, spec, headline);
    let res = install_rustup_managed(root, RustChannel::Stable, Some(&request));
    guard.finish();
    res?;
    let data = serde_json::json!({ "channel": "stable" });
    Ok(output::emit_ok(g, "rust_managed_installed", data, || {
        if !g.quiet {
            println!(
                "{}",
                envr_core::i18n::tr_key(
                    "cli.rust.install_managed.ok",
                    "已安装托管 rustup（stable 默认工具链）",
                    "Managed rustup installed (stable default toolchain)",
                )
            );
        }
    }))
}
