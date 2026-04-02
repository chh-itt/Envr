//! Node / Python / Java / Go env center: lists, install, use, uninstall via `RuntimeService`.

use envr_domain::runtime::{RuntimeKind, RuntimeVersion};
use envr_ui::theme::ThemeTokens;
use iced::alignment::Horizontal;
use iced::widget::{
    Rule, button, column, container, row, scrollable, text, text_input, vertical_space,
};
use iced::{Alignment, Element, Length, Theme};

use crate::app::Message;
use crate::icons::Lucide;
use crate::theme as gui_theme;
use crate::widget_styles::{ButtonVariant, button_style, text_input_style};

/// Fixed list viewport height (`tasks_gui.md` GUI-021); keeps layout stable vs. skeleton rows.
const ENV_LIST_VIEWPORT_H: f32 = 260.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VersionMode {
    #[default]
    Smart,
    Exact,
}

#[derive(Debug, Clone)]
pub enum EnvCenterMsg {
    PickKind(RuntimeKind),
    SetMode(VersionMode),
    InstallInput(String),
    Refresh,
    DataLoaded(Result<(Vec<RuntimeVersion>, Option<RuntimeVersion>), String>),
    SubmitInstall,
    SubmitInstallAndUse,
    InstallFinished(Result<RuntimeVersion, String>),
    SubmitUse(String),
    UseFinished(Result<(), String>),
    SubmitUninstall(String),
    UninstallFinished(Result<(), String>),
}

#[derive(Debug)]
pub struct EnvCenterState {
    pub kind: RuntimeKind,
    pub mode: VersionMode,
    pub install_input: String,
    pub installed: Vec<RuntimeVersion>,
    pub current: Option<RuntimeVersion>,
    pub busy: bool,
}

impl Default for EnvCenterState {
    fn default() -> Self {
        Self {
            kind: RuntimeKind::Node,
            mode: VersionMode::default(),
            install_input: String::new(),
            installed: Vec::new(),
            current: None,
            busy: false,
        }
    }
}

pub(crate) fn kind_label(kind: RuntimeKind) -> &'static str {
    match kind {
        RuntimeKind::Node => "Node",
        RuntimeKind::Python => "Python",
        RuntimeKind::Java => "Java",
        RuntimeKind::Go => "Go",
        RuntimeKind::Rust => "Rust",
        RuntimeKind::Php => "PHP",
        RuntimeKind::Deno => "Deno",
        RuntimeKind::Bun => "Bun",
    }
}

