use std::path::PathBuf;
use std::sync::OnceLock;

use envr_config::settings::Settings;
use envr_ui::theme::shell as layout_shell;
use iced::Task;

use super::{AppState, Message};
use crate::gui_ops;
use crate::view::downloads::DOWNLOAD_PANEL_SHELL_W;
use crate::view::settings::SettingsMsg;

pub(crate) static STARTUP_SETTINGS: OnceLock<Settings> = OnceLock::new();

pub(crate) fn load_gui_downloads_panel_settings_cached() -> (bool, bool, i32, i32) {
    let st = STARTUP_SETTINGS.get().cloned().unwrap_or_default();
    let p = &st.gui.downloads_panel;
    // Matches `envr_ui::theme` 8pt grid `md` (12px) used as shell `content_spacing`.
    let pad = 12.0_f32;
    let (x, y) = p.pixel_insets(
        layout_shell::WINDOW_DEFAULT_W,
        layout_shell::WINDOW_DEFAULT_H,
        pad,
        DOWNLOAD_PANEL_SHELL_W,
    );
    (p.visible, p.expanded, x, y)
}

pub(crate) fn load_startup_settings() -> Settings {
    let paths = match envr_platform::paths::current_platform_paths() {
        Ok(v) => v,
        Err(_) => return Settings::default(),
    };
    let settings_path = envr_config::settings::settings_path_from_platform(&paths);
    let st = Settings::load_or_default_from(&settings_path).unwrap_or_default();
    let _ = STARTUP_SETTINGS.set(st.clone());
    st
}

pub(crate) fn settings_path() -> PathBuf {
    let paths =
        envr_platform::paths::current_platform_paths().expect("platform paths for settings");
    envr_config::settings::settings_path_from_platform(&paths)
}

/// Write [`SettingsViewState::build_settings`] to `settings.toml` and finish with [`SettingsMsg::DiskSaved`].
pub(crate) fn persist_settings_draft_task(state: &AppState) -> Task<Message> {
    let path = settings_path();
    let next = state.settings.build_settings().map_err(|e| e.to_string());
    Task::perform(
        async move {
            let next = next?;
            next.save_to(&path).map_err(|e| e.to_string())?;
            Ok(next)
        },
        |res| Message::Settings(SettingsMsg::DiskSaved(res)),
    )
}

/// Save a full [`Settings`] snapshot (e.g. runtime.node edits from the env center) and mirror [`SettingsMsg::DiskSaved`].
pub(crate) fn persist_settings_clone_task(settings: Settings) -> Task<Message> {
    let path = settings_path();
    Task::perform(
        async move {
            settings.validate().map_err(|e| e.to_string())?;
            settings.save_to(&path).map_err(|e| e.to_string())?;
            Ok(settings)
        },
        |res| Message::Settings(SettingsMsg::DiskSaved(res)),
    )
}

pub(crate) fn persist_runtime_settings_update<F>(state: &mut AppState, update: F) -> Task<Message>
where
    F: FnOnce(&mut Settings),
{
    let mut st = state.settings.cache.snapshot().clone();
    update(&mut st);
    if let Err(e) = st.validate() {
        state.error = Some(e.to_string());
        return Task::none();
    }
    persist_settings_clone_task(st)
}

pub(crate) fn persist_path_proxy_toggle<F>(
    state: &mut AppState,
    kind: envr_domain::runtime::RuntimeKind,
    on: bool,
    update: F,
) -> Task<Message>
where
    F: FnOnce(&mut Settings, bool),
{
    let mut st = state.settings.cache.snapshot().clone();
    update(&mut st, on);
    if let Err(e) = st.validate() {
        state.error = Some(e.to_string());
        return Task::none();
    }
    if on {
        Task::batch([
            persist_settings_clone_task(st),
            gui_ops::sync_shims_for_kind(kind),
        ])
    } else {
        persist_settings_clone_task(st)
    }
}
