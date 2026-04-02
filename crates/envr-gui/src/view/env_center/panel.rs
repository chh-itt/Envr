//! Node / Python / Java / Go env center: lists, install, use, uninstall via `RuntimeService`.

use envr_config::settings::RuntimeInstallMode;
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
    DataLoaded(Result<(Vec<RuntimeVersion>, Option<RuntimeVersion>), String>),
    RemoteFetchedPrefix(Result<(String, Vec<RuntimeVersion>), String>),
    RemoteFetchedMajorKeys(Result<Vec<String>, String>),
    SubmitInstall(String),
    SubmitInstallAndUse(String),
    InstallFinished(Result<RuntimeVersion, String>),
    SubmitUse(String),
    UseFinished(Result<(), String>),
    SubmitUninstall(String),
    UninstallFinished(Result<(), String>),
    ToggleExpanded,
    ToggleExactMajor(String),
}

#[derive(Debug)]
pub struct EnvCenterState {
    pub kind: RuntimeKind,
    pub install_input: String,
    pub installed: Vec<RuntimeVersion>,
    pub current: Option<RuntimeVersion>,
    pub busy: bool,
    /// Node + Exact 时：仅加载 remote 的 `major` key，用于渲染可展开的分组行（不渲染所有子版本）。
    pub remote_major_keys: Vec<String>,
    pub remote_major_loading: bool,
    /// Remote versions fetched lazily by `major` prefix (e.g. `25` -> `25.x.x`).
    pub remote_cache: HashMap<String, Vec<RuntimeVersion>>,
    /// Expanded majors in Exact mode.
    pub expanded_exact_majors: HashSet<String>,
    /// Prefixes currently being fetched.
    pub remote_loading_majors: HashSet<String>,
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
            remote_major_keys: Vec::new(),
            remote_major_loading: false,
            remote_cache: HashMap::new(),
            expanded_exact_majors: HashSet::new(),
            remote_loading_majors: HashSet::new(),
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

fn version_row(
    state: &EnvCenterState,
    version: &RuntimeVersion,
    installed: bool,
    tokens: ThemeTokens,
    sp: &envr_ui::theme::SpacingScale,
    txt: iced::Color,
    install_mode: RuntimeInstallMode,
    busy: bool,
    indent_px: f32,
) -> Element<'static, Message> {
    let ver = version;
    let active = state.current.as_ref() == Some(ver);

    let use_msg = if !installed || active || busy {
        None
    } else {
        Some(Message::EnvCenter(EnvCenterMsg::SubmitUse(ver.0.clone())))
    };
    let uninstall_msg = if !installed || active || busy {
        None
    } else {
        Some(Message::EnvCenter(EnvCenterMsg::SubmitUninstall(ver.0.clone())))
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

    let install_msg = if installed || busy {
        None
    } else {
        Some(Message::EnvCenter(EnvCenterMsg::SubmitInstall(ver.0.clone())))
    };
    let install_and_use_msg = if installed || busy || install_mode != RuntimeInstallMode::Smart {
        None
    } else {
        Some(Message::EnvCenter(EnvCenterMsg::SubmitInstallAndUse(ver.0.clone())))
    };

    let pad_v = sp.sm as f32;
    let left_action: Element<'static, Message> = if installed {
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
        .style(button_style(tokens, ButtonVariant::Ghost))
        .into()
    } else {
        button(button_content_centered(
            row![
                Lucide::Download.view(14.0, txt),
                text(envr_core::i18n::tr_key("gui.action.install", "安装", "Install")),
            ]
            .spacing(sp.xs as f32)
            .align_y(Alignment::Center)
            .into(),
        ))
        .on_press_maybe(install_msg)
        .height(Length::Fixed(
            tokens
                .control_height_secondary
                .max(tokens.min_click_target_px()),
        ))
        .padding([pad_v, sp.md as f32])
        .style(button_style(tokens, ButtonVariant::Secondary))
        .into()
    };

