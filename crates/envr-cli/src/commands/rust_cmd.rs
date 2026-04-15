//! `envr rust …` — helpers that bypass the generic `RuntimeService` install path.
use crate::CliExit;
use crate::CliUxPolicy;

use crate::cli::GlobalArgs;
use crate::commands::cli_install_progress;
use crate::output;

use envr_config::settings::resolve_runtime_root;
use envr_domain::runtime::VersionSpec;
use envr_error::EnvrResult;
use envr_runtime_rust::{RustChannel, install_rustup_managed};

/// Body for [`crate::commands::dispatch`]; errors are finished at the dispatch boundary.
pub(crate) fn install_managed_inner(g: &GlobalArgs) -> EnvrResult<CliExit> {
    let root = resolve_runtime_root()?;
    let headline = envr_core::i18n::tr_key(
        "cli.rust.install_managed.downloading",
        "正在下载 rustup-init（托管安装 stable）…",
        "Downloading rustup-init (managed stable)…",
    );
    let spec = VersionSpec("stable".into());
    let (request, guard) = cli_install_progress::install_request_with_progress(g, spec, headline);
    let res = install_rustup_managed(root, RustChannel::Stable, Some(&request));
    guard.finish();
    res?;
    let data = serde_json::json!({ "channel": "stable" });
    Ok(output::emit_ok(
        g,
        crate::codes::ok::RUST_MANAGED_INSTALLED,
        data,
        || {
            if CliUxPolicy::from_global(g).human_text_primary() {
                println!(
                    "{}",
                    envr_core::i18n::tr_key(
                        "cli.rust.install_managed.ok",
                        "已安装托管 rustup（stable 默认工具链）",
                        "Managed rustup installed (stable default toolchain)",
                    )
                );
            }
        },
    ))
}
