use super::model::{DownloadPanelState, JobState};
use super::panel::DownloadMsg;
use super::panel::format_job_state_line;
use envr_ui::theme::ThemeTokens;
use iced::mouse;
use iced::widget::{
    button, column, container, mouse_area, progress_bar, row, scrollable, text, vertical_space,
};
use iced::{Alignment, Element, Length, Theme};

use crate::app::Message;
use crate::icons::Lucide;
use crate::theme as gui_theme;
use crate::view::empty_state::{EmptyTone, illustrative_block_compact};
use crate::widget_styles::{ButtonVariant, button_style};

/// Card width matches layout geometry / persistence (`tasks_gui.md` GUI-061).
pub const DOWNLOAD_PANEL_SHELL_W: f32 = 320.0;

pub fn floating_download_panel(
    state: &DownloadPanelState,
    tokens: ThemeTokens,
) -> Element<'static, Message> {
    let ty = tokens.typography();
    let sp = tokens.space();
    let btn_h = tokens
        .control_height_secondary
        .max(tokens.min_click_target_px());
    let txt = gui_theme::to_color(tokens.colors.text);
    let rev = state.reveal.clamp(0.0, 1.0);

    if !state.visible && state.reveal_anim.is_none() {
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
                .height(Length::Fixed(btn_h))
                .padding([0, sp.sm + 2])
                .style(button_style(tokens, ButtonVariant::Secondary)),
        )
        .into();
    }

    let title_txt = envr_core::i18n::tr_key("gui.downloads.panel_title", "下载任务", "Downloads");
    let title_row = container(
        row![
            Lucide::Menu.view(16.0, txt),
            text(title_txt).size(ty.body).width(Length::Fill),
        ]
        .spacing(sp.sm)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .clip(true);

    let title_drag_strip = mouse_area(title_row)
        .on_press(Message::Download(DownloadMsg::TitleBarPress))
        .interaction(mouse::Interaction::Grab);

    // Icon-only actions scroll horizontally (`tasks_gui.md` GUI-060).
    let toolbar = scrollable(
        row![
            button(Lucide::Download.view(16.0, txt))
                .on_press(Message::Download(DownloadMsg::EnqueueDemo))
                .width(Length::Fixed(btn_h))
                .height(Length::Fixed(btn_h))
                .padding(0)
                .style(button_style(tokens, ButtonVariant::Secondary)),
            button(Lucide::ChevronsUpDown.view(16.0, txt))
                .on_press(Message::Download(DownloadMsg::ToggleExpand))
                .width(Length::Fixed(btn_h))
                .height(Length::Fixed(btn_h))
                .padding(0)
                .style(button_style(tokens, ButtonVariant::Secondary)),
            button(Lucide::EyeOff.view(16.0, txt))
                .on_press(Message::Download(DownloadMsg::ToggleVisible))
                .width(Length::Fixed(btn_h))
                .height(Length::Fixed(btn_h))
                .padding(0)
                .style(button_style(tokens, ButtonVariant::Ghost)),
        ]
        .spacing(sp.sm)
        .align_y(Alignment::Center),
    )
    .direction(scrollable::Direction::Horizontal(
        scrollable::Scrollbar::default(),
    ))
    .width(Length::Fill)
    .height(Length::Fixed(btn_h));

    let header = column![title_drag_strip, toolbar]
        .spacing(sp.xs)
        .width(Length::Fill);

    let mut body = column![header].spacing(sp.sm + 2);

    if state.expanded {
        if state.jobs.is_empty() {
            let empty_title = envr_core::i18n::tr_key(
                "gui.empty.title.no_download_jobs",
                "暂无下载任务",
                "No download jobs",
            );
            let empty_body = envr_core::i18n::tr_key(
                "gui.empty.body.no_download_jobs",
                "从运行时页安装或使用演示下载时，进度会显示在面板中。",
                "Install from the Runtimes page or start a demo download to see progress here.",
            );
            let empty_hint = Some(envr_core::i18n::tr_key(
                "gui.empty.hint.no_download_jobs",
                "可折叠面板节省空间；长按标题条可拖拽停靠位置。",
                "Collapse the panel for more room; long-press the title bar to drag.",
            ));
            body = body.push(illustrative_block_compact(
                tokens,
                EmptyTone::Neutral,
                Lucide::Download,
                28.0,
                empty_title,
                empty_body,
                empty_hint,
            ));
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
                        .height(Length::Fixed(btn_h))
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
                        .height(Length::Fixed(btn_h))
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

    let slide_px = (1.0 - rev) * 14.0;
    let card = container(body)
        .padding(sp.sm + 2)
        .width(Length::Fixed(DOWNLOAD_PANEL_SHELL_W))
        .style(move |theme: &Theme| gui_theme::download_panel_container_style(tokens, rev)(theme));

    column![vertical_space().height(Length::Fixed(slide_px)), card,].into()
}
