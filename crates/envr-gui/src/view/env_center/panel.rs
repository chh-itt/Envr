//! Node / Python / Java / Go env center: lists, install, use, uninstall via `RuntimeService`.

use envr_domain::runtime::{RuntimeKind, RuntimeVersion};
use envr_ui::theme::ThemeTokens;
use iced::alignment::Horizontal;
use iced::widget::{button, column, container, row, rule, space, text, text_input};
use iced::{Alignment, Element, Length, Padding, Theme};

use std::collections::{HashMap, HashSet};

use crate::app::Message;
use crate::icons::Lucide;
use crate::theme as gui_theme;
use crate::view::empty_state::{EmptyTone, illustrative_block_compact};
use crate::widget_styles::{
    card_container_style, ButtonVariant, button_content_centered, button_style, text_input_style,
};

#[derive(Debug, Clone)]
pub enum EnvCenterMsg {
    PickKind(RuntimeKind),
    InstallInput(String),
    DirectInstallInput(String),
    DataLoaded(Result<(Vec<RuntimeVersion>, Option<RuntimeVersion>), String>),
    RemoteLatestDiskSnapshot(Vec<RuntimeVersion>),
    RemoteLatestRefreshed(Result<Vec<RuntimeVersion>, String>),
    SubmitInstall(String),
    SubmitInstallAndUse(String),
    SubmitDirectInstall,
    SubmitDirectInstallAndUse,
    InstallFinished(Result<RuntimeVersion, String>),
    SubmitUse(String),
    UseFinished(Result<(), String>),
    SubmitUninstall(String),
    UninstallFinished(Result<(), String>),
    ToggleExpanded,
}

#[derive(Debug)]
pub struct EnvCenterState {
    pub kind: RuntimeKind,
    pub install_input: String,
    pub installed: Vec<RuntimeVersion>,
    pub current: Option<RuntimeVersion>,
    pub busy: bool,
    /// Non-fatal remote fetch/parse error shown inline (keeps global UI usable).
    pub remote_error: Option<String>,
    /// Node: latest patch per major from cache/network (see `list_remote_latest_per_major`).
    pub node_remote_latest: Vec<RuntimeVersion>,
    /// Node: background refresh of `node_remote_latest` (TTL / index) is in flight.
    pub node_remote_refreshing: bool,
    /// Optional version spec for direct install (right of search).
    pub direct_install_input: String,
    /// 0..1 phase for skeleton shimmer (`tasks_gui.md` GUI-041).
    pub skeleton_phase: f32,
    /// Whether the env center panel is expanded (search + actions + list).
    pub expanded: bool,
}

impl Default for EnvCenterState {
    fn default() -> Self {
        Self {
            kind: RuntimeKind::Node,
            install_input: String::new(),
            installed: Vec::new(),
            current: None,
            busy: false,
            remote_error: None,
            node_remote_latest: Vec::new(),
            node_remote_refreshing: false,
            direct_install_input: String::new(),
            skeleton_phase: 0.0,
            expanded: true,
        }
    }
}

// (scroll_y is clamped locally during rendering; no persistent clamping helper needed)

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

fn remote_error_inline(tokens: ThemeTokens, error: &str) -> Element<'static, Message> {
    let ty = tokens.typography();
    let muted = gui_theme::to_color(tokens.colors.text_muted);
    let warn = gui_theme::to_color(tokens.colors.warning);
    let sp = tokens.space();
    container(
        row![
            Lucide::CircleAlert.view(16.0, warn),
            text(format!(
                "{}: {}",
                envr_core::i18n::tr_key("gui.runtime.remote_error", "远程列表不可用", "Remote list unavailable"),
                error
            ))
            .size(ty.caption)
            .color(muted)
            .width(Length::Fill),
        ]
        .spacing(sp.sm as f32)
        .align_y(Alignment::Center),
    )
    .padding(Padding::from([sp.sm as f32, sp.md as f32]))
    .style(card_container_style(tokens, 1))
    .into()
}

