use iced::widget::{button, column, row, text, text_input};
use iced::{Alignment, Element, Length};

use envr_domain::runtime::RuntimeKind;
use envr_ui::theme::ThemeTokens;

use crate::app::Message;
use crate::view::runtime_settings::state::RuntimeSettingsState;

#[derive(Debug, Clone)]
pub enum RuntimeSettingsMsg {
    ToggleExpand,
    ReloadDisk,
    GoGoproxyEdit(String),
    BunGlobalBinDirEdit(String),
    Save,
}

pub fn runtime_settings_view(
    state: &RuntimeSettingsState,
    active_kind: RuntimeKind,
    tokens: ThemeTokens,
) -> Element<'static, Message> {
    let header = row![
        text(envr_core::i18n::tr(
            "运行时设置（默认折叠）",
            "Runtime settings (collapsed by default)"
        ))
        .size(15),
        iced::widget::horizontal_space(),
        button(text(if state.expanded {
            envr_core::i18n::tr("折叠", "Collapse")
        } else {
            envr_core::i18n::tr("展开", "Expand")
        }))
        .on_press(Message::RuntimeSettings(RuntimeSettingsMsg::ToggleExpand))
        .padding([6, 10]),
    ]
    .align_y(Alignment::Center)
    .spacing(10);

    if !state.expanded {
        return column![header]
            .spacing(tokens.content_spacing().round() as u16)
            .into();
    }

    let mut body = column![header].spacing(tokens.content_spacing().round() as u16);
    let note = text(envr_core::i18n::tr(
        "这些设置写入 settings.toml，并会影响 CLI/GUI 的运行时行为（例如 shim 同步、环境注入等）。",
        "These settings are saved to settings.toml and affect runtime behavior (shims/env injection, etc.).",
    ))
    .size(12);
    body = body.push(note);

    match active_kind {
        RuntimeKind::Go => {
            body = body.push(text(envr_core::i18n::tr("Go", "Go")).size(16));
            body = body.push(
                text_input("runtime.go.goproxy", &state.go_goproxy_draft)
                    .on_input(|s| Message::RuntimeSettings(RuntimeSettingsMsg::GoGoproxyEdit(s)))
                    .padding(8)
                    .width(Length::Fill),
            );
            body = body.push(
                text(envr_core::i18n::tr(
                    "留空表示不注入 GOPROXY；非空则会在 envr env/run/exec 作用域内注入。",
                    "Leave empty to not inject GOPROXY; otherwise injected in envr env/run/exec scope.",
                ))
                .size(12),
            );
        }
        RuntimeKind::Bun => {
            body = body.push(text(envr_core::i18n::tr("Bun", "Bun")).size(16));
            body = body.push(
                text_input(
                    "runtime.bun.global_bin_dir",
                    &state.bun_global_bin_dir_draft,
                )
                .on_input(|s| Message::RuntimeSettings(RuntimeSettingsMsg::BunGlobalBinDirEdit(s)))
                .padding(8)
                .width(Length::Fill),
            );
            body = body.push(
                text(envr_core::i18n::tr(
                    "可选：覆盖 `bun pm bin -g` 的结果，用于 shim 同步全局 Bun 可执行文件。",
                    "Optional: overrides `bun pm bin -g` result for syncing global Bun executables.",
                ))
                .size(12),
            );
        }
        _ => {
            body = body.push(text(envr_core::i18n::tr(
                "该运行时暂无专属设置项。",
                "No runtime-specific settings yet.",
            )));
        }
    }

    let status = match state.last_message.as_ref() {
        Some(m) => text(m.clone()).size(12),
        None => text("").size(1),
    };

    let actions = row![
        button(text(envr_core::i18n::tr("保存", "Save")))
            .on_press(Message::RuntimeSettings(RuntimeSettingsMsg::Save))
            .padding([8, 12]),
        button(text(envr_core::i18n::tr(
            "从磁盘重新加载",
            "Reload from disk"
        )))
        .on_press(Message::RuntimeSettings(RuntimeSettingsMsg::ReloadDisk))
        .padding([8, 12]),
    ]
    .spacing(10);

    body = body.push(actions).push(status);
    body.into()
}
