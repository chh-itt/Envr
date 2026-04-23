use envr_config::settings::{
    GoDownloadSource, GoProxyMode, NodeDownloadSource, NpmRegistryMode, PipRegistryMode,
    PythonDownloadSource,
};
use envr_domain::runtime::RuntimeKind;
use envr_ui::theme::ThemeTokens;
use iced::widget::{button, column, container, row, text, text_input};
use iced::{Alignment, Element, Length};

use crate::app::Message;
use crate::theme as gui_theme;
use crate::view::env_center::kind_label;
use crate::view::settings::{SettingsMsg, SettingsViewState};
use crate::widget_styles::{
    ButtonVariant, SegmentPosition, button_content_centered, button_style, section_card,
    segmented_button_style, setting_row, text_input_style,
};

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
                .push(setting_row(
                    tokens,
                    envr_core::i18n::tr_key(
                        "gui.runtime.config.node.download_source",
                        "下载源",
                        "Download source",
                    ),
                    None,
                    segmented3(
                        tokens,
                        state.draft.runtime.node.download_source == NodeDownloadSource::Auto,
                        state.draft.runtime.node.download_source == NodeDownloadSource::Domestic,
                        state.draft.runtime.node.download_source == NodeDownloadSource::Official,
                        envr_core::i18n::tr_key("gui.choice.auto", "自动", "Auto"),
                        envr_core::i18n::tr_key("gui.choice.domestic", "国内", "Domestic"),
                        envr_core::i18n::tr_key("gui.choice.official", "官方", "Official"),
                        Message::Settings(SettingsMsg::SetNodeDownloadSource(
                            NodeDownloadSource::Auto,
                        )),
                        Message::Settings(SettingsMsg::SetNodeDownloadSource(
                            NodeDownloadSource::Domestic,
                        )),
                        Message::Settings(SettingsMsg::SetNodeDownloadSource(
                            NodeDownloadSource::Official,
                        )),
                    ),
                ))
                .push(setting_row(
                    tokens,
                    "NPM registry".to_string(),
                    None,
                    segmented4(
                        tokens,
                        state.draft.runtime.node.npm_registry_mode == NpmRegistryMode::Auto,
                        state.draft.runtime.node.npm_registry_mode == NpmRegistryMode::Domestic,
                        state.draft.runtime.node.npm_registry_mode == NpmRegistryMode::Official,
                        state.draft.runtime.node.npm_registry_mode == NpmRegistryMode::Restore,
                        envr_core::i18n::tr_key("gui.choice.auto", "自动", "Auto"),
                        envr_core::i18n::tr_key("gui.choice.domestic", "国内", "Domestic"),
                        envr_core::i18n::tr_key("gui.choice.official", "官方", "Official"),
                        envr_core::i18n::tr_key("gui.choice.restore", "恢复默认", "Restore"),
                        Message::Settings(SettingsMsg::SetNpmRegistryMode(NpmRegistryMode::Auto)),
                        Message::Settings(SettingsMsg::SetNpmRegistryMode(
                            NpmRegistryMode::Domestic,
                        )),
                        Message::Settings(SettingsMsg::SetNpmRegistryMode(
                            NpmRegistryMode::Official,
                        )),
                        Message::Settings(SettingsMsg::SetNpmRegistryMode(NpmRegistryMode::Restore)),
                    ),
                ));
        }
        RuntimeKind::Python => {
            content = content
                .push(setting_row(
                    tokens,
                    envr_core::i18n::tr_key(
                        "gui.runtime.config.python.download_source",
                        "下载源",
                        "Download source",
                    ),
                    None,
                    segmented3(
                        tokens,
                        state.draft.runtime.python.download_source == PythonDownloadSource::Auto,
                        state.draft.runtime.python.download_source
                            == PythonDownloadSource::Domestic,
                        state.draft.runtime.python.download_source
                            == PythonDownloadSource::Official,
                        envr_core::i18n::tr_key("gui.choice.auto", "自动", "Auto"),
                        envr_core::i18n::tr_key("gui.choice.domestic", "国内", "Domestic"),
                        envr_core::i18n::tr_key("gui.choice.official", "官方", "Official"),
                        Message::Settings(SettingsMsg::SetPythonDownloadSource(
                            PythonDownloadSource::Auto,
                        )),
                        Message::Settings(SettingsMsg::SetPythonDownloadSource(
                            PythonDownloadSource::Domestic,
                        )),
                        Message::Settings(SettingsMsg::SetPythonDownloadSource(
                            PythonDownloadSource::Official,
                        )),
                    ),
                ))
                .push(setting_row(
                    tokens,
                    "PIP index".to_string(),
                    None,
                    segmented4(
                        tokens,
                        state.draft.runtime.python.pip_registry_mode == PipRegistryMode::Auto,
                        state.draft.runtime.python.pip_registry_mode == PipRegistryMode::Domestic,
                        state.draft.runtime.python.pip_registry_mode == PipRegistryMode::Official,
                        state.draft.runtime.python.pip_registry_mode == PipRegistryMode::Restore,
                        envr_core::i18n::tr_key("gui.choice.auto", "自动", "Auto"),
                        envr_core::i18n::tr_key("gui.choice.domestic", "国内", "Domestic"),
                        envr_core::i18n::tr_key("gui.choice.official", "官方", "Official"),
                        envr_core::i18n::tr_key("gui.choice.restore", "恢复默认", "Restore"),
                        Message::Settings(SettingsMsg::SetPipRegistryMode(PipRegistryMode::Auto)),
                        Message::Settings(SettingsMsg::SetPipRegistryMode(
                            PipRegistryMode::Domestic,
                        )),
                        Message::Settings(SettingsMsg::SetPipRegistryMode(
                            PipRegistryMode::Official,
                        )),
                        Message::Settings(SettingsMsg::SetPipRegistryMode(PipRegistryMode::Restore)),
                    ),
                ));
        }
        RuntimeKind::Go => {
            content = content
                .push(setting_row(
                    tokens,
                    envr_core::i18n::tr_key(
                        "gui.runtime.config.go.download_source",
                        "下载源",
                        "Download source",
                    ),
                    None,
                    segmented3(
                        tokens,
                        state.draft.runtime.go.download_source == GoDownloadSource::Auto,
                        state.draft.runtime.go.download_source == GoDownloadSource::Domestic,
                        state.draft.runtime.go.download_source == GoDownloadSource::Official,
                        envr_core::i18n::tr_key("gui.choice.auto", "自动", "Auto"),
                        envr_core::i18n::tr_key("gui.choice.domestic", "国内", "Domestic"),
                        envr_core::i18n::tr_key("gui.choice.official", "官方", "Official"),
                        Message::Settings(SettingsMsg::SetGoDownloadSource(
                            GoDownloadSource::Auto,
                        )),
                        Message::Settings(SettingsMsg::SetGoDownloadSource(
                            GoDownloadSource::Domestic,
                        )),
                        Message::Settings(SettingsMsg::SetGoDownloadSource(
                            GoDownloadSource::Official,
                        )),
                    ),
                ))
                .push(setting_row(
                    tokens,
                    "GOPROXY".to_string(),
                    None,
                    segmented4(
                        tokens,
                        state.draft.runtime.go.proxy_mode == GoProxyMode::Auto,
                        state.draft.runtime.go.proxy_mode == GoProxyMode::Domestic,
                        state.draft.runtime.go.proxy_mode == GoProxyMode::Official,
                        state.draft.runtime.go.proxy_mode == GoProxyMode::Custom,
                        envr_core::i18n::tr_key("gui.choice.auto", "自动", "Auto"),
                        envr_core::i18n::tr_key("gui.choice.domestic", "国内", "Domestic"),
                        envr_core::i18n::tr_key("gui.choice.official", "官方", "Official"),
                        envr_core::i18n::tr_key("gui.choice.custom", "自定义", "Custom"),
                        Message::Settings(SettingsMsg::SetGoProxyMode(GoProxyMode::Auto)),
                        Message::Settings(SettingsMsg::SetGoProxyMode(GoProxyMode::Domestic)),
                        Message::Settings(SettingsMsg::SetGoProxyMode(GoProxyMode::Official)),
                        Message::Settings(SettingsMsg::SetGoProxyMode(GoProxyMode::Custom)),
                    ),
                ))
                .push(setting_row(
                    tokens,
                    envr_core::i18n::tr_key(
                        "gui.runtime.config.go.proxy_custom",
                        "自定义 GOPROXY",
                        "Custom GOPROXY",
                    ),
                    None,
                    text_input("runtime.go.proxy_custom", &state.go_proxy_custom_draft)
                        .on_input(|s| Message::Settings(SettingsMsg::GoProxyCustomEdit(s)))
                        .padding(sp.sm)
                        .width(Length::Fixed(360.0))
                        .style(text_input_style(tokens))
                        .into(),
                ))
                .push(setting_row(
                    tokens,
                    "GOPRIVATE".to_string(),
                    Some(envr_core::i18n::tr_key(
                        "gui.runtime.config.go.private_hint",
                        "逗号分隔私有模块前缀，例如：github.com/your-org/*",
                        "Comma-separated private module patterns, e.g. github.com/your-org/*",
                    )),
                    text_input("runtime.go.private_patterns", &state.go_private_patterns_draft)
                        .on_input(|s| Message::Settings(SettingsMsg::GoPrivatePatternsEdit(s)))
                        .padding(sp.sm)
                        .width(Length::Fixed(360.0))
                        .style(text_input_style(tokens))
                        .into(),
                ));
        }
        RuntimeKind::Bun => {
            content = content.push(setting_row(
                tokens,
                envr_core::i18n::tr_key(
                    "gui.runtime.config.bun.global_bin_dir",
                    "全局 bin 目录",
                    "Global bin directory",
                ),
                Some(envr_core::i18n::tr_key(
                    "gui.runtime.config.bun.global_bin_hint",
                    "可选：覆盖 `bun pm bin -g` 检测结果",
                    "Optional: override detected `bun pm bin -g` path",
                )),
                text_input("runtime.bun.global_bin_dir", &state.bun_global_bin_dir_draft)
                    .on_input(|s| Message::Settings(SettingsMsg::BunGlobalBinDirEdit(s)))
                    .padding(sp.sm)
                    .width(Length::Fixed(360.0))
                    .style(text_input_style(tokens))
                    .into(),
            ));
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
        format!(
            "{} · {}",
            envr_core::i18n::tr_key("gui.runtime.config.title", "运行时配置", "Runtime configuration"),
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

fn segmented3(
    tokens: ThemeTokens,
    a_selected: bool,
    b_selected: bool,
    c_selected: bool,
    a_label: String,
    b_label: String,
    c_label: String,
    a_msg: Message,
    b_msg: Message,
    c_msg: Message,
) -> Element<'static, Message> {
    row![
        seg_btn(tokens, a_label, a_selected, SegmentPosition::Start, a_msg),
        seg_btn(tokens, b_label, b_selected, SegmentPosition::Middle, b_msg),
        seg_btn(tokens, c_label, c_selected, SegmentPosition::End, c_msg),
    ]
    .spacing(-1.0)
    .into()
}

#[allow(clippy::too_many_arguments)]
fn segmented4(
    tokens: ThemeTokens,
    a_selected: bool,
    b_selected: bool,
    c_selected: bool,
    d_selected: bool,
    a_label: String,
    b_label: String,
    c_label: String,
    d_label: String,
    a_msg: Message,
    b_msg: Message,
    c_msg: Message,
    d_msg: Message,
) -> Element<'static, Message> {
    row![
        seg_btn(tokens, a_label, a_selected, SegmentPosition::Start, a_msg),
        seg_btn(tokens, b_label, b_selected, SegmentPosition::Middle, b_msg),
        seg_btn(tokens, c_label, c_selected, SegmentPosition::Middle, c_msg),
        seg_btn(tokens, d_label, d_selected, SegmentPosition::End, d_msg),
    ]
    .spacing(-1.0)
    .into()
}

fn seg_btn(
    tokens: ThemeTokens,
    label: String,
    selected: bool,
    pos: SegmentPosition,
    msg: Message,
) -> Element<'static, Message> {
    let variant = if selected {
        ButtonVariant::Primary
    } else {
        ButtonVariant::Secondary
    };
    button(button_content_centered(text(label).into()))
        .on_press(msg)
        .height(Length::Fixed(tokens.control_height_secondary))
        .padding([tokens.space().sm as f32, (tokens.space().sm + 2) as f32])
        .style(segmented_button_style(tokens, variant, pos))
        .into()
}
