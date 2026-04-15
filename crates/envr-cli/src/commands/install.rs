use crate::CliExit;
use crate::CliUxPolicy;
use crate::app;
use crate::cli::GlobalArgs;
use crate::commands::cli_install_progress;
use crate::commands::common::{emit_verbose_step, kind_label};
use crate::output::{self, fmt_template};

use envr_core::runtime::service::RuntimeService;
use envr_domain::runtime::{RuntimeKind, RuntimeVersion, VersionSpec};
use envr_error::EnvrResult;

fn next_steps_for_install(kind: RuntimeKind) -> Vec<(&'static str, String)> {
    vec![
        (
            "set_current_version",
            fmt_template(
                &envr_core::i18n::tr_key(
                    "cli.next_step.install.set_current",
                    "可执行 `envr use {kind} <version>` 将其设为全局 current。",
                    "Run `envr use {kind} <version>` to set it as global current.",
                ),
                &[("kind", kind_label(kind))],
            ),
        ),
        (
            "verify_executable",
            fmt_template(
                &envr_core::i18n::tr_key(
                    "cli.next_step.install.verify_executable",
                    "可执行 `envr which {kind}` 验证最终命中路径。",
                    "Run `envr which {kind}` to verify final executable path.",
                ),
                &[("kind", kind_label(kind))],
            ),
        ),
    ]
}

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
            &[
                ("kind", kind_label(kind)),
                ("version", runtime_version.trim()),
            ],
        ),
    );

    let rv = runtime_version.trim().to_string();
    let headline = fmt_template(
        &envr_core::i18n::tr_key(
            "cli.install.downloading",
            "正在下载 {kind} {version}…",
            "Downloading {kind} {version}…",
        ),
        &[("kind", kind_label(kind)), ("version", rv.as_str())],
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
    let mut data = serde_json::json!({
        "kind": kind_label(kind),
        "version": v.0,
    });
    data = output::with_next_steps(data, next_steps_for_install(kind));
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

#[cfg(test)]
mod tests {
    use super::next_steps_for_install;
    use envr_domain::runtime::RuntimeKind;

    #[test]
    fn install_next_steps_have_expected_ids() {
        let steps = next_steps_for_install(RuntimeKind::Node);
        assert!(steps.iter().any(|(id, _)| *id == "set_current_version"));
        assert!(steps.iter().any(|(id, _)| *id == "verify_executable"));
    }
}
