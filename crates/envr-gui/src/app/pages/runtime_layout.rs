use super::super::*;

use iced::Task;

pub(crate) fn fix_env_center_kind_if_hidden(state: &mut AppState) -> Task<Message> {
    let layout = &state.settings.cache.snapshot().gui.runtime_layout;
    let vis = crate::view::runtime_layout::visible_kinds(layout);
    if vis.is_empty() {
        return Task::none();
    }
    if vis.contains(&state.env_center.kind) {
        Task::none()
    } else {
        pages::env_center::handle_env_center(state, EnvCenterMsg::PickKind(vis[0]))
    }
}

pub(crate) fn persist_runtime_layout_settings(
    state: &mut AppState,
) -> Result<(), envr_error::EnvrError> {
    let paths = envr_platform::paths::current_platform_paths()?;
    let settings_path = envr_config::settings::settings_path_from_platform(&paths);
    let mut st = Settings::load_or_default_from(&settings_path)?;
    let mem = state.settings.cache.snapshot();
    let disk_rr_empty = st
        .paths
        .runtime_root
        .as_deref()
        .is_none_or(|s| s.trim().is_empty());
    if disk_rr_empty && let Some(ref r) = mem.paths.runtime_root {
        let t = r.trim();
        if !t.is_empty() {
            st.paths.runtime_root = Some(t.to_string());
        }
    }
    st.gui.runtime_layout = mem.gui.runtime_layout.clone();
    st.save_to(&settings_path)?;
    state.settings.cache.set_cached(st.clone());
    let _ = state.settings.sync_from_cache();
    Ok(())
}

pub(crate) fn persist_runtime_layout_or_warn(state: &mut AppState) -> Task<Message> {
    match persist_runtime_layout_settings(state) {
        Ok(()) => fix_env_center_kind_if_hidden(state),
        Err(e) => {
            state.error = Some(format!(
                "{}: {e}",
                envr_core::i18n::tr_key("gui.app.save_failed", "保存失败", "Save failed",)
            ));
            Task::none()
        }
    }
}

pub(crate) fn handle_runtime_layout(state: &mut AppState, msg: RuntimeLayoutMsg) -> Task<Message> {
    use envr_domain::runtime::runtime_descriptor;

    match msg {
        RuntimeLayoutMsg::ToggleDashboardLayoutEditing => {
            state.dashboard.runtime_overview_layout_editing =
                !state.dashboard.runtime_overview_layout_editing;
            Task::none()
        }
        RuntimeLayoutMsg::ToggleDashboardHiddenCollapsed => {
            state.dashboard.runtime_overview_hidden_collapsed =
                !state.dashboard.runtime_overview_hidden_collapsed;
            Task::none()
        }
        RuntimeLayoutMsg::ResetToDefaults => {
            let mut st = state.settings.cache.snapshot().clone();
            crate::view::runtime_layout::reset_runtime_layout(&mut st.gui.runtime_layout);
            state.settings.cache.set_cached(st);
            let _ = state.settings.sync_from_cache();
            persist_runtime_layout_or_warn(state)
        }
        RuntimeLayoutMsg::MoveRuntime { kind, delta } => {
            let mut st = state.settings.cache.snapshot().clone();
            crate::view::runtime_layout::move_kind_delta(&mut st.gui.runtime_layout, kind, delta);
            state.settings.cache.set_cached(st);
            let _ = state.settings.sync_from_cache();
            persist_runtime_layout_or_warn(state)
        }
        RuntimeLayoutMsg::ToggleHidden(kind) => {
            let mut st = state.settings.cache.snapshot().clone();
            crate::view::runtime_layout::toggle_hidden_key(
                &mut st.gui.runtime_layout,
                runtime_descriptor(kind).key,
            );
            state.settings.cache.set_cached(st);
            let _ = state.settings.sync_from_cache();
            persist_runtime_layout_or_warn(state)
        }
        RuntimeLayoutMsg::OpenRuntime(kind) => {
            state.route = Route::Runtime;
            // Align with `Navigate(Runtime)`: always run `runtime_page_enter_tasks` when the target
            // kind is already selected — `PickKind` returns early and would skip the initial load
            // (empty `installed` + skeleton) after dashboard → card when default kind matches.
            if state.env_center.kind != kind {
                pages::env_center::handle_env_center(state, EnvCenterMsg::PickKind(kind))
            } else {
                runtime_page_enter_tasks(state)
            }
        }
    }
}
