//! Main-window shell: left navigation, routed content, global error banner.
//! Update paths live in `pages::*`; shell logic is split under `app/{bootstrap,navigation,persist_settings,download_chrome,env_center_ops,subscription}.rs`.

use envr_config::settings::Settings;
use envr_ui::theme::{
    ThemeTokens, UiFlavor, default_flavor_for_target, scheme_for_mode, shell as layout_shell,
    tokens_for_appearance,
};
use iced::{Element, Size, Task, application};

use crate::gui_ops;
use crate::theme as gui_theme;
use crate::view::dashboard::{DashboardMsg, DashboardState};
use crate::view::downloads::{DownloadMsg, DownloadPanelState};
use crate::view::env_center::{EnvCenterMsg, EnvCenterState};
use crate::view::runtime_layout::RuntimeLayoutMsg;
use crate::view::settings::{SettingsMsg, SettingsViewState};
use crate::view::shell;

mod bootstrap;
mod download_chrome;
mod env_center_ops;
mod navigation;
mod pages;
mod persist_settings;
mod subscription;

pub(crate) use download_chrome::{
    clamp_download_panel_to_window, handle_motion_tick, on_main_window_resized,
    persist_download_panel_settings,
};
pub(crate) use env_center_ops::*;
pub(crate) use persist_settings::{
    STARTUP_SETTINGS, load_gui_downloads_panel_settings_cached, load_startup_settings,
    persist_path_proxy_toggle, persist_runtime_settings_update, persist_settings_draft_task,
    settings_path,
};

use bootstrap::{accent_from_settings, configured_default_font, env_flag, ui_text_scale_from_env};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Route {
    #[default]
    Dashboard,
    Runtime,
    RuntimeConfig,
    Downloads,
    Settings,
    About,
}

impl Route {
    pub(crate) const ALL: [Self; 6] = [
        Route::Dashboard,
        Route::Runtime,
        Route::RuntimeConfig,
        Route::Downloads,
        Route::Settings,
        Route::About,
    ];

    pub(crate) fn label(self) -> String {
        match self {
            Route::Dashboard => {
                envr_core::i18n::tr_key("gui.route.dashboard", "仪表盘", "Dashboard")
            }
            Route::Runtime => envr_core::i18n::tr_key("gui.route.runtime", "运行时", "Runtimes"),
            Route::RuntimeConfig => {
                envr_core::i18n::tr_key("gui.route.runtime_config", "运行时配置", "Runtime Config")
            }
            Route::Downloads => envr_core::i18n::tr_key("gui.route.downloads", "下载", "Downloads"),
            Route::Settings => envr_core::i18n::tr_key("gui.route.settings", "设置", "Settings"),
            Route::About => envr_core::i18n::tr_key("gui.route.about", "关于", "About"),
        }
    }
}

pub struct AppState {
    locale: envr_core::i18n::Locale,
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
}

impl Default for AppState {
    fn default() -> Self {
        let gui_defaults = load_gui_downloads_panel_settings_cached();
        let startup = STARTUP_SETTINGS.get().cloned().unwrap_or_default();
        let locale = envr_core::i18n::locale_from_settings(&startup);
        // Best-effort: configure global download bandwidth cap for this GUI process.
        let _ = envr_download::set_global_download_limit(Some(startup.download.max_bytes_per_sec));
        let _ = envr_download::set_global_download_concurrency_limit(Some(
            startup.download.max_concurrent_downloads as usize,
        ));
        Self {
            locale,
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
        }
    }
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
    /// Background work completed; no state change (e.g. disk cache invalidation).
    Idle,
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
    RuntimeLayout(RuntimeLayoutMsg),
}

pub fn run() -> iced::Result {
    let startup = load_startup_settings();
    let locale = envr_core::i18n::locale_from_settings(&startup);
    application(
        move || {
            let mut state = AppState::default();
            state.locale = locale;
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
    .subscription(subscription::shell_subscription)
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

fn update(state: &mut AppState, message: Message) -> Task<Message> {
    let locale = state.locale;
    envr_core::i18n::with_locale(locale, || match message {
        Message::Idle => Task::none(),
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
        Message::Navigate(route) => navigation::handle_navigate(state, route),
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
        Message::EnvCenter(msg) => pages::env_center::handle_env_center(state, msg),
        Message::Dashboard(msg) => pages::dashboard::handle_dashboard(state, msg),
        Message::Download(msg) => pages::downloads::handle_download(state, msg),
        Message::Settings(msg) => pages::settings::handle_settings(state, msg),
        Message::RuntimeLayout(msg) => pages::runtime_layout::handle_runtime_layout(state, msg),
    })
}

fn view(state: &AppState) -> Element<'_, Message> {
    envr_core::i18n::with_locale(state.locale, || shell::app_view(state))
}
