use iced::widget::{button, column, container, row, text};
use iced::{Alignment, Element, Length, Padding, Theme};

use envr_domain::runtime::RuntimeKind;
use envr_ui::theme::ThemeTokens;

use crate::app::{Message, Route};
use crate::icons::Lucide;
use crate::theme as gui_theme;
use crate::view::dashboard::state::{DashboardState, RuntimeRow};
use crate::view::downloads::{DownloadPanelState, JobState};
use crate::widget_styles::{ButtonVariant, button_style, card_container_style};

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
    .spacing(sp.sm)
    .align_y(Alignment::Center);
    let mut col = column![
        row![
            text(envr_core::i18n::tr_key(
                "gui.route.dashboard",
                "仪表盘",
                "Dashboard",
            ))
            .size(ty.page_title),
            iced::widget::horizontal_space(),
            button(refresh_lbl)
                .on_press(Message::Dashboard(
                    crate::view::dashboard::state::DashboardMsg::Refresh
                ))
                .height(Length::Fixed(tokens.control_height_primary))
                .padding([0, sp.md])
                .style(button_style(tokens, ButtonVariant::Secondary)),
        ]
        .align_y(Alignment::Center)
    ]
    .spacing(tokens.page_title_gap());

    if let Some(err) = state.last_error.as_deref() {
        col = col.push(text(format!(
            "{}: {err}",
            envr_core::i18n::tr_key("gui.dashboard.load_failed", "加载失败", "Failed")
        )));
    }

    let data = match state.data.as_ref() {
        Some(d) => d,
        None => {
            col = col.push(text(envr_core::i18n::tr_key(
                "gui.dashboard.no_data_yet",
                "尚无数据。点击「刷新」。",
                "No data yet. Click Refresh.",
            )));
            return col.into();
        }
    };

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
    container(column![text(title).size(ty.section), body].spacing(sp.sm + 2))
        .padding(Padding::new(pad))
        .width(Length::Fill)
        .style(move |theme: &Theme| card_s(theme))
        .into()
}

fn runtime_overview_card(rows: &[RuntimeRow], tokens: ThemeTokens) -> Element<'static, Message> {
    let ty = tokens.typography();
    let sp = tokens.space();
    let mut list = column![].spacing(sp.xs + 2);
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
    .spacing(sp.xs + 2);
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
    let mut body = column![].spacing(sp.xs + 2);
    if downloads.jobs.is_empty() {
        body = body.push(text(envr_core::i18n::tr_key(
            "gui.dashboard.no_recent_jobs",
            "暂无下载/安装任务。",
            "No download/install jobs yet.",
        )));
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
            body = body.push(text(format!("{} · {} · {}", j.label, st, j.url)).size(ty.micro));
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
        button(
            row![
                Lucide::Package.view(16.0, txt),
                text(envr_core::i18n::tr_key(
                    "gui.dashboard.open_runtimes",
                    "打开运行时",
                    "Open runtimes",
                )),
            ]
            .spacing(sp.sm)
            .align_y(Alignment::Center),
        )
        .on_press(Message::Navigate(Route::Runtime))
        .height(Length::Fixed(tokens.control_height_secondary))
        .padding([0, sp.md])
        .style(button_style(tokens, ButtonVariant::Secondary)),
        button(
            row![
                Lucide::Settings.view(16.0, txt),
                text(envr_core::i18n::tr_key(
                    "gui.dashboard.open_settings",
                    "打开设置",
                    "Open settings",
                )),
            ]
            .spacing(sp.sm)
            .align_y(Alignment::Center),
        )
        .on_press(Message::Navigate(Route::Settings))
        .height(Length::Fixed(tokens.control_height_secondary))
        .padding([0, sp.md])
        .style(button_style(tokens, ButtonVariant::Secondary)),
    ]
    .spacing(sp.sm + 2);
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
    }
}
