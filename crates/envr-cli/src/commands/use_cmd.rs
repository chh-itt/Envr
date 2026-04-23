use crate::CliExit;
use crate::CliUxPolicy;
use crate::app;
use crate::cli::GlobalArgs;
use crate::commands::common::{emit_verbose_step, kind_label};
use crate::commands::shim_cmd;
use crate::output::{self, fmt_template};

use envr_config::settings::Settings;
use envr_core::runtime::service::RuntimeService;
use envr_domain::runtime::{RuntimeKind, RuntimeVersion, VersionSpec};
use envr_error::EnvrResult;

fn next_steps_for_use(kind: RuntimeKind) -> Vec<(&'static str, String)> {
    vec![
        (
            "verify_current",
            fmt_template(
                &envr_core::i18n::tr_key(
                    "cli.next_step.use.verify_current",
                    "可执行 `envr current {kind}` 确认 current 已生效。",
                    "Run `envr current {kind}` to confirm current is active.",
                ),
                &[("kind", kind_label(kind))],
            ),
        ),
        (
            "verify_executable",
            fmt_template(
                &envr_core::i18n::tr_key(
                    "cli.next_step.use.verify_executable",
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
                "cli.verbose.use.resolve",
                "[verbose] 正在解析 current 目标：{kind} {version}",
                "[verbose] resolving current target: {kind} {version}",
            ),
            &[
                ("kind", kind_label(kind)),
                ("version", runtime_version.trim()),
            ],
        ),
    );

    let spec = VersionSpec(runtime_version.clone());
    let resolved = app::runtime_installation::set_current(service, kind, spec)?;
    let auto_sync_behavior = load_use_auto_sync_behavior();
    if auto_sync_behavior.auto_sync_shims_on_use {
        let _ = shim_cmd::sync_kind_after_use(
            kind,
            auto_sync_behavior.auto_sync_globals_on_use,
            auto_sync_behavior.auto_sync_windows_path_mirror_on_use,
        );
    }
    emit_verbose_step(
        g,
        &fmt_template(
            &envr_core::i18n::tr_key(
                "cli.verbose.use.set_current",
                "[verbose] 正在设置 current：{kind} {version}",
                "[verbose] setting current: {kind} {version}",
            ),
            &[("kind", kind_label(kind)), ("version", &resolved.0)],
        ),
    );
    Ok(print_success(g, kind, &resolved))
}

#[derive(Debug, Clone, Copy)]
struct UseAutoSyncBehavior {
    auto_sync_shims_on_use: bool,
    auto_sync_globals_on_use: bool,
    auto_sync_windows_path_mirror_on_use: bool,
}

fn load_use_auto_sync_behavior() -> UseAutoSyncBehavior {
    let defaults = UseAutoSyncBehavior {
        auto_sync_shims_on_use: true,
        auto_sync_globals_on_use: false,
        auto_sync_windows_path_mirror_on_use: cfg!(windows),
    };
    let Ok(platform) = envr_platform::paths::current_platform_paths() else {
        return defaults;
    };
    let path = envr_config::settings::settings_path_from_platform(&platform);
    let Ok(settings) = Settings::load_or_default_from(&path) else {
        return defaults;
    };
    UseAutoSyncBehavior {
        auto_sync_shims_on_use: settings.behavior.auto_sync_shims_on_use,
        auto_sync_globals_on_use: settings.behavior.auto_sync_globals_on_use,
        auto_sync_windows_path_mirror_on_use: settings.behavior.auto_sync_windows_path_mirror_on_use,
    }
}

fn print_success(g: &GlobalArgs, kind: RuntimeKind, v: &RuntimeVersion) -> CliExit {
    let mut data = serde_json::json!({
        "kind": kind_label(kind),
        "version": v.0,
    });
    data = output::with_next_steps(data, next_steps_for_use(kind));
    output::emit_ok(g, crate::codes::ok::CURRENT_RUNTIME_SET, data, || {
        if CliUxPolicy::from_global(g).human_text_primary() {
            println!(
                "{}",
                fmt_template(
                    &envr_core::i18n::tr_key(
                        "cli.use.ok",
                        "已将 {kind} 的 current 设为 {version}",
                        "{kind} current set to {version}",
                    ),
                    &[("kind", kind_label(kind)), ("version", &v.0)],
                )
            );
            println!(
                "{}",
                envr_core::i18n::tr_key(
                    "cli.use.global_current_note",
                    "这会更新 ENVR_RUNTIME_ROOT 下的全局 `current`（默认工具版本），不是仅作用于当前 shell 的临时环境。",
                    "This updates the global `current` symlink under ENVR_RUNTIME_ROOT (the default tool version), not a per-shell temporary override.",
                )
            );
        }
    })
}
