//! Node / Python / Java / Go env center: lists, install, use, uninstall via `RuntimeService`.

use envr_domain::runtime::{RuntimeKind, RuntimeVersion};
use envr_ui::theme::ThemeTokens;
use iced::alignment::Horizontal;
use iced::widget::{
    button, column, container, row, rule, scrollable, space, text, text_input, Id,
};
use iced::{Alignment, Element, Length, Theme};

use crate::app::Message;
use crate::icons::Lucide;
use crate::theme as gui_theme;
use crate::view::empty_state::{EmptyTone, illustrative_block};
use crate::widget_styles::{
    ButtonVariant, button_content_centered, button_style, section_card, text_input_style,
};

/// Fixed list viewport height (`tasks_gui.md` GUI-021); keeps layout stable vs. skeleton rows.
const ENV_LIST_VIEWPORT_H: f32 = 260.0;

/// [`Id`] on the installed-versions scrollable (scroll position + `scroll_to` sync).
pub const ENV_INSTALLED_LIST_SCROLL_ID: &str = "envr-env-installed-list";

const LIST_SEP_PX: f32 = 1.0;
const VIRT_OVERSCAN: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VersionMode {
    #[default]
    Smart,
    Exact,
}

#[derive(Debug, Clone)]
pub enum EnvCenterMsg {
    PickKind(RuntimeKind),
    SetMode(VersionMode),
    InstallInput(String),
    Refresh,
    DataLoaded(Result<(Vec<RuntimeVersion>, Option<RuntimeVersion>), String>),
    SubmitInstall,
    SubmitInstallAndUse,
    InstallFinished(Result<RuntimeVersion, String>),
    SubmitUse(String),
    UseFinished(Result<(), String>),
    SubmitUninstall(String),
    UninstallFinished(Result<(), String>),
    /// Vertical scroll offset (px) in the installed-versions list.
    ListScroll(f32),
}

#[derive(Debug)]
pub struct EnvCenterState {
    pub kind: RuntimeKind,
    pub mode: VersionMode,
    pub install_input: String,
    pub installed: Vec<RuntimeVersion>,
    pub current: Option<RuntimeVersion>,
    pub busy: bool,
    pub list_scroll_y: f32,
    /// 0..1 phase for skeleton shimmer (`tasks_gui.md` GUI-041).
    pub skeleton_phase: f32,
}

impl Default for EnvCenterState {
    fn default() -> Self {
        Self {
            kind: RuntimeKind::Node,
            mode: VersionMode::default(),
            install_input: String::new(),
            installed: Vec::new(),
            current: None,
            busy: false,
            list_scroll_y: 0.0,
            skeleton_phase: 0.0,
        }
    }
}

impl EnvCenterState {
    pub fn clamp_list_scroll(&mut self, tokens: ThemeTokens) {
        let n = self.installed.len();
        let row_h = tokens.list_row_height();
        let total = list_total_height(n, row_h);
        let max_y = (total - ENV_LIST_VIEWPORT_H).max(0.0);
        self.list_scroll_y = self.list_scroll_y.clamp(0.0, max_y);
    }
}

pub(crate) fn kind_label(kind: RuntimeKind) -> &'static str {
    match kind {
        RuntimeKind::Node => "Node",
        RuntimeKind::Python => "Python",
        RuntimeKind::Java => "Java",
        RuntimeKind::Go => "Go",
        RuntimeKind::Rust => "Rust",
        RuntimeKind::Php => "PHP",
        RuntimeKind::Deno => "Deno",
        RuntimeKind::Bun => "Bun",
    }
}