pub fn env_center_view(state: &EnvCenterState, tokens: ThemeTokens) -> Element<'static, Message> {
    let ty = tokens.typography();
    let sp = tokens.space();
    let busy = state.busy;

    let cur_line = match &state.current {
        Some(v) => format!(
            "{} {}",
            envr_core::i18n::tr_key("gui.runtime.current", "当前：", "Current:"),
            v.0
        ),
        None => envr_core::i18n::tr_key(
            "gui.runtime.current_none",
            "当前：(未设置)",
            "Current: (not set)",
        ),
    };

    let mut mode_row = row![
        text(envr_core::i18n::tr_key(
            "gui.runtime.mode_label",
            "模式",
            "Mode"
        ))
        .size(ty.body_small)
    ]
    .spacing(sp.sm);
    for (m, key, zh, en) in [
        (
            VersionMode::Smart,
            "gui.runtime.mode.smart",
            "智能（Smart）",
            "Smart",
        ),
        (
            VersionMode::Exact,
            "gui.runtime.mode.exact",
            "精确（Exact）",
            "Exact",
        ),
    ] {
        let variant = if m == state.mode {
            ButtonVariant::Primary
        } else {
            ButtonVariant::Secondary
        };
        let h = if m == state.mode {
            tokens.control_height_primary
        } else {
            tokens.control_height_secondary
        };
        let b = button(text(envr_core::i18n::tr_key(key, zh, en)))
            .on_press(Message::EnvCenter(EnvCenterMsg::SetMode(m)))
            .height(Length::Fixed(h))
            .padding([0, sp.sm + 2])
            .style(button_style(tokens, variant));
        mode_row = mode_row.push(b);
    }

    let input = state.install_input.trim();
    let installed_match = state.installed.iter().any(|v| v.0 == input);
    let current_match = state.current.as_ref().is_some_and(|c| c.0 == input);

    let spec_field = container(
        text_input(
            &envr_core::i18n::tr_key(
                "gui.runtime.spec_placeholder",
                "版本 spec（Smart）或精确版本（Exact）",
                "Version spec (Smart) or exact version (Exact)",
            ),
            &state.install_input,
        )
        .on_input(|s| Message::EnvCenter(EnvCenterMsg::InstallInput(s)))
        .width(Length::Fill)
        .padding(sp.sm)
        .style(text_input_style(tokens)),
    )
    .width(Length::Fill)
    .height(Length::Fixed(tokens.control_height_secondary))
    .align_y(iced::alignment::Vertical::Center);

    let txt = gui_theme::to_color(tokens.colors.text);
    let install_row = row![
        spec_field,
        button(
            row![
                Lucide::Download.view(15.0, txt),
                text(envr_core::i18n::tr_key(
                    "gui.action.install",
                    "安装",
                    "Install",
                )),
            ]
            .spacing(sp.xs)
            .align_y(Alignment::Center),
        )
        .on_press_maybe(
            (!busy && !input.is_empty() && (state.mode == VersionMode::Smart || !installed_match))
                .then_some(Message::EnvCenter(EnvCenterMsg::SubmitInstall)),
        )
        .height(Length::Fixed(tokens.control_height_secondary))
        .padding([0, sp.md])
        .style(button_style(tokens, ButtonVariant::Secondary)),
        button(
            row![
                Lucide::Download.view(15.0, txt),
                text(envr_core::i18n::tr_key(
                    "gui.action.install_use",
                    "安装并切换",
                    "Install & Use",
                )),
            ]
            .spacing(sp.xs)
            .align_y(Alignment::Center),
        )
        .on_press_maybe(
            (state.mode == VersionMode::Smart && !busy && !input.is_empty() && !installed_match)
                .then_some(Message::EnvCenter(EnvCenterMsg::SubmitInstallAndUse)),
        )
        .height(Length::Fixed(tokens.control_height_secondary))
        .padding([0, sp.md])
        .style(button_style(tokens, ButtonVariant::Primary)),
        button(
            row![
                Lucide::Package.view(15.0, txt),
                text(envr_core::i18n::tr_key("gui.action.use", "切换", "Use")),
            ]
            .spacing(sp.xs)
            .align_y(Alignment::Center),
        )
        .on_press_maybe(
            (!busy && !input.is_empty() && installed_match && !current_match).then_some(
                Message::EnvCenter(EnvCenterMsg::SubmitUse(input.to_string())),
            ),
        )
        .height(Length::Fixed(tokens.control_height_secondary))
        .padding([0, sp.md])
        .style(button_style(tokens, ButtonVariant::Secondary)),
    ]
    .spacing(sp.sm + 2)
    .align_y(Alignment::Center);

    let list_content: Element<'static, Message> = if busy && state.installed.is_empty() {
        container(list_loading_skeleton(tokens))
            .width(Length::Fill)
            .height(Length::Fixed(ENV_LIST_VIEWPORT_H))
            .into()
    } else if state.installed.is_empty() {
        container(
            container(
                text(envr_core::i18n::tr_key(
                    "gui.runtime.none_installed",
                    "暂无已安装版本（点击刷新）。",
                    "(No installed versions. Click Refresh.)",
                ))
                .size(ty.body_small),
            )
            .align_x(Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center)
            .width(Length::Fill)
            .height(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fixed(ENV_LIST_VIEWPORT_H))
        .into()
    } else {
        let row_h = tokens.list_row_height();
        let n = state.installed.len();
        let mut list_col = column![].spacing(0);
        for (i, ver) in state.installed.iter().enumerate() {
            let active = state.current.as_ref() == Some(ver);
            let use_msg = if active || busy {
                None
            } else {
                Some(Message::EnvCenter(EnvCenterMsg::SubmitUse(ver.0.clone())))
            };
            let uninstall_msg = if active || busy {
                None
            } else {
                Some(Message::EnvCenter(EnvCenterMsg::SubmitUninstall(
                    ver.0.clone(),
                )))
            };

            let ver_line = if active {
                format!(
                    "{} {} {}",
                    kind_label(state.kind),
                    ver.0,
                    envr_core::i18n::tr_key("gui.runtime.current_tag", "(当前)", "(current)",)
                )
            } else {
                format!("{} {}", kind_label(state.kind), ver.0)
            };

            let line = row![
                text(ver_line).width(Length::Fill),
                button(
                    row![
                        Lucide::Package.view(14.0, txt),
                        text(envr_core::i18n::tr_key("gui.action.use", "切换", "Use")),
                    ]
                    .spacing(sp.xs)
                    .align_y(Alignment::Center),
                )
                .on_press_maybe(use_msg)
                .height(Length::Fixed(tokens.control_height_secondary))
                .padding([0, sp.sm])
                .style(button_style(tokens, ButtonVariant::Ghost)),
                button(
                    row![
                        Lucide::X.view(14.0, gui_theme::to_color(tokens.colors.danger)),
                        text(envr_core::i18n::tr_key(
                            "gui.action.uninstall",
                            "卸载",
                            "Uninstall",
                        )),
                    ]
                    .spacing(sp.xs)
                    .align_y(Alignment::Center),
                )
                .on_press_maybe(uninstall_msg)
                .height(Length::Fixed(tokens.control_height_secondary))
                .padding([0, sp.sm])
                .style(button_style(tokens, ButtonVariant::Danger)),
            ]
            .spacing(sp.sm)
            .align_y(Alignment::Center)
            .height(Length::Fixed(row_h));

            list_col = list_col.push(line);
            if i + 1 < n {
                list_col = list_col.push(Rule::horizontal(1));
            }
        }
        scrollable(list_col)
            .width(Length::Fill)
            .height(Length::Fixed(ENV_LIST_VIEWPORT_H))
            .into()
    };

    let body = column![
        text(format!(
            "{}: {}",
            envr_core::i18n::tr_key("gui.runtime.title", "运行时", "Runtime"),
            kind_label(state.kind)
        ))
        .size(ty.section),
        mode_row,
        text(cur_line).size(ty.body_small),
        text(envr_core::i18n::tr_key(
            "gui.action.install",
            "安装",
            "Install"
        ))
        .size(ty.section),
        install_row,
        text(envr_core::i18n::tr_key(
            "gui.runtime.installed",
            "已安装",
            "Installed",
        ))
        .size(ty.section),
        list_content,
    ]
    .spacing(tokens.page_title_gap());

    body.into()
}

fn list_loading_skeleton(tokens: ThemeTokens) -> Element<'static, Message> {
    use iced::Background;

    let fill = gui_theme::to_color(tokens.colors.text_muted).scale_alpha(0.14);
    let row_h = tokens.list_row_height();
    let bar_h = row_h * 0.42;
    let n = tokens.list_skeleton_rows();
    let mut col = column![].spacing(0);
    for i in 0..n {
        col = col.push(
            container(
                vertical_space()
                    .width(Length::Fill)
                    .height(Length::Fixed(bar_h)),
            )
            .width(Length::Fill)
            .height(Length::Fixed(row_h))
            .align_y(iced::alignment::Vertical::Center)
            .padding([0, tokens.space().md as u16])
            .style(move |_theme: &Theme| {
                iced::widget::container::Style::default().background(Background::Color(fill))
            }),
        );
        if i + 1 < n {
            col = col.push(Rule::horizontal(1));
        }
    }
    col.into()
}
