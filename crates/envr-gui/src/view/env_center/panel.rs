//! Node / Python / Java / Go env center: lists, install, use, uninstall via `RuntimeService`.

use envr_domain::runtime::{RuntimeKind, RuntimeVersion};
use envr_ui::theme::ThemeTokens;
use iced::widget::{button, column, row, scrollable, text, text_input};
use iced::{Element, Length};

use crate::app::Message;

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
        text(envr_core::i18n::tr_key("gui.runtime.mode", "模式", "Mode")).size(14)
    ]
    .spacing(8);
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
        let b = button(text(envr_core::i18n::tr_key(key, zh, en)))
            .on_press(Message::EnvCenter(EnvCenterMsg::SetMode(m)))
            .padding([6, 10]);
        let b = if m == state.mode {
            b.style(button::primary)
        } else {
            b.style(button::secondary)
        };
        mode_row = mode_row.push(b);
    }

    let input = state.install_input.trim();
    let installed_match = state.installed.iter().any(|v| v.0 == input);
    let current_match = state.current.as_ref().is_some_and(|c| c.0 == input);

    let install_row = row![
        text_input(
            &envr_core::i18n::tr_key(
                "gui.runtime.spec_placeholder",
                "版本 spec（Smart）或精确版本（Exact）",
                "Version spec (Smart) or exact version (Exact)",
            ),
            &state.install_input
        )
        .on_input(|s| Message::EnvCenter(EnvCenterMsg::InstallInput(s)))
        .width(Length::Fill)
        .padding(8),
        button(text(envr_core::i18n::tr_key(
            "gui.action.install",
            "安装",
            "Install",
        )))
        .on_press_maybe(
            (!busy
                && !input.is_empty()
                && (state.mode == VersionMode::Smart || !installed_match))
                .then_some(Message::EnvCenter(EnvCenterMsg::SubmitInstall)),
        )
        .padding([8, 14]),
        button(text(envr_core::i18n::tr_key(
            "gui.action.install_use",
            "安装并切换",
            "Install & Use",
        )))
        .on_press_maybe(
            (state.mode == VersionMode::Smart && !busy && !input.is_empty() && !installed_match)
                .then_some(Message::EnvCenter(EnvCenterMsg::SubmitInstallAndUse)),
        )
        .padding([8, 14]),
        button(text(envr_core::i18n::tr_key("gui.action.use", "切换", "Use")))
        .on_press_maybe(
            (!busy && !input.is_empty() && installed_match && !current_match)
                .then_some(Message::EnvCenter(EnvCenterMsg::SubmitUse(input.to_string()))),
        )
        .padding([8, 14]),
    ]
    .spacing(10)
    .align_y(iced::Alignment::Center);

    let mut list_col = column![].spacing(6);
    if state.installed.is_empty() {
        list_col = list_col.push(
            text(envr_core::i18n::tr_key(
                "gui.runtime.none_installed",
                "暂无已安装版本（点击刷新）。",
                "(No installed versions. Click Refresh.)",
            ))
            .size(14),
        );
    } else {
        for ver in &state.installed {
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

            let row = row![
                text(format!("{} {}", kind_label(state.kind), ver.0)).width(Length::Fill),
                button(text(envr_core::i18n::tr_key("gui.action.use", "切换", "Use")))
                    .on_press_maybe(use_msg),
                button(text(envr_core::i18n::tr_key(
                    "gui.action.uninstall",
                    "卸载",
                    "Uninstall",
                )))
                .on_press_maybe(uninstall_msg),
            ]
            .spacing(10)
            .align_y(iced::Alignment::Center);

            let note = if active {
                Some(
                    text(envr_core::i18n::tr_key(
                        "gui.runtime.current_tag",
                        "(当前)",
                        "(current)",
                    ))
                    .size(13),
                )
            } else {
                None
            };

            if let Some(t) = note {
                list_col = list_col.push(column![row, t].spacing(4));
            } else {
                list_col = list_col.push(row);
            }
        }
    }

    let body = column![
        text(format!(
            "{}: {}",
            envr_core::i18n::tr_key("gui.runtime.title", "运行时", "Runtime"),
            kind_label(state.kind)
        ))
        .size(16),
        mode_row,
        text(cur_line).size(14),
        text(envr_core::i18n::tr_key("gui.action.install", "安装", "Install")).size(16),
        install_row,
        text(envr_core::i18n::tr_key(
            "gui.runtime.installed",
            "已安装",
            "Installed",
        ))
        .size(16),
        scrollable(list_col).height(Length::Fixed(260.0)),
    ]
    .spacing(tokens.content_spacing().round().max(8.0) as u16);

    body.into()
}