fn installed_version_row(
    state: &EnvCenterState,
    idx: usize,
    tokens: ThemeTokens,
    sp: &envr_ui::theme::SpacingScale,
    txt: iced::Color,
    busy: bool,
) -> Element<'static, Message> {
    let ver = &state.installed[idx];
    let active = state.current.as_ref() == Some(ver);
    let use_msg = if active || busy {
        None
    } else {
        Some(Message::EnvCenter(EnvCenterMsg::SubmitUse(ver.0.clone())))
    };
    let uninstall_msg = if active || busy {
        None
    } else {
        Some(Message::EnvCenter(EnvCenterMsg::SubmitUninstall(
            ver.0.clone(),
        )))
    };

    let ver_line = if active {
        format!(
            "{} {} {}",
            kind_label(state.kind),
            ver.0,
            envr_core::i18n::tr_key("gui.runtime.current_tag", "(当前)", "(current)",)
        )
    } else {
        format!("{} {}", kind_label(state.kind), ver.0)
    };

    let pad_v = sp.sm as f32;
    row![
        text(ver_line).width(Length::Fill),
        button(button_content_centered(
            row![
                Lucide::Package.view(14.0, txt),
                text(envr_core::i18n::tr_key("gui.action.use", "切换", "Use")),
            ]
            .spacing(sp.xs as f32)
            .align_y(Alignment::Center)
            .into(),
        ))
        .on_press_maybe(use_msg)
        .height(Length::Fixed(
            tokens
                .control_height_secondary
                .max(tokens.min_click_target_px()),
        ))
        .padding([pad_v, sp.sm as f32])
        .style(button_style(tokens, ButtonVariant::Ghost)),
        button(button_content_centered(
            row![
                Lucide::X.view(14.0, gui_theme::to_color(tokens.colors.danger)),
                text(envr_core::i18n::tr_key(
                    "gui.action.uninstall",
                    "卸载",
                    "Uninstall",
                )),
            ]
            .spacing(sp.xs as f32)
            .align_y(Alignment::Center)
            .into(),
        ))
        .on_press_maybe(uninstall_msg)
        .height(Length::Fixed(
            tokens
                .control_height_secondary
                .max(tokens.min_click_target_px()),
        ))
        .padding([pad_v, sp.sm as f32])
        .style(button_style(tokens, ButtonVariant::Danger)),
    ]
    .spacing(sp.sm as f32)
    .align_y(Alignment::Center)
    .height(Length::Fixed(tokens.list_row_height()))
    .into()
}

