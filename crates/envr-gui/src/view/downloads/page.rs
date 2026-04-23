use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Alignment, Element, Length};

use envr_ui::theme::ThemeTokens;

use crate::app::Message;
use crate::theme as gui_theme;
use crate::view::downloads::panel::format_job_state_line;
use crate::view::downloads::{DownloadMsg, DownloadPanelState, JobState};
use crate::widget_styles::{ButtonVariant, button_style, card_container_style};

pub fn downloads_page_view(
    downloads: &DownloadPanelState,
    tokens: ThemeTokens,
) -> Element<'static, Message> {
    let ty = tokens.typography();
    let sp = tokens.space();
    let muted = gui_theme::to_color(tokens.colors.text_muted);

    if downloads.jobs.is_empty() {
        return container(
            text(envr_core::i18n::tr_key(
                "gui.downloads.page.empty",
                "暂无下载任务。",
                "No download tasks yet.",
            ))
            .size(ty.body_small)
            .color(muted),
        )
        .width(Length::Fill)
        .into();
    }

    let mut list = column![].spacing((sp.sm + 2) as f32);
    for j in downloads.jobs.iter().rev() {
        let status = format_job_state_line(j);
        let ratio = j.progress_ratio();
        let title = text(j.label.clone()).size(ty.caption);
        let meta = text(status).size(ty.micro).color(muted);

        let mut actions = row![].spacing(sp.sm as f32).align_y(Alignment::Center);
        if matches!(j.state, JobState::Running | JobState::Queued) && j.cancellable {
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
        if j.state == JobState::Failed && !j.url.trim().is_empty() {
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

        let bar = container(iced::widget::progress_bar(0.0..=100.0, ratio))
            .width(Length::Fill)
            .height(Length::Fixed(3.0));

        let mut block = column![title, bar, meta, actions]
            .spacing(sp.xs as f32)
            .width(Length::Fill);

        if !j.url.trim().is_empty() {
            block = block.push(text(j.url.clone()).size(ty.micro).color(muted));
        }

        list = list.push(
            container(block)
                .padding(sp.sm)
                .width(Length::Fill)
                .style(card_container_style(tokens, 1)),
        );
    }

    scrollable(list).width(Length::Fill).into()
}

