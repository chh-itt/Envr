use envr_config::settings::{
    GoProxyMode, JavaDistro, NpmRegistryMode, PhpWindowsBuildFlavor, PipRegistryMode,
    PythonWindowsDistribution,
};
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
            content = content.push(enum_pick_setting_row(
                tokens,
                envr_core::i18n::tr_key(
                    "gui.runtime.config.python.windows_dist",
                    "Windows Python 发行包",
                    "Windows Python distribution",
                ),
                Some(envr_core::i18n::tr_key(
                    "gui.runtime.config.python.windows_dist_desc",
                    "控制 Windows 安装时优先使用的 Python 分发形式。",
                    "Controls which Python distribution is preferred on Windows installs.",
                )),
                python_windows_distribution_label(state.draft.runtime.python.windows_distribution),
                &["auto", "nuget", "embeddable"],
                msg_python_windows_distribution_pick,
            ));
        }
        RuntimeKind::Java => {
            content = content.push(enum_pick_setting_row(
                tokens,
                envr_core::i18n::tr_key(
                    "gui.runtime.config.java.distro",
                    "默认 Java 发行版",
                    "Default Java distribution",
                ),
                Some(envr_core::i18n::tr_key(
                    "gui.runtime.config.java.distro_desc",
                    "影响 Java 版本列表与安装来源（按发行版能力过滤）。",
                    "Affects Java version list and installer backend capability filtering.",
                )),
                java_distro_label(state.draft.runtime.java.current_distro),
                &[
                    "temurin",
                    "oracle_openjdk",
                    "amazon_corretto",
                    "microsoft",
                    "oracle_jdk",
                    "azul_zulu",
                    "alibaba_dragonwell",
                ],
                msg_java_distro_pick,
            ));
        }
        RuntimeKind::Php => {
            content = content.push(enum_pick_setting_row(
                tokens,
                envr_core::i18n::tr_key(
                    "gui.runtime.config.php.windows_build",
                    "Windows PHP 构建类型",
                    "Windows PHP build flavor",
                ),
                Some(envr_core::i18n::tr_key(
                    "gui.runtime.config.php.windows_build_desc",
                    "NTS 适合大多数 CLI 场景；TS 主要用于线程安全需求场景。",
                    "NTS fits most CLI scenarios; TS is mainly for thread-safe requirements.",
                )),
                php_windows_build_label(state.draft.runtime.php.windows_build),
                &["nts", "ts"],
                msg_php_windows_build_pick,
            ));
        }
        RuntimeKind::Deno => {
            let managed = state.draft.runtime.deno.package_source != NpmRegistryMode::Restore;
            content = content.push(managed_mode_setting_row(
                tokens,
                envr_core::i18n::tr_key(
                    "gui.runtime.config.deno.pkg_enable",
                    "托管 Deno 包源环境变量",
                    "Manage Deno package registry env",
                ),
                envr_core::i18n::tr_key(
                    "gui.runtime.config.deno.pkg_enable_desc",
                    "不勾选：不注入 NPM_CONFIG_REGISTRY/JSR_URL；勾选：按所选模式注入。",
                    "Unchecked: do not inject NPM_CONFIG_REGISTRY/JSR_URL; checked: inject by selected mode.",
                ),
                envr_core::i18n::tr_key(
                    "gui.runtime.config.deno.pkg_mode",
                    "包源模式",
                    "Package source mode",
                ),
                managed,
                msg_deno_package_toggle,
                npm_mode_label(state.draft.runtime.deno.package_source),
                &["auto", "official", "domestic"],
                msg_deno_package_pick,
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
            let pkg_managed = state.draft.runtime.bun.package_source != NpmRegistryMode::Restore;
            content = content.push(managed_mode_setting_row(
                tokens,
                envr_core::i18n::tr_key(
                    "gui.runtime.config.bun.pkg_enable",
                    "托管 Bun 包源环境变量",
                    "Manage Bun package registry env",
                ),
                envr_core::i18n::tr_key(
                    "gui.runtime.config.bun.pkg_enable_desc",
                    "不勾选：不注入 NPM_CONFIG_REGISTRY；勾选：按所选模式注入。",
                    "Unchecked: do not inject NPM_CONFIG_REGISTRY; checked: inject by selected mode.",
                ),
                envr_core::i18n::tr_key(
                    "gui.runtime.config.bun.pkg_mode",
                    "包源模式",
                    "Package source mode",
                ),
                pkg_managed,
                msg_bun_package_toggle,
                npm_mode_label(state.draft.runtime.bun.package_source),
                &["auto", "official", "domestic"],
                msg_bun_package_pick,
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

fn enum_pick_setting_row(
    tokens: ThemeTokens,
    title: String,
    desc: Option<String>,
    selected: &'static str,
    options: &[&'static str],
    on_pick: fn(&'static str) -> Message,
) -> Element<'static, Message> {
    setting_row(
        tokens,
        title,
        desc,
        pick_list(options.to_vec(), Some(selected), on_pick)
            .width(Length::Fixed(420.0))
            .into(),
    )
}

fn managed_mode_setting_row(
    tokens: ThemeTokens,
    manage_title: String,
    manage_desc: String,
    mode_title: String,
    managed: bool,
    on_toggle: fn(bool) -> Message,
    selected: &'static str,
    options: &[&'static str],
    on_pick: fn(&'static str) -> Message,
) -> Element<'static, Message> {
    let sp = tokens.space();
    let manage_row = setting_row(
        tokens,
        manage_title,
        Some(manage_desc),
        toggler(managed).on_toggle(on_toggle).into(),
    );
    let picker: Element<'static, Message> = pick_list(options.to_vec(), Some(selected), on_pick)
        .width(Length::Fixed(420.0))
        .into();
    let mode_row = setting_row(tokens, mode_title, None, picker);
    column![manage_row, mode_row].spacing(sp.md as f32).into()
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

fn python_windows_distribution_label(v: PythonWindowsDistribution) -> &'static str {
    match v {
        PythonWindowsDistribution::Auto => "auto",
        PythonWindowsDistribution::Nuget => "nuget",
        PythonWindowsDistribution::Embeddable => "embeddable",
    }
}
fn msg_python_windows_distribution_pick(v: &'static str) -> Message {
    let dist = match v {
        "nuget" => PythonWindowsDistribution::Nuget,
        "embeddable" => PythonWindowsDistribution::Embeddable,
        _ => PythonWindowsDistribution::Auto,
    };
    Message::Settings(SettingsMsg::SetPythonWindowsDistribution(dist))
}

fn java_distro_label(v: JavaDistro) -> &'static str {
    match v {
        JavaDistro::Temurin | JavaDistro::OpenJdk => "temurin",
        JavaDistro::OracleOpenJdk => "oracle_openjdk",
        JavaDistro::AmazonCorretto => "amazon_corretto",
        JavaDistro::Microsoft => "microsoft",
        JavaDistro::OracleJdk => "oracle_jdk",
        JavaDistro::AzulZulu => "azul_zulu",
        JavaDistro::AlibabaDragonwell => "alibaba_dragonwell",
    }
}
fn msg_java_distro_pick(v: &'static str) -> Message {
    let dist = match v {
        "oracle_openjdk" => JavaDistro::OracleOpenJdk,
        "amazon_corretto" => JavaDistro::AmazonCorretto,
        "microsoft" => JavaDistro::Microsoft,
        "oracle_jdk" => JavaDistro::OracleJdk,
        "azul_zulu" => JavaDistro::AzulZulu,
        "alibaba_dragonwell" => JavaDistro::AlibabaDragonwell,
        _ => JavaDistro::Temurin,
    };
    Message::Settings(SettingsMsg::SetJavaDistro(dist))
}

fn php_windows_build_label(v: PhpWindowsBuildFlavor) -> &'static str {
    match v {
        PhpWindowsBuildFlavor::Nts => "nts",
        PhpWindowsBuildFlavor::Ts => "ts",
    }
}
fn msg_php_windows_build_pick(v: &'static str) -> Message {
    let build = if v == "ts" {
        PhpWindowsBuildFlavor::Ts
    } else {
        PhpWindowsBuildFlavor::Nts
    };
    Message::Settings(SettingsMsg::SetPhpWindowsBuild(build))
}

fn npm_mode_label(v: NpmRegistryMode) -> &'static str {
    match v {
        NpmRegistryMode::Auto => "auto",
        NpmRegistryMode::Official => "official",
        NpmRegistryMode::Domestic => "domestic",
        NpmRegistryMode::Custom => "auto",
        NpmRegistryMode::Restore => "auto",
    }
}
fn msg_deno_package_toggle(on: bool) -> Message {
    Message::Settings(SettingsMsg::SetDenoPackageSource(if on {
        NpmRegistryMode::Auto
    } else {
        NpmRegistryMode::Restore
    }))
}
fn msg_deno_package_pick(v: &'static str) -> Message {
    let mode = match v {
        "official" => NpmRegistryMode::Official,
        "domestic" => NpmRegistryMode::Domestic,
        _ => NpmRegistryMode::Auto,
    };
    Message::Settings(SettingsMsg::SetDenoPackageSource(mode))
}
fn msg_bun_package_toggle(on: bool) -> Message {
    Message::Settings(SettingsMsg::SetBunPackageSource(if on {
        NpmRegistryMode::Auto
    } else {
        NpmRegistryMode::Restore
    }))
}
fn msg_bun_package_pick(v: &'static str) -> Message {
    let mode = match v {
        "official" => NpmRegistryMode::Official,
        "domestic" => NpmRegistryMode::Domestic,
        _ => NpmRegistryMode::Auto,
    };
    Message::Settings(SettingsMsg::SetBunPackageSource(mode))
}

