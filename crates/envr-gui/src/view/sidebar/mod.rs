use iced::widget::{button, column, container, text};
use iced::{Element, Length, Theme};

use envr_ui::theme::ThemeTokens;

use crate::app::{Message, Route};
use crate::theme as gui_theme;

pub fn sidebar(current: Route, tokens: ThemeTokens) -> Element<'static, Message> {
    let panel = gui_theme::panel_container_style(tokens);
    let mut col = column![].spacing(8);
    for route in Route::ALL {
        let b = button(text(route.label()))
            .on_press(Message::Navigate(route))
            .width(Length::Fill);
        let b = if route == current {
            b.style(button::primary)
        } else {
            b.style(button::secondary)
        };
        col = col.push(b);
    }
    container(col.width(Length::Fixed(tokens.sidebar_width())))
        .padding(10)
        .style(move |theme: &Theme| panel(theme))
        .into()
}