    let right_action: Element<'static, Message> = if installed {
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
        .style(button_style(tokens, ButtonVariant::Danger))
        .into()
    } else if install_mode == RuntimeInstallMode::Smart {
        button(button_content_centered(
            row![
                Lucide::Download.view(14.0, txt),
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
        .on_press_maybe(install_and_use_msg)
        .height(Length::Fixed(
            tokens
                .control_height_secondary
                .max(tokens.min_click_target_px()),
        ))
        .padding([pad_v, sp.sm as f32])
        .style(button_style(tokens, ButtonVariant::Primary))
        .into()
    } else {
        // Exact mode: keep layout stable with an empty placeholder.
        container(space().width(Length::Shrink).height(Length::Fixed(
            tokens.control_height_secondary.max(tokens.min_click_target_px()),
        )))
        .into()
    };

    let card_s = card_container_style(tokens, 1);
    let row = row![
        space().width(Length::Fixed(indent_px)),
        text(ver_line).width(Length::Fill),
        left_action,
        right_action,
    ]
    .spacing(sp.sm as f32)
    .align_y(Alignment::Center)
    .height(Length::Fixed(tokens.list_row_height()));

    container(row).style(move |theme: &Theme| card_s(theme)).into()
}

pub fn env_center_view(
    state: &EnvCenterState,
    install_mode: RuntimeInstallMode,
    tokens: ThemeTokens,
) -> Element<'static, Message> {
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
    // In Exact mode, remote major keys are loaded lazily and used only to render
    // expandable groups, not all leaf versions.
    let mut major_keys_set: HashSet<String> = installed_by_major.keys().cloned().collect();
    for m in &state.remote_major_keys {
        major_keys_set.insert(m.clone());
    }
    let mut major_keys: Vec<String> = major_keys_set.into_iter().collect();
    major_keys.sort_by(|a, b| parse_major_num(a).cmp(&parse_major_num(b)).reverse());

    let matches_empty_hint = || -> Element<'static, Message> {
        // If search keyword is empty, this is typically "no installed versions"
        // (remote is lazily loaded in Exact mode).
        let (title, body) = if query_norm.is_empty() {
            (
                envr_core::i18n::tr_key(
                    "gui.empty.title.no_installed_versions",
                    "这里还没有已安装版本",
                    "No installed versions here",
                ),
                envr_core::i18n::tr_key(
                    "gui.empty.hint.no_installed_versions",
                    "在下方输入版本并安装；智能/精确模式在「设置」中调整。也可使用「刷新当前运行时」。",
                    "Enter a version below to install; Smart/Exact mode is in Settings. You can also Refresh the runtime.",
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

    match install_mode {
        RuntimeInstallMode::Smart => {
            // Smart mode shows one row per `major` group.
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

            if busy && show_majors.is_empty() {
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

                    // Keep left layout stable: "Use"/"Install" buttons on the right.
                    let highest_installed = installed_versions.first().cloned();

                    let left_text = if is_active {
                        format!(
                            "{} {} {}",
                            kind_label(state.kind),
                            major,
                            envr_core::i18n::tr_key(
                                "gui.runtime.current_tag",
                                "(当前)",
                                "(current)",
                            )
                        )
                    } else {
                        format!("{} {}", kind_label(state.kind), major)
                    };

                    let action_btn: Element<'static, Message> = if is_active || busy {
                        container(space()).into()
                    } else if let Some(highest) = highest_installed {
                        button(
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
                            highest.0.clone()
                        ))))
                        .style(button_style(tokens, ButtonVariant::Secondary))
                        .into()
                    } else {
                        // No installed versions under this major.
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
                            major.clone(),
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
                            major.clone(),
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
                            .height(Length::Fixed(tokens.list_row_height()))
                        )
                        .style(card_container_style(tokens, 1))
                        
                    );
                }
            }
        }
        RuntimeInstallMode::Exact => {
            // Exact mode shows major rows with expandable children.
            // Display majors that are installed and/or expanded.
            let mut show_majors_set: HashSet<String> = HashSet::new();
            for k in installed_by_major.keys() {
                show_majors_set.insert(k.clone());
            }
            for k in &state.remote_major_keys {
                show_majors_set.insert(k.clone());
            }
            for k in &state.expanded_exact_majors {
                show_majors_set.insert(k.clone());
            }
            if let Some(qm) = query_major.as_ref() {
                show_majors_set.insert(qm.clone());
            }

            // Apply search filter to major rows.
            if !query_norm.is_empty() {
                if let Some(qm) = query_major.as_ref() {
                    show_majors_set = [qm.clone()].into_iter().collect();
                } else {
                    show_majors_set = show_majors_set
                        .into_iter()
                        .filter(|m| m.contains(query_norm))
                        .collect();
                }
            }

            let mut show_majors: Vec<String> = show_majors_set.into_iter().collect();
            show_majors.sort_by(|a, b| parse_major_num(a).cmp(&parse_major_num(b)).reverse());

            if (busy || state.remote_major_loading) && show_majors.is_empty() {
                list_col = list_col.push(list_loading_skeleton(tokens, state.skeleton_phase));
            } else if show_majors.is_empty() {
                list_col = list_col.push(matches_empty_hint());
            } else {
                for major in show_majors.iter() {
                    let expanded = state.expanded_exact_majors.contains(major);
                    let current_major = state
                        .current
                        .as_ref()
                        .and_then(|v| parse_major_from_ver(&v.0));
                    let is_active = current_major.as_deref() == Some(major.as_str());

                    let expand_lbl = if expanded {
                        envr_core::i18n::tr_key("gui.action.collapse", "折叠", "Collapse")
                    } else {
                        envr_core::i18n::tr_key("gui.action.expand", "展开", "Expand")
                    };
                    let expand_btn: Element<'static, Message> = button(
                        button_content_centered(
                            row![
                                Lucide::ChevronsUpDown.view(16.0, txt),
                                text(expand_lbl),
                            ]
                            .spacing(sp.xs as f32)
                            .align_y(Alignment::Center)
                            .into(),
                        ),
                    )
                    .on_press_maybe(Some(Message::EnvCenter(EnvCenterMsg::ToggleExactMajor(
                        major.clone(),
                    ))))
                    .style(button_style(tokens, ButtonVariant::Secondary))
                    .into();

                    let major_title = if is_active {
                        format!(
                            "{} {} {}",
                            kind_label(state.kind),
                            major,
                            envr_core::i18n::tr_key(
                                "gui.runtime.current_tag",
                                "(当前)",
                                "(current)",
                            )
                        )
                    } else {
                        format!("{} {}", kind_label(state.kind), major)
                    };

                    list_col = list_col.push(
                        container(
                            row![
                                text(major_title).width(Length::Fill),
                                expand_btn,
                            ]
                            .spacing(sp.sm as f32)
                            .align_y(Alignment::Center)
                            .height(Length::Fixed(tokens.list_row_height()))
                        )
                        .style(card_container_style(tokens, 1))
                        
                    );

                    if expanded {
                        // Children (leaf version rows).
                            if state.remote_loading_majors.contains(major)
                            && !state.remote_cache.contains_key(major)
                        {
                            list_col = list_col.push(list_loading_skeleton(tokens, state.skeleton_phase));
                        } else {
                            let installed_leaf = installed_by_major
                                .get(major)
                                .cloned()
                                .unwrap_or_default();
                            let installed_set = installed_leaf.iter().map(|v| v.0.clone()).collect::<HashSet<_>>();

                            let mut remote_leaf = state
                                .remote_cache
                                .get(major)
                                .cloned()
                                .unwrap_or_default();
                            remote_leaf.sort_by(|a, b| semver_cmp_desc(&a.0, &b.0));
                            remote_leaf.retain(|v| !installed_set.contains(&v.0));

                            let mut leaf_versions: Vec<RuntimeVersion> = installed_leaf.into_iter().chain(remote_leaf).collect();

                            if !query_norm.is_empty() && query_major.as_deref() != Some(major.as_str()) {
                                leaf_versions = leaf_versions
                                    .into_iter()
                                    .filter(|v| v.0.contains(query_norm) || v.0.contains(query))
                                    .collect();
                            }

                            if leaf_versions.is_empty() {
                                list_col = list_col.push(
                                    container(
                                        text(envr_core::i18n::tr_key(
                                            "gui.empty.hint.no_installed_versions",
                                            "没有匹配的版本",
                                            "No matching versions",
                                        ))
                                        .size(ty.micro)
                                        .color(gui_theme::to_color(tokens.colors.text_muted))
                                        .width(Length::Fill)
                                        .align_x(Horizontal::Center)
                                        .align_y(iced::alignment::Vertical::Center),
                                    )
                                    .width(Length::Fill),
                                );
                            } else {
                                for v in leaf_versions.iter() {
                                    let installed = state.installed.iter().any(|x| x.0 == v.0);
                                    list_col = list_col.push(version_row(
                                        state,
                                        v,
                                        installed,
                                        tokens,
                                        sp,
                                        txt,
                                        install_mode,
                                        busy,
                                        sp.sm as f32,
                                    ));
                                }
                            }
                        }
                    }

                }
            }
        }
    }

    let search_ph = envr_core::i18n::tr_key(
        "gui.runtime.search_placeholder",
        "搜索版本（例如 24）",
        "Search versions (e.g. 24)",
    );

    let search: Element<'static, Message> = container(
        text_input(&search_ph, &state.install_input)
            .on_input(|s| Message::EnvCenter(EnvCenterMsg::InstallInput(s)))
            .padding(sp.sm)
            .width(Length::Fill)
            .style(text_input_style(tokens)),
    )
    .width(Length::Fill)
    .height(Length::Fixed(
        tokens
            .control_height_secondary
            .max(tokens.min_click_target_px()),
    ))
    .align_y(iced::alignment::Vertical::Center)
    .into();

    let muted = gui_theme::to_color(tokens.colors.text_muted);
    let mut col = column![header, search].spacing(sp.sm as f32).width(Length::Fill);
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
