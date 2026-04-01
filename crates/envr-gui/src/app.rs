//! Main-window shell: left navigation, routed content, global error banner.

use iced::widget::{button, column, container, horizontal_space, row, scrollable, text};
use iced::{Alignment, Element, Length, Theme, application};

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

#[derive(Debug, Default)]
pub struct AppState {
    route: Route,
    error: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Message {
    Navigate(Route),
    DismissError,
    ReportError(String),
}

pub fn run() -> iced::Result {
    application("Envr", update, view)
        .theme(|_| Theme::default())
        .centered()
        .window_size((960.0, 640.0))
        .run()
}

fn update(state: &mut AppState, message: Message) {
    match message {
        Message::Navigate(route) => {
            tracing::debug!(?route, "navigate");
            state.route = route;
        }
        Message::DismissError => state.error = None,
        Message::ReportError(msg) => state.error = Some(msg),
    }
}

fn view(state: &AppState) -> Element<'_, Message> {
    let body = row![
        sidebar(state.route),
        container(page_body(state))
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(16),
    ]
    .spacing(12)
    .height(Length::Fill);

    let chrome = if let Some(err) = state.error.as_deref() {
        column![error_banner(err), body].spacing(8)
    } else {
        column![body]
    };

    container(chrome)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(12)
        .into()
}

fn error_banner(message: &str) -> Element<'_, Message> {
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
    .style(|_theme: &Theme| {
        container::Style::default().background(iced::Color::from_rgb8(255, 228, 225))
    })
    .into()
}

fn sidebar(current: Route) -> Element<'static, Message> {
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
    container(col.width(Length::Fixed(188.0)))
        .padding(10)
        .style(container::rounded_box)
        .into()
}

fn page_body(state: &AppState) -> Element<'static, Message> {
    let title = text(state.route.label()).size(22);
    let blurb: &'static str = match state.route {
        Route::Dashboard => "总览与快捷入口（占位）。",
        Route::Runtime => "运行时与版本列表（占位）。",
        Route::Settings => "镜像、路径与行为（占位）。",
        Route::About => "关于本应用。",
    };

    let mut col = column![title, text(blurb).size(15),].spacing(14);

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