fn node_remote_latest_for_major(state: &EnvCenterState, major: &str) -> Option<RuntimeVersion> {
    state
        .node_remote_latest
        .iter()
        .find(|v| parse_major_from_ver(&v.0).as_deref() == Some(major))
        .cloned()
}

pub fn env_center_view(state: &EnvCenterState, tokens: ThemeTokens) -> Element<'static, Message> {
    let ty = tokens.typography();
    let sp = tokens.space();
    let busy = state.busy;
    let card_s = card_container_style(tokens, 1);

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

    let header_title = format!("{}设置", kind_label(state.kind));
    let toggle_lbl = if state.expanded {
        envr_core::i18n::tr_key("gui.action.collapse", "折叠", "Collapse")
    } else {
        envr_core::i18n::tr_key("gui.action.expand", "展开", "Expand")
    };

    let toggle_btn = button(
        button_content_centered(
            row![
                Lucide::ChevronsUpDown.view(16.0, gui_theme::to_color(tokens.colors.text)),
                text(toggle_lbl),
            ]
            .spacing(sp.xs as f32)
            .align_y(Alignment::Center)
            .into()
        ),
    )
    .on_press(Message::EnvCenter(EnvCenterMsg::ToggleExpanded))
    .height(Length::Fixed(tokens.control_height_secondary))
    .style(button_style(tokens, ButtonVariant::Secondary));

    let header_content = if state.expanded {
        row![
            text(header_title).size(ty.section),
            text(cur_line).size(ty.caption).color(gui_theme::to_color(tokens.colors.text_muted)),
            toggle_btn,
        ]
        .spacing(sp.sm as f32)
        .align_y(Alignment::Center)
        .width(Length::Fill)
    } else {
        row![text(header_title).size(ty.section), toggle_btn]
            .spacing(sp.sm as f32)
            .align_y(Alignment::Center)
            .width(Length::Fill)
    };

    let header = container(header_content)
        .padding(Padding::from([sp.md as f32, sp.md as f32]))
        .style(move |theme: &Theme| card_s(theme));

    let txt = gui_theme::to_color(tokens.colors.text);
    // When collapsed, only render the header row.
    if !state.expanded {
        return header.into();
    }

    // Search/filter text (we reuse `install_input` field).
    let query = state.install_input.trim();
    let query_norm = query.strip_prefix('v').unwrap_or(query);
    let query_major = parse_major_only(query_norm);

    // Group installed versions by `major`.
    let mut installed_by_major: HashMap<String, Vec<RuntimeVersion>> = HashMap::new();
    for v in &state.installed {
        if let Some(major) = parse_major_from_ver(&v.0) {
            installed_by_major.entry(major).or_default().push(v.clone());
        }
    }
    for (_major, versions) in installed_by_major.iter_mut() {
        versions.sort_by(|a, b| semver_cmp_desc(&a.0, &b.0));
    }

    // Sort major numbers high -> low.
    let mut major_keys_set: HashSet<String> = installed_by_major.keys().cloned().collect();
    if state.kind == RuntimeKind::Node {
        for v in &state.node_remote_latest {
            if let Some(m) = parse_major_from_ver(&v.0) {
                major_keys_set.insert(m);
            }
        }
    }
    let mut major_keys: Vec<String> = major_keys_set.into_iter().collect();
    major_keys.sort_by(|a, b| parse_major_num(a).cmp(&parse_major_num(b)).reverse());

    let matches_empty_hint = || -> Element<'static, Message> {
        let (title, body) = if query_norm.is_empty() {
            (
                envr_core::i18n::tr_key(
                    "gui.empty.title.no_installed_versions",
                    "这里还没有已安装版本",
                    "No installed versions here",
                ),
                envr_core::i18n::tr_key(
                    "gui.empty.hint.no_installed_versions",
                    "左侧筛选列表；右侧输入精确版本安装。远程列表会先读本地缓存再静默更新。",
                    "Filter on the left; enter an exact version on the right to install. Remote rows load from cache first, then refresh quietly.",
                ),
            )
        } else {
            (
                envr_core::i18n::tr_key(
                    "gui.empty.title.no_versions",
                    "没有匹配的版本",
                    "No matching versions",
                ),
                envr_core::i18n::tr_key(
                    "gui.empty.body.no_versions",
                    "换个关键字试试，或清空搜索以查看完整列表。",
                    "Try another keyword, or clear search to see the full list.",
                ),
            )
        };
        container(
            container(illustrative_block_compact(
                tokens,
                EmptyTone::Neutral,
                Lucide::Package,
                36.0,
                title,
                body,
                None,
            ))
            .align_x(Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center)
            .width(Length::Fill)
            .height(Length::Fill),
        )
        .width(Length::Fill)
        .into()
    };

    // Use spacing instead of dense horizontal rules for a calmer UI.
    let mut list_col = column![].spacing(sp.sm as f32).width(Length::Fill);

    let mut show_majors: Vec<String> = if let Some(qm) = query_major.as_ref() {
        vec![qm.clone()]
    } else if query_norm.is_empty() {
        major_keys.clone()
    } else {
        major_keys
            .into_iter()
            .filter(|m| m.contains(query_norm))
            .collect()
    };
    show_majors.sort_by(|a, b| parse_major_num(a).cmp(&parse_major_num(b)).reverse());

    let node_waiting_remote = state.kind == RuntimeKind::Node
        && state.node_remote_latest.is_empty()
        && state.node_remote_refreshing;

    if let Some(err) = state.remote_error.as_deref() {
        if state.kind == RuntimeKind::Node {
            list_col = list_col.push(remote_error_inline(tokens, err));
        }
    }

    if (busy && show_majors.is_empty()) || (node_waiting_remote && show_majors.is_empty()) {
        list_col = list_col.push(list_loading_skeleton(tokens, state.skeleton_phase));
    } else if show_majors.is_empty() {
        list_col = list_col.push(matches_empty_hint());
    } else {
        for major in show_majors.iter() {
            let installed_versions = installed_by_major
                .get(major)
                .cloned()
                .unwrap_or_default();
            let current_major = state
                .current
                .as_ref()
                .and_then(|v| parse_major_from_ver(&v.0));
            let is_active = current_major.as_deref() == Some(major.as_str());

            let highest_installed = installed_versions.first().cloned();

            let label_base = if state.kind == RuntimeKind::Node {
                if let Some(rv) = node_remote_latest_for_major(state, major) {
                    format!("{} {}", kind_label(state.kind), rv.0)
                } else {
                    format!("{} {}", kind_label(state.kind), major)
                }
            } else {
                format!("{} {}", kind_label(state.kind), major)
            };

            let left_text = if is_active {
                format!(
                    "{} {}",
                    label_base,
                    envr_core::i18n::tr_key(
                        "gui.runtime.current_tag",
                        "(当前)",
                        "(current)",
                    )
                )
            } else {
                label_base
            };

            let install_spec = || -> String {
                if state.kind == RuntimeKind::Node {
                    node_remote_latest_for_major(state, major)
                        .map(|v| v.0)
                        .unwrap_or_else(|| major.clone())
                } else {
                    major.clone()
                }
            };

            let action_btn: Element<'static, Message> = if is_active || busy {
                container(space()).into()
            } else if let Some(highest) = highest_installed {
                let use_btn = button(
                    button_content_centered(
                        row![
                            Lucide::Package.view(14.0, txt),
                            text(envr_core::i18n::tr_key("gui.action.use", "切换", "Use")),
                        ]
                        .spacing(sp.xs as f32)
                        .align_y(Alignment::Center)
                        .into(),
                    ),
                )
                .on_press_maybe(Some(Message::EnvCenter(EnvCenterMsg::SubmitUse(
                    highest.0.clone(),
                ))))
                .height(Length::Fixed(
                    tokens
                        .control_height_secondary
                        .max(tokens.min_click_target_px()),
                ))
                .padding([sp.sm as f32, sp.sm as f32])
                .style(button_style(tokens, ButtonVariant::Secondary));

                let uninstall_btn = button(
                    button_content_centered(
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
                    ),
                )
                .on_press_maybe(Some(Message::EnvCenter(EnvCenterMsg::SubmitUninstall(
                    highest.0.clone(),
                ))))
                .height(Length::Fixed(
                    tokens
                        .control_height_secondary
                        .max(tokens.min_click_target_px()),
                ))
                .padding([sp.sm as f32, sp.sm as f32])
                .style(button_style(tokens, ButtonVariant::Danger));

                container(
                    row![use_btn, uninstall_btn]
                        .spacing(sp.sm as f32)
                        .align_y(Alignment::Center),
                )
                .into()
            } else {
                let spec = install_spec();
                let install_btn = button(
                    button_content_centered(
                        row![
                            Lucide::Download.view(14.0, txt),
                            text(envr_core::i18n::tr_key("gui.action.install", "安装", "Install")),
                        ]
                        .spacing(sp.xs as f32)
                        .align_y(Alignment::Center)
                        .into(),
                    ),
                )
                .on_press_maybe(Some(Message::EnvCenter(EnvCenterMsg::SubmitInstall(
                    spec.clone(),
                ))))
                .style(button_style(tokens, ButtonVariant::Primary));

                let install_and_use_btn = button(
                    button_content_centered(
                        row![
                            Lucide::RefreshCw.view(14.0, txt),
                            text(envr_core::i18n::tr_key(
                                "gui.action.install_use",
                                "安装并切换",
                                "Install & Use"
                            )),
                        ]
                        .spacing(sp.xs as f32)
                        .align_y(Alignment::Center)
                        .into(),
                    ),
                )
                .on_press_maybe(Some(Message::EnvCenter(EnvCenterMsg::SubmitInstallAndUse(
                    spec,
                ))))
                .style(button_style(tokens, ButtonVariant::Secondary));

                container(
                    row![install_btn, install_and_use_btn]
                        .spacing(sp.sm as f32)
                        .align_y(Alignment::Center),
                )
                .into()
            };

            list_col = list_col.push(
                container(
                    row![
                        text(left_text).width(Length::Fill),
                        action_btn,
                    ]
                    .spacing(sp.sm as f32)
                    .align_y(Alignment::Center)
                    .height(Length::Fixed(tokens.list_row_height())),
                )
                .style(card_container_style(tokens, 1)),
            );
        }
    }

    let search_ph = envr_core::i18n::tr_key(
        "gui.runtime.search_placeholder",
        "筛选主版本（例如 24）",
        "Filter by major (e.g. 24)",
    );

    let direct_ph = envr_core::i18n::tr_key(
        "gui.runtime.direct_install_placeholder",
        "指定版本安装",
        "Exact version to install",
    );

    let ctrl_h = tokens
        .control_height_secondary
        .max(tokens.min_click_target_px());

    let search: Element<'static, Message> = container(
        text_input(&search_ph, &state.install_input)
            .on_input(|s| Message::EnvCenter(EnvCenterMsg::InstallInput(s)))
            .padding(sp.sm)
            .width(Length::Fill)
            .style(text_input_style(tokens)),
    )
    .width(Length::Fill)
    .height(Length::Fixed(ctrl_h))
    .align_y(iced::alignment::Vertical::Center)
    .into();

    let direct_input_el: Element<'static, Message> = container(
        text_input(&direct_ph, &state.direct_install_input)
            .on_input(|s| Message::EnvCenter(EnvCenterMsg::DirectInstallInput(s)))
            .padding(sp.sm)
            .width(Length::Fill)
            .style(text_input_style(tokens)),
    )
    .width(Length::Fill)
    .height(Length::Fixed(ctrl_h))
    .align_y(iced::alignment::Vertical::Center)
    .into();

    let direct_spec_nonempty = !state.direct_install_input.trim().is_empty();
    let direct_install_btn = button(
        button_content_centered(
            row![
                Lucide::Download.view(14.0, txt),
                text(envr_core::i18n::tr_key("gui.action.install", "安装", "Install")),
            ]
            .spacing(sp.xs as f32)
            .align_y(Alignment::Center)
            .into(),
        ),
    )
    .on_press_maybe(
        (direct_spec_nonempty && !busy)
            .then_some(Message::EnvCenter(EnvCenterMsg::SubmitDirectInstall)),
    )
    .height(Length::Fixed(ctrl_h))
    .padding([sp.sm as f32, sp.md as f32])
    .style(button_style(tokens, ButtonVariant::Primary));

    let direct_install_use_btn = button(
        button_content_centered(
            row![
                Lucide::RefreshCw.view(14.0, txt),
                text(envr_core::i18n::tr_key(
                    "gui.action.install_use",
                    "安装并切换",
                    "Install & Use",
                )),
            ]
            .spacing(sp.xs as f32)
            .align_y(Alignment::Center)
            .into(),
        ),
    )
    .on_press_maybe(
        (direct_spec_nonempty && !busy)
            .then_some(Message::EnvCenter(EnvCenterMsg::SubmitDirectInstallAndUse)),
    )
    .height(Length::Fixed(ctrl_h))
    .padding([sp.sm as f32, sp.md as f32])
    .style(button_style(tokens, ButtonVariant::Secondary));

    let filter_row = row![
        container(search)
            .width(Length::FillPortion(3))
            .height(Length::Fixed(ctrl_h)),
        container(direct_input_el)
            .width(Length::FillPortion(2))
            .height(Length::Fixed(ctrl_h)),
        direct_install_btn,
        direct_install_use_btn,
    ]
    .spacing(sp.sm as f32)
    .align_y(Alignment::Center);

    let muted = gui_theme::to_color(tokens.colors.text_muted);
    let mut col = column![header, filter_row]
        .spacing(sp.sm as f32)
        .width(Length::Fill);
    if busy {
        col = col.push(
            text(envr_core::i18n::tr_key(
                "gui.app.loading",
                "正在加载…",
                "Loading…",
            ))
            .size(ty.micro)
            .color(muted),
        );
    }
    col.push(list_col).into()
}

