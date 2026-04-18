use iced::widget::{button, column, container, mouse_area, row, space, text};
use iced::{Alignment, Background, Element, Length, Padding, Theme};

use envr_config::settings::RuntimeLayoutSettings;
use envr_ui::theme::ThemeTokens;

use crate::app::{Message, Route};
use crate::icons::Lucide;
use crate::theme as gui_theme;
use crate::view::dashboard::state::{DashboardState, RuntimeRow};
use crate::view::env_center::kind_label;
use crate::view::runtime_layout::RuntimeLayoutMsg;
use crate::view::downloads::{DownloadPanelState, JobState};
use crate::view::empty_state::{EmptyTone, illustrative_block, illustrative_block_compact};
use crate::view::loading::loading_skeleton;
use crate::widget_styles::{
    ButtonVariant, button_content_centered, button_style, card_container_style,
};

pub fn dashboard_view(
    state: &DashboardState,
    downloads: &DownloadPanelState,
    runtime_layout: &RuntimeLayoutSettings,
    tokens: ThemeTokens,
) -> Element<'static, Message> {
    let ty = tokens.typography();
    let sp = tokens.space();
    let text_c = gui_theme::to_color(tokens.colors.text);
    let refresh_lbl = row![
        Lucide::RefreshCw.view(16.0, text_c),
        text(envr_core::i18n::tr_key(
            "gui.dashboard.refresh",
            "刷新",
            "Refresh",
        )),
    ]
    .spacing(sp.sm as f32)
    .align_y(Alignment::Center);
    let mut col = column![
        row![
            text(envr_core::i18n::tr_key(
                "gui.route.dashboard",
                "仪表盘",
                "Dashboard",
            ))
            .size(ty.page_title),
            space::horizontal(),
            button(button_content_centered(refresh_lbl.into()))
                .on_press(Message::Dashboard(
                    crate::view::dashboard::state::DashboardMsg::Refresh
                ))
                .height(Length::Fixed(
                    tokens
                        .control_height_primary
                        .max(tokens.min_click_target_px()),
                ))
                .padding([sp.sm as f32, sp.md as f32])
                .style(button_style(tokens, ButtonVariant::Secondary)),
        ]
        .align_y(Alignment::Center)
    ]
    .spacing(tokens.page_title_gap() as f32);

    if let Some(data) = state.data.as_ref() {
        if let Some(err) = state.last_error.as_deref() {
            let title = envr_core::i18n::tr_key(
                "gui.empty.title.dashboard_refresh_failed",
                "刷新仪表盘失败",
                "Couldn't refresh the dashboard",
            );
            let body = err.to_string();
            let hint = Some(envr_core::i18n::tr_key(
                "gui.empty.hint.dashboard_stale",
                "下方仍显示上次成功加载的数据。",
                "Below is the last data that loaded successfully.",
            ));
            col = col.push(illustrative_block(
                tokens,
                EmptyTone::Warning,
                Lucide::CircleAlert,
                36.0,
                title,
                body,
                hint,
            ));
        }
        col = col
            .push(runtime_overview_section(
                state,
                &data.rows,
                runtime_layout,
                tokens,
            ))
            .push(doctor_card(
                &data.runtime_root,
                &data.shims_dir,
                data.shims_empty,
                &data.issues,
                &data.recommendations,
                tokens,
            ))
            .push(recent_jobs_card(downloads, tokens))
            .push(recommended_actions_card(tokens));
        return col.into();
    }

    if let Some(err) = state.last_error.as_deref() {
        let title = envr_core::i18n::tr_key(
            "gui.empty.title.dashboard_error",
            "无法加载仪表盘",
            "Couldn't load dashboard",
        );
        let prefix = envr_core::i18n::tr_key(
            "gui.empty.body.dashboard_error_prefix",
            "发生了错误：",
            "Something went wrong:",
        );
        let body = format!("{prefix} {err}");
        let hint = Some(envr_core::i18n::tr_key(
            "gui.empty.hint.dashboard_error",
            "请检查本机权限、网络与防病毒软件，然后重试刷新。",
            "Check permissions, network, and security software, then try Refresh again.",
        ));
        col = col.push(illustrative_block(
            tokens,
            EmptyTone::Danger,
            Lucide::CircleAlert,
            40.0,
            title,
            body,
            hint,
        ));
        return col.into();
    }

    if state.busy {
        col = col.push(card(
            envr_core::i18n::tr_key(
                "gui.dashboard.loading_card",
                "正在加载仪表盘",
                "Loading dashboard",
            ),
            loading_skeleton(tokens, 0.35, tokens.list_skeleton_rows()),
            tokens,
        ));
        return col.into();
    }

    let title = envr_core::i18n::tr_key(
        "gui.empty.title.no_dashboard_data",
        "还没有仪表盘数据",
        "No dashboard data yet",
    );
    let body = envr_core::i18n::tr_key(
        "gui.empty.body.no_dashboard_data",
        "连接本机上的 envr core 后，可在此查看运行时与健康摘要。",
        "Once envr core responds on this machine, you'll see runtime and health summaries here.",
    );
    let hint = Some(envr_core::i18n::tr_key(
        "gui.empty.hint.no_dashboard_data",
        "请点击上方「刷新」。若首次使用，可先完成 CLI 初始化。",
        "Click Refresh above. On first use, finish CLI setup if needed.",
    ));
    col = col.push(illustrative_block(
        tokens,
        EmptyTone::Neutral,
        Lucide::LayoutDashboard,
        40.0,
        title,
        body,
        hint,
    ));
    col.into()
}

