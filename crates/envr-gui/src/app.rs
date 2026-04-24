//! Main-window shell: left navigation, routed content, global error banner.

use std::time::Duration;

use envr_config::settings::{FontMode, Settings, ThemeMode};
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

use crate::gui_ops;
use crate::theme as gui_theme;
use crate::view::dashboard::{DashboardMsg, DashboardState};
use crate::view::downloads::{
    DOWNLOAD_PANEL_SHELL_W, DownloadMsg, DownloadPanelState, JobState, TITLE_DRAG_HOLD,
};
use crate::view::env_center::{
    EnvCenterMsg, EnvCenterState, env_center_clear_unified_list_render_state,
};
use crate::view::runtime_layout::RuntimeLayoutMsg;
use crate::view::settings::{SettingsMsg, SettingsViewState};
use crate::view::shell;

mod pages;

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
    // Prefer native Windows GPU APIs first, keep GL as final fallback.
    // This avoids forcing software OpenGL paths on some VM drivers.
    #[cfg(target_os = "windows")]
    {
        if std::env::var_os("WGPU_BACKEND").is_none() {
            // Safe to do early during startup, before wgpu/iced are initialized.
            unsafe { std::env::set_var("WGPU_BACKEND", "dx12,dx11,vulkan,gl") };
        }
    }

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
    .subscription(|state| {
        let runtime_skeleton = matches!(state.route(), Route::Runtime)
            && state.env_center.installed.is_empty()
            && (state.env_center.busy
                || (envr_domain::runtime::runtime_descriptor(state.env_center.kind)
                    .supports_remote_latest
                    && state
                        .env_center
                        .unified_major_rows_by_kind
                        .get(&state.env_center.kind)
                        .is_none_or(|rows| rows.is_empty())));
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

        let theme_poll = (state.settings.draft.appearance.theme_mode == ThemeMode::FollowSystem)
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
        Message::Navigate(route) => {
            tracing::debug!(?route, "navigate");
            let leaving_runtime = state.route() == Route::Runtime && route != Route::Runtime;
            state.route = route;
            if leaving_runtime {
                env_center_clear_unified_list_render_state(&mut state.env_center);
            }
            if route == Route::Runtime {
                return runtime_page_enter_tasks(state);
            }
            if route == Route::Dashboard {
                state.dashboard.busy = true;
                state.dashboard.last_error = None;
                return gui_ops::refresh_dashboard();
            }
            if matches!(route, Route::Settings | Route::RuntimeConfig) {
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
        Message::EnvCenter(msg) => pages::env_center::handle_env_center(state, msg),
        Message::Dashboard(msg) => handle_dashboard(state, msg),
        Message::Download(msg) => pages::downloads::handle_download(state, msg),
        Message::Settings(msg) => pages::settings::handle_settings(state, msg),
        Message::RuntimeLayout(msg) => pages::runtime_layout::handle_runtime_layout(state, msg),
    })
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

/// Save a full [`Settings`] snapshot (e.g. runtime.node edits from the env center) and mirror [`SettingsMsg::DiskSaved`].
fn persist_settings_clone_task(settings: Settings) -> Task<Message> {
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

fn persist_runtime_settings_update<F>(state: &mut AppState, update: F) -> Task<Message>
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

fn persist_path_proxy_toggle<F>(
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

fn mark_unified_major_rows_dirty_for_kind(
    state: &mut AppState,
    kind: envr_domain::runtime::RuntimeKind,
) {
    state.env_center.unified_major_rows_by_kind.remove(&kind);
    state
        .env_center
        .unified_children_rows_by_kind_major
        .retain(|(k, _), _| *k != kind);
}

fn runtime_path_proxy_blocks_use(state: &AppState) -> bool {
    envr_config::runtime_path_proxy::path_proxy_blocks_managed_use(
        state.env_center.kind,
        &state.settings.cache.snapshot().runtime,
    )
}

fn handle_motion_tick(state: &mut AppState) -> Task<Message> {
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

fn looks_like_user_cancelled(err: &str) -> bool {
    let l = err.to_ascii_lowercase();
    l.contains("cancelled") || l.contains("canceled") || l.contains("download cancel")
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

fn duplicate_runtime_install_blocked(
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

fn rust_runtime_task_label(action: &str, detail: &str) -> String {
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

fn runtime_page_enter_tasks(state: &mut AppState) -> Task<Message> {
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

fn sync_go_env_center_drafts_from_settings(state: &mut AppState) {
    let g = &state.settings.cache.snapshot().runtime.go;
    state.env_center.go_proxy_custom_draft = g
        .proxy_custom
        .clone()
        .or_else(|| g.goproxy.clone())
        .unwrap_or_default();
    state.env_center.go_private_patterns_draft = g.private_patterns.clone().unwrap_or_default();
}

fn sync_bun_env_center_drafts_from_settings(state: &mut AppState) {
    let b = &state.settings.cache.snapshot().runtime.bun;
    state.env_center.bun_global_bin_dir_draft = b.global_bin_dir.clone().unwrap_or_default();
}

fn recompute_env_center_derived(state: &mut AppState) {
    let _ = state;
}

fn env_center_jvm_check_task(kind: envr_domain::runtime::RuntimeKind) -> Task<Message> {
    let key = envr_domain::runtime::runtime_descriptor(kind).key;
    if envr_domain::jvm_hosted::is_jvm_hosted_runtime(key) {
        gui_ops::check_jvm_runtime_java_compat(kind)
    } else {
        Task::none()
    }
}

fn set_jvm_java_hint(
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

fn bun_direct_spec_blocked_on_windows(kind: envr_domain::runtime::RuntimeKind, spec: &str) -> bool {
    if !cfg!(windows) || kind != envr_domain::runtime::RuntimeKind::Bun {
        return false;
    }
    let t = spec.trim().trim_start_matches('v');
    t.starts_with("0.")
}

fn deno_direct_spec_blocked(kind: envr_domain::runtime::RuntimeKind, spec: &str) -> bool {
    if kind != envr_domain::runtime::RuntimeKind::Deno {
        return false;
    }
    let t = spec.trim().trim_start_matches('v');
    t.starts_with("0.")
}

fn sanitize_runtime_filter_input(kind: envr_domain::runtime::RuntimeKind, raw: &str) -> String {
    let t = raw.trim();
    match kind {
        // Node filter supports major only.
        envr_domain::runtime::RuntimeKind::Node => {
            t.chars().filter(|c| c.is_ascii_digit()).take(4).collect()
        }
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
        envr_domain::runtime::RuntimeKind::Java => {
            t.chars().filter(|c| c.is_ascii_digit()).take(3).collect()
        }
        envr_domain::runtime::RuntimeKind::Go => {
            let mut out = String::new();
            let mut dot_seen = false;
            for ch in t.chars() {
                if ch.is_ascii_digit() {
                    out.push(ch);
                } else if ch == '.' && !dot_seen {
                    out.push(ch);
                    dot_seen = true;
                }
                if out.len() >= 12 {
                    break;
                }
            }
            out
        }
        _ => t.chars().take(32).collect(),
    }
}

fn view(state: &AppState) -> Element<'_, Message> {
    envr_core::i18n::with_locale(state.locale, || shell::app_view(state))
}
