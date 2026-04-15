use crate::cli::GlobalArgs;
use crate::CliExit;
use crate::CliUxPolicy;
use crate::app;
use crate::commands::cli_install_progress;
use crate::commands::common::{emit_verbose_step, kind_label};
use crate::output::{self, fmt_template};

use envr_core::runtime::service::RuntimeService;
use envr_domain::runtime::{
    RuntimeKind, RuntimeVersion, VersionSpec,
};
use envr_error::EnvrResult;

/// Body for [`crate::commands::dispatch`]; errors are finished at the dispatch boundary.
pub(crate) fn run_inner(
    g: &GlobalArgs,
    service: &RuntimeService,
    runtime: String,
    runtime_version: String,
) -> EnvrResult<CliExit> {
    let kind = app::runtime_installation::parse_kind(&runtime)?;
    emit_verbose_step(
        g,
        &fmt_template(
            &envr_core::i18n::tr_key(
                "cli.verbose.install.resolve",
                "[verbose] 正在解析版本：{kind} {version}",
                "[verbose] resolving version: {kind} {version}",
            ),
            &[("kind", kind_label(kind)), ("version", runtime_version.trim())],
        ),
    );

    let rv = runtime_version.trim().to_string();
    let headline = fmt_template(
        &envr_core::i18n::tr_key(
            "cli.install.downloading",
            "正在下载 {kind} {version}…",
            "Downloading {kind} {version}…",
        ),
        &[
            ("kind", kind_label(kind)),
            ("version", rv.as_str()),
        ],
    );
    let spec = VersionSpec(rv);
    let (request, guard) = cli_install_progress::install_request_with_progress(g, spec, headline);
    emit_verbose_step(
        g,
        &fmt_template(
            &envr_core::i18n::tr_key(
                "cli.verbose.install.download",
                "[verbose] 开始安装与下载：{kind}",
                "[verbose] starting install/download: {kind}",
            ),
            &[("kind", kind_label(kind))],
        ),
    );
    let res = service.install(kind, &request);
    guard.finish();
    let v = res?;
    Ok(print_success(g, kind, &v))
}

fn print_success(g: &GlobalArgs, kind: RuntimeKind, v: &RuntimeVersion) -> CliExit {
    let data = serde_json::json!({
        "kind": kind_label(kind),
        "version": v.0,
    });
    output::emit_ok(g, crate::codes::ok::INSTALLED, data, || {
        if CliUxPolicy::from_global(g).human_text_primary() {
            println!(
                "{}",
                fmt_template(
                    &envr_core::i18n::tr_key(
                        "cli.install.ok",
                        "已安装 {kind} {version}",
                        "{kind} {version} installed",
                    ),
                    &[("kind", kind_label(kind)), ("version", &v.0)],
                )
            );
        }
    })
}