fn card(
    title: String,
    body: Element<'static, Message>,
    tokens: ThemeTokens,
) -> Element<'static, Message> {
    let card_s = card_container_style(tokens, 1);
    let ty = tokens.typography();
    let sp = tokens.space();
    let pad = tokens.card_padding_px();
    // Extra vertical + horizontal inset so copy is not visually "stuck" to card edges.
    let inset = Padding::from([pad + 6.0, pad + 4.0]);
    container(
        column![text(title).size(ty.section), body]
            .spacing(sp.md as f32)
            .align_x(Alignment::Start),
    )
    .padding(inset)
    .width(Length::Fill)
    .style(move |theme: &Theme| card_s(theme))
    .into()
}

fn runtime_overview_section(
    dash: &DashboardState,
    rows: &[RuntimeRow],
    layout: &RuntimeLayoutSettings,
    tokens: ThemeTokens,
) -> Element<'static, Message> {
    let ty = tokens.typography();
    let sp = tokens.space();
    let text_c = gui_theme::to_color(tokens.colors.text);
    let muted = gui_theme::to_color(tokens.colors.text_muted);
    let prim = gui_theme::to_color(tokens.colors.primary);
    let (visible_rows, hidden_rows) = crate::view::runtime_layout::partition_dashboard_rows(layout, rows);
    let editing = dash.runtime_overview_layout_editing;
    let hidden_collapsed = dash.runtime_overview_hidden_collapsed;

    let edit_btn_lbl = if editing {
        envr_core::i18n::tr_key(
            "gui.dashboard.runtime_overview_done_editing",
            "完成",
            "Done",
        )
    } else {
        envr_core::i18n::tr_key(
            "gui.dashboard.runtime_overview_edit_layout",
            "编辑布局",
            "Edit layout",
        )
    };
    let edit_btn = button(button_content_centered(text(edit_btn_lbl).into()))
        .on_press(Message::RuntimeLayout(
            RuntimeLayoutMsg::ToggleDashboardLayoutEditing,
        ))
        .height(Length::Fixed(
            tokens
                .control_height_secondary
                .max(tokens.min_click_target_px()),
        ))
        .padding([sp.sm as f32, sp.md as f32])
        .style(button_style(tokens, ButtonVariant::Secondary));

    let reset_btn = button(button_content_centered(
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
    .style(button_style(tokens, ButtonVariant::Ghost));

    let reset_or_space: Element<'static, Message> = if editing {
        reset_btn.into()
    } else {
        space::horizontal().into()
    };
    let header = row![
        text(envr_core::i18n::tr_key(
            "gui.dashboard.runtimes_overview",
            "运行时概览",
            "Runtimes overview",
        ))
        .size(ty.section),
        space::horizontal(),
        reset_or_space,
        edit_btn,
    ]
    .spacing(sp.sm as f32)
    .align_y(Alignment::Center);

    let legend = text(envr_core::i18n::tr_key(
        "gui.dashboard.runtime_overview_legend",
        "以下为各运行时的「已安装版本数」与「当前全局版本」；点击卡片进入对应页面。",
        "Installed count and current global version per runtime; click a card to open.",
    ))
    .size(ty.micro)
    .color(muted);

    let mut body = column![header, legend]
        .spacing(sp.sm as f32)
        .width(Length::Fill);
    for r in &visible_rows {
        body = body.push(runtime_overview_runtime_card(
            r,
            layout,
            editing,
            tokens,
            text_c,
            muted,
            prim,
        ));
    }

    if !hidden_rows.is_empty() {
        let n = hidden_rows.len();
        let hidden_title = envr_core::i18n::tr_key(
            "gui.dashboard.runtime_overview_hidden_section",
            "已隐藏（仍可在下方恢复）",
            "Hidden (restore below)",
        );
        let count_lbl = format!(
            "{} ({n})",
            envr_core::i18n::tr_key(
                "gui.dashboard.runtime_overview_hidden_count",
                "已隐藏",
                "Hidden",
            )
        );
        let toggle_hidden = button(button_content_centered(
            text(if hidden_collapsed {
                envr_core::i18n::tr_key(
                    "gui.dashboard.runtime_overview_expand_hidden",
                    "展开",
                    "Show",
                )
            } else {
                envr_core::i18n::tr_key(
                    "gui.dashboard.runtime_overview_collapse_hidden",
                    "收起",
                    "Hide",
                )
            })
            .into(),
        ))
        .on_press(Message::RuntimeLayout(
            RuntimeLayoutMsg::ToggleDashboardHiddenCollapsed,
        ))
        .height(Length::Fixed(
            tokens
                .control_height_secondary
                .max(tokens.min_click_target_px()),
        ))
        .padding([sp.sm as f32, sp.md as f32])
        .style(button_style(tokens, ButtonVariant::Ghost));

        let hidden_hdr = row![
            text(hidden_title).size(ty.body_small).color(muted),
            space::horizontal(),
            text(count_lbl).size(ty.caption).color(muted),
            space::horizontal(),
            toggle_hidden,
        ]
        .spacing(sp.sm as f32)
        .align_y(Alignment::Center);

        body = body.push(hidden_hdr);
        if !hidden_collapsed {
            for r in &hidden_rows {
                body = body.push(runtime_overview_runtime_card(
                    r,
                    layout,
                    editing,
                    tokens,
                    text_c,
                    muted,
                    prim,
                ));
            }
        }
    }

    let card_s = card_container_style(tokens, 2);
    let pad = tokens.card_padding_px();
    let inset = Padding::from([pad + 6.0, pad + 4.0]);
    container(
        column![body]
            .spacing(sp.sm as f32)
            .width(Length::Fill),
    )
    .padding(inset)
    .width(Length::Fill)
    .style(move |theme: &Theme| card_s(theme))
    .into()
}

