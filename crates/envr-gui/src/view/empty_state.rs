//! Illustrative empty / error blocks (`tasks_gui.md` GUI-070): icon + geometry accent + tiered copy.

use envr_ui::theme::ThemeTokens;
use iced::widget::{column, container, horizontal_space, row, text, vertical_space};
use iced::{Alignment, Background, Color, Element, Length, Theme, border};

use crate::icons::Lucide;
use crate::theme as gui_theme;

/// Visual weight for the hero icon.
#[derive(Clone, Copy)]
pub enum EmptyTone {
    Neutral,
    Warning,
    Danger,
}

fn icon_color(tokens: ThemeTokens, tone: EmptyTone) -> Color {
    match tone {
        EmptyTone::Neutral => gui_theme::to_color(tokens.colors.primary),
        EmptyTone::Warning => gui_theme::to_color(tokens.colors.warning),
        EmptyTone::Danger => gui_theme::to_color(tokens.colors.danger),
    }
}

fn rounded_blob<Message: 'static>(
    w: f32,
    h: f32,
    color: Color,
    radius: f32,
) -> Element<'static, Message> {
    container(vertical_space().height(Length::Fixed(1.0)))
        .width(Length::Fixed(w))
        .height(Length::Fixed(h))
        .style(move |_t: &Theme| {
            container::Style::default()
                .background(Background::Color(color))
                .border(border::rounded(radius))
        })
        .into()
}

/// Soft bar + dots — simple geometric illustration.
fn geometry_accent<Message: 'static>(
    tokens: ThemeTokens,
    tone: EmptyTone,
) -> Element<'static, Message> {
    let bar_c = icon_color(tokens, tone).scale_alpha(0.18);
    let dot_c = icon_color(tokens, tone).scale_alpha(0.35);
    let r = tokens.radius_sm;
    column![
        rounded_blob::<Message>(72.0, 4.0, bar_c, r),
        vertical_space().height(Length::Fixed(10.0)),
        row![
            rounded_blob::<Message>(6.0, 6.0, dot_c, 3.0),
            horizontal_space().width(Length::Fixed(8.0)),
            rounded_blob::<Message>(6.0, 6.0, dot_c, 3.0),
            horizontal_space().width(Length::Fixed(8.0)),
            rounded_blob::<Message>(6.0, 6.0, dot_c, 3.0),
        ]
        .align_y(Alignment::Center),
    ]
    .spacing(0)
    .align_x(Alignment::Center)
    .into()
}

/// Dense variant (e.g. inside cards / small panels): no geometry strip, less padding.
pub fn illustrative_block_compact<Message: Clone + 'static>(
    tokens: ThemeTokens,
    tone: EmptyTone,
    icon: Lucide,
    icon_px: f32,
    title: String,
    body: String,
    hint: Option<String>,
) -> Element<'static, Message> {
    let ty = tokens.typography();
    let sp = tokens.space();
    let ic = icon_color(tokens, tone);
    let muted = gui_theme::to_color(tokens.colors.text_muted);
    let mut col = column![
        icon.view(icon_px, ic),
        vertical_space().height(Length::Fixed(sp.xs as f32)),
        text(title).size(ty.body_small),
        vertical_space().height(Length::Fixed(4.0)),
        text(body).size(ty.caption).color(muted),
    ]
    .spacing(0)
    .align_x(Alignment::Center);
    if let Some(h) = hint {
        col = col.push(vertical_space().height(Length::Fixed(sp.xs as f32)));
        col = col.push(text(h).size(ty.micro).color(muted));
    }
    container(col)
        .width(Length::Fill)
        .padding(sp.sm + 2)
        .center_x(Length::Fill)
        .into()
}

/// Centered block: illustration, large icon, title, body, optional hint (muted).
pub fn illustrative_block<Message: Clone + 'static>(
    tokens: ThemeTokens,
    tone: EmptyTone,
    icon: Lucide,
    icon_px: f32,
    title: String,
    body: String,
    hint: Option<String>,
) -> Element<'static, Message> {
    let ty = tokens.typography();
    let sp = tokens.space();
    let ic = icon_color(tokens, tone);
    let muted = gui_theme::to_color(tokens.colors.text_muted);
    let mut col = column![
        geometry_accent::<Message>(tokens, tone),
        vertical_space().height(Length::Fixed(sp.md as f32)),
        icon.view(icon_px, ic),
        vertical_space().height(Length::Fixed(sp.sm as f32)),
        text(title).size(ty.subsection),
        vertical_space().height(Length::Fixed(sp.xs as f32)),
        text(body).size(ty.body_small).color(muted),
    ]
    .spacing(0)
    .align_x(Alignment::Center);
    if let Some(h) = hint {
        col = col.push(vertical_space().height(Length::Fixed(sp.sm as f32)));
        col = col.push(text(h).size(ty.caption).color(muted));
    }
    container(col)
        .width(Length::Fill)
        .padding(sp.lg)
        .center_x(Length::Fill)
        .into()
}
