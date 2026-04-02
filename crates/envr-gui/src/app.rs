//! Main-window shell: left navigation, routed content, global error banner.

use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::time::Duration;

use envr_config::settings::{FontMode, RuntimeInstallMode, Settings, ThemeMode};
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
            let need_motion = state.downloads.needs_motion_tick()
                || state.downloads.title_drag_armed_since.is_some()
                || (matches!(state.route(), Route::Runtime)
                    && state.env_center.busy
                    && state.env_center.installed.is_empty());
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
                // Avoid any expensive remote fetches until the user actually navigates
                // to the Runtime page.
                let mode = state.settings.draft.behavior.runtime_install_mode;
                if state.env_center.kind == envr_domain::runtime::RuntimeKind::Node
                    && mode == RuntimeInstallMode::Exact
                    && state.env_center.remote_major_keys.is_empty()
                    && !state.env_center.remote_major_loading
                {
                    state.env_center.remote_major_loading = true;
                    return Task::batch([
                        gui_ops::fetch_remote_major_keys(envr_domain::runtime::RuntimeKind::Node),
                        gui_ops::refresh_runtimes(state.env_center.kind),
                    ]);
                }
                return gui_ops::refresh_runtimes(state.env_center.kind);
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
            }
            Task::none()
        }
        SettingsMsg::ClearRuntimeRoot => {
            state.settings.runtime_root_draft.clear();
            Task::none()
        }
        SettingsMsg::SetRuntimeInstallMode(m) => {
            state.settings.draft.behavior.runtime_install_mode = m;
            // Defer any expensive remote major-key fetching until navigation
            // to the Runtime page (see Message::Navigate(Route::Runtime)).
            Task::none()
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
                    } else {
                        state.settings.last_message = Some(envr_core::i18n::tr_key(
                            "gui.app.saved_settings_toml",
                            "已保存到 settings.toml。",
                            "Saved.",
                        ));
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
    if !state.reduce_motion
        && matches!(state.route(), Route::Runtime)
        && state.env_center.busy
        && state.env_center.installed.is_empty()
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
                        if e.contains("cancelled") {
                            j.state = JobState::Cancelled;
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

fn persist_download_panel_settings(state: &AppState) -> Result<(), envr_error::EnvrError> {
    let paths = envr_platform::paths::current_platform_paths()?;
    let settings_path = envr_config::settings::settings_path_from_platform(&paths);
    let mut st = Settings::load_or_default_from(&settings_path).unwrap_or_default();
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

fn handle_env_center(state: &mut AppState, msg: EnvCenterMsg) -> Task<Message> {
    match msg {
        EnvCenterMsg::PickKind(k) => {
            if state.env_center.kind == k {
                return Task::none();
            }
            state.env_center.kind = k;
            // Reset remote caches / expansion state when switching runtime kind.
            state.env_center.remote_cache.clear();
            state.env_center.remote_major_keys.clear();
            state.env_center.remote_major_loading = false;
            state.env_center.expanded_exact_majors.clear();
            state.env_center.remote_loading_majors.clear();
            state.env_center.install_input.clear();
            state.env_center.expanded = true;
            state.env_center.busy = false;
            Task::batch([gui_ops::refresh_runtimes(k)])
        }
        EnvCenterMsg::InstallInput(s) => {
            state.env_center.install_input = s;
            // In Exact mode, interpret query as a `major` prefix (e.g. "25") and lazy-fetch children.
            let mode = state.settings.draft.behavior.runtime_install_mode;
            if state.env_center.kind == envr_domain::runtime::RuntimeKind::Node && mode == RuntimeInstallMode::Exact {
                if let Some(major) = parse_major_only(&state.env_center.install_input) {
                    state.env_center.expanded_exact_majors.insert(major.clone());
                    if !state.env_center.remote_cache.contains_key(&major)
                        && !state.env_center.remote_loading_majors.contains(&major)
                    {
                        state.env_center.remote_loading_majors.insert(major.clone());
                        return gui_ops::fetch_remote_prefix(state.env_center.kind, major);
                    }
                }
            }
            Task::none()
        }
        EnvCenterMsg::DataLoaded(res) => {
            state.env_center.busy = false;
            match res {
                Ok((list, cur)) => {
                    state.env_center.installed = list;
                    state.env_center.current = cur;

                    // In Exact mode, auto-expand the current major and lazily fetch its remote children.
                    if state.settings.draft.behavior.runtime_install_mode == RuntimeInstallMode::Exact
                        && state.env_center.kind == envr_domain::runtime::RuntimeKind::Node
                    {
                        if let Some(cur_v) = state.env_center.current.as_ref() {
                            if let Some(major) = parse_major_from_ver(&cur_v.0) {
                                state.env_center
                                    .expanded_exact_majors
                                    .insert(major.clone());
                                if !state.env_center.remote_cache.contains_key(&major)
                                    && !state.env_center.remote_loading_majors.contains(&major)
                                {
                                    state.env_center.remote_loading_majors.insert(major.clone());
                                    return gui_ops::fetch_remote_prefix(state.env_center.kind, major);
                                }
                            }
                        }
                    }
                }
                Err(e) => state.error = Some(e),
            }
            Task::none()
        }
        EnvCenterMsg::RemoteFetchedPrefix(res) => {
            match res {
                Ok((prefix, list)) => {
                    state.env_center.remote_cache.insert(prefix.clone(), list);
                    state.env_center.remote_loading_majors.remove(&prefix);
                }
                Err(e) => state.error = Some(e),
            }
            Task::none()
        }
        EnvCenterMsg::RemoteFetchedMajorKeys(res) => {
            state.env_center.remote_major_loading = false;
            match res {
                Ok(keys) => state.env_center.remote_major_keys = keys,
                Err(e) => state.error = Some(e),
            }
            Task::none()
        }
        EnvCenterMsg::SubmitInstall(spec) => {
            if spec.trim().is_empty() {
                return Task::none();
            }
            state.env_center.busy = true;
            state.error = None;
            gui_ops::install_version(state.env_center.kind, spec)
        }
        EnvCenterMsg::SubmitInstallAndUse(spec) => {
            if spec.trim().is_empty() {
                return Task::none();
            }
            state.env_center.busy = true;
            state.error = None;
            gui_ops::install_then_use(state.env_center.kind, spec)
        }
        EnvCenterMsg::InstallFinished(res) => {
            state.env_center.busy = false;
            match &res {
                Ok(v) => {
                    tracing::info!(version = %v.0, "gui install ok");
                    // `install_input` is now the search keyword; keep it for better feedback.
                }
                Err(e) => state.error = Some(e.clone()),
            }
            gui_ops::refresh_runtimes(state.env_center.kind)
        }
        EnvCenterMsg::SubmitUse(v) => {
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
        EnvCenterMsg::ToggleExpanded => {
            state.env_center.expanded = !state.env_center.expanded;
            Task::none()
        }
        EnvCenterMsg::ToggleExactMajor(major) => {
            if state.env_center.expanded_exact_majors.contains(&major) {
                state.env_center.expanded_exact_majors.remove(&major);
                return Task::none();
            }
            state.env_center.expanded_exact_majors.insert(major.clone());
            if !state.env_center.remote_cache.contains_key(&major)
                && !state.env_center.remote_loading_majors.contains(&major)
            {
                state.env_center.remote_loading_majors.insert(major.clone());
                return gui_ops::fetch_remote_prefix(state.env_center.kind, major);
            }
            Task::none()
        }
    }
}

fn parse_major_only(s: &str) -> Option<String> {
    let t = s.trim().strip_prefix('v').unwrap_or(s.trim());
    if t.is_empty() {
        return None;
    }
    if t.chars().all(|c| c.is_ascii_digit()) {
        Some(t.to_string())
    } else {
        None
    }
}

fn parse_major_from_ver(ver: &str) -> Option<String> {
    let t = ver.trim().strip_prefix('v').unwrap_or(ver.trim());
    let first = t.split('.').next()?;
    // Node versions are normally `MAJOR.MINOR.PATCH` (e.g. `20.11.1`).
    if first.chars().all(|c| c.is_ascii_digit()) {
        Some(first.to_string())
    } else {
        None
    }
}

fn view(state: &AppState) -> Element<'_, Message> {
    shell::app_view(state)
}
