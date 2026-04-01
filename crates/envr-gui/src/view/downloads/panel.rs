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
        text("下载任务").size(16),
        button(text(if state.expanded { "折叠" } else { "展开" }))
            .on_press(Message::Download(DownloadMsg::ToggleExpand))
            .padding([4, 10]),
        button(text("添加演示下载"))
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
            text("暂无任务。点击「添加演示下载」使用与 CLI 相同的 envr-download 引擎（含取消 / 重试 / 进度）。")
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
                .map(|s| format!(" · 约剩余 {s}s"))
                .unwrap_or_default();
            let sz = if t > 0 {
                format!("{d} / {t} 字节")
            } else {
                format!("{d} 字节")
            };
            format!("进行中 · {sz} · {spd}{eta}")
        }
        JobState::Done => format!("完成 · {} 字节", job.downloaded_display()),
        JobState::Failed => format!("失败：{}", job.last_error.as_deref().unwrap_or("未知错误")),
        JobState::Cancelled => "已取消".to_string(),
    };

    let bar = progress_bar(0.0..=100.0, ratio);

    let mut actions = row![].spacing(8);
    if job.state == JobState::Running {
        actions = actions
            .push(button(text("取消")).on_press(Message::Download(DownloadMsg::Cancel(job.id))));
    }
    if job.state == JobState::Failed {
        actions = actions
            .push(button(text("重试")).on_press(Message::Download(DownloadMsg::Retry(job.id))));
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
