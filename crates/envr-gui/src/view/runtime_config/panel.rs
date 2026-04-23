use envr_config::settings::{
    GoDownloadSource, GoProxyMode, NodeDownloadSource, NpmRegistryMode, PipRegistryMode,
    PythonDownloadSource,
};
use envr_domain::runtime::RuntimeKind;
use envr_ui::theme::ThemeTokens;
use iced::widget::{button, column, container, row, text, text_input};
use iced::{Alignment, Element, Length};

use crate::app::Message;
use crate::view::settings::{SettingsMsg, SettingsViewState};
use crate::widget_styles::{ButtonVariant, button_content_centered, button_style, section_card};

pub fn runtime_config_view(
    state: &SettingsViewState,
    active_kind: RuntimeKind,
    tokens: ThemeTokens,
) -> Element<'static, Message> {
    let sp = tokens.space();
    let ty = tokens.typography();

    let mut content = column![].spacing(sp.md as f32);
    match active_kind {
        RuntimeKind::Node => {
            content = content
                .push(text("Node download_source").size(ty.caption))
                .push(enum_row3(
                    tokens,
                    state.draft.runtime.node.download_source == NodeDownloadSource::Auto,
                    state.draft.runtime.node.download_source == NodeDownloadSource::Domestic,
                    state.draft.runtime.node.download_source == NodeDownloadSource::Official,
                    Message::Settings(SettingsMsg::SetNodeDownloadSource(NodeDownloadSource::Auto)),
                    Message::Settings(SettingsMsg::SetNodeDownloadSource(
                        NodeDownloadSource::Domestic,
                    )),
                    Message::Settings(SettingsMsg::SetNodeDownloadSource(
                        NodeDownloadSource::Official,
                    )),
                ))
                .push(text("Node npm_registry_mode").size(ty.caption))
                .push(enum_row4(
                    tokens,
                    state.draft.runtime.node.npm_registry_mode == NpmRegistryMode::Auto,
                    state.draft.runtime.node.npm_registry_mode == NpmRegistryMode::Domestic,
                    state.draft.runtime.node.npm_registry_mode == NpmRegistryMode::Official,
                    state.draft.runtime.node.npm_registry_mode == NpmRegistryMode::Restore,
                    Message::Settings(SettingsMsg::SetNpmRegistryMode(NpmRegistryMode::Auto)),
                    Message::Settings(SettingsMsg::SetNpmRegistryMode(NpmRegistryMode::Domestic)),
                    Message::Settings(SettingsMsg::SetNpmRegistryMode(NpmRegistryMode::Official)),
                    Message::Settings(SettingsMsg::SetNpmRegistryMode(NpmRegistryMode::Restore)),
                ));
        }
        RuntimeKind::Python => {
            content = content
                .push(text("Python download_source").size(ty.caption))
                .push(enum_row3(
                    tokens,
                    state.draft.runtime.python.download_source == PythonDownloadSource::Auto,
                    state.draft.runtime.python.download_source == PythonDownloadSource::Domestic,
                    state.draft.runtime.python.download_source == PythonDownloadSource::Official,
                    Message::Settings(SettingsMsg::SetPythonDownloadSource(
                        PythonDownloadSource::Auto,
                    )),
                    Message::Settings(SettingsMsg::SetPythonDownloadSource(
                        PythonDownloadSource::Domestic,
                    )),
                    Message::Settings(SettingsMsg::SetPythonDownloadSource(
                        PythonDownloadSource::Official,
                    )),
                ))
                .push(text("Python pip_registry_mode").size(ty.caption))
                .push(enum_row4(
                    tokens,
                    state.draft.runtime.python.pip_registry_mode == PipRegistryMode::Auto,
                    state.draft.runtime.python.pip_registry_mode == PipRegistryMode::Domestic,
                    state.draft.runtime.python.pip_registry_mode == PipRegistryMode::Official,
                    state.draft.runtime.python.pip_registry_mode == PipRegistryMode::Restore,
                    Message::Settings(SettingsMsg::SetPipRegistryMode(PipRegistryMode::Auto)),
                    Message::Settings(SettingsMsg::SetPipRegistryMode(PipRegistryMode::Domestic)),
                    Message::Settings(SettingsMsg::SetPipRegistryMode(PipRegistryMode::Official)),
                    Message::Settings(SettingsMsg::SetPipRegistryMode(PipRegistryMode::Restore)),
                ));
        }
        RuntimeKind::Go => {
            content = content
                .push(text("Go download_source").size(ty.caption))
                .push(enum_row3(
                    tokens,
                    state.draft.runtime.go.download_source == GoDownloadSource::Auto,
                    state.draft.runtime.go.download_source == GoDownloadSource::Domestic,
                    state.draft.runtime.go.download_source == GoDownloadSource::Official,
                    Message::Settings(SettingsMsg::SetGoDownloadSource(GoDownloadSource::Auto)),
                    Message::Settings(SettingsMsg::SetGoDownloadSource(GoDownloadSource::Domestic)),
                    Message::Settings(SettingsMsg::SetGoDownloadSource(GoDownloadSource::Official)),
                ))
                .push(text("Go proxy_mode").size(ty.caption))
                .push(enum_row4(
                    tokens,
                    state.draft.runtime.go.proxy_mode == GoProxyMode::Auto,
                    state.draft.runtime.go.proxy_mode == GoProxyMode::Domestic,
                    state.draft.runtime.go.proxy_mode == GoProxyMode::Official,
                    state.draft.runtime.go.proxy_mode == GoProxyMode::Custom,
                    Message::Settings(SettingsMsg::SetGoProxyMode(GoProxyMode::Auto)),
                    Message::Settings(SettingsMsg::SetGoProxyMode(GoProxyMode::Domestic)),
                    Message::Settings(SettingsMsg::SetGoProxyMode(GoProxyMode::Official)),
                    Message::Settings(SettingsMsg::SetGoProxyMode(GoProxyMode::Custom)),
                ))
                .push(text("Go proxy_custom").size(ty.caption))
                .push(
                    text_input("runtime.go.proxy_custom", &state.go_proxy_custom_draft)
                        .on_input(|s| Message::Settings(SettingsMsg::GoProxyCustomEdit(s)))
                        .padding(sp.sm)
                        .width(Length::Fill),
                )
                .push(text("Go private_patterns").size(ty.caption))
                .push(
                    text_input("runtime.go.private_patterns", &state.go_private_patterns_draft)
                        .on_input(|s| Message::Settings(SettingsMsg::GoPrivatePatternsEdit(s)))
                        .padding(sp.sm)
                        .width(Length::Fill),
                );
        }
        RuntimeKind::Bun => {
            content = content.push(text("Bun global_bin_dir").size(ty.caption)).push(
                text_input("runtime.bun.global_bin_dir", &state.bun_global_bin_dir_draft)
                    .on_input(|s| Message::Settings(SettingsMsg::BunGlobalBinDirEdit(s)))
                    .padding(sp.sm)
                    .width(Length::Fill),
            );
        }
        _ => {
            content = content.push(text(envr_core::i18n::tr_key(
                "gui.runtime.config.todo",
                "该运行时的专属配置页正在迁移中。",
                "This runtime-specific config page is being migrated.",
            )));
        }
    }

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
        envr_core::i18n::tr_key(
            "gui.runtime.config.title",
            "运行时配置",
            "Runtime configuration",
        ),
        column![
            content,
            actions,
            text(state.last_message.clone().unwrap_or_default()).size(ty.micro)
        ]
        .spacing(sp.md as f32)
        .into(),
    );
    container(card).width(Length::Fill).into()
}

