use envr_config::settings::{FontMode, LocaleMode, MirrorMode, ThemeMode};
use envr_ui::font;
use envr_ui::theme::ThemeTokens;
use iced::alignment::Vertical;
use iced::widget::{button, column, container, pick_list, row, text, text_input, toggler};
use iced::{Alignment, Element, Length};

use crate::app::Message;
use crate::icons::Lucide;
use crate::theme as gui_theme;
use crate::view::runtime_layout::RuntimeLayoutMsg;
use crate::view::settings::state::SettingsViewState;
use crate::widget_styles::{
    ButtonVariant, SegmentPosition, button_content_centered, button_label_for_variant,
    button_style, section_card, segmented_button_style, setting_row, text_input_style,
};

#[derive(Debug, Clone)]
pub enum SettingsMsg {
    BrowseRuntimeRoot,
    RuntimeRootBrowseResult(Option<std::path::PathBuf>),
    ClearRuntimeRoot,
    ManualIdEdit(String),
    MaxConcEdit(String),
    MaxBpsEdit(String),
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
    let ty = tokens.typography();
    let sp = tokens.space();

    let env_note = if SettingsViewState::env_overrides_runtime_root() {
        text(envr_core::i18n::tr_key(
            "gui.settings.note.env_override",
            "提示：已设置环境变量 ENVR_RUNTIME_ROOT，将覆盖下方的运行时根与 settings.toml。",
            "Note: ENVR_RUNTIME_ROOT is set and overrides the runtime root below and settings.toml.",
        ))
        .size(ty.micro)
    } else {
        text(envr_core::i18n::tr_key(
            "gui.settings.note.runtime_root",
            "运行时根：留空表示使用平台默认；与 CLI 共用 settings.toml。",
            "Runtime root: leave empty to use platform default; shared with CLI via settings.toml.",
        ))
        .size(ty.micro)
    };

