//! Main-window shell: left navigation, routed content, global error banner.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
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
        Message::EnvCenter(msg) => handle_env_center(state, msg),
        Message::Dashboard(msg) => handle_dashboard(state, msg),
        Message::Download(msg) => handle_download(state, msg),
        Message::Settings(msg) => pages::settings::handle_settings(state, msg),
        Message::RuntimeLayout(msg) => handle_runtime_layout(state, msg),
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

fn fix_env_center_kind_if_hidden(state: &mut AppState) -> Task<Message> {
    let layout = &state.settings.cache.snapshot().gui.runtime_layout;
    let vis = crate::view::runtime_layout::visible_kinds(layout);
    if vis.is_empty() {
        return Task::none();
    }
    if vis.contains(&state.env_center.kind) {
        Task::none()
    } else {
        handle_env_center(state, EnvCenterMsg::PickKind(vis[0]))
    }
}

fn persist_runtime_layout_settings(state: &mut AppState) -> Result<(), envr_error::EnvrError> {
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

fn persist_runtime_layout_or_warn(state: &mut AppState) -> Task<Message> {
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

fn handle_runtime_layout(state: &mut AppState, msg: RuntimeLayoutMsg) -> Task<Message> {
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
                handle_env_center(state, EnvCenterMsg::PickKind(kind))
            } else {
                runtime_page_enter_tasks(state)
            }
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
                    Ok(_) => {
                        j.state = JobState::Done;
                        let d = j.downloaded.load(Ordering::Relaxed);
                        let t = j.total.load(Ordering::Relaxed);
                        if t == 0 || d < t {
                            j.total.store(d.max(t), Ordering::Relaxed);
                            j.downloaded.store(d.max(t), Ordering::Relaxed);
                        }
                    }
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
                if j.cancel.is_cancelled() {
                    return Task::none();
                }
                j.cancel.cancel();
                if j.state == JobState::Queued {
                    j.state = JobState::Cancelled;
                    j.last_error = None;
                }
            }
            maybe_start_queued_jobs(state)
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
    if url_str.trim().is_empty() {
        state.error = Some(envr_core::i18n::tr_key(
            "gui.error.retry_requires_url",
            "该任务没有可重试的下载 URL，请回到运行时页面重新安装。",
            "This task has no retryable download URL. Please retry install from runtime page.",
        ));
        return Task::none();
    }
    // Validate URL early so queued jobs don't fail later with a generic message.
    if let Err(e) = reqwest::Url::parse(url_str) {
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
        state: JobState::Queued,
        cancellable: true,
        downloaded: downloaded.clone(),
        total: total.clone(),
        cancel: cancel.clone(),
        last_error: None,
        tick_prev_bytes: 0,
        tick_prev_at: None,
        speed_bps: 0.0,
        payload: Some(crate::view::downloads::DownloadJobPayload::HttpDownload {
            url: url_str.to_string(),
            dest,
        }),
    });
    maybe_start_queued_jobs(state)
}

fn enqueue_runtime_install_job(
    state: &mut AppState,
    label: String,
    cancellable: bool,
    payload: crate::view::downloads::DownloadJobPayload,
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
        state: JobState::Queued,
        cancellable,
        downloaded: downloaded.clone(),
        total: total.clone(),
        cancel: cancel.clone(),
        last_error: None,
        tick_prev_bytes: 0,
        tick_prev_at: None,
        speed_bps: 0.0,
        payload: Some(payload),
    });
    let tokens = state.tokens();
    if !state.downloads.visible {
        state.downloads.start_show_anim(tokens);
    }
    (id, downloaded, total, cancel)
}

fn enqueue_op_job_running(
    state: &mut AppState,
    label: String,
    cancellable: bool,
) -> (u64, Arc<AtomicU64>, Arc<AtomicU64>, CancelToken) {
    let id = state.downloads.next_id;
    state.downloads.next_id += 1;
    let downloaded = Arc::new(AtomicU64::new(0));
    let total = Arc::new(AtomicU64::new(0));
    let cancel = CancelToken::new();
    state.downloads.jobs.push(DownloadJob {
        id,
        label,
        url: String::new(),
        state: JobState::Running,
        cancellable,
        downloaded: downloaded.clone(),
        total: total.clone(),
        cancel: cancel.clone(),
        last_error: None,
        tick_prev_bytes: 0,
        tick_prev_at: None,
        speed_bps: 0.0,
        payload: None,
    });
    let tokens = state.tokens();
    if !state.downloads.visible {
        state.downloads.start_show_anim(tokens);
    }
    (id, downloaded, total, cancel)
}