fn enum_row3(
    tokens: ThemeTokens,
    a_selected: bool,
    b_selected: bool,
    c_selected: bool,
    a_msg: Message,
    b_msg: Message,
    c_msg: Message,
) -> Element<'static, Message> {
    row![
        option_btn(tokens, "auto", a_selected, a_msg),
        option_btn(tokens, "domestic", b_selected, b_msg),
        option_btn(tokens, "official", c_selected, c_msg),
    ]
    .spacing(tokens.space().sm as f32)
    .into()
}

#[allow(clippy::too_many_arguments)]
fn enum_row4(
    tokens: ThemeTokens,
    a_selected: bool,
    b_selected: bool,
    c_selected: bool,
    d_selected: bool,
    a_msg: Message,
    b_msg: Message,
    c_msg: Message,
    d_msg: Message,
) -> Element<'static, Message> {
    row![
        option_btn(tokens, "auto", a_selected, a_msg),
        option_btn(tokens, "domestic", b_selected, b_msg),
        option_btn(tokens, "official", c_selected, c_msg),
        option_btn(tokens, "restore/custom", d_selected, d_msg),
    ]
    .spacing(tokens.space().sm as f32)
    .into()
}

fn option_btn(tokens: ThemeTokens, label: &'static str, selected: bool, msg: Message) -> Element<'static, Message> {
    let variant = if selected {
        ButtonVariant::Primary
    } else {
        ButtonVariant::Secondary
    };
    button(button_content_centered(text(label).into()))
        .on_press(msg)
        .style(button_style(tokens, variant))
        .into()
}
