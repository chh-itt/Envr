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
use iced::{Element, Task, application};

use crate::download_runner;
use crate::gui_ops;
use crate::theme as gui_theme;
use crate::view::downloads::{DownloadJob, DownloadMsg, DownloadPanelState, JobState};
use crate::view::env_center::{EnvCenterMsg, EnvCenterState};
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
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            route: Route::default(),
            error: None,
            flavor: default_flavor_for_target(),
            env_center: EnvCenterState::default(),
            downloads: DownloadPanelState::default(),
            settings: SettingsViewState::new(),
        }
    }
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
    Download(DownloadMsg),
    Settings(SettingsMsg),
}

pub fn run() -> iced::Result {
    init_i18n();
    application("Envr", update, view)
        .default_font(configured_default_font())
        .theme(|state| gui_theme::iced_theme(state.tokens()))
        .subscription(|_state| {
            iced::time::every(Duration::from_millis(400))
                .map(|_| Message::Download(DownloadMsg::Tick))
        })
        .centered()
        .window_size((960.0, 640.0))
        .run()
}

fn init_i18n() {
    let paths = match envr_platform::paths::current_platform_paths() {
        Ok(v) => v,
        Err(_) => return,
    };
    let settings_path = envr_config::settings::settings_path_from_platform(&paths);
    let st = Settings::load_or_default_from(&settings_path).unwrap_or_default();
    envr_core::i18n::init_from_settings(&st);
}

fn configured_default_font() -> iced::Font {
    let paths = match envr_platform::paths::current_platform_paths() {
        Ok(v) => v,
        Err(_) => {
            return iced::Font::with_name(font::preferred_system_sans_family());
        }
    };

    let settings_path = envr_config::settings::settings_path_from_platform(&paths);
    let st = Settings::load_or_default_from(&settings_path).unwrap_or_default();

    match st.appearance.font.mode {
        FontMode::Auto => iced::Font::with_name(font::preferred_system_sans_family()),
        FontMode::Custom => {
            let fam = st
                .appearance
                .font
                .family
                .unwrap_or_else(|| font::preferred_system_sans_family().to_string());
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
            if route == Route::Settings
                && let Err(e) = state.settings.reload_from_disk()
            {
                state.error = Some(format!(
                    "{}: {e}",
                    envr_core::i18n::tr("设置加载失败", "Failed to load settings")
                ));
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
        Message::Download(msg) => handle_download(state, msg),
        Message::Settings(msg) => handle_settings(state, msg),
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
            state.settings.last_message = None;
            match state.settings.save() {
                Ok(()) => {
                    state.settings.last_message =
                        Some(envr_core::i18n::tr("已保存到 settings.toml。", "Saved.").into());
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
        SettingsMsg::ReloadDisk => {
            state.settings.last_message = None;
            match state.settings.reload_from_disk() {
                Ok(()) => {
                    state.settings.last_message = Some(
                        envr_core::i18n::tr("已从磁盘重新加载。", "Reloaded from disk.").into(),
                    );
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
    }
}

fn handle_download(state: &mut AppState, msg: DownloadMsg) -> Task<Message> {
    match msg {
        DownloadMsg::Tick => {
            state.downloads.on_tick();
            Task::none()
        }
        DownloadMsg::ToggleExpand => {
            state.downloads.expanded = !state.downloads.expanded;
            Task::none()
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
            retry_download(state, &url_str, &format!("{label} (重试)"))
        }
    }
}

fn enqueue_demo_download(state: &mut AppState) -> Task<Message> {
    retry_download(
        state,
        download_runner::DEMO_URL,
        &format!("演示 #{}", state.downloads.next_id),
    )
}

fn retry_download(state: &mut AppState, url_str: &str, label: &str) -> Task<Message> {
    let url = match reqwest::Url::parse(url_str) {
        Ok(u) => u,
        Err(e) => {
            state.error = Some(format!("URL 解析失败: {e}"));
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
            state.env_center.kind = k;
            gui_ops::refresh_runtimes(k)
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
                state.error = Some("请输入版本 spec".into());
                return Task::none();
            }
            state.env_center.busy = true;
            state.error = None;
            gui_ops::install_version(state.env_center.kind, spec)
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
