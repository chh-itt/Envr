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
    ButtonVariant, button_content_centered, button_style, section_card, setting_row, text_input_style,
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
            content = content.push(managed_url_setting_row(
                tokens,
                envr_core::i18n::tr_key("gui.runtime.config.node.npm_enable", "托管 NPM registry", "Manage NPM registry"),
                envr_core::i18n::tr_key(
                    "gui.runtime.config.node.npm_enable_desc",
                    "不勾选：不修改用户 npm 配置；勾选：保存后写入 registry。",
                    "Unchecked: do not touch user npm config; checked: Save writes registry.",
                ),
                "registry",
                managed,
                msg_npm_toggle,
                &state.npm_registry_url_draft,
                "https://registry.npmjs.org/",
                msg_npm_edit,
                &presets,
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
            content = content.push(managed_url_setting_row(
                tokens,
                envr_core::i18n::tr_key("gui.runtime.config.python.pip_enable", "托管 PIP index-url", "Manage PIP index-url"),
                envr_core::i18n::tr_key(
                    "gui.runtime.config.python.pip_enable_desc",
                    "不勾选：不修改用户 pip 配置；勾选：保存后写入 pip.ini 的 index-url。",
                    "Unchecked: do not touch user pip config; checked: Save writes index-url to pip.ini.",
                ),
                "index-url",
                managed,
                msg_pip_toggle,
                &state.pip_index_url_draft,
                "https://pypi.org/simple",
                msg_pip_edit,
                &presets,
            ));
        }
        RuntimeKind::Go => {
            let goproxy_managed = state.draft.runtime.go.proxy_mode == GoProxyMode::Custom;
            let goprivate_managed = !state.go_private_patterns_draft.trim().is_empty();
            let proxy_presets = [
                "",
                "https://proxy.golang.org,direct",
                "https://goproxy.cn,direct",
                "https://goproxy.io,direct",
            ];
            let private_presets = [
                "",
                "github.com/your-org/*",
                "gitlab.com/your-group/*",
                "gitee.com/your-team/*",
            ];
            content = content
                .push(managed_url_setting_row(
                    tokens,
                    envr_core::i18n::tr_key("gui.runtime.config.go.custom_enable", "托管 GOPROXY", "Manage GOPROXY"),
                    envr_core::i18n::tr_key(
                        "gui.runtime.config.go.custom_enable_desc",
                        "不勾选：不处理该项；勾选：保存后应用输入值。",
                        "Unchecked: do not manage this; checked: Save applies input value.",
                    ),
                    "GOPROXY",
                    goproxy_managed,
                    msg_go_proxy_toggle,
                    &state.go_proxy_custom_draft,
                    "runtime.go.proxy_custom (e.g. https://proxy.golang.org,direct)",
                    msg_go_proxy_edit,
                    &proxy_presets,
                ))
                .push(managed_url_setting_row(
                    tokens,
                    envr_core::i18n::tr_key("gui.runtime.config.go.private_enable", "托管 GOPRIVATE", "Manage GOPRIVATE"),
                    envr_core::i18n::tr_key(
                        "gui.runtime.config.go.private_enable_desc",
                        "不勾选：不处理该项；勾选：保存后应用输入值。",
                        "Unchecked: do not manage this; checked: Save applies input value.",
                    ),
                    "GOPRIVATE",
                    goprivate_managed,
                    msg_go_private_toggle,
                    &state.go_private_patterns_draft,
                    "runtime.go.private_patterns",
                    msg_go_private_edit,
                    &private_presets,
                ));
        }
        RuntimeKind::Bun => {
            let bun_enabled = !state.bun_global_bin_dir_draft.trim().is_empty();
            let bun_bin_presets = [
                "",
                "C:\\Users\\<you>\\AppData\\Roaming\\npm",
                "C:\\Users\\<you>\\.bun\\bin",
                "/usr/local/bin",
                "~/.bun/bin",
            ];
            content = content.push(managed_url_setting_row(
                tokens,
                envr_core::i18n::tr_key("gui.runtime.config.bun.bin_enable", "启用全局 bin 覆盖", "Enable global bin override"),
                envr_core::i18n::tr_key(
                    "gui.runtime.config.bun.global_bin_hint",
                    "可选：覆盖 `bun pm bin -g` 检测结果",
                    "Optional: override detected `bun pm bin -g` path",
                ),
                &envr_core::i18n::tr_key(
                    "gui.runtime.config.bun.global_bin_dir",
                    "全局 bin 目录",
                    "Global bin directory",
                ),
                bun_enabled,
                msg_bun_bin_toggle,
                &state.bun_global_bin_dir_draft,
                "runtime.bun.global_bin_dir",
                msg_bun_bin_edit,
                &bun_bin_presets,
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

fn managed_url_setting_row(
    tokens: ThemeTokens,
    manage_title: String,
    manage_desc: String,
    value_title: &str,
    managed: bool,
    on_toggle: fn(bool) -> Message,
    value: &str,
    placeholder: &str,
    on_edit: fn(String) -> Message,
    presets: &[&'static str],
) -> Element<'static, Message> {
    let sp = tokens.space();
    let manage_row = setting_row(
        tokens,
        manage_title,
        Some(manage_desc),
        toggler(managed).on_toggle(on_toggle).into(),
    );
    let input_el: Element<'static, Message> = if managed {
        text_input(placeholder, value)
            .on_input(on_edit)
            .padding(sp.sm)
            .width(Length::Fixed(420.0))
            .style(text_input_style(tokens))
            .into()
    } else {
        text_input(placeholder, value)
            .padding(sp.sm)
            .width(Length::Fixed(420.0))
            .style(text_input_style(tokens))
            .into()
    };
    let input_row = setting_row(tokens, value_title.to_string(), None, input_el);
    let presets_row = setting_row(
        tokens,
        envr_core::i18n::tr_key("gui.runtime.config.common.presets", "常用值", "Presets"),
        None,
        pick_list(presets.to_vec(), None::<&'static str>, move |v| on_edit(v.to_string()))
            .width(Length::Fixed(420.0))
            .into(),
    );
    column![manage_row, input_row, presets_row]
        .spacing(sp.md as f32)
        .into()
}

fn msg_npm_toggle(on: bool) -> Message {
    Message::Settings(SettingsMsg::SetNpmRegistryMode(if on {
        NpmRegistryMode::Custom
    } else {
        NpmRegistryMode::Restore
    }))
}
fn msg_npm_edit(s: String) -> Message {
    Message::Settings(SettingsMsg::NpmRegistryUrlEdit(s))
}
fn msg_pip_toggle(on: bool) -> Message {
    Message::Settings(SettingsMsg::SetPipRegistryMode(if on {
        PipRegistryMode::Custom
    } else {
        PipRegistryMode::Restore
    }))
}
fn msg_pip_edit(s: String) -> Message {
    Message::Settings(SettingsMsg::PipIndexUrlEdit(s))
}
fn msg_go_proxy_toggle(on: bool) -> Message {
    Message::Settings(SettingsMsg::SetGoProxyMode(if on {
        GoProxyMode::Custom
    } else {
        GoProxyMode::Auto
    }))
}
fn msg_go_proxy_edit(s: String) -> Message {
    Message::Settings(SettingsMsg::GoProxyCustomEdit(s))
}
fn msg_go_private_toggle(on: bool) -> Message {
    Message::Settings(SettingsMsg::GoPrivatePatternsEdit(if on {
        "github.com/your-org/*".to_string()
    } else {
        String::new()
    }))
}
fn msg_go_private_edit(s: String) -> Message {
    Message::Settings(SettingsMsg::GoPrivatePatternsEdit(s))
}
fn msg_bun_bin_toggle(on: bool) -> Message {
    Message::Settings(SettingsMsg::BunGlobalBinDirEdit(if on {
        "C:/path/to/.bun/bin".to_string()
    } else {
        String::new()
    }))
}
fn msg_bun_bin_edit(s: String) -> Message {
    Message::Settings(SettingsMsg::BunGlobalBinDirEdit(s))
}