fn maybe_start_queued_jobs(state: &mut AppState) -> Task<Message> {
    let max = state
        .settings
        .cache
        .snapshot()
        .download
        .max_concurrent_downloads
        .max(1) as usize;
    let running = state
        .downloads
        .jobs
        .iter()
        .filter(|j| j.state == JobState::Running)
        .count();
    if running >= max {
        return Task::none();
    }
    let available = max - running;
    let mut tasks: Vec<Task<Message>> = Vec::new();
    for _ in 0..available {
        let Some(j) = state
            .downloads
            .jobs
            .iter_mut()
            .find(|j| j.state == JobState::Queued)
        else {
            break;
        };
        if j.cancel.is_cancelled() {
            j.state = JobState::Cancelled;
            j.last_error = None;
            continue;
        }
        let Some(payload) = j.payload.clone() else {
            j.state = JobState::Failed;
            j.last_error = Some("internal: missing job payload".into());
            continue;
        };
        j.state = JobState::Running;
        let id = j.id;
        match payload {
            crate::view::downloads::DownloadJobPayload::HttpDownload { url, dest } => {
                let Ok(url) = reqwest::Url::parse(&url) else {
                    j.state = JobState::Failed;
                    j.last_error = Some("invalid url".into());
                    continue;
                };
                tasks.push(download_runner::start_http_job(
                    id,
                    url,
                    dest,
                    j.cancel.clone(),
                    j.downloaded.clone(),
                    j.total.clone(),
                ));
            }
            crate::view::downloads::DownloadJobPayload::RuntimeInstall {
                kind,
                spec,
                resolve_precheck,
                install_and_use,
            } => {
                let cancel = j.cancel.clone();
                let downloaded = j.downloaded.clone();
                let total = j.total.clone();
                let task = if resolve_precheck && install_and_use {
                    gui_ops::install_then_use_with_resolve_precheck(
                        id, kind, spec, downloaded, total, cancel,
                    )
                } else if resolve_precheck {
                    gui_ops::install_version_with_resolve_precheck(
                        id, kind, spec, downloaded, total, cancel,
                    )
                } else if install_and_use {
                    gui_ops::install_then_use(id, kind, spec, downloaded, total, cancel)
                } else {
                    gui_ops::install_version(id, kind, spec, downloaded, total, cancel)
                };
                tasks.push(task);
            }
        }
    }
    if tasks.is_empty() {
        Task::none()
    } else {
        Task::batch(tasks)
    }
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

fn persist_jvm_path_proxy_toggle(
    state: &mut AppState,
    kind: envr_domain::runtime::RuntimeKind,
    on: bool,
) -> Task<Message> {
    match kind {
        envr_domain::runtime::RuntimeKind::Kotlin => {
            persist_path_proxy_toggle(state, kind, on, |st, on| {
                st.runtime.kotlin.path_proxy_enabled = on
            })
        }
        envr_domain::runtime::RuntimeKind::Scala => {
            persist_path_proxy_toggle(state, kind, on, |st, on| {
                st.runtime.scala.path_proxy_enabled = on
            })
        }
        envr_domain::runtime::RuntimeKind::Clojure => {
            persist_path_proxy_toggle(state, kind, on, |st, on| {
                st.runtime.clojure.path_proxy_enabled = on
            })
        }
        envr_domain::runtime::RuntimeKind::Groovy => {
            persist_path_proxy_toggle(state, kind, on, |st, on| {
                st.runtime.groovy.path_proxy_enabled = on
            })
        }
        _ => Task::none(),
    }
}

fn handle_env_center(state: &mut AppState, msg: EnvCenterMsg) -> Task<Message> {
    match msg {
        EnvCenterMsg::PickKind(k) => {
            if state.env_center.kind == k {
                return Task::none();
            }
            state.env_center.kind = k;
            state.env_center.jvm_java_hints.clear();
            state.env_center.remote_error = None;
            state.env_center.rust_status = None;
            state.env_center.rust_components.clear();
            state.env_center.rust_targets.clear();
            state.env_center.install_input.clear();
            state.env_center.direct_install_input.clear();
            state.env_center.unified_expanded_major_keys.clear();
            state.env_center.active_install_job_ids.clear();
            if k == envr_domain::runtime::RuntimeKind::Elixir {
                state.env_center.elixir_prereq_error = None;
            }
            if k == envr_domain::runtime::RuntimeKind::Go {
                sync_go_env_center_drafts_from_settings(state);
            }
            if k == envr_domain::runtime::RuntimeKind::Bun {
                sync_bun_env_center_drafts_from_settings(state);
            }
            if k == envr_domain::runtime::RuntimeKind::Rust {
                state.env_center.busy = true;
                recompute_env_center_derived(state);
                return Task::batch([
                    gui_ops::rust_refresh(),
                    gui_ops::rust_load_components(),
                    gui_ops::rust_load_targets(),
                ]);
            }
            if envr_domain::runtime::runtime_descriptor(k).supports_remote_latest {
                recompute_env_center_derived(state);
                return Task::batch([
                    gui_ops::refresh_runtimes(k),
                    envr_domain::runtime::unified_major_list_rollout_enabled(k)
                        .then_some(gui_ops::load_unified_major_rows_cached(k))
                        .unwrap_or_else(Task::none),
                    envr_domain::runtime::unified_major_list_rollout_enabled(k)
                        .then_some(gui_ops::refresh_unified_major_rows(k))
                        .unwrap_or_else(Task::none),
                    (k == envr_domain::runtime::RuntimeKind::Elixir)
                        .then_some(gui_ops::check_elixir_prereqs())
                        .unwrap_or_else(Task::none),
                    env_center_jvm_check_task(k),
                ]);
            }
            recompute_env_center_derived(state);
            Task::batch([gui_ops::refresh_runtimes(k)])
        }
        EnvCenterMsg::ElixirPrereqChecked(res) => {
            state.env_center.elixir_prereq_error = res.err();
            Task::none()
        }
        EnvCenterMsg::JvmJavaChecked(kind, res) => {
            set_jvm_java_hint(state, kind, res);
            Task::none()
        }
        EnvCenterMsg::InstallInput(s) => {
            state.env_center.install_input =
                sanitize_runtime_filter_input(state.env_center.kind, &s);
            recompute_env_center_derived(state);
            Task::none()
        }
        EnvCenterMsg::DirectInstallInput(s) => {
            state.env_center.direct_install_input = s;
            Task::none()
        }
        EnvCenterMsg::DataLoaded(res) => {
            state.env_center.busy = false;
            match res {
                Ok((list, cur, php_global_ts)) => {
                    state.env_center.installed = list;
                    state.env_center.current = cur;
                    state.env_center.php_global_current_want_ts = php_global_ts;
                }
                Err(e) => state.error = Some(e),
            }
            recompute_env_center_derived(state);
            Task::none()
        }
        EnvCenterMsg::UnifiedMajorRowsCached(kind, res) => {
            if !envr_domain::runtime::unified_major_list_rollout_enabled(kind) {
                return Task::none();
            }
            match res {
                Ok(rows) => {
                    state.env_center.remote_error = None;
                    state
                        .env_center
                        .unified_major_rows_by_kind
                        .insert(kind, rows);
                }
                Err(e) => {
                    state.env_center.remote_error = Some(e);
                }
            }
            Task::none()
        }
        EnvCenterMsg::UnifiedMajorRowsRefreshed(kind, res) => {
            if !envr_domain::runtime::unified_major_list_rollout_enabled(kind) {
                return Task::none();
            }
            match res {
                Ok(rows) => {
                    state.env_center.remote_error = None;
                    state
                        .env_center
                        .unified_major_rows_by_kind
                        .insert(kind, rows);
                }
                Err(e) => {
                    state.env_center.remote_error = Some(e);
                }
            }
            Task::none()
        }
        EnvCenterMsg::UnifiedChildrenCached(kind, major_key, res) => {
            if !envr_domain::runtime::unified_major_list_rollout_enabled(kind) {
                return Task::none();
            }
            match res {
                Ok(rows) => {
                    state.env_center.remote_error = None;
                    state
                        .env_center
                        .unified_children_rows_by_kind_major
                        .insert((kind, major_key), rows);
                }
                Err(e) => {
                    state.env_center.remote_error = Some(e);
                }
            }
            Task::none()
        }
        EnvCenterMsg::UnifiedChildrenRefreshed(kind, major_key, res) => {
            if !envr_domain::runtime::unified_major_list_rollout_enabled(kind) {
                return Task::none();
            }
            match res {
                Ok(rows) => {
                    state.env_center.remote_error = None;
                    state
                        .env_center
                        .unified_children_rows_by_kind_major
                        .insert((kind, major_key), rows);
                }
                Err(e) => {
                    state.env_center.remote_error = Some(e);
                }
            }
            Task::none()
        }
        EnvCenterMsg::ToggleUnifiedMajorExpanded(major_key) => {
            if !state
                .env_center
                .unified_expanded_major_keys
                .remove(&major_key)
            {
                state
                    .env_center
                    .unified_expanded_major_keys
                    .insert(major_key.clone());
                let kind = state.env_center.kind;
                return Task::batch([
                    gui_ops::load_unified_children_cached(kind, major_key.clone()),
                    gui_ops::refresh_unified_children(kind, major_key),
                ]);
            }
            Task::none()
        }
        EnvCenterMsg::SubmitDirectInstall => {
            let spec = state.env_center.direct_install_input.trim().to_string();
            if spec.is_empty() || !direct_install_spec_ok(&spec) {
                return Task::none();
            }
            if duplicate_runtime_install_blocked(state, state.env_center.kind, &spec) {
                return Task::none();
            }
            if bun_direct_spec_blocked_on_windows(state.env_center.kind, &spec) {
                state.error = Some(envr_core::i18n::tr_key(
                    "gui.runtime.bun.win_0x_blocked",
                    "Windows 不支持 Bun 0.x，请输入 1.x 及以上版本。",
                    "Bun 0.x is unavailable on Windows. Please enter version 1.x or newer.",
                ));
                return Task::none();
            }
            if deno_direct_spec_blocked(state.env_center.kind, &spec) {
                state.error = Some(envr_core::i18n::tr_key(
                    "gui.runtime.deno.0x_blocked",
                    "Deno 0.x 不受托管安装支持，请输入 1.x/2.x 版本。",
                    "Deno 0.x is not supported for managed install. Please enter a 1.x/2.x version.",
                ));
                return Task::none();
            }
            state.error = None;
            let (id, _downloaded, _total, _cancel) = enqueue_runtime_install_job(
                state,
                runtime_install_task_label(state.env_center.kind, &spec, false),
                true,
                crate::view::downloads::DownloadJobPayload::RuntimeInstall {
                    kind: state.env_center.kind,
                    spec,
                    resolve_precheck: true,
                    install_and_use: false,
                },
            );
            state.env_center.active_install_job_ids.insert(id);
            maybe_start_queued_jobs(state)
        }
        EnvCenterMsg::SubmitDirectInstallAndUse => {
            if runtime_path_proxy_blocks_use(state) {
                return Task::none();
            }
            let spec = state.env_center.direct_install_input.trim().to_string();
            if spec.is_empty() || !direct_install_spec_ok(&spec) {
                return Task::none();
            }
            if duplicate_runtime_install_blocked(state, state.env_center.kind, &spec) {
                return Task::none();
            }
            if bun_direct_spec_blocked_on_windows(state.env_center.kind, &spec) {
                state.error = Some(envr_core::i18n::tr_key(
                    "gui.runtime.bun.win_0x_blocked",
                    "Windows 不支持 Bun 0.x，请输入 1.x 及以上版本。",
                    "Bun 0.x is unavailable on Windows. Please enter version 1.x or newer.",
                ));
                return Task::none();
            }
            if deno_direct_spec_blocked(state.env_center.kind, &spec) {
                state.error = Some(envr_core::i18n::tr_key(
                    "gui.runtime.deno.0x_blocked",
                    "Deno 0.x 不受托管安装支持，请输入 1.x/2.x 版本。",
                    "Deno 0.x is not supported for managed install. Please enter a 1.x/2.x version.",
                ));
                return Task::none();
            }
            state.error = None;
            let (id, _downloaded, _total, _cancel) = enqueue_runtime_install_job(
                state,
                runtime_install_task_label(state.env_center.kind, &spec, true),
                true,
                crate::view::downloads::DownloadJobPayload::RuntimeInstall {
                    kind: state.env_center.kind,
                    spec,
                    resolve_precheck: true,
                    install_and_use: true,
                },
            );
            state.env_center.active_install_job_ids.insert(id);
            maybe_start_queued_jobs(state)
        }
        EnvCenterMsg::SubmitInstall(spec) => {
            if spec.trim().is_empty() {
                return Task::none();
            }
            if duplicate_runtime_install_blocked(state, state.env_center.kind, &spec) {
                return Task::none();
            }
            state.error = None;
            let (id, _downloaded, _total, _cancel) = enqueue_runtime_install_job(
                state,
                runtime_install_task_label(state.env_center.kind, &spec, false),
                true,
                crate::view::downloads::DownloadJobPayload::RuntimeInstall {
                    kind: state.env_center.kind,
                    spec,
                    resolve_precheck: false,
                    install_and_use: false,
                },
            );
            state.env_center.active_install_job_ids.insert(id);
            maybe_start_queued_jobs(state)
        }
        EnvCenterMsg::SubmitInstallAndUse(spec) => {
            if runtime_path_proxy_blocks_use(state) {
                return Task::none();
            }
            if spec.trim().is_empty() {
                return Task::none();
            }
            if duplicate_runtime_install_blocked(state, state.env_center.kind, &spec) {
                return Task::none();
            }
            state.error = None;
            let (id, _downloaded, _total, _cancel) = enqueue_runtime_install_job(
                state,
                runtime_install_task_label(state.env_center.kind, &spec, true),
                true,
                crate::view::downloads::DownloadJobPayload::RuntimeInstall {
                    kind: state.env_center.kind,
                    spec,
                    resolve_precheck: false,
                    install_and_use: true,
                },
            );
            state.env_center.active_install_job_ids.insert(id);
            maybe_start_queued_jobs(state)
        }
        EnvCenterMsg::InstallFinished {
            job_id,
            kind,
            result,
        } => {
            state.env_center.active_install_job_ids.remove(&job_id);
            if let Some(j) = state.downloads.jobs.iter_mut().find(|j| j.id == job_id) {
                match &result {
                    Ok(_) => {
                        j.state = JobState::Done;
                        let d = j.downloaded.load(Ordering::Relaxed);
                        let t = j.total.load(Ordering::Relaxed);
                        if t == 0 || d < t {
                            j.total.store(d.max(t), Ordering::Relaxed);
                            j.downloaded.store(d.max(t), Ordering::Relaxed);
                        }
                    }
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
            match &result {
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
            match result {
                Ok(_) => {
                    let mut tasks = vec![
                        gui_ops::sync_shims_for_kind(kind),
                        gui_ops::refresh_runtimes(kind),
                    ];
                    tasks.push(env_center_jvm_check_task(kind));
                    tasks.push(maybe_start_queued_jobs(state));
                    Task::batch(tasks)
                }
                Err(_) => Task::batch([
                    gui_ops::refresh_runtimes(kind),
                    maybe_start_queued_jobs(state),
                ]),
            }
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
            let kind = state.env_center.kind;
            if envr_domain::jvm_hosted::is_jvm_hosted_runtime(
                envr_domain::runtime::runtime_descriptor(kind).key,
            ) {
                Task::batch([
                    gui_ops::refresh_runtimes(kind),
                    env_center_jvm_check_task(kind),
                ])
            } else {
                gui_ops::refresh_runtimes(kind)
            }
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
        EnvCenterMsg::SetNodeDownloadSource(src) => {
            persist_runtime_settings_update(state, move |st| {
                st.runtime.node.download_source = src;
            })
        }
        EnvCenterMsg::SetNpmRegistryMode(mode) => {
            persist_runtime_settings_update(state, move |st| {
                st.runtime.node.npm_registry_mode = mode;
            })
        }
        EnvCenterMsg::SetNodePathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Node,
            on,
            |st, on| st.runtime.node.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetPythonDownloadSource(src) => {
            persist_runtime_settings_update(state, move |st| {
                st.runtime.python.download_source = src;
            })
        }
        EnvCenterMsg::SetPipRegistryMode(mode) => {
            persist_runtime_settings_update(state, move |st| {
                st.runtime.python.pip_registry_mode = mode;
            })
        }
        EnvCenterMsg::SetPythonPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Python,
            on,
            |st, on| st.runtime.python.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetJavaDistro(distro) => {
            state.env_center.remote_error = None;
            persist_runtime_settings_update(state, move |st| {
                st.runtime.java.current_distro = distro;
            })
        }
        EnvCenterMsg::SetJavaDownloadSource(src) => {
            persist_runtime_settings_update(state, move |st| {
                st.runtime.java.download_source = src;
            })
        }
        EnvCenterMsg::SetJavaPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Java,
            on,
            |st, on| st.runtime.java.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetJvmPathProxy(kind, on) => persist_jvm_path_proxy_toggle(state, kind, on),
        EnvCenterMsg::SetTerraformPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Terraform,
            on,
            |st, on| st.runtime.terraform.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetVPathProxy(on) => {
            persist_path_proxy_toggle(state, envr_domain::runtime::RuntimeKind::V, on, |st, on| {
                st.runtime.v.path_proxy_enabled = on
            })
        }
        EnvCenterMsg::SetOdinPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Odin,
            on,
            |st, on| st.runtime.odin.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetPurescriptPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Purescript,
            on,
            |st, on| st.runtime.purescript.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetElmPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Elm,
            on,
            |st, on| st.runtime.elm.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetGleamPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Gleam,
            on,
            |st, on| st.runtime.gleam.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetRacketPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Racket,
            on,
            |st, on| st.runtime.racket.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetDartPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Dart,
            on,
            |st, on| st.runtime.dart.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetFlutterPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Flutter,
            on,
            |st, on| st.runtime.flutter.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetGoDownloadSource(src) => {
            persist_runtime_settings_update(state, move |st| {
                st.runtime.go.download_source = src;
            })
        }
        EnvCenterMsg::SetGoProxyMode(mode) => {
            let draft_proxy = state.env_center.go_proxy_custom_draft.trim().to_string();
            persist_runtime_settings_update(state, move |st| {
                st.runtime.go.proxy_mode = mode;
                if mode == envr_config::settings::GoProxyMode::Custom {
                    // Prevent validation failure when the user switches to Custom
                    // before applying text input.
                    let existing = st.runtime.go.proxy_custom.as_deref().unwrap_or("").trim();
                    let legacy = st.runtime.go.goproxy.as_deref().unwrap_or("").trim();
                    let chosen = if !draft_proxy.is_empty() {
                        draft_proxy
                    } else if !existing.is_empty() {
                        existing.to_string()
                    } else if !legacy.is_empty() {
                        legacy.to_string()
                    } else {
                        "https://proxy.golang.org,direct".to_string()
                    };
                    st.runtime.go.proxy_custom = Some(chosen);
                }
            })
        }
        EnvCenterMsg::SetGoPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Go,
            on,
            |st, on| st.runtime.go.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetGoProxyCustomDraft(s) => {
            state.env_center.go_proxy_custom_draft = s;
            Task::none()
        }
        EnvCenterMsg::SetGoPrivatePatternsDraft(s) => {
            state.env_center.go_private_patterns_draft = s;
            Task::none()
        }
        EnvCenterMsg::ApplyGoNetworkSettings => {
            let p = state.env_center.go_proxy_custom_draft.trim().to_string();
            let pr = state
                .env_center
                .go_private_patterns_draft
                .trim()
                .to_string();
            persist_runtime_settings_update(state, move |st| {
                st.runtime.go.proxy_custom = if p.is_empty() { None } else { Some(p) };
                st.runtime.go.private_patterns = if pr.is_empty() { None } else { Some(pr) };
            })
        }
        EnvCenterMsg::SetRustDownloadSource(src) => {
            persist_runtime_settings_update(state, move |st| {
                st.runtime.rust.download_source = src;
            })
        }
        EnvCenterMsg::SetPhpDownloadSource(src) => {
            mark_unified_major_rows_dirty_for_kind(state, envr_domain::runtime::RuntimeKind::Php);
            Task::batch([
                gui_ops::invalidate_unified_list_disk_cache(envr_domain::runtime::RuntimeKind::Php),
                persist_runtime_settings_update(state, move |st| {
                    st.runtime.php.download_source = src;
                }),
            ])
        }
        EnvCenterMsg::SetPhpWindowsBuild(flavor) => {
            mark_unified_major_rows_dirty_for_kind(state, envr_domain::runtime::RuntimeKind::Php);
            Task::batch([
                gui_ops::invalidate_unified_list_disk_cache(envr_domain::runtime::RuntimeKind::Php),
                persist_runtime_settings_update(state, move |st| {
                    st.runtime.php.windows_build = flavor;
                }),
            ])
        }
        EnvCenterMsg::SetPhpPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Php,
            on,
            |st, on| st.runtime.php.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetDenoDownloadSource(src) => {
            persist_runtime_settings_update(state, move |st| {
                st.runtime.deno.download_source = src;
            })
        }
        EnvCenterMsg::SetDenoPackageSource(mode) => {
            persist_runtime_settings_update(state, move |st| {
                st.runtime.deno.package_source = mode;
            })
        }
        EnvCenterMsg::SetDenoPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Deno,
            on,
            |st, on| st.runtime.deno.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetBunPackageSource(mode) => {
            persist_runtime_settings_update(state, move |st| {
                st.runtime.bun.package_source = mode;
            })
        }
        EnvCenterMsg::SetBunPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Bun,
            on,
            |st, on| st.runtime.bun.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetDotnetPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Dotnet,
            on,
            |st, on| st.runtime.dotnet.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetZigPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Zig,
            on,
            |st, on| st.runtime.zig.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetJuliaPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Julia,
            on,
            |st, on| st.runtime.julia.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetJanetPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Janet,
            on,
            |st, on| st.runtime.janet.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetC3PathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::C3,
            on,
            |st, on| st.runtime.c3.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetBabashkaPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Babashka,
            on,
            |st, on| st.runtime.babashka.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetSbclPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Sbcl,
            on,
            |st, on| st.runtime.sbcl.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetHaxePathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Haxe,
            on,
            |st, on| st.runtime.haxe.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetLuaPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Lua,
            on,
            |st, on| st.runtime.lua.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetNimPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Nim,
            on,
            |st, on| st.runtime.nim.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetCrystalPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Crystal,
            on,
            |st, on| st.runtime.crystal.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetPerlPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Perl,
            on,
            |st, on| st.runtime.perl.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetUnisonPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Unison,
            on,
            |st, on| st.runtime.unison.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetRLangPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::RLang,
            on,
            |st, on| st.runtime.r.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetRubyPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Ruby,
            on,
            |st, on| st.runtime.ruby.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetElixirPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Elixir,
            on,
            |st, on| st.runtime.elixir.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetErlangPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Erlang,
            on,
            |st, on| st.runtime.erlang.path_proxy_enabled = on,
        ),
        EnvCenterMsg::BunGlobalBinDirEdit(s) => {
            state.env_center.bun_global_bin_dir_draft = s;
            Task::none()
        }
        EnvCenterMsg::ApplyBunGlobalBinDir => {
            let t = state.env_center.bun_global_bin_dir_draft.trim().to_string();
            persist_runtime_settings_update(state, move |st| {
                st.runtime.bun.global_bin_dir = if t.is_empty() { None } else { Some(t) };
            })
        }
        EnvCenterMsg::RustRefresh => {
            state.env_center.busy = true;
            state.env_center.remote_error = None;
            state.env_center.rust_status = None;
            state.env_center.rust_components.clear();
            state.env_center.rust_targets.clear();
            Task::batch([
                gui_ops::rust_refresh(),
                gui_ops::rust_load_components(),
                gui_ops::rust_load_targets(),
            ])
        }
        EnvCenterMsg::RustStatusLoaded(res) => {
            state.env_center.busy = false;
            match res {
                Ok(s) => {
                    state.env_center.remote_error = None;
                    state.env_center.rust_status = Some(s);
                }
                Err(e) => {
                    state.env_center.remote_error = Some(e);
                }
            }
            Task::none()
        }
        EnvCenterMsg::RustSelectTab(tab) => {
            state.env_center.rust_tab = tab;
            Task::none()
        }
        EnvCenterMsg::RustComponentsLoaded(res) => {
            if let Ok(list) = res {
                state.env_center.rust_components = list;
            }
            Task::none()
        }
        EnvCenterMsg::RustTargetsLoaded(res) => {
            if let Ok(list) = res {
                state.env_center.rust_targets = list;
            }
            Task::none()
        }
        EnvCenterMsg::RustChannelInstallOrSwitch(channel) => {
            if state.env_center.busy {
                return Task::none();
            }
            state.env_center.busy = true;
            state.error = None;
            let label = rust_runtime_task_label("channel", &channel);
            let (id, _downloaded, _total, _cancel) = enqueue_op_job_running(state, label, false);
            state.env_center.op_job_id = Some(id);
            gui_ops::rust_channel_install_or_switch(channel)
        }
        EnvCenterMsg::RustUpdateCurrent => {
            if state.env_center.busy {
                return Task::none();
            }
            state.env_center.busy = true;
            state.error = None;
            let label = rust_runtime_task_label("update", "");
            let (id, _downloaded, _total, _cancel) = enqueue_op_job_running(state, label, false);
            state.env_center.op_job_id = Some(id);
            gui_ops::rust_update_current()
        }
        EnvCenterMsg::RustManagedInstallStable => {
            if state.env_center.busy {
                return Task::none();
            }
            state.env_center.busy = true;
            state.error = None;
            let label = envr_core::i18n::tr_key(
                "gui.runtime.rust.managed_install_task",
                "正在安装托管 rustup（stable）…",
                "Installing managed rustup (stable)…",
            );
            let (id, downloaded, total, cancel) = enqueue_op_job_running(state, label, true);
            state.env_center.op_job_id = Some(id);
            gui_ops::rust_managed_install_stable(downloaded, total, cancel)
        }
        EnvCenterMsg::RustManagedUninstall => {
            if state.env_center.busy {
                return Task::none();
            }
            state.env_center.busy = true;
            state.error = None;
            let label = rust_runtime_task_label("managed_uninstall", "");
            let (id, _downloaded, _total, _cancel) = enqueue_op_job_running(state, label, false);
            state.env_center.op_job_id = Some(id);
            gui_ops::rust_managed_uninstall()
        }
        EnvCenterMsg::RustComponentToggle(name, install) => {
            if state.env_center.busy {
                return Task::none();
            }
            state.env_center.busy = true;
            state.error = None;
            let label = if install {
                rust_runtime_task_label("component_install", &name)
            } else {
                rust_runtime_task_label("component_uninstall", &name)
            };
            let (id, _downloaded, _total, _cancel) = enqueue_op_job_running(state, label, false);
            state.env_center.op_job_id = Some(id);
            gui_ops::rust_component_toggle(name, install)
        }
        EnvCenterMsg::RustTargetToggle(name, install) => {
            if state.env_center.busy {
                return Task::none();
            }
            state.env_center.busy = true;
            state.error = None;
            let label = if install {
                rust_runtime_task_label("target_install", &name)
            } else {
                rust_runtime_task_label("target_uninstall", &name)
            };
            let (id, _downloaded, _total, _cancel) = enqueue_op_job_running(state, label, false);
            state.env_center.op_job_id = Some(id);
            gui_ops::rust_target_toggle(name, install)
        }
        EnvCenterMsg::RustOpFinished(res) => {
            state.env_center.busy = false;
            if let Some(id) = state.env_center.op_job_id.take()
                && let Some(j) = state.downloads.jobs.iter_mut().find(|j| j.id == id)
            {
                match &res {
                    Ok(()) => {
                        j.state = JobState::Done;
                        let d = j.downloaded.load(Ordering::Relaxed);
                        let t = j.total.load(Ordering::Relaxed);
                        if t == 0 || d < t {
                            j.total.store(d.max(t), Ordering::Relaxed);
                            j.downloaded.store(d.max(t), Ordering::Relaxed);
                        }
                    }
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
            if let Err(e) = &res
                && !looks_like_user_cancelled(e)
            {
                state.error = Some(e.clone());
            }
            Task::batch([
                gui_ops::rust_refresh(),
                gui_ops::rust_load_components(),
                gui_ops::rust_load_targets(),
            ])
        }
        EnvCenterMsg::SyncShimsFinished(res) => {
            if let Err(e) = res {
                state.error = Some(e);
            }
            Task::none()
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
