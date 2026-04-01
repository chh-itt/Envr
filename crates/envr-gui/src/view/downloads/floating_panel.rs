use super::model::{DownloadPanelState, JobState};
use super::panel::DownloadMsg;
use super::panel::format_job_state_line;
use envr_ui::theme::ThemeTokens;
use iced::widget::{button, column, container, progress_bar, row, scrollable, text};
use iced::{Alignment, Element, Length, Theme};

use crate::app::Message;
use crate::theme as gui_theme;

pub fn floating_download_panel(
    state: &DownloadPanelState,
    tokens: ThemeTokens,
) -> Element<'static, Message> {
    if !state.visible {
        // Small reopen button pinned to corner (same position).
        return container(
            button(text(envr_core::i18n::tr("下载", "Downloads")))
                .on_press(Message::Download(DownloadMsg::ToggleVisible))
                .padding([6, 10]),
        )
        .into();
    }

    let header = row![
        button(text("≡"))
            .on_press(Message::Download(DownloadMsg::StartDrag))
            .padding([4, 8]),
        text(envr_core::i18n::tr("下载任务", "Downloads")).size(15),
        iced::widget::horizontal_space(),
        button(text(envr_core::i18n::tr(
            "添加演示下载",
            "Add demo download"
        )))
        .on_press(Message::Download(DownloadMsg::EnqueueDemo))
        .padding([4, 10]),
        button(text(if state.expanded {
            envr_core::i18n::tr("折叠", "Collapse")
        } else {
            envr_core::i18n::tr("展开", "Expand")
        }))
        .on_press(Message::Download(DownloadMsg::ToggleExpand))
        .padding([4, 10]),
        button(text(envr_core::i18n::tr("隐藏", "Hide")))
            .on_press(Message::Download(DownloadMsg::ToggleVisible))
            .padding([4, 10]),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    let mut body = column![header].spacing(10);

    if state.expanded {
        if state.jobs.is_empty() {
            body = body.push(text(envr_core::i18n::tr("暂无任务。", "No jobs.")).size(12));
        } else {
            let mut list = column![].spacing(10);
            for j in state.jobs.iter().rev().take(6) {
                let ratio = j.progress_ratio();
                let line = format_job_state_line(j);
                let mut actions = row![].spacing(8);
                if j.state == JobState::Running {
                    actions = actions.push(
                        button(text(envr_core::i18n::tr("取消", "Cancel")))
                            .on_press(Message::Download(DownloadMsg::Cancel(j.id))),
                    );
                }
                if j.state == JobState::Failed {
                    actions = actions.push(
                        button(text(envr_core::i18n::tr("重试", "Retry")))
                            .on_press(Message::Download(DownloadMsg::Retry(j.id))),
                    );
                }
                list = list.push(
                    column![
                        text(format!("{} — {}", j.label, j.url)).size(12),
                        progress_bar(0.0..=100.0, ratio),
                        text(line).size(11),
                        actions,
                    ]
                    .spacing(4),
                );
            }
            body = body.push(scrollable(list).height(Length::Fixed(220.0)));
        }
    }

    let panel = gui_theme::panel_container_style(tokens);
    container(body)
        .padding(10)
        .width(Length::Fixed(320.0))
        .style(move |theme: &Theme| panel(theme))
        .into()
}