pub fn env_center_view(state: &EnvCenterState, tokens: ThemeTokens) -> Element<'static, Message> {
    let ty = tokens.typography();
    let sp = tokens.space();
    let busy = state.busy;

    let cur_line = match &state.current {
        Some(v) => format!(
            "{} {}",
            envr_core::i18n::tr_key("gui.runtime.current", "当前：", "Current:"),
            v.0
        ),
        None => envr_core::i18n::tr_key(
            "gui.runtime.current_none",
            "当前：(未设置)",
            "Current: (not set)",
        ),
    };

    let mut mode_row = row![
        text(envr_core::i18n::tr_key(
            "gui.runtime.mode_label",
            "模式",
            "Mode"
        ))
        .size(ty.body_small)
    ]
    .spacing(sp.sm as f32);
    for (m, key, zh, en) in [
        (
            VersionMode::Smart,
            "gui.runtime.mode.smart",
            "智能（Smart）",
            "Smart",
        ),
        (
            VersionMode::Exact,
            "gui.runtime.mode.exact",
            "精确（Exact）",
            "Exact",
        ),
    ] {
        let variant = if m == state.mode {
            ButtonVariant::Primary
        } else {
            ButtonVariant::Secondary
        };
        let h = if m == state.mode {
            tokens.control_height_primary
        } else {
            tokens.control_height_secondary
        }
        .max(tokens.min_click_target_px());
        let b = button(button_content_centered(
            text(envr_core::i18n::tr_key(key, zh, en)).into(),
        ))
        .on_press(Message::EnvCenter(EnvCenterMsg::SetMode(m)))
        .width(Length::FillPortion(1))
        .height(Length::Fixed(h))
        .padding([sp.sm as f32, (sp.sm + 2) as f32])
        .style(button_style(tokens, variant));
        mode_row = mode_row.push(b);
    }

    let input = state.install_input.trim();
    let installed_match = state.installed.iter().any(|v| v.0 == input);
    let current_match = state.current.as_ref().is_some_and(|c| c.0 == input);

    let spec_field = container(
        text_input(
            &envr_core::i18n::tr_key(
                "gui.runtime.spec_placeholder",
                "版本 spec（Smart）或精确版本（Exact）",
                "Version spec (Smart) or exact version (Exact)",
            ),
            &state.install_input,
        )
        .on_input(|s| Message::EnvCenter(EnvCenterMsg::InstallInput(s)))
        .width(Length::Fill)
        .padding(sp.sm)
        .style(text_input_style(tokens)),
    )
    .width(Length::Fill)
    .height(Length::Fixed(
        tokens
            .control_height_secondary
            .max(tokens.min_click_target_px()),
    ))
    .align_y(iced::alignment::Vertical::Center);

    let txt = gui_theme::to_color(tokens.colors.text);
    let pad_v = sp.sm as f32;
    let install_row = row![
        spec_field,
        button(button_content_centered(
            row![
                Lucide::Download.view(15.0, txt),
                text(envr_core::i18n::tr_key(
                    "gui.action.install",
                    "安装",
                    "Install",
                )),
            ]
            .spacing(sp.xs as f32)
            .align_y(Alignment::Center)
            .into(),
        ))
        .on_press_maybe(
            (!busy && !input.is_empty() && (state.mode == VersionMode::Smart || !installed_match))
                .then_some(Message::EnvCenter(EnvCenterMsg::SubmitInstall)),
        )
        .height(Length::Fixed(
            tokens
                .control_height_secondary
                .max(tokens.min_click_target_px()),
        ))
        .padding([pad_v, sp.md as f32])
        .style(button_style(tokens, ButtonVariant::Secondary)),
        button(button_content_centered(
            row![
                Lucide::Download.view(15.0, txt),
                text(envr_core::i18n::tr_key(
                    "gui.action.install_use",
                    "安装并切换",
                    "Install & Use",
                )),
            ]
            .spacing(sp.xs as f32)
            .align_y(Alignment::Center)
            .into(),
        ))
        .on_press_maybe(
            (state.mode == VersionMode::Smart && !busy && !input.is_empty() && !installed_match)
                .then_some(Message::EnvCenter(EnvCenterMsg::SubmitInstallAndUse)),
        )
        .height(Length::Fixed(
            tokens
                .control_height_secondary
                .max(tokens.min_click_target_px()),
        ))
        .padding([pad_v, sp.md as f32])
        .style(button_style(tokens, ButtonVariant::Primary)),
        button(button_content_centered(
            row![
                Lucide::Package.view(15.0, txt),
                text(envr_core::i18n::tr_key("gui.action.use", "切换", "Use")),
            ]
            .spacing(sp.xs as f32)
            .align_y(Alignment::Center)
            .into(),
        ))
        .on_press_maybe(
            (!busy && !input.is_empty() && installed_match && !current_match).then_some(
                Message::EnvCenter(EnvCenterMsg::SubmitUse(input.to_string())),
            ),
        )
        .height(Length::Fixed(
            tokens
                .control_height_secondary
                .max(tokens.min_click_target_px()),
        ))
        .padding([pad_v, sp.md as f32])
        .style(button_style(tokens, ButtonVariant::Secondary)),
    ]
    .spacing((sp.sm + 2) as f32)
    .align_y(Alignment::Center);

    let list_content: Element<'static, Message> = if busy && state.installed.is_empty() {
        container(list_loading_skeleton(tokens, state.skeleton_phase))
            .width(Length::Fill)
            .height(Length::Fixed(ENV_LIST_VIEWPORT_H))
            .into()
    } else if state.installed.is_empty() {
        let title = envr_core::i18n::tr_key(
            "gui.empty.title.no_installed_versions",
            "这里还没有已安装版本",
            "No installed versions here",
        );
        let body = envr_core::i18n::tr_key(
            "gui.empty.body.no_installed_versions",
            "安装成功后，已安装列表会出现在此区域。",
            "After you install a version, it will appear in this list.",
        );
        let hint = Some(envr_core::i18n::tr_key(
            "gui.empty.hint.no_installed_versions",
            "在上方输入版本 spec 并点击安装，或使用「刷新当前运行时」后再试。",
            "Enter a spec above and install, or Refresh the current runtime.",
        ));
        container(
            container(illustrative_block(
                tokens,
                EmptyTone::Neutral,
                Lucide::Package,
                36.0,
                title,
                body,
                hint,
            ))
            .align_x(Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center)
            .width(Length::Fill)
            .height(Length::Fill),
        )
        .width(Length::Fill)
        .height(Length::Fixed(ENV_LIST_VIEWPORT_H))
        .into()
    } else {
        let row_h = tokens.list_row_height();
        let n = state.installed.len();
        let scroll_on = |v: iced::widget::scrollable::Viewport| {
            Message::EnvCenter(EnvCenterMsg::ListScroll(v.absolute_offset().y))
        };

        let list_col: Element<'static, Message> = if n >= tokens.list_virtualize_min_row_count() {
            let scroll_y = state.list_scroll_y;
            let first = first_visible_row(scroll_y, n, row_h).saturating_sub(VIRT_OVERSCAN);
            let last = (last_visible_row(scroll_y, ENV_LIST_VIEWPORT_H, n, row_h) + VIRT_OVERSCAN)
                .min(n.saturating_sub(1));
            let top_h = list_prefix_height(first, n, row_h);
            let bottom_h = list_total_height(n, row_h) - list_prefix_height(last + 1, n, row_h);
            let mut col = column![].spacing(0);
            if top_h > 0.5 {
                col = col.push(space().height(Length::Fixed(top_h)));
            }
            for i in first..=last {
                col = col.push(installed_version_row(state, i, tokens, sp, txt, busy));
                if i + 1 < n {
                    col = col.push(rule::horizontal(1.0));
                }
            }
            if bottom_h > 0.5 {
                col = col.push(space().height(Length::Fixed(bottom_h)));
            }
            scrollable(col)
                .id(Id::new(ENV_INSTALLED_LIST_SCROLL_ID))
                .on_scroll(scroll_on)
                .width(Length::Fill)
                .height(Length::Fixed(ENV_LIST_VIEWPORT_H))
                .into()
        } else {
            let mut col = column![].spacing(0);
            for i in 0..n {
                col = col.push(installed_version_row(state, i, tokens, sp, txt, busy));
                if i + 1 < n {
                    col = col.push(rule::horizontal(1.0));
                }
            }
            scrollable(col)
                .id(Id::new(ENV_INSTALLED_LIST_SCROLL_ID))
                .on_scroll(scroll_on)
                .width(Length::Fill)
                .height(Length::Fixed(ENV_LIST_VIEWPORT_H))
                .into()
        };
        list_col
    };

    let runtime_title = format!(
        "{}: {}",
        envr_core::i18n::tr_key("gui.runtime.title", "运行时", "Runtime"),
        kind_label(state.kind)
    );

    let context_card = section_card(
        tokens,
        runtime_title,
        column![
            mode_row,
            text(cur_line).size(ty.body_small),
        ]
        .spacing(sp.md as f32)
        .width(Length::Fill)
        .into(),
    );

    let install_card = section_card(
        tokens,
        envr_core::i18n::tr_key("gui.action.install", "安装", "Install"),
        container(install_row)
            .width(Length::Fill)
            .into(),
    );

    let installed_card = section_card(
        tokens,
        envr_core::i18n::tr_key("gui.runtime.installed", "已安装", "Installed"),
        list_content,
    );

    column![context_card, install_card, installed_card]
        .spacing(sp.lg as f32)
        .width(Length::Fill)
        .into()
}