fn semver_parts(s: &str) -> Option<(u64, u64, u64)> {
    let t = s.trim().strip_prefix('v').unwrap_or(s.trim());
    let mut it = t.split('.');
    let a: u64 = it.next()?.parse().ok()?;
    let b: u64 = it.next()?.parse().ok()?;
    let c: u64 = it.next()?.parse().ok()?;
    if it.next().is_some() {
        return None;
    }
    Some((a, b, c))
}

fn semver_cmp_desc(a: &str, b: &str) -> std::cmp::Ordering {
    match (semver_parts(a), semver_parts(b)) {
        (Some((ma, m1, p)), Some((mb, n1, q))) => (mb, n1, q).cmp(&(ma, m1, p)),
        _ => b.cmp(a),
    }
}

fn parse_major_num(major: &str) -> u64 {
    major.parse::<u64>().unwrap_or(0)
}

fn parse_major_from_ver(ver: &str) -> Option<String> {
    semver_parts(ver).map(|(ma, _mi, _patch)| ma.to_string())
}

fn parse_major_only(s: &str) -> Option<String> {
    let t = s.trim().strip_prefix('v').unwrap_or(s.trim());
    if t.is_empty() {
        return None;
    }
    if t.chars().all(|c| c.is_ascii_digit()) {
        Some(t.to_string())
    } else {
        None
    }
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
