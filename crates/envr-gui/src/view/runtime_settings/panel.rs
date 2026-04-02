use iced::alignment::Vertical;
use iced::widget::{button, column, container, row, text, text_input};
use iced::{Alignment, Element, Length, Padding, Theme};

use envr_domain::runtime::RuntimeKind;
use envr_ui::theme::ThemeTokens;

use crate::app::Message;
use crate::icons::Lucide;
use crate::theme as gui_theme;
use crate::view::runtime_settings::state::RuntimeSettingsState;
use crate::widget_styles::{
    card_container_style, ButtonVariant, button_content_centered, button_style, text_input_style,
};

#[derive(Debug, Clone)]
pub enum RuntimeSettingsMsg {
    ToggleExpand,
    ReloadDisk,
    GoGoproxyEdit(String),
    BunGlobalBinDirEdit(String),
    Save,
    DiskLoaded(Result<envr_config::settings::Settings, String>),
    DiskSaved(Result<envr_config::settings::Settings, String>),
}

pub fn runtime_settings_view(
    state: &RuntimeSettingsState,
    active_kind: RuntimeKind,
    tokens: ThemeTokens,
) -> Element<'static, Message> {
    let ty = tokens.typography();
    let sp = tokens.space();

    let txt = gui_theme::to_color(tokens.colors.text);
    let expand_lbl = row![
        Lucide::ChevronsUpDown.view(16.0, txt),
        text(if state.expanded {
            envr_core::i18n::tr_key("gui.action.collapse", "折叠", "Collapse")
        } else {
            envr_core::i18n::tr_key("gui.action.expand", "展开", "Expand")
        }),
    ]
    .spacing(sp.xs as f32)
    .align_y(Alignment::Center);

    let expand_btn = button(button_content_centered(expand_lbl.into()))
        .on_press(Message::RuntimeSettings(RuntimeSettingsMsg::ToggleExpand))
        .height(Length::Fixed(tokens.control_height_secondary))
        .padding([sp.sm as f32, (sp.sm + 2) as f32])
        .style(button_style(tokens, ButtonVariant::Secondary));

    let card_title = envr_core::i18n::tr_key(
        "gui.runtime_settings.card_title",
        "运行时设置",
        "Runtime settings",
    );

    let header_row = row![text(card_title).size(ty.section), expand_btn]
        .align_y(Alignment::Center)
        .spacing(sp.sm as f32)
        .width(Length::Fill);

    let pad = tokens.card_padding_px();
    let inset = Padding::from([pad + 4.0, pad + 4.0]);
    let card_s = card_container_style(tokens, 1);

    if !state.expanded {
        return container(header_row)
            .padding(inset)
            .width(Length::Fill)
            .style(move |theme: &Theme| card_s(theme))
            .into();
    }

    let on_prim = gui_theme::contrast_on_primary(tokens);
    let pad_v = sp.sm as f32;
    let mut body = column![header_row].spacing(sp.md as f32);
    let note = text(envr_core::i18n::tr_key(
        "gui.runtime_settings.note",
        "这些设置写入 settings.toml，并会影响 CLI/GUI 的运行时行为（例如 shim 同步、环境注入等）。",
        "These settings are saved to settings.toml and affect runtime behavior (shims/env injection, etc.).",
    ))
    .size(ty.micro);
    body = body.push(note);

    match active_kind {
        RuntimeKind::Go => {
            body = body.push(
                text(envr_core::i18n::tr_key("gui.runtime.lang.go", "Go", "Go")).size(ty.section),
            );
            body = body.push(
                container(
                    text_input("runtime.go.goproxy", &state.go_goproxy_draft)
                        .on_input(|s| {
                            Message::RuntimeSettings(RuntimeSettingsMsg::GoGoproxyEdit(s))
                        })
                        .padding(sp.sm)
                        .width(Length::Fill)
                        .style(text_input_style(tokens)),
                )
                .width(Length::Fill)
                .height(Length::Fixed(tokens.control_height_secondary))
                .align_y(Vertical::Center),
            );
            body = body.push(
                text(envr_core::i18n::tr_key(
                    "gui.runtime_settings.goproxy_hint",
                    "留空表示不注入 GOPROXY；非空则会在 envr env/run/exec 作用域内注入。",
                    "Leave empty to not inject GOPROXY; otherwise injected in envr env/run/exec scope.",
                ))
                .size(ty.micro),
            );
        }
        RuntimeKind::Bun => {
            body = body.push(
                text(envr_core::i18n::tr_key(
                    "gui.runtime.lang.bun",
                    "Bun",
                    "Bun",
                ))
                .size(ty.section),
            );
            body = body.push(
                container(
                    text_input(
                        "runtime.bun.global_bin_dir",
                        &state.bun_global_bin_dir_draft,
                    )
                    .on_input(|s| {
                        Message::RuntimeSettings(RuntimeSettingsMsg::BunGlobalBinDirEdit(s))
                    })
                    .padding(sp.sm)
                    .width(Length::Fill)
                    .style(text_input_style(tokens)),
                )
                .width(Length::Fill)
                .height(Length::Fixed(tokens.control_height_secondary))
                .align_y(Vertical::Center),
            );
            body = body.push(
                text(envr_core::i18n::tr_key(
                    "gui.runtime_settings.bun_bin_hint",
                    "可选：覆盖 `bun pm bin -g` 的结果，用于 shim 同步全局 Bun 可执行文件。",
                    "Optional: overrides `bun pm bin -g` result for syncing global Bun executables.",
                ))
                .size(ty.micro),
            );
        }
        _ => {
            body = body.push(text(envr_core::i18n::tr_key(
                "gui.runtime_settings.no_extra",
                "该运行时暂无专属设置项。",
                "No runtime-specific settings yet.",
            )));
        }
    }

    let status = match state.last_message.as_ref() {
        Some(m) => text(m.clone()).size(ty.micro),
        None => text("").size(1),
    };

    let actions = row![
        button(button_content_centered(
            row![
                Lucide::Settings.view(16.0, on_prim),
                text(envr_core::i18n::tr_key("gui.action.save", "保存", "Save")),
            ]
            .spacing(sp.sm as f32)
            .align_y(Alignment::Center)
            .into(),
        ))
        .on_press(Message::RuntimeSettings(RuntimeSettingsMsg::Save))
        .height(Length::Fixed(tokens.control_height_primary))
        .padding([pad_v, sp.md as f32])
        .style(button_style(tokens, ButtonVariant::Primary)),
        button(button_content_centered(
            row![
                Lucide::RefreshCw.view(16.0, txt),
                text(envr_core::i18n::tr_key(
                    "gui.settings.reload_disk",
                    "从磁盘重新加载",
                    "Reload from disk",
                )),
            ]
            .spacing(sp.sm as f32)
            .align_y(Alignment::Center)
            .into(),
        ))
        .on_press(Message::RuntimeSettings(RuntimeSettingsMsg::ReloadDisk))
        .height(Length::Fixed(tokens.control_height_secondary))
        .padding([pad_v, sp.md as f32])
        .style(button_style(tokens, ButtonVariant::Secondary)),
    ]
    .spacing((sp.sm + 2) as f32);

    body = body.push(actions).push(status);
    container(body)
        .padding(inset)
        .width(Length::Fill)
        .style(move |theme: &Theme| card_s(theme))
        .into()
}
