use envr_ui::theme::ThemeTokens;
use iced::widget::{column, container, rule, space};
use iced::{Element, Length, Theme};

use crate::theme as gui_theme;

pub fn loading_skeleton<Message: 'static>(
    tokens: ThemeTokens,
    phase: f32,
    rows: u8,
) -> Element<'static, Message> {
    use iced::Background;

    let pulse = (phase * std::f32::consts::TAU).sin() * 0.5 + 0.5;
    let fill = gui_theme::to_color(tokens.colors.text_muted).scale_alpha(0.07 + 0.16 * pulse);
    let row_h = tokens.list_row_height();
    let bar_h = row_h * 0.42;
    let n = rows.max(1);
    let mut col = column![].spacing(0);
    for i in 0..n {
        col = col.push(
            container(space().width(Length::Fill).height(Length::Fixed(bar_h)))
                .width(Length::Fill)
                .height(Length::Fixed(row_h))
                .align_y(iced::alignment::Vertical::Center)
                .padding([0, tokens.space().md])
                .style(move |_theme: &Theme| {
                    iced::widget::container::Style::default().background(Background::Color(fill))
                }),
        );
        if i + 1 < n {
            col = col.push(rule::horizontal(1.0));
        }
    }
    col.into()
}
