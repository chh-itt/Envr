use envr_domain::runtime::RuntimeKind;
use envr_ui::theme::ThemeTokens;
use iced::widget::{button, column, container, row, text};
use iced::{Alignment, Element, Length};

use crate::app::Message;
use crate::theme as gui_theme;
use crate::view::env_center::kind_label;
use crate::view::settings::{SettingsMsg, SettingsViewState};
use crate::widget_styles::{ButtonVariant, button_content_centered, button_style, section_card};

pub fn runtime_config_view(
    state: &SettingsViewState,
    active_kind: RuntimeKind,
    tokens: ThemeTokens,
) -> Element<'static, Message> {
    let sp = tokens.space();
    let ty = tokens.typography();
    let content = column![
        text(envr_core::i18n::tr_key(
            "gui.runtime.config.simplified.title",
            "该页面暂时精简",
            "This page is temporarily simplified",
        ))
        .size(ty.subsection),
        text(envr_core::i18n::tr_key(
            "gui.runtime.config.simplified.desc",
            "运行时高级配置（registry/proxy/bin 等）先作为未来功能保留，不在当前版本开放。",
            "Advanced runtime settings (registry/proxy/bin, etc.) are deferred for a future version.",
        ))
        .size(ty.body_small)
        .color(gui_theme::to_color(tokens.colors.text_muted)),
        text(envr_core::i18n::tr_key(
            "gui.runtime.config.simplified.path_proxy_hint",
            "当前每个运行时仅保留「PATH 代理」开关，请在对应运行时页面中调整。",
            "For now, only the PATH proxy toggle is kept per runtime; adjust it from the runtime page.",
        ))
        .size(ty.body_small)
        .color(gui_theme::to_color(tokens.colors.text_muted)),
    ]
    .spacing(sp.sm as f32);

    let actions = row![
        button(button_content_centered(
            text(envr_core::i18n::tr_key("gui.action.save", "保存", "Save")).into()
        ))
        .on_press(Message::Settings(SettingsMsg::Save))
        .style(button_style(tokens, ButtonVariant::Primary)),
        button(button_content_centered(
            text(envr_core::i18n::tr_key(
                "gui.settings.reload_disk",
                "从磁盘重新加载",
                "Reload from disk",
            ))
            .into()
        ))
        .on_press(Message::Settings(SettingsMsg::ReloadDisk))
        .style(button_style(tokens, ButtonVariant::Secondary)),
    ]
    .spacing(sp.sm as f32)
    .align_y(Alignment::Center);

    let card = section_card(
        tokens,
        format!(
            "{} · {}",
            envr_core::i18n::tr_key(
                "gui.runtime.config.title",
                "运行时配置",
                "Runtime configuration"
            ),
            kind_label(active_kind)
        ),
        column![
            content,
            actions,
            text(state.last_message.clone().unwrap_or_default())
                .size(ty.micro)
                .color(gui_theme::to_color(tokens.colors.text_muted))
        ]
        .spacing(sp.md as f32)
        .into(),
    );
    container(card).width(Length::Fill).into()
}
