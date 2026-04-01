//! Node / Python / Java env center: lists, install, use, uninstall via `RuntimeService`.

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

fn kind_label(kind: RuntimeKind) -> &'static str {
    match kind {
        RuntimeKind::Node => "Node",
        RuntimeKind::Python => "Python",
        RuntimeKind::Java => "Java",
    }
}

pub fn env_center_view(state: &EnvCenterState, tokens: ThemeTokens) -> Element<'static, Message> {
    let busy = state.busy;
    let header = text("环境中心（Node / Python / Java）").size(20);
    let intro = text("安装、切换与卸载与 CLI 共用 envr-core，无重复业务逻辑。").size(14);

    let mut kind_row = row![].spacing(8);
    for kind in [RuntimeKind::Node, RuntimeKind::Python, RuntimeKind::Java] {
        let label = text(kind_label(kind));
        let b = button(label)
            .on_press(Message::EnvCenter(EnvCenterMsg::PickKind(kind)))
            .padding([6, 10]);
        let b = if kind == state.kind {
            b.style(button::primary)
        } else {
            b.style(button::secondary)
        };
        let b = if busy { b.on_press_maybe(None) } else { b };
        kind_row = kind_row.push(b);
    }

    let refresh = button(text("刷新列表"))
        .on_press_maybe((!busy).then_some(Message::EnvCenter(EnvCenterMsg::Refresh)))
        .padding([6, 12]);

    let cur_line = match &state.current {
        Some(v) => format!("当前使用: {}", v.0),
        None => "当前使用: （未设置）".to_string(),
    };

    let install_row = row![
        text_input(
            "版本 spec（与 CLI install 相同，如 20 / lts）",
            &state.install_input
        )
        .on_input(|s| Message::EnvCenter(EnvCenterMsg::InstallInput(s)))
        .width(Length::Fill)
        .padding(8),
        button(text("安装"))
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
        list_col = list_col.push(text("（暂无已安装版本，或点击刷新）").size(14));
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
                Some(Message::EnvCenter(EnvCenterMsg::SubmitUninstall(ver.0.clone())))
            };

            let row = row![
                text(format!("{} {}", kind_label(state.kind), ver.0)).width(Length::Fill),
                button(text("使用")).on_press_maybe(use_msg),
                button(text("卸载")).on_press_maybe(uninstall_msg),
            ]
            .spacing(10)
            .align_y(iced::Alignment::Center);

            let note = if active {
                Some(text("（当前）").size(13))
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
        header,
        intro,
        text("运行时根可在启动前设置环境变量 ENVR_RUNTIME_ROOT，与 CLI 一致。").size(12),
        kind_row,
        row![text(cur_line).size(15), refresh,]
            .spacing(14)
            .align_y(iced::Alignment::Center),
        text("新安装").size(16),
        install_row,
        text("已安装版本").size(16),
        scrollable(list_col).height(Length::Fixed(260.0)),
    ]
    .spacing(tokens.content_spacing().round().max(8.0) as u16);

    body.into()
}
