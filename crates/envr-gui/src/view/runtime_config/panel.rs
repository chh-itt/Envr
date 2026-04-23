use envr_config::settings::{GoProxyMode, NpmRegistryMode, PipRegistryMode};
use envr_domain::runtime::RuntimeKind;
use envr_ui::theme::ThemeTokens;
use iced::widget::{button, column, container, pick_list, row, text, text_input, toggler};
use iced::{Alignment, Element, Length};

use crate::app::Message;
use crate::theme as gui_theme;
use crate::view::env_center::kind_label;
use crate::view::settings::{SettingsMsg, SettingsViewState};
use crate::widget_styles::{
    ButtonVariant, SegmentPosition, button_content_centered, button_style, section_card, segmented_button_style,
    setting_row, text_input_style,
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
            let managed = state.draft.runtime.node.npm_registry_mode != NpmRegistryMode::Restore;
            let presets = [
                "",
                "https://registry.npmjs.org/",
                "https://registry.npmmirror.com/",
                "https://mirrors.huaweicloud.com/repository/npm/",
                "https://mirrors.aliyun.com/npm/",
            ];
            content = content
                .push(setting_row(
                    tokens,
                    envr_core::i18n::tr_key("gui.runtime.config.node.npm_enable", "托管 NPM registry", "Manage NPM registry"),
                    Some(envr_core::i18n::tr_key(
                        "gui.runtime.config.node.npm_enable_desc",
                        "不勾选：不修改用户 npm 配置；勾选：保存后写入 registry。",
                        "Unchecked: do not touch user npm config; checked: Save writes registry.",
                    )),
                    toggler(managed).on_toggle(|on| {
                        Message::Settings(SettingsMsg::SetNpmRegistryMode(if on {
                            NpmRegistryMode::Custom
                        } else {
                            NpmRegistryMode::Restore
                        }))
                    })
                        .into(),
                ))
                .push(setting_row(
                    tokens,
                    "registry".to_string(),
                    None,
                    text_input(
                        "https://registry.npmjs.org/",
                        &state.npm_registry_url_draft,
                    )
                    .on_input(|s| Message::Settings(SettingsMsg::NpmRegistryUrlEdit(s)))
                    .padding(sp.sm)
                    .width(Length::Fixed(420.0))
                    .style(text_input_style(tokens))
                    .into(),
                ))
                .push(setting_row(
                    tokens,
                    envr_core::i18n::tr_key("gui.runtime.config.common.presets", "常用值", "Presets"),
                    None,
                    pick_list(
                        presets,
                        None::<&'static str>,
                        |v| Message::Settings(SettingsMsg::NpmRegistryUrlEdit(v.to_string())),
                    )
                    .width(Length::Fixed(420.0))
                    .into(),
                ));
        }
        RuntimeKind::Python => {
            let managed = state.draft.runtime.python.pip_registry_mode != PipRegistryMode::Restore;
            let presets = [
                "",
                "https://pypi.org/simple",
                "https://pypi.tuna.tsinghua.edu.cn/simple",
                "https://mirrors.aliyun.com/pypi/simple",
                "https://repo.huaweicloud.com/repository/pypi/simple",
            ];
            content = content
                .push(setting_row(
                    tokens,
                    envr_core::i18n::tr_key("gui.runtime.config.python.pip_enable", "托管 PIP index-url", "Manage PIP index-url"),
                    Some(envr_core::i18n::tr_key(
                        "gui.runtime.config.python.pip_enable_desc",
                        "不勾选：不修改用户 pip 配置；勾选：保存后写入 pip.ini 的 index-url。",
                        "Unchecked: do not touch user pip config; checked: Save writes index-url to pip.ini.",
                    )),
                    toggler(managed).on_toggle(|on| {
                        Message::Settings(SettingsMsg::SetPipRegistryMode(if on {
                            PipRegistryMode::Custom
                        } else {
                            PipRegistryMode::Restore
                        }))
                    })
                        .into(),
                ))
                .push(setting_row(
                    tokens,
                    "index-url".to_string(),
                    None,
                    text_input("https://pypi.org/simple", &state.pip_index_url_draft)
                        .on_input(|s| Message::Settings(SettingsMsg::PipIndexUrlEdit(s)))
                        .padding(sp.sm)
                        .width(Length::Fixed(420.0))
                        .style(text_input_style(tokens))
                        .into(),
                ))
                .push(setting_row(
                    tokens,
                    envr_core::i18n::tr_key("gui.runtime.config.common.presets", "常用值", "Presets"),
                    None,
                    pick_list(
                        presets,
                        None::<&'static str>,
                        |v| Message::Settings(SettingsMsg::PipIndexUrlEdit(v.to_string())),
                    )
                    .width(Length::Fixed(420.0))
                    .into(),
                ));
        }
        RuntimeKind::Go => {
            let go_custom_enabled = state.draft.runtime.go.proxy_mode == GoProxyMode::Custom;
            content = content
                .push(setting_row(
                    tokens,
                    envr_core::i18n::tr_key("gui.runtime.config.go.custom_enable", "启用自定义 GOPROXY", "Enable custom GOPROXY"),
                    Some(envr_core::i18n::tr_key(
                        "gui.runtime.config.go.custom_enable_desc",
                        "未启用时不改写为自定义值（保留默认模式）。",
                        "When disabled, custom override is not applied (keep default mode).",
                    )),
                    toggler(go_custom_enabled)
                        .on_toggle(|on| {
                            Message::Settings(SettingsMsg::SetGoProxyMode(if on {
                                GoProxyMode::Custom
                            } else {
                                GoProxyMode::Auto
                            }))
                        })
                        .into(),
                ))
                .push(setting_row(
                    tokens,
                    "GOPROXY".to_string(),
                    None,
                    segmented3(
                        tokens,
                        true,
                        state.draft.runtime.go.proxy_mode == GoProxyMode::Auto,
                        state.draft.runtime.go.proxy_mode == GoProxyMode::Domestic,
                        state.draft.runtime.go.proxy_mode == GoProxyMode::Official,
                        envr_core::i18n::tr_key("gui.choice.auto", "自动", "Auto"),
                        envr_core::i18n::tr_key("gui.choice.domestic", "国内", "Domestic"),
                        envr_core::i18n::tr_key("gui.choice.official", "官方", "Official"),
                        Message::Settings(SettingsMsg::SetGoProxyMode(GoProxyMode::Auto)),
                        Message::Settings(SettingsMsg::SetGoProxyMode(GoProxyMode::Domestic)),
                        Message::Settings(SettingsMsg::SetGoProxyMode(GoProxyMode::Official)),
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
                    if go_custom_enabled {
                        text_input(
                            "runtime.go.proxy_custom (e.g. https://proxy.golang.org,direct)",
                            &state.go_proxy_custom_draft,
                        )
                        .on_input(|s| Message::Settings(SettingsMsg::GoProxyCustomEdit(s)))
                        .padding(sp.sm)
                        .width(Length::Fixed(360.0))
                        .style(text_input_style(tokens))
                        .into()
                    } else {
                        text_input(
                            "runtime.go.proxy_custom (e.g. https://proxy.golang.org,direct)",
                            &state.go_proxy_custom_draft,
                        )
                        .padding(sp.sm)
                        .width(Length::Fixed(360.0))
                        .style(text_input_style(tokens))
                        .into()
                    },
                ))
                .push(row![
                    button(button_content_centered(text("proxy.golang.org").into()))
                        .on_press(Message::Settings(SettingsMsg::GoProxyCustomEdit(
                            "https://proxy.golang.org,direct".to_string()
                        )))
                        .style(button_style(tokens, ButtonVariant::Secondary)),
                    button(button_content_centered(text("goproxy.cn").into()))
                        .on_press(Message::Settings(SettingsMsg::GoProxyCustomEdit(
                            "https://goproxy.cn,direct".to_string()
                        )))
                        .style(button_style(tokens, ButtonVariant::Secondary)),
                ]
                .spacing(sp.sm as f32))
                .push(setting_row(
                    tokens,
                    "GOPRIVATE".to_string(),
                    Some(envr_core::i18n::tr_key(
                        "gui.runtime.config.go.private_hint",
                        "逗号分隔私有模块前缀，例如：github.com/your-org/*",
                        "Comma-separated private module patterns, e.g. github.com/your-org/*",
                    )),
                    if go_custom_enabled {
                        text_input("runtime.go.private_patterns", &state.go_private_patterns_draft)
                            .on_input(|s| Message::Settings(SettingsMsg::GoPrivatePatternsEdit(s)))
                            .padding(sp.sm)
                            .width(Length::Fixed(360.0))
                            .style(text_input_style(tokens))
                            .into()
                    } else {
                        text_input("runtime.go.private_patterns", &state.go_private_patterns_draft)
                            .padding(sp.sm)
                            .width(Length::Fixed(360.0))
                            .style(text_input_style(tokens))
                            .into()
                    },
                ));
        }
        RuntimeKind::Bun => {
            let bun_enabled = !state.bun_global_bin_dir_draft.trim().is_empty();
            content = content.push(setting_row(
                tokens,
                envr_core::i18n::tr_key("gui.runtime.config.bun.bin_enable", "启用全局 bin 覆盖", "Enable global bin override"),
                None,
                toggler(bun_enabled)
                    .on_toggle(|on| {
                        Message::Settings(SettingsMsg::BunGlobalBinDirEdit(if on {
                            "C:/path/to/.bun/bin".to_string()
                        } else {
                            String::new()
                        }))
                    })
                    .into(),
            ));
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
                if bun_enabled {
                    text_input("runtime.bun.global_bin_dir", &state.bun_global_bin_dir_draft)
                        .on_input(|s| Message::Settings(SettingsMsg::BunGlobalBinDirEdit(s)))
                        .padding(sp.sm)
                        .width(Length::Fixed(360.0))
                        .style(text_input_style(tokens))
                        .into()
                } else {
                    text_input("runtime.bun.global_bin_dir", &state.bun_global_bin_dir_draft)
                        .padding(sp.sm)
                        .width(Length::Fixed(360.0))
                        .style(text_input_style(tokens))
                        .into()
                },
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
    enabled: bool,
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
        seg_btn(tokens, a_label, a_selected, SegmentPosition::Start, enabled.then_some(a_msg)),
        seg_btn(tokens, b_label, b_selected, SegmentPosition::Middle, enabled.then_some(b_msg)),
        seg_btn(tokens, c_label, c_selected, SegmentPosition::End, enabled.then_some(c_msg)),
    ]
    .spacing(-1.0)
    .into()
}

fn seg_btn(
    tokens: ThemeTokens,
    label: String,
    selected: bool,
    pos: SegmentPosition,
    msg: Option<Message>,
) -> Element<'static, Message> {
    let variant = if selected {
        ButtonVariant::Primary
    } else {
        ButtonVariant::Secondary
    };
    button(button_content_centered(text(label).into()))
        .on_press_maybe(msg)
        .height(Length::Fixed(tokens.control_height_secondary))
        .padding([tokens.space().sm as f32, (tokens.space().sm + 2) as f32])
        .style(segmented_button_style(tokens, variant, pos))
        .into()
}
