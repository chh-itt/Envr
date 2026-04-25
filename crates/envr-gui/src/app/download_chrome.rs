use envr_config::settings::Settings;
use envr_ui::theme::shell as layout_shell;
use iced::{Size, Task};

use super::{AppState, Message, Route};
use crate::view::downloads::{DOWNLOAD_PANEL_SHELL_W, TITLE_DRAG_HOLD};

/// ~32ms: panel reveal, skeleton shimmer, throttled download progress (`tasks_gui.md` GUI-040–042, 041).
pub(crate) fn handle_motion_tick(state: &mut AppState) -> Task<Message> {
    if let Some(since) = state.downloads.title_drag_armed_since
        && !state.downloads.dragging
        && since.elapsed() >= TITLE_DRAG_HOLD
    {
        state.downloads.dragging = true;
        state.downloads.drag_from_cursor = None;
        state.downloads.drag_from_pos = Some((state.downloads.x, state.downloads.y));
        state.downloads.title_drag_armed_since = None;
    }
    let tokens = state.tokens();
    state.downloads.advance_reveal(tokens);
    if state.downloads.take_persist_after_hide() {
        let _ = persist_download_panel_settings(state);
    }
    state.downloads.maybe_progress_tick_on_motion_frame();
    let waiting_installed_list = state.env_center.busy && state.env_center.installed.is_empty();
    let waiting_remote = envr_domain::runtime::runtime_descriptor(state.env_center.kind)
        .supports_remote_latest
        && state.env_center.installed.is_empty()
        && state
            .env_center
            .unified_major_rows_by_kind
            .get(&state.env_center.kind)
            .is_none_or(|rows| rows.is_empty());
    if !state.reduce_motion
        && !state.disable_runtime_skeleton_shimmer
        && matches!(state.route(), Route::Runtime)
        && (waiting_installed_list || waiting_remote)
    {
        state.env_center.skeleton_phase = (state.env_center.skeleton_phase + 0.045) % 1.0;
    }
    Task::none()
}

pub(crate) fn persist_download_panel_settings(
    state: &mut AppState,
) -> Result<(), envr_error::EnvrError> {
    let paths = envr_platform::paths::current_platform_paths()?;
    let settings_path = envr_config::settings::settings_path_from_platform(&paths);
    let mut st = Settings::load_or_default_from(&settings_path)?;

    // `paths.runtime_root` is edited on the Settings page and lives in `state.settings.cache`.
    // If we only round-trip what `load_or_default_from` returns, a sparse `[paths]` on disk (or a
    // failed parse that fell back to defaults earlier in the session) can drop the in-memory
    // runtime root when we rewrite the whole file for the download panel.
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

    let panel = &state.downloads;
    st.gui.downloads_panel.visible = panel.visible;
    st.gui.downloads_panel.expanded = panel.expanded;
    let (cw, ch) = state.window_inner_px.unwrap_or((
        layout_shell::WINDOW_DEFAULT_W,
        layout_shell::WINDOW_DEFAULT_H,
    ));
    let pad = state.tokens().content_spacing();
    st.gui.downloads_panel.sync_frac_from_pixels(
        panel.x,
        panel.y,
        cw,
        ch,
        pad,
        DOWNLOAD_PANEL_SHELL_W,
    );
    st.save_to(&settings_path)?;
    state.settings.cache.set_cached(st.clone());
    let _ = state.settings.sync_from_cache();
    Ok(())
}

/// Main window resized — keep downloads panel in client bounds (`tasks_gui.md` GUI-061).
pub(crate) fn on_main_window_resized(state: &mut AppState, new: Size) {
    let pad = state.tokens().content_spacing();
    if let Some((old_w, old_h)) = state.window_inner_px {
        let inner_w_old = (old_w - 2.0 * pad).max(1.0);
        let inner_h_old = (old_h - 2.0 * pad).max(1.0);
        let avail_x_old = (inner_w_old - DOWNLOAD_PANEL_SHELL_W).max(1.0);
        let xf = state.downloads.x as f32 / avail_x_old;
        let yf = state.downloads.y as f32 / inner_h_old;
        let inner_w = (new.width - 2.0 * pad).max(1.0);
        let inner_h = (new.height - 2.0 * pad).max(1.0);
        let avail_x = (inner_w - DOWNLOAD_PANEL_SHELL_W).max(1.0);
        state.downloads.x = (xf.clamp(0.0, 1.0) * avail_x).round() as i32;
        state.downloads.y = (yf.clamp(0.0, 1.0) * inner_h).round() as i32;
    }
    state.window_inner_px = Some((new.width, new.height));
    clamp_download_panel_to_window(state);
}

pub(crate) fn clamp_download_panel_to_window(state: &mut AppState) {
    let Some((ww, wh)) = state.window_inner_px else {
        return;
    };
    let pad = state.tokens().content_spacing();
    let inner_w = (ww - 2.0 * pad).max(1.0);
    let inner_h = (wh - 2.0 * pad).max(1.0);
    let max_x = (inner_w - DOWNLOAD_PANEL_SHELL_W).max(0.0).round() as i32;
    let max_y = inner_h.max(0.0).round() as i32;
    state.downloads.x = state.downloads.x.clamp(0, max_x);
    state.downloads.y = state.downloads.y.clamp(0, max_y);
}
