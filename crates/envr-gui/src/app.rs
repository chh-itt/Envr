//! Main-window shell: left navigation, routed content, global error banner.

use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::time::Duration;

use envr_config::settings::{FontMode, Settings, ThemeMode};
use envr_download::task::CancelToken;
use envr_ui::font;
use envr_ui::theme::Srgb;
use envr_ui::theme::{
    ThemeTokens, UiFlavor, default_flavor_for_target, scheme_for_mode, shell as layout_shell,
    tokens_for_appearance,
};
use iced::font::Family;
use iced::window;
use iced::{Element, Size, Subscription, Task, application};
use std::path::PathBuf;
use std::sync::OnceLock;

use crate::download_runner;
use crate::gui_ops;
use crate::theme as gui_theme;
use crate::view::dashboard::{DashboardMsg, DashboardState};
use crate::view::downloads::{
    DOWNLOAD_PANEL_SHELL_W, DownloadJob, DownloadMsg, DownloadPanelState, JobState, TITLE_DRAG_HOLD,
};
use crate::view::env_center::{EnvCenterMsg, EnvCenterState};
use crate::view::runtime_settings::{RuntimeSettingsMsg, RuntimeSettingsState};
use crate::view::settings::{SettingsMsg, SettingsViewState};
use crate::view::shell;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Route {
    #[default]
    Dashboard,
    Runtime,
    Settings,
    About,
}

impl Route {
    pub(crate) const ALL: [Self; 4] = [
        Route::Dashboard,
        Route::Runtime,
        Route::Settings,
        Route::About,
    ];

    pub(crate) fn label(self) -> String {
        match self {
            Route::Dashboard => {
                envr_core::i18n::tr_key("gui.route.dashboard", "仪表盘", "Dashboard")
            }
            Route::Runtime => envr_core::i18n::tr_key("gui.route.runtime", "运行时", "Runtimes"),
            Route::Settings => envr_core::i18n::tr_key("gui.route.settings", "设置", "Settings"),
            Route::About => envr_core::i18n::tr_key("gui.route.about", "关于", "About"),
        }
    }
}

pub struct AppState {
    route: Route,
    error: Option<String>,
    /// Last main window **inner** size (physical px) for panel geometry (`tasks_gui.md` GUI-061).
    window_inner_px: Option<(f32, f32)>,
    /// Active skin; user can override the OS default on the Settings page.
    flavor: UiFlavor,
    /// System / env reduced motion (`tasks_gui.md` GUI-052).
    reduce_motion: bool,
    /// Text ramp scale from `ENVR_UI_SCALE` (`tasks_gui.md` GUI-051).
    ui_text_scale: f32,
    /// GUI-101 experiment: disable runtime skeleton shimmer motion.
    disable_runtime_skeleton_shimmer: bool,
    pub env_center: EnvCenterState,
    pub downloads: DownloadPanelState,
    pub settings: SettingsViewState,
    pub dashboard: DashboardState,
    pub runtime_settings: RuntimeSettingsState,
}

impl Default for AppState {
    fn default() -> Self {
        let gui_defaults = load_gui_downloads_panel_settings_cached();
        Self {
            route: Route::default(),
            error: None,
            window_inner_px: None,
            flavor: default_flavor_for_target(),
            reduce_motion: envr_platform::a11y::prefers_reduced_motion(),
            ui_text_scale: ui_text_scale_from_env(),
            disable_runtime_skeleton_shimmer: env_flag("ENVR_GUI_DISABLE_SKELETON_SHIMMER"),
            env_center: EnvCenterState::default(),
            downloads: {
                let vis = gui_defaults.0;
                DownloadPanelState {
                    visible: vis,
                    reveal: if vis { 1.0 } else { 0.0 },
                    expanded: gui_defaults.1,
                    x: gui_defaults.2,
                    y: gui_defaults.3,
                    ..DownloadPanelState::default()
                }
            },
            settings: SettingsViewState::new(),
            dashboard: DashboardState::default(),
            runtime_settings: RuntimeSettingsState::default(),
        }
    }
}