    let env_lock = SettingsViewState::env_overrides_runtime_root();
    let rr_display: Element<'static, Message> = if state.runtime_root_draft.trim().is_empty() {
        text(envr_core::i18n::tr_key(
            "gui.settings.runtime_root_placeholder",
            "运行时根目录（可选）",
            "Runtime root (optional)",
        ))
        .size(ty.body_small)
        .color(gui_theme::to_color(tokens.colors.text_muted))
        .into()
    } else {
        text(state.runtime_root_draft.clone())
            .size(ty.body_small)
            .color(gui_theme::to_color(tokens.colors.text))
            .into()
    };
    let path_area = container(rr_display)
        .width(Length::Fill)
        .height(Length::Fixed(tokens.control_height_secondary))
        .align_y(Vertical::Center)
        .padding([0.0, sp.sm as f32]);

    let browse_h = tokens
        .control_height_secondary
        .max(tokens.min_click_target_px());
    let pad_v = sp.sm as f32;
    let rr_row = row![
        path_area,
        button(button_content_centered(
            text(envr_core::i18n::tr_key(
                "gui.settings.runtime_root_browse",
                "浏览文件夹…",
                "Choose folder…",
            ))
            .into(),
        ))
        .on_press_maybe((!env_lock).then_some(Message::Settings(SettingsMsg::BrowseRuntimeRoot)),)
        .height(Length::Fixed(browse_h))
        .padding([pad_v, sp.md as f32])
        .style(button_style(tokens, ButtonVariant::Secondary)),
        button(button_content_centered(
            text(envr_core::i18n::tr_key(
                "gui.settings.runtime_root_clear",
                "清除",
                "Clear",
            ))
            .into(),
        ))
        .on_press_maybe(
            (!env_lock && !state.runtime_root_draft.trim().is_empty())
                .then_some(Message::Settings(SettingsMsg::ClearRuntimeRoot)),
        )
        .height(Length::Fixed(browse_h))
        .padding([pad_v, sp.md as f32])
        .style(button_style(tokens, ButtonVariant::Ghost)),
    ]
    .spacing(sp.sm as f32)
    .align_y(Alignment::Center);

    let mut mirror_buttons = row![].spacing(-1.0);
    let mirror_modes = [
        MirrorMode::Official,
        MirrorMode::Auto,
        MirrorMode::Manual,
        MirrorMode::Offline,
    ];
    for (idx, mode) in mirror_modes.iter().copied().enumerate() {
        let lab = SettingsViewState::mirror_mode_label(mode);
        let variant = if mode == state.draft.mirror.mode {
            ButtonVariant::Primary
        } else {
            ButtonVariant::Secondary
        };
        let h = if mode == state.draft.mirror.mode {
            tokens.control_height_primary
        } else {
            tokens.control_height_secondary
        };
        let pos = if mirror_modes.len() == 1 {
            SegmentPosition::Single
        } else if idx == 0 {
            SegmentPosition::Start
        } else if idx + 1 == mirror_modes.len() {
            SegmentPosition::End
        } else {
            SegmentPosition::Middle
        };
        let b = button(button_content_centered(text(lab).into()))
            .on_press(Message::Settings(SettingsMsg::SetMirrorMode(mode)))
            .width(Length::Shrink)
            .height(Length::Fixed(h))
            .padding([sp.sm as f32, (sp.sm + 2) as f32])
            .style(segmented_button_style(tokens, variant, pos));
        mirror_buttons = mirror_buttons.push(b);
    }

    let manual = if state.draft.mirror.mode == MirrorMode::Manual {
        column![
            text(envr_core::i18n::tr_key(
                "gui.settings.manual_mirror_hint",
                "manual 模式下请填写镜像 ID（与 envr-mirror 预设一致，如 official、cn-1、cn-2）。",
                "In manual mode, enter a mirror ID (from envr-mirror presets, e.g. official, cn-1, cn-2).",
            ))
            .size(ty.micro),
            container(
                text_input("mirror.manual_id", &state.manual_id_draft)
                    .on_input(|s| Message::Settings(SettingsMsg::ManualIdEdit(s)))
                    .padding(sp.sm)
                    .width(Length::Fill)
                    .style(text_input_style(tokens)),
            )
            .width(Length::Fill)
            .height(Length::Fixed(tokens.control_height_secondary))
            .align_y(Vertical::Center),
        ]
        .spacing((sp.xs + 2) as f32)
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

    let font_options = [
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
    ];
    let mut font_mode_row = row![].spacing(-1.0);
    for (idx, (mode, key, zh, en)) in font_options.iter().copied().enumerate() {
        let variant = if mode == state.draft.appearance.font.mode {
            ButtonVariant::Primary
        } else {
            ButtonVariant::Secondary
        };
        let h = if mode == state.draft.appearance.font.mode {
            tokens.control_height_primary
        } else {
            tokens.control_height_secondary
        };
        let pos = if font_options.len() == 1 {
            SegmentPosition::Single
        } else if idx == 0 {
            SegmentPosition::Start
        } else if idx + 1 == font_options.len() {
            SegmentPosition::End
        } else {
            SegmentPosition::Middle
        };
        let b = button(button_content_centered(button_label_for_variant(
            envr_core::i18n::tr_key(key, zh, en),
            tokens,
            variant,
        )))
        .on_press(Message::Settings(SettingsMsg::SetFontMode(mode)))
        .width(Length::Shrink)
        .height(Length::Fixed(h))
        .padding([sp.sm as f32, (sp.sm + 2) as f32])
        .style(segmented_button_style(tokens, variant, pos));
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
            .size(ty.micro),
            row![
                pick_list(font::font_candidates(), picked, |v| {
                    Message::Settings(SettingsMsg::PickFontFamily(v.to_string()))
                })
                .placeholder(envr_core::i18n::tr_key(
                    "gui.settings.font.pick_placeholder",
                    "从候选字体中选择",
                    "Pick from candidates",
                ))
                .width(Length::Fixed(240.0)),
                container(
                    text_input(
                        &envr_core::i18n::tr_key(
                            "gui.settings.font.family_name",
                            "字体族名（Font family）",
                            "Font family name",
                        ),
                        &state.font_family_draft,
                    )
                    .on_input(|s| Message::Settings(SettingsMsg::FontFamilyEdit(s)))
                    .padding(sp.sm)
                    .width(Length::Fill)
                    .style(text_input_style(tokens)),
                )
                .width(Length::Fill)
                .height(Length::Fixed(tokens.control_height_secondary))
                .align_y(Vertical::Center),
            ]
            .spacing((sp.sm + 2) as f32),
        ]
        .spacing((sp.xs + 2) as f32)
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
            .size(ty.micro),
        ]
        .spacing((sp.xs + 2) as f32)
    };

    let theme_options = [
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
    ];
    let mut theme_mode_row = row![].spacing(-1.0);
    for (idx, (mode, key, zh, en)) in theme_options.iter().copied().enumerate() {
        let variant = if mode == state.draft.appearance.theme_mode {
            ButtonVariant::Primary
        } else {
            ButtonVariant::Secondary
        };
        let h = if mode == state.draft.appearance.theme_mode {
            tokens.control_height_primary
        } else {
            tokens.control_height_secondary
        };
        let pos = if theme_options.len() == 1 {
            SegmentPosition::Single
        } else if idx == 0 {
            SegmentPosition::Start
        } else if idx + 1 == theme_options.len() {
            SegmentPosition::End
        } else {
            SegmentPosition::Middle
        };
        let b = button(button_content_centered(button_label_for_variant(
            envr_core::i18n::tr_key(key, zh, en),
            tokens,
            variant,
        )))
        .on_press(Message::Settings(SettingsMsg::SetThemeMode(mode)))
        .width(Length::Shrink)
        .height(Length::Fixed(h))
        .padding([sp.sm as f32, (sp.sm + 2) as f32])
        .style(segmented_button_style(tokens, variant, pos));
        theme_mode_row = theme_mode_row.push(b);
    }

    let accent_row = column![
        text(envr_core::i18n::tr_key(
            "gui.settings.accent_color",
            "基准色（可选，#RGB / #RRGGBB）",
            "Accent color (optional, #RGB / #RRGGBB)",
        ))
        .size(ty.subsection),
        container(
            text_input(
                &envr_core::i18n::tr_key(
                    "gui.settings.accent_placeholder",
                    "留空则使用平台默认主色",
                    "Leave empty for platform default primary",
                ),
                &state.accent_color_draft,
            )
            .on_input(|s| Message::Settings(SettingsMsg::AccentColorEdit(s)))
            .padding(sp.sm)
            .width(Length::Fill)
            .style(text_input_style(tokens)),
        )
        .width(Length::Fill)
        .height(Length::Fixed(tokens.control_height_secondary))
        .align_y(Vertical::Center),
    ]
    .spacing(sp.xs as f32);

    let dl_row = row![
        column![
            text(envr_core::i18n::tr_key(
                "gui.settings.max_concurrent",
                "最大并发下载",
                "Max concurrent downloads",
            ))
            .size(ty.caption),
            text_input(
                &envr_core::i18n::tr_key("gui.settings.max_conc_example", "例如 4", "e.g. 4"),
                &state.max_conc_text,
            )
            .on_input(|s| Message::Settings(SettingsMsg::MaxConcEdit(s)))
            .padding(sp.xs + 2)
            .style(text_input_style(tokens)),
        ]
        .spacing(sp.xs as f32),
        column![
            text(envr_core::i18n::tr_key(
                "gui.settings.max_bps",
                "全局下载限速（字节/秒，0 不限制）",
                "Global bandwidth cap (bytes/sec, 0 = unlimited)",
            ))
            .size(ty.caption),
            text_input(
                &envr_core::i18n::tr_key("gui.settings.max_bps_example", "例如 10485760", "e.g. 10485760"),
                &state.max_bps_text,
            )
            .on_input(|s| Message::Settings(SettingsMsg::MaxBpsEdit(s)))
            .padding(sp.xs + 2)
            .style(text_input_style(tokens)),
        ]
        .spacing(sp.xs as f32),
        column![
            text(envr_core::i18n::tr_key(
                "gui.settings.retry_limit",
                "重试次数上限",
                "Retry limit",
            ))
            .size(ty.caption),
            text_input(
                &envr_core::i18n::tr_key("gui.settings.retry_example", "例如 3", "e.g. 3"),
                &state.retry_text,
            )
            .on_input(|s| Message::Settings(SettingsMsg::RetryEdit(s)))
            .padding(sp.xs + 2)
            .style(text_input_style(tokens)),
        ]
        .spacing(sp.xs as f32),
    ]
    .spacing(sp.lg as f32);

    let status = match state.last_message.as_ref() {
        Some(m) => text(m.clone()).size(ty.caption),
        None => text("").size(1),
    };

    let on_prim = gui_theme::contrast_on_primary(tokens);
    let txt = gui_theme::to_color(tokens.colors.text);
    let actions = row![
        button(button_content_centered(
            row![
                Lucide::Settings.view(16.0, on_prim),
                text(envr_core::i18n::tr_key(
                    "gui.settings.save_to_file",
                    "保存到 settings.toml",
                    "Save to settings.toml",
                ))
                .color(on_prim),
            ]
            .spacing(sp.sm as f32)
            .align_y(Alignment::Center)
            .into(),
        ))
        .on_press(Message::Settings(SettingsMsg::Save))
        .height(Length::Fixed(tokens.control_height_primary))
        .padding([sp.sm as f32, (sp.md + 2) as f32])
        .style(button_style(tokens, ButtonVariant::Primary)),
        button(button_content_centered(
            row![
                Lucide::RefreshCw.view(16.0, txt),
                text(envr_core::i18n::tr_key(
                    "gui.settings.reload_disk",
                    "从磁盘重新加载",
                    "Reload from disk",
                ))
                .color(txt),
            ]
            .spacing(sp.sm as f32)
            .align_y(Alignment::Center)
            .into(),
        ))
        .on_press(Message::Settings(SettingsMsg::ReloadDisk))
        .height(Length::Fixed(tokens.control_height_secondary))
        .padding([sp.sm as f32, sp.md as f32])
        .style(button_style(tokens, ButtonVariant::Secondary)),
    ]
    .spacing((sp.sm + 2) as f32);

    let locale_options = [
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
    ];
    let mut locale_row = row![].spacing(-1.0);
    for (idx, (mode, key, zh, en)) in locale_options.iter().copied().enumerate() {
        let variant = if mode == state.locale_mode_draft {
            ButtonVariant::Primary
        } else {
            ButtonVariant::Secondary
        };
        let h = if mode == state.locale_mode_draft {
            tokens.control_height_primary
        } else {
            tokens.control_height_secondary
        };
        let pos = if locale_options.len() == 1 {
            SegmentPosition::Single
        } else if idx == 0 {
            SegmentPosition::Start
        } else if idx + 1 == locale_options.len() {
            SegmentPosition::End
        } else {
            SegmentPosition::Middle
        };
        let b = button(button_content_centered(button_label_for_variant(
            envr_core::i18n::tr_key(key, zh, en),
            tokens,
            variant,
        )))
        .on_press(Message::Settings(SettingsMsg::SetLocaleMode(mode)))
        .width(Length::Shrink)
        .height(Length::Fixed(h))
        .padding([sp.sm as f32, (sp.sm + 2) as f32])
        .style(segmented_button_style(tokens, variant, pos));
        locale_row = locale_row.push(b);
    }

    let paths_card = section_card(
        tokens,
        envr_core::i18n::tr_key(
            "gui.settings.group.paths_mirror",
            "存储路径与镜像",
            "Storage & mirrors",
        ),
        column![
            env_note,
            rr_row,
            setting_row(
                tokens,
                envr_core::i18n::tr_key(
                    "gui.settings.mirror_strategy",
                    "镜像策略",
                    "Mirror strategy",
                ),
                Some(envr_core::i18n::tr_key(
                    "gui.settings.mirror_strategy_desc",
                    "控制运行时下载镜像选择方式。",
                    "Choose how runtime mirrors are selected.",
                )),
                mirror_buttons.into(),
            ),
            manual,
            cleanup,
        ]
        .spacing(sp.md as f32)
        .into(),
    );

    let look_card = section_card(
        tokens,
        envr_core::i18n::tr_key(
            "gui.settings.group.look_feel",
            "外观、字体与语言",
            "Appearance, font & language",
        ),
        column![
            setting_row(
                tokens,
                envr_core::i18n::tr_key("gui.settings.font_section", "字体", "Font",),
                None,
                font_mode_row.into(),
            ),
            font_custom,
            setting_row(
                tokens,
                envr_core::i18n::tr_key("gui.settings.theme_section", "主题", "Theme",),
                None,
                theme_mode_row.into(),
            ),
            accent_row,
            setting_row(
                tokens,
                envr_core::i18n::tr_key("gui.settings.language", "语言", "Language",),
                None,
                locale_row.into(),
            ),
        ]
        .spacing(sp.md as f32)
        .into(),
    );

    let dl_card = section_card(
        tokens,
        envr_core::i18n::tr_key("gui.settings.downloads_section", "下载", "Downloads"),
        column![dl_row].spacing(sp.sm as f32).into(),
    );

    let runtime_layout_help = text(envr_core::i18n::tr_key(
        "gui.settings.runtime_layout_help",
        "在仪表盘「运行时概览」中可调整顺序、隐藏或恢复；此处可一键恢复默认。",
        "Reorder or hide runtimes from the dashboard overview; reset defaults here.",
    ))
    .size(ty.micro)
    .color(gui_theme::to_color(tokens.colors.text_muted));
    let reset_layout_btn = button(button_content_centered(
        text(envr_core::i18n::tr_key(
            "gui.runtime_layout.reset_defaults",
            "恢复默认排序与显示",
            "Reset order & visibility",
        ))
        .into(),
    ))
    .on_press(Message::RuntimeLayout(RuntimeLayoutMsg::ResetToDefaults))
    .height(Length::Fixed(
        tokens
            .control_height_secondary
            .max(tokens.min_click_target_px()),
    ))
    .padding([sp.sm as f32, sp.md as f32])
    .style(button_style(tokens, ButtonVariant::Secondary));

    let runtime_ui_card = section_card(
        tokens,
        envr_core::i18n::tr_key(
            "gui.settings.runtime_layout_section",
            "运行时显示",
            "Runtime display",
        ),
        column![runtime_layout_help, reset_layout_btn]
            .spacing(sp.md as f32)
            .into(),
    );

    column![
        paths_card,
        runtime_ui_card,
        look_card,
        dl_card,
        actions,
        status,
    ]
    .spacing(sp.lg as f32)
    .into()
}
