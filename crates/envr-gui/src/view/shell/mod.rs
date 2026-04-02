use iced::widget::{container, rule, scrollable, space, text};
use iced::{Alignment, Element, Length, Padding, Theme};

use envr_ui::theme::{ThemeTokens, UiFlavor};
use envr_domain::runtime::RuntimeKind;

use crate::app::{AppState, Message, Route};
use crate::icons::Lucide;
use crate::theme as gui_theme;
use crate::view::dashboard::dashboard_view;
use crate::view::downloads::floating_download_panel;
use crate::view::env_center::env_center_view;
use crate::view::runtime_nav::runtime_nav_bar;
use crate::view::runtime_settings::runtime_settings_view;
use crate::view::settings::settings_view;
use crate::widget_styles::{ButtonVariant, button_content_centered, button_style};

use iced::widget::{button, column, row, stack};

pub fn app_view(state: &AppState) -> Element<'_, Message> {
    let t = state.tokens();
    let bg = gui_theme::to_color(t.colors.background);
    let sp = t.space();

    // `spacing`: embed vertical scrollbar in layout so it does not overlay card edges (`GUI` polish).
    let page_scroll = scrollable(if state.route() == Route::Dashboard {
        dashboard_view(&state.dashboard, &state.downloads, t)
    } else {
        page_body(state, t)
    })
    .width(Length::Fill)
    .height(Length::Fill)
    .spacing(sp.sm as f32);

    let page = container(page_scroll)
        .width(Length::Fill)
        .max_width(t.content_max_width())
        .align_x(Alignment::Center);

    let main_row = row![
        crate::view::sidebar::sidebar(state.route(), t),
        rule::vertical(1.0),
        container(page)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(Padding::from(t.content_spacing())),
    ]
    .spacing(0)
    .height(Length::Fill);

    let body = column![main_row].spacing(sp.sm as f32).height(Length::Fill);

    let chrome = if let Some(err) = state.error_message() {
        column![error_banner(t, err), body].spacing(sp.sm as f32)
    } else {
        column![body]
    };

    let x = state.downloads.x.max(0) as f32;
    let y = state.downloads.y.max(0) as f32;
    let floating = container(column![
        space::vertical(),
        row![
            container(space().width(Length::Fixed(x))).width(Length::Fixed(x)),
            floating_download_panel(&state.downloads, t),
        ],
        container(space().height(Length::Fixed(y))).height(Length::Fixed(y)),
    ])
    .width(Length::Fill)
    .height(Length::Fill);

    // `stack` draw order: main chrome first, downloads overlay second → above content, below future modals (`tasks_gui.md` GUI-062).
    container(stack![chrome, floating])
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(Padding::from(t.content_spacing()))
        .style(move |_theme: &Theme| container::Style::default().background(bg))
        .into()
}

fn error_banner(tokens: ThemeTokens, message: &str) -> Element<'_, Message> {
    let style = gui_theme::error_banner_style(tokens);
    let ty = tokens.typography();
    let sp = tokens.space();
    let danger = gui_theme::to_color(tokens.colors.danger);
    let muted = gui_theme::to_color(tokens.colors.text_muted);
    let hint = envr_core::i18n::tr_key(
        "gui.empty.hint.global_error",
        "若问题持续，请重试或导出日志以便反馈。",
        "If this keeps happening, retry or export logs for support.",
    );
    let close_lbl = row![
        Lucide::X.view(14.0, gui_theme::to_color(tokens.colors.text)),
        text(envr_core::i18n::tr_key("gui.action.close", "关闭", "Close",)),
    ]
    .spacing(sp.xs as f32)
    .align_y(Alignment::Center);
    let main_row = row![
        Lucide::CircleAlert.view(20.0, danger),
        column![
            text(message).size(ty.body_small),
            text(hint).size(ty.caption).color(muted),
        ]
        .spacing(sp.xs as f32)
        .width(Length::Fill),
        button(button_content_centered(close_lbl.into()))
            .on_press(Message::DismissError)
            .height(Length::Fixed(
                tokens
                    .control_height_secondary
                    .max(tokens.min_click_target_px()),
            ))
            .padding([sp.sm as f32, sp.sm as f32])
            .style(button_style(tokens, ButtonVariant::Ghost)),
    ]
    .spacing(sp.sm as f32)
    .align_y(Alignment::Center);
    container(main_row)
        .padding(sp.md)
        .style(move |_theme: &Theme| style)
        .into()
}

