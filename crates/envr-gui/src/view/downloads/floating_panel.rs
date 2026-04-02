use super::model::{DownloadPanelState, JobState};
use super::panel::DownloadMsg;
use super::panel::format_job_state_line;
use envr_ui::theme::ThemeTokens;
use iced::widget::{button, column, container, progress_bar, row, scrollable, text};
use iced::{Alignment, Element, Length, Theme};

use crate::app::Message;
use crate::icons::Lucide;
use crate::theme as gui_theme;
use crate::widget_styles::{ButtonVariant, button_style};

pub fn floating_download_panel(
    state: &DownloadPanelState,
    tokens: ThemeTokens,
) -> Element<'static, Message> {
    let ty = tokens.typography();
    let sp = tokens.space();

    let txt = gui_theme::to_color(tokens.colors.text);
    if !state.visible {
        let open_lbl = row![
            Lucide::Download.view(16.0, txt),
            text(envr_core::i18n::tr_key(
                "gui.downloads.open_button",
                "下载",
                "Downloads",
            )),
        ]
        .spacing(sp.xs)
        .align_y(Alignment::Center);
        return container(
            button(open_lbl)
                .on_press(Message::Download(DownloadMsg::ToggleVisible))
                .height(Length::Fixed(tokens.control_height_secondary))
                .padding([0, sp.sm + 2])
                .style(button_style(tokens, ButtonVariant::Secondary)),
        )
        .into();
    }

    let title_row = row![
        Lucide::PanelLeftOpen.view(18.0, txt),
        text(envr_core::i18n::tr_key(
            "gui.downloads.panel_title",
            "下载任务",
            "Downloads",
        ))
        .size(ty.body),
    ]
    .spacing(sp.sm)
    .align_y(Alignment::Center);

    let header = row![
        button(Lucide::Menu.view(18.0, txt))
            .on_press(Message::Download(DownloadMsg::StartDrag))
            .height(Length::Fixed(tokens.control_height_secondary))
            .padding([0, sp.sm])
            .style(button_style(tokens, ButtonVariant::Ghost)),
        title_row,
        iced::widget::horizontal_space(),
        button(
            row![
                Lucide::Download.view(14.0, txt),
                text(envr_core::i18n::tr_key(
                    "gui.downloads.add_demo",
                    "添加演示下载",
                    "Add demo download",
                )),
            ]
            .spacing(sp.xs)
            .align_y(Alignment::Center),
        )
        .on_press(Message::Download(DownloadMsg::EnqueueDemo))
        .height(Length::Fixed(tokens.control_height_secondary))
        .padding([0, sp.sm + 2])
        .style(button_style(tokens, ButtonVariant::Secondary)),
        button(
            row![
                Lucide::ChevronsUpDown.view(14.0, txt),
                text(if state.expanded {
                    envr_core::i18n::tr_key("gui.action.collapse", "折叠", "Collapse")
                } else {
                    envr_core::i18n::tr_key("gui.action.expand", "展开", "Expand")
                }),
            ]
            .spacing(sp.xs)
            .align_y(Alignment::Center),
        )
        .on_press(Message::Download(DownloadMsg::ToggleExpand))
        .height(Length::Fixed(tokens.control_height_secondary))
        .padding([0, sp.sm + 2])
        .style(button_style(tokens, ButtonVariant::Secondary)),
        button(
            row![
                Lucide::EyeOff.view(14.0, txt),
                text(envr_core::i18n::tr_key("gui.action.hide", "隐藏", "Hide")),
            ]
            .spacing(sp.xs)
            .align_y(Alignment::Center),
        )
        .on_press(Message::Download(DownloadMsg::ToggleVisible))
        .height(Length::Fixed(tokens.control_height_secondary))
        .padding([0, sp.sm + 2])
        .style(button_style(tokens, ButtonVariant::Ghost)),
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
                        .on_press(Message::Download(DownloadMsg::Cancel(j.id)))
                        .height(Length::Fixed(tokens.control_height_secondary))
                        .padding([0, sp.sm])
                        .style(button_style(tokens, ButtonVariant::Ghost)),
                    );
                }
                if j.state == JobState::Failed {
                    actions = actions.push(
                        button(text(envr_core::i18n::tr_key(
                            "gui.action.retry",
                            "重试",
                            "Retry",
                        )))
                        .on_press(Message::Download(DownloadMsg::Retry(j.id)))
                        .height(Length::Fixed(tokens.control_height_secondary))
                        .padding([0, sp.sm])
                        .style(button_style(tokens, ButtonVariant::Secondary)),
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
