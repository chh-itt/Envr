use envr_config::settings::{FontMode, LocaleMode, MirrorMode, ThemeMode};
use envr_ui::font;
use envr_ui::theme::ThemeTokens;
use iced::widget::{button, column, pick_list, row, text, text_input, toggler};
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
    SetFontMode(FontMode),
    FontFamilyEdit(String),
    PickFontFamily(String),
    SetThemeMode(ThemeMode),
    AccentColorEdit(String),
    SetLocaleMode(LocaleMode),
    Save,
    ReloadDisk,
    DiskLoaded(Result<envr_config::settings::Settings, String>),
    DiskSaved(Result<envr_config::settings::Settings, String>),
}

pub fn settings_view(state: &SettingsViewState, tokens: ThemeTokens) -> Element<'static, Message> {
    let env_note = if SettingsViewState::env_overrides_runtime_root() {
        text(envr_core::i18n::tr_key(
            "gui.settings.note.env_override",
            "提示：已设置环境变量 ENVR_RUNTIME_ROOT，将覆盖下方的运行时根与 settings.toml。",
            "Note: ENVR_RUNTIME_ROOT is set and overrides the runtime root below and settings.toml.",
        ))
        .size(12)
    } else {
        text(envr_core::i18n::tr_key(
            "gui.settings.note.runtime_root",
            "运行时根：留空表示使用平台默认；与 CLI 共用 settings.toml。",
            "Runtime root: leave empty to use platform default; shared with CLI via settings.toml.",
        ))
        .size(12)
    };

    let rr = text_input(
        &envr_core::i18n::tr_key(
            "gui.settings.runtime_root_placeholder",
            "运行时根目录（可选）",
            "Runtime root (optional)",
        ),
        &state.runtime_root_draft,
    )
    .on_input(|s| Message::Settings(SettingsMsg::RuntimeRootEdit(s)))
    .padding(8)
    .width(Length::Fill);

    let mut mirror_row = row![
        text(envr_core::i18n::tr_key(
            "gui.settings.mirror_strategy",
            "镜像策略",
            "Mirror strategy",
        ))
        .size(15),
    ]
    .spacing(8);
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

    let manual = if state.draft.mirror.mode == MirrorMode::Manual {
        column![
            text(envr_core::i18n::tr_key(
                "gui.settings.manual_mirror_hint",
                "manual 模式下请填写镜像 ID（与 envr-mirror 预设一致，如 official、cn-1、cn-2）。",
                "In manual mode, enter a mirror ID (from envr-mirror presets, e.g. official, cn-1, cn-2).",
            ))
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
        .label(envr_core::i18n::tr_key(
            "gui.settings.cleanup_after_install",
            "安装成功后清理下载缓存（供后续运行时实现）",
            "Clean download cache after successful install (future runtime support)",
        ))
        .on_toggle(|v| Message::Settings(SettingsMsg::SetCleanup(v)));

    let mut font_mode_row = row![
        text(envr_core::i18n::tr_key(
            "gui.settings.font_section",
            "字体",
            "Font"
        ))
        .size(15)
    ]
    .spacing(8);
    for (mode, key, zh, en) in [
        (
            FontMode::Auto,
            "gui.settings.font.auto",
            "自动（系统字体）",
            "Auto (system font)",
        ),
        (
            FontMode::Custom,
            "gui.settings.font.custom",
            "自定义",
            "Custom",
        ),
    ] {
        let b = button(text(envr_core::i18n::tr_key(key, zh, en)))
            .on_press(Message::Settings(SettingsMsg::SetFontMode(mode)))
            .padding([6, 8]);
        let b = if mode == state.draft.appearance.font.mode {
            b.style(button::primary)
        } else {
            b.style(button::secondary)
        };
        font_mode_row = font_mode_row.push(b);
    }

    let picked = font::font_candidates()
        .iter()
        .copied()
        .find(|n| n.eq_ignore_ascii_case(state.font_family_draft.trim()));

    let font_custom = if state.draft.appearance.font.mode == FontMode::Custom {
        column![
            text(envr_core::i18n::tr_key(
                "gui.settings.font.inject_note",
                "提示：字体将作为 iced 的 default_font 注入，保存后需重启 GUI 才能全局生效。",
                "Note: the font is injected as iced default_font; restart the GUI after saving to apply globally.",
            ))
            .size(12),
            row![
                pick_list(font::font_candidates(), picked, |v| {
                    Message::Settings(SettingsMsg::PickFontFamily(v.to_string()))
                })
                .placeholder(envr_core::i18n::tr_key(
                    "gui.settings.font.pick_placeholder",
                    "从候选字体中选择",
                    "Pick from candidates",
                )),
                text_input(
                    &envr_core::i18n::tr_key(
                        "gui.settings.font.family_name",
                        "字体族名（Font family）",
                        "Font family name",
                    ),
                    &state.font_family_draft
                )
                .on_input(|s| Message::Settings(SettingsMsg::FontFamilyEdit(s)))
                .padding(8)
                .width(Length::Fill),
            ]
            .spacing(10),
        ]
        .spacing(6)
    } else {
        column![
            text(format!(
                "{} {}",
                envr_core::i18n::tr_key(
                    "gui.settings.font.auto_line_prefix",
                    "当前自动字体：",
                    "Auto font:",
                ),
                font::preferred_system_sans_family(),
            ))
            .size(12),
        ]
        .spacing(6)
    };

    let mut theme_mode_row = row![
        text(envr_core::i18n::tr_key(
            "gui.settings.theme_section",
            "主题",
            "Theme"
        ))
        .size(15)
    ]
    .spacing(8);
    for (mode, key, zh, en) in [
        (
            ThemeMode::FollowSystem,
            "gui.settings.theme.follow",
            "跟随系统",
            "Follow system",
        ),
        (
            ThemeMode::Light,
            "gui.settings.theme.light",
            "浅色",
            "Light",
        ),
        (ThemeMode::Dark, "gui.settings.theme.dark", "深色", "Dark"),
    ] {
        let b = button(text(envr_core::i18n::tr_key(key, zh, en)))
            .on_press(Message::Settings(SettingsMsg::SetThemeMode(mode)))
            .padding([6, 8]);
        let b = if mode == state.draft.appearance.theme_mode {
            b.style(button::primary)
        } else {
            b.style(button::secondary)
        };
        theme_mode_row = theme_mode_row.push(b);
    }

    let accent_row = column![
        text(envr_core::i18n::tr_key(
            "gui.settings.accent_color",
            "基准色（可选，#RGB / #RRGGBB）",
            "Accent color (optional, #RGB / #RRGGBB)",
        ))
        .size(tokens.typography().subsection),
        text_input(
            &envr_core::i18n::tr_key(
                "gui.settings.accent_placeholder",
                "留空则使用平台默认主色",
                "Leave empty for platform default primary",
            ),
            &state.accent_color_draft,
        )
        .on_input(|s| Message::Settings(SettingsMsg::AccentColorEdit(s)))
        .padding(tokens.space().sm)
        .width(Length::Fill),
    ]
    .spacing(tokens.space().xs);

    let dl_row = row![
        column![
            text(envr_core::i18n::tr_key(
                "gui.settings.max_concurrent",
                "最大并发下载",
                "Max concurrent downloads",
            ))
            .size(13),
            text_input(
                &envr_core::i18n::tr_key("gui.settings.max_conc_example", "例如 4", "e.g. 4"),
                &state.max_conc_text
            )
            .on_input(|s| Message::Settings(SettingsMsg::MaxConcEdit(s)))
            .padding(6),
        ]
        .spacing(4),
        column![
            text(envr_core::i18n::tr_key(
                "gui.settings.retry_limit",
                "重试次数上限",
                "Retry limit",
            ))
            .size(13),
            text_input(
                &envr_core::i18n::tr_key("gui.settings.retry_example", "例如 3", "e.g. 3"),
                &state.retry_text
            )
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
        button(text(envr_core::i18n::tr_key(
            "gui.settings.save_to_file",
            "保存到 settings.toml",
            "Save to settings.toml",
        )))
        .on_press(Message::Settings(SettingsMsg::Save))
        .padding([8, 12]),
        button(text(envr_core::i18n::tr_key(
            "gui.settings.reload_disk",
            "从磁盘重新加载",
            "Reload from disk",
        )))
        .on_press(Message::Settings(SettingsMsg::ReloadDisk))
        .padding([8, 12]),
    ]
    .spacing(10);

    let mut locale_row = row![
        text(envr_core::i18n::tr_key(
            "gui.settings.language",
            "语言",
            "Language"
        ))
        .size(15)
    ]
    .spacing(8);
    for (mode, key, zh, en) in [
        (
            LocaleMode::FollowSystem,
            "gui.settings.locale.follow",
            "跟随系统",
            "Follow system",
        ),
        (
            LocaleMode::ZhCn,
            "gui.settings.locale.zh_cn",
            "简体中文",
            "中文",
        ),
        (
            LocaleMode::EnUs,
            "gui.settings.locale.en_us",
            "English",
            "English",
        ),
    ] {
        let b = button(text(envr_core::i18n::tr_key(key, zh, en)))
            .on_press(Message::Settings(SettingsMsg::SetLocaleMode(mode)))
            .padding([6, 8]);
        let b = if mode == state.locale_mode_draft {
            b.style(button::primary)
        } else {
            b.style(button::secondary)
        };
        locale_row = locale_row.push(b);
    }

    column![
        text(envr_core::i18n::tr_key(
            "gui.settings.title",
            "设置",
            "Settings"
        ))
        .size(20),
        env_note,
        rr,
        mirror_row,
        manual,
        cleanup,
        font_mode_row,
        font_custom,
        theme_mode_row,
        accent_row,
        locale_row,
        text(envr_core::i18n::tr_key(
            "gui.settings.downloads_section",
            "下载",
            "Downloads",
        ))
        .size(16),
        dl_row,
        actions,
        status,
    ]
    .spacing(tokens.content_spacing().round() as u16)
    .into()
}
