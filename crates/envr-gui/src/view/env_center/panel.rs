//! Node / Python / Java / Go env center: lists, install, use, uninstall via `RuntimeService`.

use envr_domain::runtime::{RuntimeKind, RuntimeVersion};
use envr_ui::theme::ThemeTokens;
use iced::widget::{button, column, row, scrollable, text, text_input};
use iced::{Element, Length};

use crate::app::Message;

#[derive(Debug, Clone)]
pub enum EnvCenterMsg {
    PickKind(RuntimeKind),
    InstallInput(String),
    Refresh,
    DataLoaded(Result<(Vec<RuntimeVersion>, Option<RuntimeVersion>), String>),
    SubmitInstall,
    InstallFinished(Result<RuntimeVersion, String>),
    SubmitUse(String),
    UseFinished(Result<(), String>),
    SubmitUninstall(String),
    UninstallFinished(Result<(), String>),
}

#[derive(Debug)]
pub struct EnvCenterState {
    pub kind: RuntimeKind,
    pub install_input: String,
    pub installed: Vec<RuntimeVersion>,
    pub current: Option<RuntimeVersion>,
    pub busy: bool,
}

impl Default for EnvCenterState {
    fn default() -> Self {
        Self {
            kind: RuntimeKind::Node,
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
        Some(v) => format!("{} {}", envr_core::i18n::tr("????:", "Current:"), v.0),
        None => envr_core::i18n::tr("????: (???)", "Current: (not set)").to_string(),
    };

    let install_row = row![
        text_input(
            envr_core::i18n::tr(
                "?? spec?? CLI install ???? 20 / lts?",
                "Version spec (same as CLI install, e.g. 20 / lts)",
            ),
            &state.install_input
        )
        .on_input(|s| Message::EnvCenter(EnvCenterMsg::InstallInput(s)))
        .width(Length::Fill)
        .padding(8),
        button(text(envr_core::i18n::tr("??", "Install")))
            .on_press_maybe(
                (!busy && !state.install_input.trim().is_empty())
                    .then_some(Message::EnvCenter(EnvCenterMsg::SubmitInstall)),
            )
            .padding([8, 14]),
    ]
    .spacing(10)
    .align_y(iced::Alignment::Center);

    let mut list_col = column![].spacing(6);
    if state.installed.is_empty() {
        list_col = list_col.push(
            text(envr_core::i18n::tr(
                "???????????????",
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
                button(text(envr_core::i18n::tr("??", "Use"))).on_press_maybe(use_msg),
                button(text(envr_core::i18n::tr("??", "Uninstall"))).on_press_maybe(uninstall_msg),
            ]
            .spacing(10)
            .align_y(iced::Alignment::Center);

            let note = if active {
                Some(text(envr_core::i18n::tr("????", "(current)")).size(13))
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
            envr_core::i18n::tr("?????", "Runtime"),
            kind_label(state.kind)
        ))
        .size(16),
        text(cur_line).size(14),
        text(envr_core::i18n::tr("???", "Install")).size(16),
        install_row,
        text(envr_core::i18n::tr("?????", "Installed")).size(16),
        scrollable(list_col).height(Length::Fixed(260.0)),
    ]
    .spacing(tokens.content_spacing().round().max(8.0) as u16);

    body.into()
}