fn page_body(state: &AppState, tokens: ThemeTokens) -> Element<'_, Message> {
    let ty = tokens.typography();
    let sp = tokens.space();
    let title = text(state.route().label()).size(ty.page_title);
    let mut col = column![title].spacing(tokens.page_title_gap() as f32);

    match state.route() {
        Route::Runtime => {
            col = col.push(runtime_nav_bar(
                state.env_center.kind,
                state.env_center.busy,
                tokens,
            ));
            // Only show the extra per-runtime settings panel when there are
            // actually runtime-specific options to configure.
            if matches!(state.env_center.kind, RuntimeKind::Go | RuntimeKind::Bun) {
                col = col.push(runtime_settings_view(
                    &state.runtime_settings,
                    state.env_center.kind,
                    tokens,
                ));
            }
            col = col.push(env_center_view(
                &state.env_center,
                state.settings.draft.behavior.runtime_install_mode,
                tokens,
            ));
        }
        Route::Settings => {
            col = col.push(settings_view(&state.settings, tokens));
            col = col.push(
                text(envr_core::i18n::tr_key(
                    "gui.label.appearance",
                    "外观",
                    "Appearance",
                ))
                .size(ty.subsection),
            );
            col = col.push(flavor_picker_row(state.flavor(), tokens));
            col = col.push(
                text(format!(
                    "{} {} · {} md {:.1} · {} blur {:.0} · {} {} ms",
                    envr_core::i18n::tr_key("gui.flavor.current", "当前：", "Current:"),
                    flavor_label_i18n(state.flavor()),
                    envr_core::i18n::tr_key("gui.label.radius", "圆角", "Radius"),
                    tokens.radius_md,
                    envr_core::i18n::tr_key("gui.label.shadow", "阴影", "Shadow"),
                    tokens.shadow.blur_radius,
                    envr_core::i18n::tr_key("gui.label.motion", "动效", "Motion"),
                    tokens.motion.standard_ms
                ))
                .size(ty.caption),
            );
        }
        Route::About => {
            let prim = gui_theme::to_color(tokens.colors.primary);
            col = col.push(
                row![
                    Lucide::Info.view(22.0, prim),
                    text(envr_core::i18n::tr_key(
                        "gui.about.description",
                        "关于本应用。",
                        "About this app.",
                    ))
                    .size(ty.body),
                ]
                .spacing(sp.sm as f32)
                .align_y(Alignment::Center),
            );
        }
        Route::Dashboard => unreachable!("handled in app_view"),
    }

    if state.route() == Route::About {
        let warn_icon = gui_theme::to_color(tokens.colors.warning);
        let demo = row![
            Lucide::CircleAlert.view(16.0, warn_icon),
            text(envr_core::i18n::tr_key(
                "gui.about.trigger_error",
                "触发全局错误示例",
                "Trigger global error (demo)",
            )),
        ]
        .spacing(sp.sm as f32)
        .align_y(Alignment::Center);
        col = col.push(
            button(button_content_centered(demo.into()))
                .on_press(Message::ReportError(envr_core::i18n::tr_key(
                    "gui.about.error_demo",
                    "示例：后台任务失败时可经此通道提示用户。",
                    "Demo: background task failures can be surfaced here.",
                )))
                .height(Length::Fixed(
                    tokens
                        .control_height_secondary
                        .max(tokens.min_click_target_px()),
                ))
                .padding([sp.sm as f32, sp.md as f32])
                .style(button_style(tokens, ButtonVariant::Secondary)),
        );
    }

    col.into()
}

fn flavor_label_i18n(flavor: UiFlavor) -> String {
    match flavor {
        UiFlavor::Fluent => {
            envr_core::i18n::tr_key("gui.flavor.fluent", flavor.label_zh(), flavor.label_en())
        }
        UiFlavor::LiquidGlass => envr_core::i18n::tr_key(
            "gui.flavor.liquid_glass",
            flavor.label_zh(),
            flavor.label_en(),
        ),
        UiFlavor::Material3 => {
            envr_core::i18n::tr_key("gui.flavor.material3", flavor.label_zh(), flavor.label_en())
        }
    }
}

fn flavor_picker_row(active: UiFlavor, tokens: ThemeTokens) -> Element<'static, Message> {
    let sp = tokens.space();
    let txt = gui_theme::to_color(tokens.colors.text);
    let mut r = row![].spacing(sp.sm as f32);
    for flavor in UiFlavor::ALL {
        let mut lbl = row![].spacing(sp.xs as f32).align_y(Alignment::Center);
        if flavor == active {
            lbl = lbl.push(Lucide::ChevronsUpDown.view(14.0, txt));
        }
        lbl = lbl.push(text(flavor_label_i18n(flavor)));
        let variant = if flavor == active {
            ButtonVariant::Primary
        } else {
            ButtonVariant::Secondary
        };
        let h = if flavor == active {
            tokens.control_height_primary
        } else {
            tokens.control_height_secondary
        }
        .max(tokens.min_click_target_px());
        let b = button(button_content_centered(lbl.into()))
            .on_press(Message::SetFlavor(flavor))
            .height(Length::Fixed(h))
            .padding([sp.sm as f32, (sp.sm + 2) as f32])
            .style(button_style(tokens, variant));
        r = r.push(b);
    }
    r.into()
}
