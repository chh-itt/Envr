use iced::widget::{button, column, container, row, space, text};
use iced::{Alignment, Element, Length, Padding, Theme};

use envr_domain::runtime::RuntimeKind;
use envr_ui::theme::ThemeTokens;

use crate::app::{Message, Route};
use crate::icons::Lucide;
use crate::theme as gui_theme;
use crate::view::dashboard::state::{DashboardState, RuntimeRow};
use crate::view::downloads::{DownloadPanelState, JobState};
use crate::view::empty_state::{EmptyTone, illustrative_block, illustrative_block_compact};
use crate::view::loading::loading_skeleton;
use crate::widget_styles::{
    ButtonVariant, button_content_centered, button_style, card_container_style,
};

pub fn dashboard_view(
    state: &DashboardState,
    downloads: &DownloadPanelState,
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
            .push(runtime_overview_card(&data.rows, tokens))
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

fn runtime_overview_card(rows: &[RuntimeRow], tokens: ThemeTokens) -> Element<'static, Message> {
    let ty = tokens.typography();
    let sp = tokens.space();
    let mut list = column![].spacing((sp.xs + 2) as f32);
    for r in rows {
        let label = kind_label(r.kind);
        let cur = r.current.clone().unwrap_or_else(|| {
            envr_core::i18n::tr_key("gui.dashboard.not_set", "(未设置)", "(none)")
        });
        list = list.push(text(format!("{label}: {} · {}", r.installed, cur)).size(ty.caption));
    }
    card(
        envr_core::i18n::tr_key(
            "gui.dashboard.runtimes_overview",
            "运行时概览",
            "Runtimes overview",
        ),
        list.into(),
        tokens,
    )
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

fn kind_label(kind: RuntimeKind) -> &'static str {
    match kind {
        RuntimeKind::Node => "Node",
        RuntimeKind::Python => "Python",
        RuntimeKind::Java => "Java",
        RuntimeKind::Go => "Go",
        RuntimeKind::Rust => "Rust",
        RuntimeKind::Php => "PHP",
        RuntimeKind::Deno => "Deno",
        RuntimeKind::Bun => "Bun",
        RuntimeKind::Dotnet => ".NET",
    }
}
