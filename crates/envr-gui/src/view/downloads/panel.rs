use super::model::{DownloadJob, DownloadPanelState, JobState};
use envr_ui::theme::ThemeTokens;
use iced::widget::{button, column, progress_bar, row, scrollable, text};
use iced::{Alignment, Element, Length};

use crate::app::Message;

#[derive(Debug, Clone)]
pub enum DownloadMsg {
    Tick,
    ToggleExpand,
    EnqueueDemo,
    Finished {
        id: u64,
        result: Result<u64, String>,
    },
    Cancel(u64),
    Retry(u64),
}

pub fn download_dock(state: &DownloadPanelState, tokens: ThemeTokens) -> Element<'static, Message> {
    let header = row![
        text(envr_core::i18n::tr("下载任务", "Downloads")).size(16),
        button(text(if state.expanded {
            envr_core::i18n::tr("折叠", "Collapse")
        } else {
            envr_core::i18n::tr("展开", "Expand")
        }))
        .on_press(Message::Download(DownloadMsg::ToggleExpand))
        .padding([4, 10]),
        button(text(envr_core::i18n::tr(
            "添加演示下载",
            "Add demo download"
        )))
        .on_press(Message::Download(DownloadMsg::EnqueueDemo))
        .padding([4, 10]),
    ]
    .spacing(12)
    .align_y(Alignment::Center);

    if !state.expanded {
        return column![header].spacing(8).into();
    }

    let mut body = column![header].spacing(10);
    if state.jobs.is_empty() {
        body = body.push(
            text(envr_core::i18n::tr(
                "暂无任务。点击「添加演示下载」使用与 CLI 相同的 envr-download 引擎（含取消 / 重试 / 进度）。",
                "No jobs yet. Click \"Add demo download\" to run the same envr-download engine as the CLI (cancel/retry/progress).",
            ))
                .size(13),
        );
        return body.into();
    }

    let mut list = column![].spacing(12);
    for job in &state.jobs {
        list = list.push(job_row(job, tokens));
    }

    body = body.push(
        scrollable(list)
            .height(Length::Fixed(220.0))
            .width(Length::Fill),
    );
    body.into()
}

fn job_row(job: &DownloadJob, _tokens: ThemeTokens) -> Element<'static, Message> {
    let ratio = job.progress_ratio();
    let state_line = match job.state {
        JobState::Running => {
            let spd = format_speed(job.speed_bps);
            let d = job.downloaded_display();
            let t = job.total_display();
            let eta = job
                .eta_secs()
                .map(|s| format!(" · {} {s}s", envr_core::i18n::tr("约剩余", "ETA")))
                .unwrap_or_default();
            let sz = if t > 0 {
                format!("{d} / {t} {}", envr_core::i18n::tr("字节", "bytes"))
            } else {
                format!("{d} {}", envr_core::i18n::tr("字节", "bytes"))
            };
            format!(
                "{} · {sz} · {spd}{eta}",
                envr_core::i18n::tr("进行中", "Running")
            )
        }
        JobState::Done => format!(
            "{} · {} {}",
            envr_core::i18n::tr("完成", "Done"),
            job.downloaded_display(),
            envr_core::i18n::tr("字节", "bytes")
        ),
        JobState::Failed => format!(
            "{}: {}",
            envr_core::i18n::tr("失败", "Failed"),
            job.last_error
                .as_deref()
                .unwrap_or(envr_core::i18n::tr("未知错误", "unknown error"))
        ),
        JobState::Cancelled => envr_core::i18n::tr("已取消", "Cancelled").to_string(),
    };

    let bar = progress_bar(0.0..=100.0, ratio);

    let mut actions = row![].spacing(8);
    if job.state == JobState::Running {
        actions = actions.push(
            button(text(envr_core::i18n::tr("取消", "Cancel")))
                .on_press(Message::Download(DownloadMsg::Cancel(job.id))),
        );
    }
    if job.state == JobState::Failed {
        actions = actions.push(
            button(text(envr_core::i18n::tr("重试", "Retry")))
                .on_press(Message::Download(DownloadMsg::Retry(job.id))),
        );
    }

    column![
        text(format!("{} — {}", job.label, job.url)).size(13),
        bar,
        text(state_line).size(12),
        actions,
    ]
    .spacing(6)
    .into()
}

fn format_speed(bps: f64) -> String {
    if bps >= 1_048_576.0 {
        format!("{:.1} MiB/s", bps / 1_048_576.0)
    } else if bps >= 1024.0 {
        format!("{:.1} KiB/s", bps / 1024.0)
    } else {
        format!("{:.0} B/s", bps)
    }
}