fn runtime_overview_runtime_card(
    r: &RuntimeRow,
    layout: &RuntimeLayoutSettings,
    editing: bool,
    tokens: ThemeTokens,
    text_c: iced::Color,
    muted: iced::Color,
    prim: iced::Color,
) -> Element<'static, Message> {
    let ty = tokens.typography();
    let sp = tokens.space();
    let label = kind_label(r.kind);
    let cur = r.current.clone().unwrap_or_else(|| {
        envr_core::i18n::tr_key("gui.dashboard.not_set", "(未设置)", "(none)")
    });
    let summary_tpl = envr_core::i18n::tr_key(
        "gui.dashboard.runtime_card_summary",
        "已安装 {installed} 个 · 当前 {current}",
        "{installed} installed · current {current}",
    );
    let detail = summary_tpl
        .replace("{installed}", &r.installed.to_string())
        .replace("{current}", &cur);
    let kind = r.kind;
    let is_hidden = crate::view::runtime_layout::is_kind_hidden(layout, kind);

    let accent = container(space().width(Length::Fixed(4.0)))
        .height(Length::Fixed(52.0))
        .style(move |_theme: &Theme| {
            container::Style::default()
                .background(Background::Color(prim.scale_alpha(if is_hidden { 0.35 } else { 0.85 })))
        });

    let title_col = column![
        text(label).size(ty.body),
        text(detail).size(ty.caption).color(muted),
    ]
    .spacing(3.0)
    .width(Length::Fill);

    let chevron = text("→")
        .size(ty.section)
        .color(muted.scale_alpha(0.75));

    let hide_lbl = if is_hidden {
        envr_core::i18n::tr_key("gui.dashboard.runtime_card_show", "显示", "Show")
    } else {
        envr_core::i18n::tr_key("gui.dashboard.runtime_card_hide", "隐藏", "Hide")
    };
    let hide_btn = button(button_content_centered(
        row![
            Lucide::EyeOff.view(14.0, text_c),
            text(hide_lbl),
        ]
        .spacing(sp.xs as f32)
        .align_y(Alignment::Center)
        .into(),
    ))
    .on_press(Message::RuntimeLayout(RuntimeLayoutMsg::ToggleHidden(kind)))
    .height(Length::Fixed(
        tokens
            .control_height_secondary
            .max(tokens.min_click_target_px()),
    ))
    .padding([sp.sm as f32, sp.sm as f32])
    .style(button_style(tokens, ButtonVariant::Ghost));

    let btn_h = tokens
        .control_height_secondary
        .max(tokens.min_click_target_px());
    let up_btn = button(button_content_centered(text("↑").into()))
        .on_press(Message::RuntimeLayout(RuntimeLayoutMsg::MoveRuntime {
            kind,
            delta: -1,
        }))
        .height(Length::Fixed(btn_h))
        .width(Length::Fixed(btn_h.max(40.0)))
        .style(button_style(tokens, ButtonVariant::Secondary));
    let down_btn = button(button_content_centered(text("↓").into()))
        .on_press(Message::RuntimeLayout(RuntimeLayoutMsg::MoveRuntime {
            kind,
            delta: 1,
        }))
        .height(Length::Fixed(btn_h))
        .width(Length::Fixed(btn_h.max(40.0)))
        .style(button_style(tokens, ButtonVariant::Secondary));

    let inner: Element<'static, Message> = if editing {
        row![
            up_btn,
            down_btn,
            accent,
            title_col,
            hide_btn,
        ]
        .spacing(sp.sm as f32)
        .align_y(Alignment::Center)
        .into()
    } else {
        let main_row = row![
            accent,
            title_col,
            chevron,
        ]
        .spacing(sp.md as f32)
        .align_y(Alignment::Center)
        .width(Length::Fill);
        let tappable = mouse_area(main_row)
            .on_press(Message::RuntimeLayout(RuntimeLayoutMsg::OpenRuntime(kind)))
            .interaction(iced::mouse::Interaction::Pointer);
        row![
            container(tappable).width(Length::Fill),
            hide_btn,
        ]
        .spacing(sp.sm as f32)
        .align_y(Alignment::Center)
        .into()
    };

    let inner_card = card_container_style(tokens, if is_hidden { 0 } else { 1 });
    container(inner)
        .width(Length::Fill)
        .padding(Padding::from([sp.sm as f32, sp.md as f32]))
        .style(move |theme: &Theme| inner_card(theme))
        .into()
}

