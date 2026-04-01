//! Main-window shell: left navigation, routed content, global error banner.

use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::time::Duration;

use envr_config::settings::{FontMode, Settings};
use envr_download::task::CancelToken;
use envr_ui::font;
use envr_ui::theme::{
    ThemeTokens, UiFlavor, default_flavor_for_target, scheme_for_mode, tokens_for_scheme,
};
use iced::font::Family;
use iced::{Element, Subscription, Task, application};
use std::path::PathBuf;
use std::sync::OnceLock;

use crate::download_runner;
use crate::gui_ops;
use crate::theme as gui_theme;
use crate::view::dashboard::{DashboardMsg, DashboardState};
use crate::view::downloads::{DownloadJob, DownloadMsg, DownloadPanelState, JobState};
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

    pub(crate) fn label(self) -> &'static str {
        envr_core::i18n::tr(
            match self {
                Route::Dashboard => "仪表盘",
                Route::Runtime => "运行时",
                Route::Settings => "设置",
                Route::About => "关于",
            },
            match self {
                Route::Dashboard => "Dashboard",
                Route::Runtime => "Runtimes",
                Route::Settings => "Settings",
                Route::About => "About",
            },
        )
    }
}

pub struct AppState {
    route: Route,
    error: Option<String>,
    /// Active skin; user can override the OS default on the Settings page.
    flavor: UiFlavor,
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
            flavor: default_flavor_for_target(),
            env_center: EnvCenterState::default(),
            downloads: DownloadPanelState {
                visible: gui_defaults.0,
                expanded: gui_defaults.1,
                x: gui_defaults.2,
                y: gui_defaults.3,
                ..DownloadPanelState::default()
            },
            settings: SettingsViewState::new(),
            dashboard: DashboardState::default(),
            runtime_settings: RuntimeSettingsState::default(),
        }
    }
}

fn load_gui_downloads_panel_settings_cached() -> (bool, bool, i32, i32) {
    let st = STARTUP_SETTINGS.get().cloned().unwrap_or_default();
    let p = st.gui.downloads_panel;
    (p.visible, p.expanded, p.x.max(0), p.y.max(0))
}

