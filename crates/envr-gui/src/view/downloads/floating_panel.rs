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
    let ty = tokens.typography();
    let sp = tokens.space();

    if !state.visible {
        return container(
            button(text(envr_core::i18n::tr_key(
                "gui.downloads.open_button",
                "下载",
                "Downloads",
            )))
            .on_press(Message::Download(DownloadMsg::ToggleVisible))
            .padding([sp.xs + 2, sp.sm + 2]),
        )
        .into();
    }

    let header = row![
        button(text("≡"))
            .on_press(Message::Download(DownloadMsg::StartDrag))
            .padding([sp.xs, sp.sm]),
        text(envr_core::i18n::tr_key(
            "gui.downloads.panel_title",
            "下载任务",
            "Downloads",
        ))
        .size(ty.body),
        iced::widget::horizontal_space(),
        button(text(envr_core::i18n::tr_key(
            "gui.downloads.add_demo",
            "添加演示下载",
            "Add demo download",
        )))
        .on_press(Message::Download(DownloadMsg::EnqueueDemo))
        .padding([sp.xs, sp.sm + 2]),
        button(text(if state.expanded {
            envr_core::i18n::tr_key("gui.action.collapse", "折叠", "Collapse")
        } else {
            envr_core::i18n::tr_key("gui.action.expand", "展开", "Expand")
        }))
        .on_press(Message::Download(DownloadMsg::ToggleExpand))
        .padding([sp.xs, sp.sm + 2]),
        button(text(envr_core::i18n::tr_key(
            "gui.action.hide",
            "隐藏",
            "Hide"
        )))
        .on_press(Message::Download(DownloadMsg::ToggleVisible))
        .padding([sp.xs, sp.sm + 2]),
    ]
    .spacing(sp.sm + 2)
    .align_y(Alignment::Center);

    let mut body = column![header].spacing(sp.sm + 2);

    if state.expanded {
        if state.jobs.is_empty() {
            body = body.push(
                text(envr_core::i18n::tr_key(
                    "gui.downloads.no_jobs",
                    "暂无任务。",
                    "No jobs.",
                ))
                .size(ty.micro),
            );
        } else {
            let mut list = column![].spacing(sp.sm + 2);
            for j in state.jobs.iter().rev().take(6) {
                let ratio = j.progress_ratio();
                let line = format_job_state_line(j);
                let mut actions = row![].spacing(sp.sm);
                if j.state == JobState::Running {
                    actions = actions.push(
                        button(text(envr_core::i18n::tr_key(
                            "gui.action.cancel",
                            "取消",
                            "Cancel",
                        )))
                        .on_press(Message::Download(DownloadMsg::Cancel(j.id))),
                    );
                }
                if j.state == JobState::Failed {
                    actions = actions.push(
                        button(text(envr_core::i18n::tr_key(
                            "gui.action.retry",
                            "重试",
                            "Retry",
                        )))
                        .on_press(Message::Download(DownloadMsg::Retry(j.id))),
                    );
                }
                list = list.push(
                    column![
                        text(format!("{} — {}", j.label, j.url)).size(ty.micro),
                        progress_bar(0.0..=100.0, ratio),
                        text(line).size(ty.tiny),
                        actions,
                    ]
                    .spacing(sp.xs),
                );
            }
            body = body.push(scrollable(list).height(Length::Fixed(220.0)));
        }
    }

    let panel = gui_theme::panel_container_style(tokens);
    container(body)
        .padding(sp.sm + 2)
        .width(Length::Fixed(320.0))
        .style(move |theme: &Theme| panel(theme))
        .into()
}
