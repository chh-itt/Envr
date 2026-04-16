use iced::widget::{button, row, scrollable, text};
use iced::{Alignment, Element, Length};

use envr_domain::runtime::RuntimeKind;
use envr_ui::theme::ThemeTokens;

use crate::app::Message;
use crate::icons::Lucide;
use crate::theme as gui_theme;
use crate::view::env_center::EnvCenterMsg;
use crate::widget_styles::{ButtonVariant, button_content_centered, button_style};

pub fn runtime_nav_bar(
    active: RuntimeKind,
    _busy: bool,
    tokens: ThemeTokens,
) -> Element<'static, Message> {
    let sp = tokens.space();
    let txt = gui_theme::to_color(tokens.colors.text_muted);
    let on_primary = gui_theme::contrast_on_primary(tokens);
    let mut r = row![].spacing(sp.sm as f32).align_y(Alignment::Center);

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
        let icon_c = if kind == active { on_primary } else { txt };
        let label = row![
            Lucide::Package.view(14.0, icon_c),
            text(crate::view::env_center::kind_label(kind)),
        ]
        .spacing(sp.xs as f32)
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
        let b = button(button_content_centered(label.into()))
            .on_press(Message::EnvCenter(EnvCenterMsg::PickKind(kind)))
            .height(Length::Fixed(h))
            .padding([sp.sm as f32, (sp.sm + 2) as f32])
            .style(button_style(tokens, variant));
        r = r.push(b);
    }

    scrollable(r)
        .direction(scrollable::Direction::Horizontal(
            scrollable::Scrollbar::default(),
        ))
        .width(Length::Fill)
        .into()
}
