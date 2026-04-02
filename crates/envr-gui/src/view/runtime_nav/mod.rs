use iced::widget::{button, row, text};
use iced::{Alignment, Element, Length};

use envr_domain::runtime::RuntimeKind;
use envr_ui::theme::ThemeTokens;

use crate::app::Message;
use crate::icons::Lucide;
use crate::theme as gui_theme;
use crate::view::env_center::EnvCenterMsg;
use crate::widget_styles::{ButtonVariant, button_style};

pub fn runtime_nav_bar(
    active: RuntimeKind,
    busy: bool,
    tokens: ThemeTokens,
) -> Element<'static, Message> {
    let sp = tokens.space();
    let txt = gui_theme::to_color(tokens.colors.text);
    let mut r = row![].spacing(sp.sm).align_y(Alignment::Center);

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
        let label = row![
            Lucide::Package.view(14.0, txt),
            text(crate::view::env_center::kind_label(kind)),
        ]
        .spacing(sp.xs)
        .align_y(Alignment::Center);
        let variant = if kind == active {
            ButtonVariant::Primary
        } else {
            ButtonVariant::Secondary
        };
        let h = if kind == active {
            tokens.control_height_primary
        } else {
            tokens.control_height_secondary
        }
        .max(tokens.min_click_target_px());
        let b = button(label)
            .on_press(Message::EnvCenter(EnvCenterMsg::PickKind(kind)))
            .height(Length::Fixed(h))
            .padding([0, sp.sm + 2])
            .style(button_style(tokens, variant));
        let b = if busy { b.on_press_maybe(None) } else { b };
        r = r.push(b);
    }

    r.into()
}