impl AppState {
    pub(crate) fn tokens(&self) -> ThemeTokens {
        let scheme = scheme_for_mode(self.settings.draft.appearance.theme_mode);
        tokens_for_scheme(self.flavor, scheme)
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
    let startup = load_startup_settings();
    envr_core::i18n::init_from_settings(&startup);
    application("Envr", update, view)
        .default_font(configured_default_font(&startup))
        .theme(|state| gui_theme::iced_theme(state.tokens()))
        .subscription(|state| {
            let maybe_tick = state
                .downloads
                .needs_tick()
                .then(|| iced::time::every(Duration::from_millis(400)))
                .map(|s| s.map(|_| Message::Download(DownloadMsg::Tick)));

            let maybe_events = state
                .downloads
                .dragging
                .then(|| iced::event::listen().map(|e| Message::Download(DownloadMsg::Event(e))));

            match (maybe_tick, maybe_events) {
                (Some(tick), Some(events)) => Subscription::batch([tick, events]),
                (Some(tick), None) => tick,
                (None, Some(events)) => events,
                (None, None) => Subscription::none(),
            }
        })
        .centered()
        .window_size((960.0, 640.0))
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
        Message::Navigate(route) => {
            tracing::debug!(?route, "navigate");
            state.route = route;
            if route == Route::Runtime {
                return gui_ops::refresh_runtimes(state.env_center.kind);
            }
            if route == Route::Dashboard {
                return gui_ops::refresh_dashboard();
            }
            if route == Route::Settings {
                state.settings.last_message =
                    Some(envr_core::i18n::tr("正在加载…", "Loading…").into());
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
            state.runtime_settings.last_message =
                Some(envr_core::i18n::tr("正在加载…", "Loading…").into());
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
            state.runtime_settings.last_message =
                Some(envr_core::i18n::tr("正在保存…", "Saving…").into());
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
                            envr_core::i18n::tr("同步失败", "Sync failed")
                        ));
                    } else {
                        state.runtime_settings.last_message = None;
                    }
                }
                Err(e) => {
                    state.runtime_settings.last_message = Some(format!(
                        "{}: {e}",
                        envr_core::i18n::tr("重新加载失败", "Reload failed")
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
                            envr_core::i18n::tr("同步失败", "Sync failed")
                        ));
                    } else {
                        state.runtime_settings.last_message =
                            Some(envr_core::i18n::tr("已保存。", "Saved.").into());
                    }
                }
                Err(e) => {
                    state.runtime_settings.last_message = Some(format!(
                        "{}: {e}",
                        envr_core::i18n::tr("保存失败", "Save failed")
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

fn handle_settings(state: &mut AppState, msg: SettingsMsg) -> Task<Message> {
    match msg {
        SettingsMsg::RuntimeRootEdit(s) => {
            state.settings.runtime_root_draft = s;
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
        SettingsMsg::SetLocaleMode(m) => {
            state.settings.locale_mode_draft = m;
            // Apply immediately so all views re-render with new language.
            let mut st = state.settings.draft.clone();
            st.i18n.locale = m;
            envr_core::i18n::init_from_settings(&st);
            Task::none()
        }
        SettingsMsg::Save => {
            state.settings.last_message = Some(envr_core::i18n::tr("正在保存…", "Saving…").into());
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
            state.settings.last_message = Some(envr_core::i18n::tr("正在加载…", "Loading…").into());
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
                            envr_core::i18n::tr("同步失败", "Sync failed")
                        ));
                    } else {
                        state.settings.last_message = Some(
                            envr_core::i18n::tr("已从磁盘重新加载。", "Reloaded from disk.").into(),
                        );
                    }
                }
                Err(e) => {
                    state.settings.last_message = Some(format!(
                        "{}: {e}",
                        envr_core::i18n::tr("重新加载失败", "Reload failed")
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
                            envr_core::i18n::tr("同步失败", "Sync failed")
                        ));
                    } else {
                        state.settings.last_message =
                            Some(envr_core::i18n::tr("已保存到 settings.toml。", "Saved.").into());
                    }
                }
                Err(e) => {
                    state.settings.last_message = Some(format!(
                        "{}: {e}",
                        envr_core::i18n::tr("保存失败", "Save failed")
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

fn handle_download(state: &mut AppState, msg: DownloadMsg) -> Task<Message> {
    match msg {
        DownloadMsg::Tick => {
            state.downloads.on_tick();
            Task::none()
        }
        DownloadMsg::ToggleVisible => {
            state.downloads.visible = !state.downloads.visible;
            let _ = persist_download_panel_settings(&state.downloads);
            Task::none()
        }
        DownloadMsg::ToggleExpand => {
            state.downloads.expanded = !state.downloads.expanded;
            let _ = persist_download_panel_settings(&state.downloads);
            Task::none()
        }
        DownloadMsg::StartDrag => {
            state.downloads.dragging = true;
            state.downloads.drag_from_cursor = None;
            state.downloads.drag_from_pos = Some((state.downloads.x, state.downloads.y));
            Task::none()
        }
        DownloadMsg::Event(e) => {
            use iced::Event;
            use iced::mouse;
            match e {
                Event::Mouse(mouse::Event::CursorMoved { position }) => {
                    if !state.downloads.dragging {
                        return Task::none();
                    }
                    let (cx, cy) = (position.x, position.y);
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
                    Task::none()
                }
                Event::Mouse(mouse::Event::ButtonReleased(_btn)) => {
                    if state.downloads.dragging {
                        state.downloads.dragging = false;
                        state.downloads.drag_from_cursor = None;
                        state.downloads.drag_from_pos = None;
                        let _ = persist_download_panel_settings(&state.downloads);
                    }
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
                    envr_core::i18n::tr("(重试)", "(retry)")
                ),
            )
        }
    }
}

fn persist_download_panel_settings(
    panel: &DownloadPanelState,
) -> Result<(), envr_error::EnvrError> {
    let paths = envr_platform::paths::current_platform_paths()?;
    let settings_path = envr_config::settings::settings_path_from_platform(&paths);
    let mut st = Settings::load_or_default_from(&settings_path).unwrap_or_default();
    st.gui.downloads_panel.visible = panel.visible;
    st.gui.downloads_panel.expanded = panel.expanded;
    st.gui.downloads_panel.x = panel.x.max(0);
    st.gui.downloads_panel.y = panel.y.max(0);
    st.save_to(&settings_path)?;
    Ok(())
}

fn enqueue_demo_download(state: &mut AppState) -> Task<Message> {
    retry_download(
        state,
        download_runner::DEMO_URL,
        &format!(
            "{} #{}",
            envr_core::i18n::tr("演示", "Demo"),
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
                envr_core::i18n::tr("URL 解析失败", "URL parse failed")
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
            gui_ops::refresh_runtimes(k)
        }
        EnvCenterMsg::SetMode(m) => {
            state.env_center.mode = m;
            // Keep input; mode affects button availability only.
            Task::none()
        }
        EnvCenterMsg::InstallInput(s) => {
            state.env_center.install_input = s;
            Task::none()
        }
        EnvCenterMsg::Refresh => gui_ops::refresh_runtimes(state.env_center.kind),
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
        EnvCenterMsg::SubmitInstall => {
            let spec = state.env_center.install_input.trim().to_string();
            if spec.is_empty() {
                state.error = Some(
                    envr_core::i18n::tr("请输入版本 spec", "Please enter a version spec").into(),
                );
                return Task::none();
            }
            state.env_center.busy = true;
            state.error = None;
            gui_ops::install_version(state.env_center.kind, spec)
        }
        EnvCenterMsg::SubmitInstallAndUse => {
            let spec = state.env_center.install_input.trim().to_string();
            if spec.is_empty() {
                state.error = Some(
                    envr_core::i18n::tr("请输入版本 spec", "Please enter a version spec").into(),
                );
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
                    state.env_center.install_input.clear();
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
    }
}

fn view(state: &AppState) -> Element<'_, Message> {
    shell::app_view(state)
}
