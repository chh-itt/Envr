use iced::widget::{button, column, container, row, text};
use iced::{Alignment, Element, Length, Theme};

use envr_domain::runtime::RuntimeKind;
use envr_ui::theme::ThemeTokens;

use crate::app::{Message, Route};
use crate::theme as gui_theme;
use crate::view::dashboard::state::{DashboardState, RuntimeRow};
use crate::view::downloads::{DownloadPanelState, JobState};

pub fn dashboard_view(
    state: &DashboardState,
    downloads: &DownloadPanelState,
    tokens: ThemeTokens,
) -> Element<'static, Message> {
    let mut col = column![
        row![
            text(envr_core::i18n::tr("仪表盘", "Dashboard")).size(22),
            iced::widget::horizontal_space(),
            button(text(envr_core::i18n::tr("刷新", "Refresh")))
                .on_press(Message::Dashboard(
                    crate::view::dashboard::state::DashboardMsg::Refresh
                ))
                .padding([6, 12]),
        ]
        .align_y(Alignment::Center)
    ]
    .spacing(12);

    if let Some(err) = state.last_error.as_deref() {
        col = col.push(text(format!(
            "{}: {err}",
            envr_core::i18n::tr("加载失败", "Failed")
        )));
    }

    let data = match state.data.as_ref() {
        Some(d) => d,
        None => {
            col = col.push(text(envr_core::i18n::tr(
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
    let panel = gui_theme::panel_container_style(tokens);
    container(column![text(title).size(16), body].spacing(10))
        .padding(12)
        .width(Length::Fill)
        .style(move |theme: &Theme| panel(theme))
        .into()
}

fn runtime_overview_card(rows: &[RuntimeRow], tokens: ThemeTokens) -> Element<'static, Message> {
    let mut list = column![].spacing(6);
    for r in rows {
        let label = kind_label(r.kind);
        let cur = r
            .current
            .as_deref()
            .unwrap_or(envr_core::i18n::tr("(未设置)", "(none)"));
        list = list.push(text(format!("{label}: {} · {}", r.installed, cur)).size(13));
    }
    card(
        envr_core::i18n::tr("运行时概览", "Runtimes overview").to_string(),
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
    let mut body = column![
        text(format!(
            "{}: {runtime_root}",
            envr_core::i18n::tr("运行时根目录", "Runtime root")
        ))
        .size(13)
    ]
    .spacing(6);
    body = body.push(text(format!(
        "{}: {shims_dir}{}",
        envr_core::i18n::tr("Shims", "Shims"),
        if shims_empty {
            envr_core::i18n::tr("（空）", " (empty)")
        } else {
            ""
        }
    )));

    if issues.is_empty() {
        body = body.push(text(envr_core::i18n::tr(
            "健康检查：通过",
            "Health check: OK",
        )));
    } else {
        body = body.push(text(envr_core::i18n::tr(
            "健康检查：发现问题",
            "Health check: issues found",
        )));
        for i in issues {
            body = body.push(text(format!("- {i}")).size(12));
        }
    }
    if !recs.is_empty() {
        body = body.push(text(envr_core::i18n::tr("建议：", "Suggestions:")).size(13));
        for r in recs {
            body = body.push(text(format!("- {r}")).size(12));
        }
    }

    card(
        envr_core::i18n::tr("健康检查", "Health").to_string(),
        body.into(),
        tokens,
    )
}

fn recent_jobs_card(
    downloads: &DownloadPanelState,
    tokens: ThemeTokens,
) -> Element<'static, Message> {
    let mut body = column![].spacing(6);
    if downloads.jobs.is_empty() {
        body = body.push(text(envr_core::i18n::tr(
            "暂无下载/安装任务。",
            "No download/install jobs yet.",
        )));
    } else {
        for j in downloads.jobs.iter().rev().take(5) {
            let st = match j.state {
                JobState::Running => envr_core::i18n::tr("进行中", "Running"),
                JobState::Done => envr_core::i18n::tr("完成", "Done"),
                JobState::Failed => envr_core::i18n::tr("失败", "Failed"),
                JobState::Cancelled => envr_core::i18n::tr("已取消", "Cancelled"),
            };
            body = body.push(text(format!("{} · {} · {}", j.label, st, j.url)).size(12));
        }
    }
    card(
        envr_core::i18n::tr("最近任务", "Recent activity").to_string(),
        body.into(),
        tokens,
    )
}

fn recommended_actions_card(tokens: ThemeTokens) -> Element<'static, Message> {
    let actions = row![
        button(text(envr_core::i18n::tr("打开运行时", "Open runtimes")))
            .on_press(Message::Navigate(Route::Runtime))
            .padding([6, 12]),
        button(text(envr_core::i18n::tr("打开设置", "Open settings")))
            .on_press(Message::Navigate(Route::Settings))
            .padding([6, 12]),
    ]
    .spacing(10);
    card(
        envr_core::i18n::tr("推荐操作", "Recommended actions").to_string(),
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