fn load_gui_downloads_panel_settings_cached() -> (bool, bool, i32, i32) {
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

fn ui_text_scale_from_env() -> f32 {
    std::env::var("ENVR_UI_SCALE")
        .ok()
        .and_then(|s| s.trim().parse::<f32>().ok())
        .unwrap_or(1.0)
        .clamp(0.85, 1.35)
}

fn env_flag(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .map(|s| {
            let t = s.trim().to_ascii_lowercase();
            t == "1" || t == "true" || t == "yes" || t == "on"
        })
        .unwrap_or(false)
}

fn accent_from_settings(st: &Settings) -> Option<Srgb> {
    st.appearance.accent_color.as_deref().and_then(|s| {
        let t = s.trim();
        if t.is_empty() {
            None
        } else {
            Srgb::from_hex(t).ok()
        }
    })
}

impl AppState {
    pub(crate) fn tokens(&self) -> ThemeTokens {
        let scheme = scheme_for_mode(self.settings.draft.appearance.theme_mode);
        let accent = accent_from_settings(&self.settings.draft);
        let mut t = tokens_for_appearance(self.flavor, scheme, accent);
        t.content_text_scale = self.ui_text_scale;
        if self.reduce_motion {
            t.motion.standard_ms = 0;
            t.motion.emphasized_ms = 0;
        }
        t
    }

    pub fn route(&self) -> Route {
        self.route
    }

    pub(crate) fn error_message(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub(crate) fn flavor(&self) -> UiFlavor {
        self.flavor
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    /// Re-resolve `FollowSystem` scheme when OS appearance changes (cheap tick).
    ThemePollTick,
    /// ~32ms: panel reveal, skeleton shimmer, throttled download progress (`tasks_gui.md` GUI-040–042, 041).
    MotionTick,
    /// Re-check OS / env accessibility hints (`tasks_gui.md` GUI-052).
    A11yPollTick,
    /// Main window resized — keep downloads panel in client bounds (`tasks_gui.md` GUI-061).
    WindowResized(Size),
    Navigate(Route),
    DismissError,
    ReportError(String),
    SetFlavor(UiFlavor),
    EnvCenter(EnvCenterMsg),
    Dashboard(DashboardMsg),
    Download(DownloadMsg),
    Settings(SettingsMsg),
    RuntimeSettings(RuntimeSettingsMsg),
}

pub fn run() -> iced::Result {
    // Ensure wgpu uses the GL backend when available.
    // This helps keep the baseline memory stable on some systems.
    #[cfg(target_os = "windows")]
    {
        if std::env::var_os("WGPU_BACKEND").is_none() {
            // Safe to do early during startup, before wgpu/iced are initialized.
            unsafe { std::env::set_var("WGPU_BACKEND", "gl") };
        }
    }

    let startup = load_startup_settings();
    envr_core::i18n::init_from_settings(&startup);
    application(
        || {
            let mut state = AppState::default();
            state.dashboard.busy = true;
            state.dashboard.last_error = None;
            (state, gui_ops::refresh_dashboard())
        },
        update,
        view,
    )
    .title("Envr")
    .default_font(configured_default_font(&startup))
    .theme(|state: &AppState| gui_theme::iced_theme(state.tokens()))
        .subscription(|state| {
            let runtime_skeleton = matches!(state.route(), Route::Runtime)
                && state.env_center.installed.is_empty()
                && (state.env_center.busy
                    || (state.env_center.kind == envr_domain::runtime::RuntimeKind::Node
                        && state.env_center.node_remote_refreshing
                        && state.env_center.node_remote_latest.is_empty()));
            let need_motion = state.downloads.needs_motion_tick()
                || state.downloads.title_drag_armed_since.is_some()
                || runtime_skeleton;
            let maybe_motion = need_motion
                .then(|| iced::time::every(Duration::from_millis(32)))
                .map(|s| s.map(|_| Message::MotionTick));

            let progress_only = state.downloads.needs_tick() && !need_motion;
            let maybe_tick = progress_only
                .then(|| iced::time::every(Duration::from_millis(400)))
                .map(|s| s.map(|_| Message::Download(DownloadMsg::Tick)));

            let need_pointer_events =
                state.downloads.dragging || state.downloads.title_drag_armed_since.is_some();
            let maybe_events = need_pointer_events
                .then(|| iced::event::listen().map(|e| Message::Download(DownloadMsg::Event(e))));

            let theme_poll = (state.settings.draft.appearance.theme_mode
                == ThemeMode::FollowSystem)
                .then(|| iced::time::every(Duration::from_secs(1)))
                .map(|s| s.map(|_| Message::ThemePollTick));

            let mut subs = Vec::new();
            if let Some(t) = maybe_motion {
                subs.push(t);
            }
            if let Some(t) = maybe_tick {
                subs.push(t);
            }
            if let Some(e) = maybe_events {
                subs.push(e);
            }
            if let Some(t) = theme_poll {
                subs.push(t);
            }
            subs.push(iced::time::every(Duration::from_secs(3)).map(|_| Message::A11yPollTick));
            subs.push(window::resize_events().map(|(_id, s)| Message::WindowResized(s)));
            Subscription::batch(subs)
        })
        .window(iced::window::Settings {
            size: Size::new(
                layout_shell::WINDOW_DEFAULT_W,
                layout_shell::WINDOW_DEFAULT_H,
            ),
            min_size: Some(Size::new(
                layout_shell::WINDOW_MIN_W,
                layout_shell::WINDOW_MIN_H,
            )),
            position: iced::window::Position::Centered,
            ..iced::window::Settings::default()
        })
        .run()
}

static STARTUP_SETTINGS: OnceLock<Settings> = OnceLock::new();

fn load_startup_settings() -> Settings {
    let paths = match envr_platform::paths::current_platform_paths() {
        Ok(v) => v,
        Err(_) => return Settings::default(),
    };
    let settings_path = envr_config::settings::settings_path_from_platform(&paths);
    let st = Settings::load_or_default_from(&settings_path).unwrap_or_default();
    let _ = STARTUP_SETTINGS.set(st.clone());
    st
}

fn configured_default_font(st: &Settings) -> iced::Font {
    match st.appearance.font.mode {
        FontMode::Auto => iced::Font::with_name(font::preferred_system_sans_family()),
        FontMode::Custom => {
            let fam = st
                .appearance
                .font
                .family
                .as_deref()
                .unwrap_or(font::preferred_system_sans_family())
                .to_string();
            let leaked: &'static str = Box::leak(fam.into_boxed_str());
            iced::Font {
                family: Family::Name(leaked),
                ..iced::Font::default()
            }
        }
    }
}

fn update(state: &mut AppState, message: Message) -> Task<Message> {
    match message {
        Message::ThemePollTick => Task::none(),
        Message::A11yPollTick => {
            state.reduce_motion = envr_platform::a11y::prefers_reduced_motion();
            state.ui_text_scale = ui_text_scale_from_env();
            Task::none()
        }
        Message::WindowResized(size) => {
            on_main_window_resized(state, size);
            Task::none()
        }
        Message::MotionTick => handle_motion_tick(state),
        Message::Navigate(route) => {
            tracing::debug!(?route, "navigate");
            state.route = route;
            if route == Route::Runtime {
                return runtime_page_enter_tasks(state);
            }
            if route == Route::Dashboard {
                state.dashboard.busy = true;
                state.dashboard.last_error = None;
                return gui_ops::refresh_dashboard();
            }
            if route == Route::Settings {
                state.settings.last_message = Some(envr_core::i18n::tr_key(
                    "gui.app.loading",
                    "正在加载…",
                    "Loading…",
                ));
                let path = settings_path();
                return Task::perform(
                    async move {
                        envr_config::settings::Settings::load_or_default_from(&path)
                            .map_err(|e| e.to_string())
                    },
                    |res| Message::Settings(SettingsMsg::DiskLoaded(res)),
                );
            }
            Task::none()
        }
        Message::DismissError => {
            state.error = None;
            Task::none()
        }
        Message::ReportError(msg) => {
            state.error = Some(msg);
            Task::none()
        }
        Message::SetFlavor(flavor) => {
            tracing::debug!(%flavor, "set flavor");
            state.flavor = flavor;
            Task::none()
        }
        Message::EnvCenter(msg) => handle_env_center(state, msg),
        Message::Dashboard(msg) => handle_dashboard(state, msg),
        Message::Download(msg) => handle_download(state, msg),
        Message::Settings(msg) => handle_settings(state, msg),
        Message::RuntimeSettings(msg) => handle_runtime_settings(state, msg),
    }
}

fn handle_runtime_settings(state: &mut AppState, msg: RuntimeSettingsMsg) -> Task<Message> {
    match msg {
        RuntimeSettingsMsg::ToggleExpand => {
            state.runtime_settings.expanded = !state.runtime_settings.expanded;
            Task::none()
        }
        RuntimeSettingsMsg::ReloadDisk => {
            state.runtime_settings.last_message = Some(envr_core::i18n::tr_key(
                "gui.app.loading",
                "正在加载…",
                "Loading…",
            ));
            let path = settings_path();
            Task::perform(
                async move {
                    envr_config::settings::Settings::load_or_default_from(&path)
                        .map_err(|e| e.to_string())
                },
                |res| Message::RuntimeSettings(RuntimeSettingsMsg::DiskLoaded(res)),
            )
        }
        RuntimeSettingsMsg::GoGoproxyEdit(s) => {
            state.runtime_settings.go_goproxy_draft = s;
            Task::none()
        }
        RuntimeSettingsMsg::BunGlobalBinDirEdit(s) => {
            state.runtime_settings.bun_global_bin_dir_draft = s;
            Task::none()
        }
        RuntimeSettingsMsg::Save => {
            state.runtime_settings.last_message = Some(envr_core::i18n::tr_key(
                "gui.app.saving",
                "正在保存…",
                "Saving…",
            ));
            let path = settings_path();
            let next = state
                .runtime_settings
                .build_settings()
                .map_err(|e| e.to_string());
            Task::perform(
                async move {
                    let next = next?;
                    next.save_to(&path).map_err(|e| e.to_string())?;
                    Ok(next)
                },
                |res| Message::RuntimeSettings(RuntimeSettingsMsg::DiskSaved(res)),
            )
        }
        RuntimeSettingsMsg::DiskLoaded(res) => {
            match res {
                Ok(st) => {
                    state.runtime_settings.cache.set_cached(st);
                    if let Err(e) = state.runtime_settings.sync_from_cache() {
                        state.runtime_settings.last_message = Some(format!(
                            "{}: {e}",
                            envr_core::i18n::tr_key(
                                "gui.app.sync_failed",
                                "同步失败",
                                "Sync failed"
                            )
                        ));
                    } else {
                        state.runtime_settings.last_message = None;
                    }
                }
                Err(e) => {
                    state.runtime_settings.last_message = Some(format!(
                        "{}: {e}",
                        envr_core::i18n::tr_key(
                            "gui.app.reload_failed",
                            "重新加载失败",
                            "Reload failed"
                        )
                    ));
                }
            }
            Task::none()
        }
        RuntimeSettingsMsg::DiskSaved(res) => {
            match res {
                Ok(st) => {
                    state.runtime_settings.cache.set_cached(st);
                    if let Err(e) = state.runtime_settings.sync_from_cache() {
                        state.runtime_settings.last_message = Some(format!(
                            "{}: {e}",
                            envr_core::i18n::tr_key(
                                "gui.app.sync_failed",
                                "同步失败",
                                "Sync failed"
                            )
                        ));
                    } else {
                        state.runtime_settings.last_message = Some(envr_core::i18n::tr_key(
                            "gui.app.saved_short",
                            "已保存。",
                            "Saved.",
                        ));
                    }
                }
                Err(e) => {
                    state.runtime_settings.last_message = Some(format!(
                        "{}: {e}",
                        envr_core::i18n::tr_key("gui.app.save_failed", "保存失败", "Save failed")
                    ));
                }
            }
            Task::none()
        }
    }
}

fn handle_dashboard(state: &mut AppState, msg: DashboardMsg) -> Task<Message> {
    match msg {
        DashboardMsg::Refresh => {
            state.dashboard.busy = true;
            state.dashboard.last_error = None;
            gui_ops::refresh_dashboard()
        }
        DashboardMsg::DataLoaded(res) => {
            state.dashboard.busy = false;
            match res {
                Ok(d) => {
                    state.dashboard.data = Some(d);
                }
                Err(e) => {
                    state.dashboard.last_error = Some(e);
                }
            }
            Task::none()
        }
    }
}

async fn browse_runtime_root_folder(start: Option<std::path::PathBuf>) -> Option<std::path::PathBuf> {
    tokio::task::spawn_blocking(move || {
        let mut dlg = rfd::FileDialog::new();
        if let Some(p) = start {
            if p.is_dir() {
                dlg = dlg.set_directory(p);
            }
        }
        dlg.pick_folder()
    })
    .await
    .ok()
    .flatten()
}

fn handle_settings(state: &mut AppState, msg: SettingsMsg) -> Task<Message> {
    match msg {
        SettingsMsg::BrowseRuntimeRoot => {
            let start = {
                let t = state.settings.runtime_root_draft.trim();
                if t.is_empty() {
                    None
                } else {
                    let p = std::path::PathBuf::from(t);
                    p.is_dir().then_some(p)
                }
            };
            Task::perform(
                browse_runtime_root_folder(start),
                |r| Message::Settings(SettingsMsg::RuntimeRootBrowseResult(r)),
            )
        }
        SettingsMsg::RuntimeRootBrowseResult(pb) => {
            if let Some(pb) = pb {
                state.settings.runtime_root_draft = pb.to_string_lossy().to_string();
                let rr = state.settings.runtime_root_draft.trim();
                state.settings.draft.paths.runtime_root = if rr.is_empty() {
                    None
                } else {
                    Some(rr.to_string())
                };
                state.settings.last_message = Some(envr_core::i18n::tr_key(
                    "gui.app.saving",
                    "正在保存…",
                    "Saving…",
                ));
                return persist_settings_draft_task(state);
            }
            Task::none()
        }
        SettingsMsg::ClearRuntimeRoot => {
            state.settings.runtime_root_draft.clear();
            state.settings.draft.paths.runtime_root = None;
            state.settings.last_message = Some(envr_core::i18n::tr_key(
                "gui.app.saving",
                "正在保存…",
                "Saving…",
            ));
            persist_settings_draft_task(state)
        }
        SettingsMsg::ManualIdEdit(s) => {
            state.settings.manual_id_draft = s;
            Task::none()
        }
        SettingsMsg::MaxConcEdit(s) => {
            state.settings.max_conc_text = s;
            Task::none()
        }
        SettingsMsg::RetryEdit(s) => {
            state.settings.retry_text = s;
            Task::none()
        }
        SettingsMsg::SetMirrorMode(m) => {
            state.settings.draft.mirror.mode = m;
            Task::none()
        }
        SettingsMsg::SetCleanup(v) => {
            state
                .settings
                .draft
                .behavior
                .cleanup_downloads_after_install = v;
            Task::none()
        }
        SettingsMsg::SetFontMode(m) => {
            state.settings.draft.appearance.font.mode = m;
            Task::none()
        }
        SettingsMsg::FontFamilyEdit(s) => {
            state.settings.font_family_draft = s;
            Task::none()
        }
        SettingsMsg::PickFontFamily(s) => {
            state.settings.font_family_draft = s;
            Task::none()
        }
        SettingsMsg::SetThemeMode(m) => {
            state.settings.draft.appearance.theme_mode = m;
            Task::none()
        }
        SettingsMsg::AccentColorEdit(s) => {
            state.settings.accent_color_draft = s;
            let t = state.settings.accent_color_draft.trim();
            state.settings.draft.appearance.accent_color = if t.is_empty() {
                None
            } else {
                Srgb::from_hex(t).ok().map(|_| t.to_string())
            };
            Task::none()
        }
        SettingsMsg::SetLocaleMode(m) => {
            state.settings.locale_mode_draft = m;
            // Apply immediately so all views re-render with new language.
            let mut st = state.settings.draft.clone();
            st.i18n.locale = m;
            envr_core::i18n::init_from_settings(&st);
            Task::none()
        }
        SettingsMsg::Save => {
            state.settings.last_message = Some(envr_core::i18n::tr_key(
                "gui.app.saving",
                "正在保存…",
                "Saving…",
            ));
            persist_settings_draft_task(state)
        }
        SettingsMsg::ReloadDisk => {
            state.settings.last_message = Some(envr_core::i18n::tr_key(
                "gui.app.loading",
                "正在加载…",
                "Loading…",
            ));
            let path = settings_path();
            Task::perform(
                async move {
                    envr_config::settings::Settings::load_or_default_from(&path)
                        .map_err(|e| e.to_string())
                },
                |res| Message::Settings(SettingsMsg::DiskLoaded(res)),
            )
        }
        SettingsMsg::DiskLoaded(res) => {
            match res {
                Ok(st) => {
                    // If the user picked a folder but never got a successful save, disk can still be
                    // empty while `runtime_root_draft` holds the path — reloading would wipe it.
                    let unsaved_rr = state.settings.runtime_root_draft.trim().to_string();
                    let had_unsaved = !unsaved_rr.is_empty();
                    let disk_rr_empty = st
                        .paths
                        .runtime_root
                        .as_deref()
                        .map_or(true, |r| r.trim().is_empty());

                    state.settings.cache.set_cached(st);
                    if let Err(e) = state.settings.sync_from_cache() {
                        state.settings.last_message = Some(format!(
                            "{}: {e}",
                            envr_core::i18n::tr_key(
                                "gui.app.sync_failed",
                                "同步失败",
                                "Sync failed"
                            )
                        ));
                    } else if had_unsaved && disk_rr_empty {
                        state.settings.runtime_root_draft = unsaved_rr.clone();
                        state.settings.draft.paths.runtime_root = Some(unsaved_rr.clone());
                        let mut merged = state.settings.cache.snapshot().clone();
                        merged.paths.runtime_root = Some(unsaved_rr);
                        state.settings.cache.set_cached(merged.clone());
                        state.runtime_settings.cache.set_cached(merged);
                        let _ = state.settings.sync_from_cache();
                        let _ = state.runtime_settings.sync_from_cache();
                        state.settings.last_message = None;
                    } else {
                        state.settings.last_message = Some(envr_core::i18n::tr_key(
                            "gui.app.reloaded_from_disk",
                            "已从磁盘重新加载。",
                            "Reloaded from disk.",
                        ));
                    }
                }
                Err(e) => {
                    state.settings.last_message = Some(format!(
                        "{}: {e}",
                        envr_core::i18n::tr_key(
                            "gui.app.reload_failed",
                            "重新加载失败",
                            "Reload failed"
                        )
                    ));
                }
            }
            Task::none()
        }
        SettingsMsg::DiskSaved(res) => {
            match res {
                Ok(st) => {
                    state.settings.cache.set_cached(st.clone());
                    state.runtime_settings.cache.set_cached(st.clone());
                    if let Err(e) = state.settings.sync_from_cache() {
                        state.settings.last_message = Some(format!(
                            "{}: {e}",
                            envr_core::i18n::tr_key(
                                "gui.app.sync_failed",
                                "同步失败",
                                "Sync failed"
                            )
                        ));
                    } else {
                        let _ = state.runtime_settings.sync_from_cache();
                        state.settings.last_message = Some(envr_core::i18n::tr_key(
                            "gui.app.saved_settings_toml",
                            "已保存到 settings.toml。",
                            "Saved.",
                        ));
                    }
                    let refresh_node_remote = matches!(state.route(), Route::Runtime)
                        && state.env_center.kind == envr_domain::runtime::RuntimeKind::Node;
                    if refresh_node_remote {
                        state.env_center.node_remote_refreshing = true;
                        return Task::batch([
                            gui_ops::load_remote_latest_disk_snapshot(
                                envr_domain::runtime::RuntimeKind::Node,
                            ),
                            gui_ops::refresh_remote_latest_per_major(
                                envr_domain::runtime::RuntimeKind::Node,
                            ),
                        ]);
                    }
                }
                Err(e) => {
                    state.settings.last_message = Some(format!(
                        "{}: {e}",
                        envr_core::i18n::tr_key("gui.app.save_failed", "保存失败", "Save failed")
                    ));
                }
            }
            Task::none()
        }
    }
}

fn settings_path() -> PathBuf {
    let paths =
        envr_platform::paths::current_platform_paths().expect("platform paths for settings");
    envr_config::settings::settings_path_from_platform(&paths)
}

/// Write [`SettingsViewState::build_settings`] to `settings.toml` and finish with [`SettingsMsg::DiskSaved`].
fn persist_settings_draft_task(state: &AppState) -> Task<Message> {
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

fn apply_npm_registry_cli(url: &str) -> Result<(), String> {
    #[cfg(windows)]
    let npm = "npm.cmd";
    #[cfg(not(windows))]
    let npm = "npm";
    let status = std::process::Command::new(npm)
        .args(["config", "set", "registry", url])
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("npm config set registry failed: {status}"))
    }
}

fn pip_user_config_paths() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(appdata) = std::env::var("APPDATA")
        && !appdata.trim().is_empty()
    {
        out.push(PathBuf::from(appdata).join("pip").join("pip.ini"));
    }
    if let Ok(home) = std::env::var("USERPROFILE")
        && !home.trim().is_empty()
    {
        out.push(PathBuf::from(home).join("pip").join("pip.ini"));
    }
    out
}

fn write_pip_user_ini(
    path: &std::path::Path,
    index_url: &str,
    trusted_host: &str,
    extra_index_url: Option<&str>,
) -> Result<(), String> {
    fn append_missing_global_keys(
        buf: &mut Vec<String>,
        index_url: &str,
        trusted_host: &str,
        extra_index_url: Option<&str>,
        want_extra: bool,
        wrote_index: &mut bool,
        wrote_host: &mut bool,
        wrote_timeout: &mut bool,
        wrote_extra: &mut bool,
    ) {
        if !*wrote_index {
            buf.push(format!("index-url = {index_url}"));
            *wrote_index = true;
        }
        if !*wrote_host {
            buf.push(format!("trusted-host = {trusted_host}"));
            *wrote_host = true;
        }
        if !*wrote_timeout {
            buf.push("timeout = 120".to_string());
            *wrote_timeout = true;
        }
        if want_extra && !*wrote_extra {
            if let Some(extra) = extra_index_url
                && !extra.trim().is_empty()
            {
                buf.push(format!("extra-index-url = {extra}"));
            }
            *wrote_extra = true;
        }
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let existing = std::fs::read_to_string(path).unwrap_or_default();
    let mut out: Vec<String> = Vec::new();
    let mut in_global = false;
    let mut skipping_duplicate_global = false;
    let mut saw_global = false;
    let mut wrote_index = false;
    let mut wrote_host = false;
    let mut wrote_timeout = false;
    let mut wrote_extra = false;
    let want_extra = extra_index_url.is_some_and(|s| !s.trim().is_empty());

    for raw in existing.lines() {
        let line = raw.to_string();
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            if in_global {
                append_missing_global_keys(
                    &mut out,
                    index_url,
                    trusted_host,
                    extra_index_url,
                    want_extra,
                    &mut wrote_index,
                    &mut wrote_host,
                    &mut wrote_timeout,
                    &mut wrote_extra,
                );
            }
            let is_global = trimmed.eq_ignore_ascii_case("[global]");
            if is_global && saw_global {
                // Self-heal invalid INI: keep the first [global], drop duplicate sections.
                in_global = false;
                skipping_duplicate_global = true;
                continue;
            }
            skipping_duplicate_global = false;
            in_global = is_global;
            if is_global {
                saw_global = true;
            }
            out.push(line);
            continue;
        }
        if skipping_duplicate_global {
            continue;
        }
        if in_global && !trimmed.starts_with('#') && !trimmed.starts_with(';') {
            if let Some((k, _v)) = line.split_once('=') {
                let key = k.trim().to_ascii_lowercase();
                if key == "index-url" {
                    if !wrote_index {
                        out.push(format!("index-url = {index_url}"));
                        wrote_index = true;
                    }
                    continue;
                }
                if key == "trusted-host" {
                    if !wrote_host {
                        out.push(format!("trusted-host = {trusted_host}"));
                        wrote_host = true;
                    }
                    continue;
                }
                if key == "timeout" {
                    if !wrote_timeout {
                        out.push("timeout = 120".to_string());
                        wrote_timeout = true;
                    }
                    continue;
                }
                if key == "extra-index-url" {
                    if !wrote_extra {
                        if let Some(extra) = extra_index_url
                            && !extra.trim().is_empty()
                        {
                            out.push(format!("extra-index-url = {extra}"));
                        }
                        wrote_extra = true;
                    }
                    continue;
                }
            }
        }
        out.push(line);
    }
    if in_global {
        append_missing_global_keys(
            &mut out,
            index_url,
            trusted_host,
            extra_index_url,
            want_extra,
            &mut wrote_index,
            &mut wrote_host,
            &mut wrote_timeout,
            &mut wrote_extra,
        );
    }

    if !saw_global {
        if !out.is_empty() && !out.last().is_some_and(|s| s.trim().is_empty()) {
            out.push(String::new());
        }
        out.push("[global]".to_string());
        out.push(format!("index-url = {index_url}"));
        out.push(format!("trusted-host = {trusted_host}"));
        out.push("timeout = 120".to_string());
        if let Some(extra) = extra_index_url
            && !extra.trim().is_empty()
        {
            out.push(format!("extra-index-url = {extra}"));
        }
    }

    let body = format!("{}\n", out.join("\n"));
    std::fs::write(path, body).map_err(|e| e.to_string())
}

fn apply_pip_registry_config(settings: &Settings) -> Result<(), String> {
    if matches!(
        settings.runtime.python.pip_registry_mode,
        envr_config::settings::PipRegistryMode::Restore
    ) {
        return Ok(());
    }
    let index_urls = envr_config::settings::pip_registry_urls_for_bootstrap(settings);
    let Some(index_url) = index_urls.first().copied() else {
        return Ok(());
    };
    let extra = if index_urls.len() > 1 {
        Some(index_urls.iter().skip(1).copied().collect::<Vec<_>>().join(" "))
    } else {
        None
    };
    let host = reqwest::Url::parse(index_url)
        .ok()
        .and_then(|u| u.host_str().map(|s| s.to_string()))
        .ok_or_else(|| format!("invalid pip index url: {index_url}"))?;

    let candidates = pip_user_config_paths();
    let existing: Vec<PathBuf> = candidates.iter().filter(|p| p.exists()).cloned().collect();
    let targets: Vec<PathBuf> = if existing.is_empty() {
        candidates.into_iter().take(1).collect()
    } else {
        existing
    };
    for p in targets {
        write_pip_user_ini(&p, index_url, &host, extra.as_deref())?;
    }
    Ok(())
}

/// Save a full [`Settings`] snapshot (e.g. runtime.node edits from the env center) and mirror [`SettingsMsg::DiskSaved`].
fn persist_settings_clone_task(settings: Settings) -> Task<Message> {
    let path = settings_path();
    Task::perform(
        async move {
            settings.validate().map_err(|e| e.to_string())?;
            settings.save_to(&path).map_err(|e| e.to_string())?;
            if let Some(url) = envr_config::settings::npm_registry_url_to_apply(&settings) {
                if let Err(e) = apply_npm_registry_cli(url) {
                    tracing::warn!(%e, "npm config set registry skipped after settings save");
                }
            }
            if let Err(e) = apply_pip_registry_config(&settings) {
                tracing::warn!(%e, "pip user config update skipped after settings save");
            }
            Ok(settings)
        },
        |res| Message::Settings(SettingsMsg::DiskSaved(res)),
    )
}

fn runtime_path_proxy_blocks_use(state: &AppState) -> bool {
    match state.env_center.kind {
        envr_domain::runtime::RuntimeKind::Node => !state
            .settings
            .cache
            .snapshot()
            .runtime
            .node
            .path_proxy_enabled,
        envr_domain::runtime::RuntimeKind::Python => !state
            .settings
            .cache
            .snapshot()
            .runtime
            .python
            .path_proxy_enabled,
        _ => false,
    }
}

fn handle_motion_tick(state: &mut AppState) -> Task<Message> {
    if let Some(since) = state.downloads.title_drag_armed_since {
        if !state.downloads.dragging && since.elapsed() >= TITLE_DRAG_HOLD {
            state.downloads.dragging = true;
            state.downloads.drag_from_cursor = None;
            state.downloads.drag_from_pos = Some((state.downloads.x, state.downloads.y));
            state.downloads.title_drag_armed_since = None;
        }
    }
    let tokens = state.tokens();
    state.downloads.advance_reveal(tokens);
    if state.downloads.take_persist_after_hide() {
        let _ = persist_download_panel_settings(state);
    }
    state.downloads.maybe_progress_tick_on_motion_frame();
    let waiting_installed_list =
        state.env_center.busy && state.env_center.installed.is_empty();
    let waiting_node_remote = state.env_center.kind == envr_domain::runtime::RuntimeKind::Node
        && state.env_center.node_remote_refreshing
        && state.env_center.node_remote_latest.is_empty()
        && state.env_center.installed.is_empty();
    let waiting_python_remote = state.env_center.kind == envr_domain::runtime::RuntimeKind::Python
        && state.env_center.python_remote_refreshing
        && state.env_center.python_remote_latest.is_empty()
        && state.env_center.installed.is_empty();
    if !state.reduce_motion
        && !state.disable_runtime_skeleton_shimmer
        && matches!(state.route(), Route::Runtime)
        && (waiting_installed_list || waiting_node_remote || waiting_python_remote)
    {
        state.env_center.skeleton_phase = (state.env_center.skeleton_phase + 0.045) % 1.0;
    }
    Task::none()
}

fn handle_download(state: &mut AppState, msg: DownloadMsg) -> Task<Message> {
    match msg {
        DownloadMsg::Tick => {
            state.downloads.on_tick();
            Task::none()
        }
        DownloadMsg::ToggleVisible => {
            let tokens = state.tokens();
            if state.downloads.visible && state.downloads.reveal_anim.is_none() {
                state.downloads.start_hide_anim(tokens);
            } else if !state.downloads.visible {
                state.downloads.start_show_anim(tokens);
                let _ = persist_download_panel_settings(state);
            }
            Task::none()
        }
        DownloadMsg::ToggleExpand => {
            state.downloads.expanded = !state.downloads.expanded;
            let _ = persist_download_panel_settings(state);
            Task::none()
        }
        DownloadMsg::TitleBarPress => {
            state.downloads.title_drag_armed_since = Some(std::time::Instant::now());
            state.downloads.last_drag_pointer = None;
            Task::none()
        }
        DownloadMsg::Event(e) => {
            use iced::Event;
            use iced::mouse;
            match e {
                Event::Mouse(mouse::Event::CursorMoved { position }) => {
                    let (cx, cy) = (position.x, position.y);
                    if state.downloads.title_drag_armed_since.is_some() && !state.downloads.dragging
                    {
                        state.downloads.last_drag_pointer = Some((cx, cy));
                    }
                    if !state.downloads.dragging {
                        return Task::none();
                    }
                    if state.downloads.drag_from_cursor.is_none() {
                        state.downloads.drag_from_cursor = Some((cx, cy));
                        return Task::none();
                    }
                    let (sx, sy) = state.downloads.drag_from_cursor.unwrap();
                    let (px, py) = state
                        .downloads
                        .drag_from_pos
                        .unwrap_or((state.downloads.x, state.downloads.y));
                    // Interpret y as bottom offset; moving cursor down decreases bottom offset.
                    let dx = cx - sx;
                    let dy = cy - sy;
                    state.downloads.x = (px + dx.round() as i32).max(0);
                    state.downloads.y = (py - dy.round() as i32).max(0);
                    clamp_download_panel_to_window(state);
                    Task::none()
                }
                Event::Mouse(mouse::Event::ButtonReleased(_btn)) => {
                    if state.downloads.dragging {
                        state.downloads.dragging = false;
                        state.downloads.drag_from_cursor = None;
                        state.downloads.drag_from_pos = None;
                        let _ = persist_download_panel_settings(state);
                    }
                    state.downloads.title_drag_armed_since = None;
                    Task::none()
                }
                _ => Task::none(),
            }
        }
        DownloadMsg::EnqueueDemo => enqueue_demo_download(state),
        DownloadMsg::Finished { id, result } => {
            if let Some(j) = state.downloads.jobs.iter_mut().find(|j| j.id == id) {
                match &result {
                    Ok(_) => j.state = JobState::Done,
                    Err(e) => {
                        if looks_like_user_cancelled(e) {
                            j.state = JobState::Cancelled;
                            j.last_error = None;
                        } else {
                            j.state = JobState::Failed;
                            j.last_error = Some(e.clone());
                        }
                    }
                }
            }
            Task::none()
        }
        DownloadMsg::Cancel(id) => {
            if let Some(j) = state.downloads.jobs.iter_mut().find(|j| j.id == id) {
                j.cancel.cancel();
            }
            Task::none()
        }
        DownloadMsg::Retry(id) => {
            let Some(failed) = state
                .downloads
                .jobs
                .iter()
                .find(|j| j.id == id && j.state == JobState::Failed)
                .map(|j| (j.url.clone(), j.label.clone()))
            else {
                return Task::none();
            };
            let (url_str, label) = failed;
            state.downloads.jobs.retain(|j| j.id != id);
            retry_download(
                state,
                &url_str,
                &format!(
                    "{label} {}",
                    envr_core::i18n::tr_key("gui.action.retry_suffix", "(重试)", "(retry)")
                ),
            )
        }
    }
}

fn persist_download_panel_settings(state: &mut AppState) -> Result<(), envr_error::EnvrError> {
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
        .map_or(true, |s| s.trim().is_empty());
    if disk_rr_empty {
        if let Some(ref r) = mem.paths.runtime_root {
            let t = r.trim();
            if !t.is_empty() {
                st.paths.runtime_root = Some(t.to_string());
            }
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
    state.runtime_settings.cache.set_cached(st);
    let _ = state.settings.sync_from_cache();
    let _ = state.runtime_settings.sync_from_cache();
    Ok(())
}

fn on_main_window_resized(state: &mut AppState, new: Size) {
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

fn clamp_download_panel_to_window(state: &mut AppState) {
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

fn enqueue_demo_download(state: &mut AppState) -> Task<Message> {
    retry_download(
        state,
        download_runner::DEMO_URL,
        &format!(
            "{} #{}",
            envr_core::i18n::tr_key("gui.label.demo", "演示", "Demo"),
            state.downloads.next_id
        ),
    )
}

fn retry_download(state: &mut AppState, url_str: &str, label: &str) -> Task<Message> {
    let url = match reqwest::Url::parse(url_str) {
        Ok(u) => u,
        Err(e) => {
            state.error = Some(format!(
                "{}: {e}",
                envr_core::i18n::tr_key(
                    "gui.error.url_parse_failed",
                    "URL 解析失败",
                    "URL parse failed",
                )
            ));
            return Task::none();
        }
    };
    let id = state.downloads.next_id;
    state.downloads.next_id += 1;
    let dest = std::env::temp_dir().join(format!("envr-gui-dl-{id}.tmp"));
    let downloaded = Arc::new(AtomicU64::new(0));
    let total = Arc::new(AtomicU64::new(0));
    let cancel = CancelToken::new();
    state.downloads.jobs.push(DownloadJob {
        id,
        label: label.to_string(),
        url: url_str.to_string(),
        state: JobState::Running,
        downloaded: downloaded.clone(),
        total: total.clone(),
        cancel: cancel.clone(),
        last_error: None,
        tick_prev_bytes: 0,
        tick_prev_at: None,
        speed_bps: 0.0,
    });
    download_runner::start_http_job(id, url, dest, cancel, downloaded, total)
}

fn enqueue_runtime_install_job(
    state: &mut AppState,
    label: String,
) -> (u64, Arc<AtomicU64>, Arc<AtomicU64>, CancelToken) {
    let id = state.downloads.next_id;
    state.downloads.next_id += 1;
    let downloaded = Arc::new(AtomicU64::new(0));
    let total = Arc::new(AtomicU64::new(0));
    let cancel = CancelToken::new();
    // Empty `url` marks a local install task (see downloads panel / `format_job_state_line`).
    state.downloads.jobs.push(DownloadJob {
        id,
        label,
        url: String::new(),
        state: JobState::Running,
        downloaded: downloaded.clone(),
        total: total.clone(),
        cancel: cancel.clone(),
        last_error: None,
        tick_prev_bytes: 0,
        tick_prev_at: None,
        speed_bps: 0.0,
    });
    let tokens = state.tokens();
    if !state.downloads.visible {
        state.downloads.start_show_anim(tokens);
    }
    (id, downloaded, total, cancel)
}

fn looks_like_user_cancelled(err: &str) -> bool {
    let l = err.to_ascii_lowercase();
    l.contains("cancelled")
        || l.contains("canceled")
        || l.contains("download cancel")
}

fn runtime_install_task_label(
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

fn runtime_page_enter_tasks(state: &mut AppState) -> Task<Message> {
    let kind = state.env_center.kind;
    match kind {
        envr_domain::runtime::RuntimeKind::Node => {
            state.env_center.node_remote_refreshing = true;
            state.env_center.python_remote_refreshing = false;
            Task::batch([
                gui_ops::refresh_runtimes(kind),
                gui_ops::load_remote_latest_disk_snapshot(kind),
                gui_ops::refresh_remote_latest_per_major(kind),
            ])
        }
        envr_domain::runtime::RuntimeKind::Python => {
            state.env_center.node_remote_refreshing = false;
            state.env_center.python_remote_refreshing = true;
            Task::batch([
                gui_ops::refresh_runtimes(kind),
                gui_ops::load_remote_latest_disk_snapshot(kind),
                gui_ops::refresh_remote_latest_per_major(kind),
            ])
        }
        _ => {
            state.env_center.node_remote_refreshing = false;
            state.env_center.python_remote_refreshing = false;
            gui_ops::refresh_runtimes(kind)
        }
    }
}

fn handle_env_center(state: &mut AppState, msg: EnvCenterMsg) -> Task<Message> {
    match msg {
        EnvCenterMsg::PickKind(k) => {
            if state.env_center.kind == k {
                return Task::none();
            }
            state.env_center.kind = k;
            state.env_center.remote_error = None;
            state.env_center.node_remote_latest.clear();
            state.env_center.node_remote_refreshing = false;
            state.env_center.python_remote_latest.clear();
            state.env_center.python_remote_refreshing = false;
            state.env_center.install_input.clear();
            state.env_center.direct_install_input.clear();
            state.env_center.runtime_settings_expanded = false;
            if k == envr_domain::runtime::RuntimeKind::Node {
                state.env_center.node_remote_refreshing = true;
                Task::batch([
                    gui_ops::refresh_runtimes(k),
                    gui_ops::load_remote_latest_disk_snapshot(k),
                    gui_ops::refresh_remote_latest_per_major(k),
                ])
            } else if k == envr_domain::runtime::RuntimeKind::Python {
                state.env_center.python_remote_refreshing = true;
                Task::batch([
                    gui_ops::refresh_runtimes(k),
                    gui_ops::load_remote_latest_disk_snapshot(k),
                    gui_ops::refresh_remote_latest_per_major(k),
                ])
            } else {
                Task::batch([gui_ops::refresh_runtimes(k)])
            }
        }
        EnvCenterMsg::InstallInput(s) => {
            state.env_center.install_input = sanitize_runtime_filter_input(state.env_center.kind, &s);
            Task::none()
        }
        EnvCenterMsg::DirectInstallInput(s) => {
            state.env_center.direct_install_input = s;
            Task::none()
        }
        EnvCenterMsg::DataLoaded(res) => {
            state.env_center.busy = false;
            match res {
                Ok((list, cur)) => {
                    state.env_center.installed = list;
                    state.env_center.current = cur;
                }
                Err(e) => state.error = Some(e),
            }
            Task::none()
        }
        EnvCenterMsg::RemoteLatestDiskSnapshot(kind, rows) => {
            match kind {
                envr_domain::runtime::RuntimeKind::Node => {
                    if state.env_center.node_remote_latest.is_empty()
                        || rows.len() > state.env_center.node_remote_latest.len()
                    {
                        state.env_center.node_remote_latest = rows;
                    }
                }
                envr_domain::runtime::RuntimeKind::Python => {
                    if state.env_center.python_remote_latest.is_empty()
                        || rows.len() > state.env_center.python_remote_latest.len()
                    {
                        state.env_center.python_remote_latest = rows;
                    }
                }
                _ => {}
            }
            Task::none()
        }
        EnvCenterMsg::RemoteLatestRefreshed(kind, res) => {
            match res {
                Ok(rows) => {
                    state.env_center.remote_error = None;
                    match kind {
                        envr_domain::runtime::RuntimeKind::Node => {
                            state.env_center.node_remote_refreshing = false;
                            state.env_center.node_remote_latest = rows;
                        }
                        envr_domain::runtime::RuntimeKind::Python => {
                            state.env_center.python_remote_refreshing = false;
                            state.env_center.python_remote_latest = rows;
                        }
                        _ => {}
                    }
                }
                Err(e) => {
                    state.env_center.remote_error = Some(e);
                    match kind {
                        envr_domain::runtime::RuntimeKind::Node => {
                            state.env_center.node_remote_refreshing = false;
                        }
                        envr_domain::runtime::RuntimeKind::Python => {
                            state.env_center.python_remote_refreshing = false;
                        }
                        _ => {}
                    }
                }
            }
            Task::none()
        }
        EnvCenterMsg::SubmitDirectInstall => {
            if state.env_center.busy {
                return Task::none();
            }
            let spec = state.env_center.direct_install_input.trim().to_string();
            if spec.is_empty() || !direct_install_spec_ok(&spec) {
                return Task::none();
            }
            state.env_center.busy = true;
            state.error = None;
            let (id, downloaded, total, cancel) = enqueue_runtime_install_job(
                state,
                runtime_install_task_label(state.env_center.kind, &spec, false),
            );
            state.env_center.op_job_id = Some(id);
            gui_ops::install_version_with_resolve_precheck(
                state.env_center.kind,
                spec,
                downloaded,
                total,
                cancel,
            )
        }
        EnvCenterMsg::SubmitDirectInstallAndUse => {
            if state.env_center.busy || runtime_path_proxy_blocks_use(state) {
                return Task::none();
            }
            let spec = state.env_center.direct_install_input.trim().to_string();
            if spec.is_empty() || !direct_install_spec_ok(&spec) {
                return Task::none();
            }
            state.env_center.busy = true;
            state.error = None;
            let (id, downloaded, total, cancel) = enqueue_runtime_install_job(
                state,
                runtime_install_task_label(state.env_center.kind, &spec, true),
            );
            state.env_center.op_job_id = Some(id);
            gui_ops::install_then_use_with_resolve_precheck(
                state.env_center.kind,
                spec,
                downloaded,
                total,
                cancel,
            )
        }
        EnvCenterMsg::SubmitInstall(spec) => {
            if state.env_center.busy {
                return Task::none();
            }
            if spec.trim().is_empty() {
                return Task::none();
            }
            state.env_center.busy = true;
            state.error = None;
            let (id, downloaded, total, cancel) = enqueue_runtime_install_job(
                state,
                runtime_install_task_label(state.env_center.kind, &spec, false),
            );
            state.env_center.op_job_id = Some(id);
            gui_ops::install_version(state.env_center.kind, spec, downloaded, total, cancel)
        }
        EnvCenterMsg::SubmitInstallAndUse(spec) => {
            if state.env_center.busy || runtime_path_proxy_blocks_use(state) {
                return Task::none();
            }
            if spec.trim().is_empty() {
                return Task::none();
            }
            state.env_center.busy = true;
            state.error = None;
            let (id, downloaded, total, cancel) = enqueue_runtime_install_job(
                state,
                runtime_install_task_label(state.env_center.kind, &spec, true),
            );
            state.env_center.op_job_id = Some(id);
            gui_ops::install_then_use(state.env_center.kind, spec, downloaded, total, cancel)
        }
        EnvCenterMsg::InstallFinished(res) => {
            state.env_center.busy = false;
            if let Some(id) = state.env_center.op_job_id.take()
                && let Some(j) = state.downloads.jobs.iter_mut().find(|j| j.id == id)
            {
                match &res {
                    Ok(_) => j.state = JobState::Done,
                    Err(e) => {
                        if looks_like_user_cancelled(e) {
                            j.state = JobState::Cancelled;
                            j.last_error = None;
                        } else {
                            j.state = JobState::Failed;
                            j.last_error = Some(e.clone());
                        }
                    }
                }
            }
            match &res {
                Ok(v) => {
                    tracing::info!(version = %v.0, "gui install ok");
                    // `install_input` is now the search keyword; keep it for better feedback.
                }
                Err(e) => {
                    if !looks_like_user_cancelled(e) {
                        state.error = Some(e.clone());
                    }
                }
            }
            gui_ops::refresh_runtimes(state.env_center.kind)
        }
        EnvCenterMsg::SubmitUse(v) => {
            if state.env_center.busy || runtime_path_proxy_blocks_use(state) {
                return Task::none();
            }
            state.env_center.busy = true;
            state.error = None;
            gui_ops::use_version(state.env_center.kind, v)
        }
        EnvCenterMsg::UseFinished(res) => {
            state.env_center.busy = false;
            if let Err(e) = res {
                state.error = Some(e);
            }
            gui_ops::refresh_runtimes(state.env_center.kind)
        }
        EnvCenterMsg::SubmitUninstall(v) => {
            if state.env_center.busy {
                return Task::none();
            }
            state.env_center.busy = true;
            state.error = None;
            gui_ops::uninstall_version(state.env_center.kind, v)
        }
        EnvCenterMsg::UninstallFinished(res) => {
            state.env_center.busy = false;
            if let Err(e) = res {
                state.error = Some(e);
            }
            gui_ops::refresh_runtimes(state.env_center.kind)
        }
        EnvCenterMsg::ToggleRuntimeSettings => {
            state.env_center.runtime_settings_expanded = !state.env_center.runtime_settings_expanded;
            Task::none()
        }
        EnvCenterMsg::SetNodeDownloadSource(src) => {
            let mut st = state.settings.cache.snapshot().clone();
            st.runtime.node.download_source = src;
            if let Err(e) = st.validate() {
                state.error = Some(e.to_string());
                return Task::none();
            }
            persist_settings_clone_task(st)
        }
        EnvCenterMsg::SetNpmRegistryMode(mode) => {
            let mut st = state.settings.cache.snapshot().clone();
            st.runtime.node.npm_registry_mode = mode;
            if let Err(e) = st.validate() {
                state.error = Some(e.to_string());
                return Task::none();
            }
            persist_settings_clone_task(st)
        }
        EnvCenterMsg::SetNodePathProxy(on) => {
            let mut st = state.settings.cache.snapshot().clone();
            st.runtime.node.path_proxy_enabled = on;
            if let Err(e) = st.validate() {
                state.error = Some(e.to_string());
                return Task::none();
            }
            persist_settings_clone_task(st)
        }
        EnvCenterMsg::SetPythonDownloadSource(src) => {
            let mut st = state.settings.cache.snapshot().clone();
            st.runtime.python.download_source = src;
            if let Err(e) = st.validate() {
                state.error = Some(e.to_string());
                return Task::none();
            }
            persist_settings_clone_task(st)
        }
        EnvCenterMsg::SetPipRegistryMode(mode) => {
            let mut st = state.settings.cache.snapshot().clone();
            st.runtime.python.pip_registry_mode = mode;
            if let Err(e) = st.validate() {
                state.error = Some(e.to_string());
                return Task::none();
            }
            persist_settings_clone_task(st)
        }
        EnvCenterMsg::SetPythonPathProxy(on) => {
            let mut st = state.settings.cache.snapshot().clone();
            st.runtime.python.path_proxy_enabled = on;
            if let Err(e) = st.validate() {
                state.error = Some(e.to_string());
                return Task::none();
            }
            persist_settings_clone_task(st)
        }
    }
}

fn direct_install_spec_ok(spec: &str) -> bool {
    let t = spec.trim();
    if t.is_empty() || t.len() > 80 {
        return false;
    }
    if t.chars().any(|c| c.is_control()) {
        return false;
    }
    true
}

fn sanitize_runtime_filter_input(kind: envr_domain::runtime::RuntimeKind, raw: &str) -> String {
    let t = raw.trim();
    match kind {
        // Node filter supports major only.
        envr_domain::runtime::RuntimeKind::Node => t
            .chars()
            .filter(|c| c.is_ascii_digit())
            .take(4)
            .collect(),
        // Python filter supports major.minor.
        envr_domain::runtime::RuntimeKind::Python => {
            let mut out = String::new();
            let mut dot_seen = false;
            for ch in t.chars() {
                if ch.is_ascii_digit() {
                    out.push(ch);
                } else if ch == '.' && !dot_seen {
                    out.push(ch);
                    dot_seen = true;
                }
                if out.len() >= 8 {
                    break;
                }
            }
            out
        }
        _ => t.chars().take(32).collect(),
    }
}

fn view(state: &AppState) -> Element<'_, Message> {
    shell::app_view(state)
}