fn doctor_card(
    runtime_root: &str,
    shims_dir: &str,
    shims_empty: bool,
    issues: &[String],
    recs: &[String],
    tokens: ThemeTokens,
) -> Element<'static, Message> {
    let ty = tokens.typography();
    let sp = tokens.space();
    let mut body = column![
        text(format!(
            "{}: {runtime_root}",
            envr_core::i18n::tr_key("gui.dashboard.runtime_root", "运行时根目录", "Runtime root",)
        ))
        .size(ty.caption)
    ]
    .spacing((sp.xs + 2) as f32);
    let shims_suffix = if shims_empty {
        envr_core::i18n::tr_key("gui.dashboard.shims_empty", "（空）", " (empty)")
    } else {
        String::new()
    };
    body = body.push(text(format!(
        "{}: {shims_dir}{shims_suffix}",
        envr_core::i18n::tr_key("gui.label.shims", "Shims", "Shims"),
    )));

    if issues.is_empty() {
        body = body.push(text(envr_core::i18n::tr_key(
            "gui.dashboard.health_check_ok",
            "健康检查：通过",
            "Health check: OK",
        )));
    } else {
        body = body.push(text(envr_core::i18n::tr_key(
            "gui.dashboard.health_check_issues",
            "健康检查：发现问题",
            "Health check: issues found",
        )));
        for i in issues {
            body = body.push(text(format!("- {i}")).size(ty.micro));
        }
    }
    if !recs.is_empty() {
        body = body.push(
            text(envr_core::i18n::tr_key(
                "gui.dashboard.suggestions_label",
                "建议：",
                "Suggestions:",
            ))
            .size(ty.caption),
        );
        for r in recs {
            body = body.push(text(format!("- {r}")).size(ty.micro));
        }
    }

    card(
        envr_core::i18n::tr_key("gui.dashboard.health_card_title", "健康检查", "Health"),
        body.into(),
        tokens,
    )
}

