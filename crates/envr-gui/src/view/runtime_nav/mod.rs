use iced::widget::{button, row, text};
use iced::{Alignment, Element};

use envr_domain::runtime::RuntimeKind;
use envr_ui::theme::ThemeTokens;

use crate::app::Message;
use crate::view::env_center::EnvCenterMsg;

pub fn runtime_nav_bar(
    active: RuntimeKind,
    busy: bool,
    tokens: ThemeTokens,
) -> Element<'static, Message> {
    let mut r = row![].spacing(8).align_y(Alignment::Center);

    for kind in [
        RuntimeKind::Node,
        RuntimeKind::Python,
        RuntimeKind::Java,
        RuntimeKind::Go,
        RuntimeKind::Rust,
        RuntimeKind::Php,
        RuntimeKind::Deno,
        RuntimeKind::Bun,
    ] {
        let label = text(crate::view::env_center::kind_label(kind));
        let b = button(label)
            .on_press(Message::EnvCenter(EnvCenterMsg::PickKind(kind)))
            .padding([6, 10]);
        let b = if kind == active {
            b.style(button::primary)
        } else {
            b.style(button::secondary)
        };
        let b = if busy { b.on_press_maybe(None) } else { b };
        r = r.push(b);
    }

    let _ = tokens; // reserved for future spacing/styling
    r.into()
}
