use iced::widget::{container, scrollable, text};
use iced::{Alignment, Element, Length, Padding, Theme};

use envr_ui::theme::ThemeTokens;

use crate::app::{AppState, Message, Route};
use crate::theme as gui_theme;
use crate::view::dashboard::dashboard_view;
use crate::view::downloads::floating_download_panel;
use crate::view::env_center::env_center_view;
use crate::view::runtime_nav::runtime_nav_bar;
use crate::view::runtime_settings::runtime_settings_view;
use crate::view::settings::settings_view;

use iced::widget::{button, column, horizontal_space, row, stack, vertical_space};

pub fn app_view(state: &AppState) -> Element<'_, Message> {
    let t = state.tokens();
    let bg = gui_theme::to_color(t.colors.background);

    let main_row = row![
        crate::view::sidebar::sidebar(state.route(), t),
        container(page_body(state, t))
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(Padding::from(t.content_spacing())),
    ]
    .spacing(t.content_spacing().round() as u16)
    .height(Length::Fill);

    let body = column![main_row].spacing(8).height(Length::Fill);

    let chrome = if let Some(err) = state.error_message() {
        column![error_banner(t, err), body].spacing(8)
    } else {
        column![body]
    };

    let x = state.downloads.x.max(0) as f32;
    let y = state.downloads.y.max(0) as f32;
    let floating = container(column![
        vertical_space().height(Length::Fill),
        row![
            container(horizontal_space().width(Length::Fixed(x))).width(Length::Fixed(x)),
            floating_download_panel(&state.downloads, t),
        ],
        container(vertical_space().height(Length::Fixed(y))).height(Length::Fixed(y)),
    ])
    .width(Length::Fill)
    .height(Length::Fill);

    container(stack![chrome, floating])
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
            button(text(envr_core::i18n::tr("关闭", "Close"))).on_press(Message::DismissError),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    )
    .padding(12)
    .style(move |_theme: &Theme| style)
    .into()
}

fn page_body(state: &AppState, tokens: ThemeTokens) -> Element<'_, Message> {
    if state.route() == Route::Dashboard {
        return dashboard_view(&state.dashboard, &state.downloads, tokens);
    }

    let title = text(state.route().label()).size(22);
    let mut col = column![title].spacing(14);

    match state.route() {
        Route::Runtime => {
            col = col.push(runtime_nav_bar(
                state.env_center.kind,
                state.env_center.busy,
                tokens,
            ));
            col = col.push(
                button(text(envr_core::i18n::tr(
                    "刷新当前运行时",
                    "Refresh current runtime",
                )))
                .on_press_maybe((!state.env_center.busy).then_some(Message::EnvCenter(
                    crate::view::env_center::EnvCenterMsg::Refresh,
                )))
                .padding([6, 12]),
            );
            col = col.push(runtime_settings_view(
                &state.runtime_settings,
                state.env_center.kind,
                tokens,
            ));
            col = col.push(env_center_view(&state.env_center, tokens));
        }
        Route::Settings => {
            col = col.push(settings_view(&state.settings, tokens));
            col = col.push(text(envr_core::i18n::tr("外观", "Appearance")).size(17));
            col = col.push(flavor_picker_row(state.flavor()));
            col = col.push(
                text(format!(
                    "{} {} · {} md {:.1} · {} blur {:.0} · {} {} ms",
                    envr_core::i18n::tr("当前：", "Current:"),
                    state.flavor().label_zh(),
                    envr_core::i18n::tr("圆角", "Radius"),
                    tokens.radius_md,
                    envr_core::i18n::tr("阴影", "Shadow"),
                    tokens.shadow.blur_radius,
                    envr_core::i18n::tr("动效", "Motion"),
                    tokens.motion.standard_ms
                ))
                .size(13),
            );
        }
        Route::Dashboard => unreachable!("handled by early return"),
        Route::About => {
            col = col.push(text(envr_core::i18n::tr("关于本应用。", "About this app.")).size(15));
        }
    }

    if state.route() == Route::About {
        col = col.push(
            button(text(envr_core::i18n::tr(
                "触发全局错误示例",
                "Trigger global error (demo)",
            )))
            .on_press(Message::ReportError(
                envr_core::i18n::tr(
                    "示例：后台任务失败时可经此通道提示用户。",
                    "Demo: background task failures can be surfaced here.",
                )
                .into(),
            )),
        );
    }

    scrollable(col)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn flavor_picker_row(active: envr_ui::theme::UiFlavor) -> Element<'static, Message> {
    let mut r = row![].spacing(8);
    for flavor in envr_ui::theme::UiFlavor::ALL {
        let b = button(text(envr_core::i18n::tr(
            flavor.label_zh(),
            flavor.label_en(),
        )))
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