fn recent_jobs_card(
    downloads: &DownloadPanelState,
    tokens: ThemeTokens,
) -> Element<'static, Message> {
    let ty = tokens.typography();
    let sp = tokens.space();
    let mut body = column![].spacing((sp.xs + 2) as f32);
    if downloads.jobs.is_empty() {
        let title = envr_core::i18n::tr_key(
            "gui.empty.title.no_recent_activity",
            "暂无最近任务",
            "No recent activity",
        );
        let sub = envr_core::i18n::tr_key(
            "gui.empty.body.no_recent_activity",
            "安装或下载完成后，这里会显示最近几条记录。",
            "After installs or downloads finish, recent entries show up here.",
        );
        let hint = Some(envr_core::i18n::tr_key(
            "gui.empty.hint.no_recent_activity",
            "进行中的任务可在左下角「下载」面板查看。",
            "In-progress work stays in the Downloads panel (bottom-left).",
        ));
        body = body.push(illustrative_block_compact(
            tokens,
            EmptyTone::Neutral,
            Lucide::Download,
            30.0,
            title,
            sub,
            hint,
        ));
    } else {
        for j in downloads.jobs.iter().rev().take(5) {
            let st = match j.state {
                JobState::Running => {
                    envr_core::i18n::tr_key("gui.job.running", "进行中", "Running")
                }
                JobState::Done => envr_core::i18n::tr_key("gui.job.done", "完成", "Done"),
                JobState::Failed => envr_core::i18n::tr_key("gui.job.failed", "失败", "Failed"),
                JobState::Cancelled => {
                    envr_core::i18n::tr_key("gui.job.cancelled", "已取消", "Cancelled")
                }
            };
            let line = if j.url.is_empty() {
                format!("{} · {}", j.label, st)
            } else {
                format!("{} · {} · {}", j.label, st, j.url)
            };
            body = body.push(text(line).size(ty.micro));
        }
    }
    card(
        envr_core::i18n::tr_key(
            "gui.dashboard.recent_activity",
            "最近任务",
            "Recent activity",
        ),
        body.into(),
        tokens,
    )
}

fn recommended_actions_card(tokens: ThemeTokens) -> Element<'static, Message> {
    let sp = tokens.space();
    let txt = gui_theme::to_color(tokens.colors.text);
    let actions = row![
        button(button_content_centered(
            row![
                Lucide::Package.view(16.0, txt),
                text(envr_core::i18n::tr_key(
                    "gui.dashboard.open_runtimes",
                    "打开运行时",
                    "Open runtimes",
                )),
            ]
            .spacing(sp.sm as f32)
            .align_y(Alignment::Center)
            .into(),
        ))
        .on_press(Message::Navigate(Route::Runtime))
        .width(Length::FillPortion(1))
        .height(Length::Fixed(
            tokens
                .control_height_secondary
                .max(tokens.min_click_target_px()),
        ))
        .padding([sp.sm as f32, sp.md as f32])
        .style(button_style(tokens, ButtonVariant::Secondary)),
        button(button_content_centered(
            row![
                Lucide::Settings.view(16.0, txt),
                text(envr_core::i18n::tr_key(
                    "gui.dashboard.open_settings",
                    "打开设置",
                    "Open settings",
                )),
            ]
            .spacing(sp.sm as f32)
            .align_y(Alignment::Center)
            .into(),
        ))
        .on_press(Message::Navigate(Route::Settings))
        .width(Length::FillPortion(1))
        .height(Length::Fixed(
            tokens
                .control_height_secondary
                .max(tokens.min_click_target_px()),
        ))
        .padding([sp.sm as f32, sp.md as f32])
        .style(button_style(tokens, ButtonVariant::Secondary)),
    ]
    .spacing((sp.sm + 2) as f32);
    card(
        envr_core::i18n::tr_key(
            "gui.dashboard.recommended_actions",
            "推荐操作",
            "Recommended actions",
        ),
        actions.into(),
        tokens,
    )
}