fn list_loading_skeleton(tokens: ThemeTokens, phase: f32) -> Element<'static, Message> {
    use iced::Background;

    let pulse = (phase * std::f32::consts::TAU).sin() * 0.5 + 0.5;
    let fill = gui_theme::to_color(tokens.colors.text_muted).scale_alpha(0.07 + 0.16 * pulse);
    let row_h = tokens.list_row_height();
    let bar_h = row_h * 0.42;
    let n = tokens.list_skeleton_rows();
    let mut col = column![].spacing(0);
    for i in 0..n {
        col = col.push(
            container(
                space()
                    .width(Length::Fill)
                    .height(Length::Fixed(bar_h)),
            )
            .width(Length::Fill)
            .height(Length::Fixed(row_h))
            .align_y(iced::alignment::Vertical::Center)
            .padding([0, tokens.space().md as u16])
            .style(move |_theme: &Theme| {
                iced::widget::container::Style::default().background(Background::Color(fill))
            }),
        );
        if i + 1 < n {
            col = col.push(rule::horizontal(1.0));
        }
    }
    col.into()
}

fn list_prefix_height(k: usize, n: usize, row_h: f32) -> f32 {
    if k == 0 || n == 0 {
        return 0.0;
    }
    let k = k.min(n);
    k as f32 * row_h + k.saturating_sub(1).min(n.saturating_sub(1)) as f32 * LIST_SEP_PX
}

fn list_total_height(n: usize, row_h: f32) -> f32 {
    if n == 0 {
        0.0
    } else {
        n as f32 * row_h + (n - 1) as f32 * LIST_SEP_PX
    }
}

fn first_visible_row(scroll_y: f32, n: usize, row_h: f32) -> usize {
    if n == 0 || scroll_y <= 0.0 {
        return 0;
    }
    let total = list_prefix_height(n, n, row_h);
    if scroll_y >= total {
        return n.saturating_sub(1);
    }
    let mut lo = 0usize;
    let mut hi = n.saturating_sub(1);
    while lo < hi {
        let mid = (lo + hi) / 2;
        if list_prefix_height(mid + 1, n, row_h) <= scroll_y {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}

fn last_visible_row(scroll_y: f32, viewport_h: f32, n: usize, row_h: f32) -> usize {
    if n == 0 {
        return 0;
    }
    let target = scroll_y + viewport_h;
    let tot = list_total_height(n, row_h);
    if target >= tot {
        return n - 1;
    }
    let mut lo = 0usize;
    let mut hi = n - 1;
    while lo < hi {
        let mid = (lo + hi + 1) / 2;
        if list_prefix_height(mid, n, row_h) < target {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    lo.min(n - 1)
}
