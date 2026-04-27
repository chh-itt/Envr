use iced::Task;

use super::{AppState, Message};
use crate::gui_ops;
use crate::view::downloads::JobState;

pub(crate) fn mark_unified_major_rows_dirty_for_kind(
    state: &mut AppState,
    kind: envr_domain::runtime::RuntimeKind,
) {
    state.env_center.unified_major_rows_by_kind.remove(&kind);
    state
        .env_center
        .unified_children_rows_by_kind_major
        .retain(|(k, _), _| *k != kind);
}

pub(crate) fn runtime_path_proxy_blocks_use(state: &AppState) -> bool {
    envr_config::runtime_path_proxy::path_proxy_blocks_managed_use(
        state.env_center.kind,
        &state.settings.cache.snapshot().runtime,
    )
}

pub(crate) fn looks_like_user_cancelled(err: &str) -> bool {
    let l = err.to_ascii_lowercase();
    l.contains("cancelled") || l.contains("canceled") || l.contains("download cancel")
}

pub(crate) fn runtime_install_task_label(
    kind: envr_domain::runtime::RuntimeKind,
    spec: &str,
    install_and_use: bool,
) -> String {
    let k = crate::view::env_center::kind_label_zh(kind);
    if install_and_use {
        format!("正在安装并切换为 {k} {spec}")
    } else {
        format!("正在安装 {k} {spec}")
    }
}

pub(crate) fn duplicate_runtime_install_blocked(
    state: &mut AppState,
    kind: envr_domain::runtime::RuntimeKind,
    spec: &str,
) -> bool {
    let spec_trimmed = spec.trim();
    if spec_trimmed.is_empty() {
        return false;
    }
    if state
        .env_center
        .installed
        .iter()
        .any(|v| v.0.trim() == spec_trimmed)
    {
        state.error = Some(format!(
            "{}: {}",
            envr_core::i18n::tr_key(
                "gui.runtime.install.duplicate_installed",
                "该版本已安装",
                "Version is already installed",
            ),
            spec_trimmed
        ));
        return true;
    }

    let inflight = state.downloads.jobs.iter().any(|j| {
        if !matches!(j.state, JobState::Queued | JobState::Running) {
            return false;
        }
        match j.payload.as_ref() {
            Some(crate::view::downloads::DownloadJobPayload::RuntimeInstall {
                kind: k,
                spec: s,
                ..
            }) => *k == kind && s.trim() == spec_trimmed,
            _ => false,
        }
    });
    if inflight {
        state.error = Some(format!(
            "{}: {}",
            envr_core::i18n::tr_key(
                "gui.runtime.install.duplicate_inflight",
                "该版本已在安装队列或进行中",
                "Version is already queued or installing",
            ),
            spec_trimmed
        ));
        return true;
    }
    false
}

pub(crate) fn rust_runtime_task_label(action: &str, detail: &str) -> String {
    match action {
        "channel" => format!("Rust 正在安装/切换工具链 {detail}"),
        "update" => "Rust 正在更新当前工具链".to_string(),
        "managed_uninstall" => "Rust 正在卸载托管 rustup".to_string(),
        "component_install" => format!("Rust 正在安装组件 {detail}"),
        "component_uninstall" => format!("Rust 正在卸载组件 {detail}"),
        "target_install" => format!("Rust 正在安装目标 {detail}"),
        "target_uninstall" => format!("Rust 正在卸载目标 {detail}"),
        _ => "Rust 正在执行任务".to_string(),
    }
}

pub(crate) fn runtime_page_enter_tasks(state: &mut AppState) -> Task<Message> {
    let layout = &state.settings.cache.snapshot().gui.runtime_layout;
    let vis = crate::view::runtime_layout::visible_kinds(layout);
    if !vis.is_empty() && !vis.contains(&state.env_center.kind) {
        state.env_center.kind = vis[0];
    }
    let kind = state.env_center.kind;
    if kind == envr_domain::runtime::RuntimeKind::Rust {
        state.env_center.busy = true;
        return Task::batch([
            gui_ops::rust_refresh(),
            gui_ops::rust_load_components(),
            gui_ops::rust_load_targets(),
        ]);
    }
    if envr_domain::runtime::runtime_descriptor(kind).supports_remote_latest {
        return Task::batch([
            gui_ops::refresh_runtimes(kind),
            envr_domain::runtime::unified_major_list_rollout_enabled(kind)
                .then_some(gui_ops::load_unified_major_rows_cached(kind))
                .unwrap_or_else(Task::none),
            envr_domain::runtime::unified_major_list_rollout_enabled(kind)
                .then_some(gui_ops::refresh_unified_major_rows(kind))
                .unwrap_or_else(Task::none),
        ]);
    }
    gui_ops::refresh_runtimes(kind)
}

pub(crate) fn sync_go_env_center_drafts_from_settings(state: &mut AppState) {
    let g = &state.settings.cache.snapshot().runtime.go;
    state.env_center.go_proxy_custom_draft = g
        .proxy_custom
        .clone()
        .or_else(|| g.goproxy.clone())
        .unwrap_or_default();
    state.env_center.go_private_patterns_draft = g.private_patterns.clone().unwrap_or_default();
}

pub(crate) fn sync_bun_env_center_drafts_from_settings(state: &mut AppState) {
    let b = &state.settings.cache.snapshot().runtime.bun;
    state.env_center.bun_global_bin_dir_draft = b.global_bin_dir.clone().unwrap_or_default();
}

pub(crate) fn recompute_env_center_derived(state: &mut AppState) {
    let _ = state;
}

pub(crate) fn env_center_jvm_check_task(kind: envr_domain::runtime::RuntimeKind) -> Task<Message> {
    let key = envr_domain::runtime::runtime_descriptor(kind).key;
    if envr_domain::jvm_hosted::is_jvm_hosted_runtime(key) {
        gui_ops::check_jvm_runtime_java_compat(kind)
    } else {
        Task::none()
    }
}

pub(crate) fn set_jvm_java_hint(
    state: &mut AppState,
    kind: envr_domain::runtime::RuntimeKind,
    res: Result<(), String>,
) {
    match res {
        Ok(()) => {
            state.env_center.jvm_java_hints.remove(&kind);
        }
        Err(msg) => {
            state.env_center.jvm_java_hints.insert(kind, msg);
        }
    }
}

pub(crate) fn direct_install_spec_ok(spec: &str) -> bool {
    let t = spec.trim();
    if t.is_empty() || t.len() > 80 {
        return false;
    }
    if t.chars().any(|c| c.is_control()) {
        return false;
    }
    true
}

pub(crate) fn bun_direct_spec_blocked_on_windows(
    kind: envr_domain::runtime::RuntimeKind,
    spec: &str,
) -> bool {
    if !cfg!(windows) || kind != envr_domain::runtime::RuntimeKind::Bun {
        return false;
    }
    let t = spec.trim().trim_start_matches('v');
    t.starts_with("0.")
}

pub(crate) fn deno_direct_spec_blocked(
    kind: envr_domain::runtime::RuntimeKind,
    spec: &str,
) -> bool {
    if kind != envr_domain::runtime::RuntimeKind::Deno {
        return false;
    }
    let t = spec.trim().trim_start_matches('v');
    t.starts_with("0.")
}

pub(crate) fn sanitize_runtime_filter_input(
    _kind: envr_domain::runtime::RuntimeKind,
    raw: &str,
) -> String {
    // Keep filter expressive across runtimes (`.`, `-`, spaces, prerelease tags),
    // while still guarding against control characters and pathological lengths.
    raw.chars()
        .filter(|c| !c.is_control())
        .take(64)
        .collect()
}
