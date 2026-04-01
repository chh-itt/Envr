use envr_config::settings::MirrorMode;
use envr_ui::theme::ThemeTokens;
use iced::widget::{button, column, row, text, text_input, toggler};
use iced::{Element, Length};

use crate::app::Message;
use crate::view::settings::state::SettingsViewState;

#[derive(Debug, Clone)]
pub enum SettingsMsg {
    RuntimeRootEdit(String),
    ManualIdEdit(String),
    MaxConcEdit(String),
    RetryEdit(String),
    SetMirrorMode(MirrorMode),
    SetCleanup(bool),
    Save,
    ReloadDisk,
}

pub fn settings_view(state: &SettingsViewState, tokens: ThemeTokens) -> Element<'static, Message> {
    let env_note = if SettingsViewState::env_overrides_runtime_root() {
        text("提示：已设置环境变量 ENVR_RUNTIME_ROOT，将覆盖下方的运行时根与 settings.toml。")
            .size(12)
    } else {
        text("运行时根：留空表示使用平台默认；与 CLI 共用 settings.toml。").size(12)
    };

    let rr = text_input("运行时根目录（可选）", &state.runtime_root_draft)
        .on_input(|s| Message::Settings(SettingsMsg::RuntimeRootEdit(s)))
        .padding(8)
        .width(Length::Fill);

    let mut mirror_row = row![text("镜像策略").size(15),].spacing(8);
    for mode in [
        MirrorMode::Official,
        MirrorMode::Auto,
        MirrorMode::Manual,
        MirrorMode::Offline,
    ] {
        let lab = SettingsViewState::mirror_mode_label(mode);
        let b = button(text(lab))
            .on_press(Message::Settings(SettingsMsg::SetMirrorMode(mode)))
            .padding([6, 8]);
        let b = if mode == state.draft.mirror.mode {
            b.style(button::primary)
        } else {
            b.style(button::secondary)
        };
        mirror_row = mirror_row.push(b);
    }

    let manual =
        if state.draft.mirror.mode == MirrorMode::Manual {
            column![
            text("manual 模式下请填写镜像 ID（与 envr-mirror 预设一致，如 official、cn-1、cn-2）。")
                .size(12),
            text_input("mirror.manual_id", &state.manual_id_draft)
                .on_input(|s| Message::Settings(SettingsMsg::ManualIdEdit(s)))
                .padding(8)
                .width(Length::Fill),
        ]
            .spacing(6)
        } else {
            column![]
        };

    let cleanup = toggler(state.draft.behavior.cleanup_downloads_after_install)
        .label("安装成功后清理下载缓存（供后续运行时实现）")
        .on_toggle(|v| Message::Settings(SettingsMsg::SetCleanup(v)));

    let dl_row = row![
        column![
            text("最大并发下载").size(13),
            text_input("例如 4", &state.max_conc_text)
                .on_input(|s| Message::Settings(SettingsMsg::MaxConcEdit(s)))
                .padding(6),
        ]
        .spacing(4),
        column![
            text("重试次数上限").size(13),
            text_input("例如 3", &state.retry_text)
                .on_input(|s| Message::Settings(SettingsMsg::RetryEdit(s)))
                .padding(6),
        ]
        .spacing(4),
    ]
    .spacing(16);

    let status = match state.last_message.as_ref() {
        Some(m) => text(m.clone()).size(13),
        None => text("").size(1),
    };

    let actions = row![
        button(text("保存到 settings.toml"))
            .on_press(Message::Settings(SettingsMsg::Save))
            .padding([8, 12]),
        button(text("从磁盘重新加载"))
            .on_press(Message::Settings(SettingsMsg::ReloadDisk))
            .padding([8, 12]),
    ]
    .spacing(10);

    column![
        text("设置").size(20),
        env_note,
        rr,
        mirror_row,
        manual,
        cleanup,
        text("下载").size(16),
        dl_row,
        actions,
        status,
    ]
    .spacing(tokens.content_spacing().round() as u16)
    .into()
}
