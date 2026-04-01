//! Main-window shell: left navigation, routed content, global error banner.

use envr_ui::theme::{ThemeTokens, UiFlavor, default_flavor_for_target, tokens_for};
use iced::widget::{button, column, container, horizontal_space, row, scrollable, text};
use iced::{Alignment, Element, Length, Padding, Task, Theme, application};

use crate::gui_ops;
use crate::theme as gui_theme;
use crate::view::env_center::{EnvCenterMsg, EnvCenterState, env_center_view};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Route {
    #[default]
    Dashboard,
    Runtime,
    Settings,
    About,
}

impl Route {
    const ALL: [Self; 4] = [
        Route::Dashboard,
        Route::Runtime,
        Route::Settings,
        Route::About,
    ];

    fn label(self) -> &'static str {
        match self {
            Route::Dashboard => "仪表盘",
            Route::Runtime => "运行时",
            Route::Settings => "设置",
            Route::About => "关于",
        }
    }
}

#[derive(Debug)]
pub struct AppState {
    route: Route,
    error: Option<String>,
    /// Active skin; user can override the OS default on the Settings page.
    flavor: UiFlavor,
    pub env_center: EnvCenterState,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            route: Route::default(),
            error: None,
            flavor: default_flavor_for_target(),
            env_center: EnvCenterState::default(),
        }
    }
}

impl AppState {
    fn tokens(&self) -> ThemeTokens {
        tokens_for(self.flavor)
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    Navigate(Route),
    DismissError,
    ReportError(String),
    SetFlavor(UiFlavor),
    EnvCenter(EnvCenterMsg),
}

pub fn run() -> iced::Result {
    application("Envr", update, view)
        .theme(|state| gui_theme::iced_theme(state.tokens()))
        .centered()
        .window_size((960.0, 640.0))
        .run()
}

fn update(state: &mut AppState, message: Message) -> Task<Message> {
    match message {
        Message::Navigate(route) => {
            tracing::debug!(?route, "navigate");
            state.route = route;
            if route == Route::Runtime {
                return gui_ops::refresh_runtimes(state.env_center.kind);
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
    }
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
    let t = state.tokens();
    let bg = gui_theme::to_color(t.colors.background);

    let body = row![
        sidebar(state.route, t),
        container(page_body(state, t))
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(Padding::from(t.content_spacing())),
    ]
    .spacing(t.content_spacing().round() as u16)
    .height(Length::Fill);

    let chrome = if let Some(err) = state.error.as_deref() {
        column![error_banner(t, err), body].spacing(8)
    } else {
        column![body]
    };

    container(chrome)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(Padding::from(t.content_spacing()))
        .style(move |_theme: &Theme| container::Style::default().background(bg))
        .into()
}

fn error_banner(tokens: ThemeTokens, message: &str) -> Element<'_, Message> {
    let style = gui_theme::error_banner_style(tokens);
    container(
        row![
            text(message).size(14),
            horizontal_space(),
            button(text("关闭")).on_press(Message::DismissError),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .padding(12)
    .style(move |_theme: &Theme| style)
    .into()
}

fn sidebar(current: Route, tokens: ThemeTokens) -> Element<'static, Message> {
    let panel = gui_theme::panel_container_style(tokens);
    let mut col = column![].spacing(8);
    for route in Route::ALL {
        let b = button(text(route.label()))
            .on_press(Message::Navigate(route))
            .width(Length::Fill);
        let b = if route == current {
            b.style(button::primary)
        } else {
            b.style(button::secondary)
        };
        col = col.push(b);
    }
    container(col.width(Length::Fixed(tokens.sidebar_width())))
        .padding(10)
        .style(move |theme: &Theme| panel(theme))
        .into()
}

fn page_body(state: &AppState, tokens: ThemeTokens) -> Element<'_, Message> {
    let title = text(state.route.label()).size(22);

    let mut col = column![title].spacing(14);

    match state.route {
        Route::Runtime => {
            col = col.push(env_center_view(&state.env_center, tokens));
        }
        _ => {
            let blurb: &'static str = match state.route {
                Route::Dashboard => "总览与快捷入口（占位）。",
                Route::Settings => "镜像、路径、行为与外观（占位）。",
                Route::About => "关于本应用。",
                Route::Runtime => unreachable!(),
            };
            col = col.push(text(blurb).size(15));
        }
    }

    if state.route == Route::Settings {
        col = col.push(text("视觉风格").size(17));
        col = col.push(flavor_picker_row(state.flavor));
        col = col.push(
            text(format!(
                "当前：{} · 圆角 md {:.1} · 阴影 blur {:.0} · 动效 {} ms",
                state.flavor.label_zh(),
                tokens.radius_md,
                tokens.shadow.blur_radius,
                tokens.motion.standard_ms
            ))
            .size(13),
        );
    }

    if state.route == Route::About {
        col = col.push(
            button(text("触发全局错误示例")).on_press(Message::ReportError(
                "示例：后台任务失败时可经此通道提示用户。".into(),
            )),
        );
    }

    scrollable(col)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn flavor_picker_row(active: UiFlavor) -> Element<'static, Message> {
    let mut r = row![].spacing(8);
    for flavor in UiFlavor::ALL {
        let b = button(text(flavor.label_zh()))
            .on_press(Message::SetFlavor(flavor))
            .padding([8, 10]);
        let b = if flavor == active {
            b.style(button::primary)
        } else {
            b.style(button::secondary)
        };
        r = r.push(b);
    }
    r.into()
}
