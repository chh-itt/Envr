//! Iced styles for buttons, inputs, and cards (`tasks_gui.md` GUI-020).

use envr_ui::theme::ThemeTokens;
use iced::border;
use iced::widget::button;
use iced::widget::column;
use iced::widget::container;
use iced::widget::text;
use iced::widget::text_input;
use iced::{Background, Color, Element, Length, Padding, Theme};

use crate::theme::{contrast_on_primary, to_color};

#[derive(Debug, Clone, Copy)]
pub enum ButtonVariant {
    Primary,
    Secondary,
    Ghost,
    Danger,
}

fn darken(c: Color, amount: f32) -> Color {
    Color {
        r: (c.r - amount).max(0.0),
        g: (c.g - amount).max(0.0),
        b: (c.b - amount).max(0.0),
        a: c.a,
    }
}

fn lighten(c: Color, amount: f32) -> Color {
    Color {
        r: (c.r + amount).min(1.0),
        g: (c.g + amount).min(1.0),
        b: (c.b + amount).min(1.0),
        a: c.a,
    }
}

fn contrast_text_on(bg: Color) -> Color {
    let lum = 0.2126 * bg.r + 0.7152 * bg.g + 0.0722 * bg.b;
    if lum > 0.55 {
        Color::from_rgb(0.15, 0.15, 0.16)
    } else {
        Color::from_rgb(0.98, 0.98, 0.99)
    }
}

fn pill_border(r: f32) -> iced::Border {
    border::rounded(r).width(0.0).color(Color::TRANSPARENT)
}

/// Token-aware button (hover / pressed / disabled).
pub fn button_style(
    tokens: ThemeTokens,
    variant: ButtonVariant,
) -> impl Fn(&Theme, button::Status) -> button::Style + Copy {
    move |_theme: &Theme, status: button::Status| {
        let r = tokens.radius_sm;
        match variant {
            ButtonVariant::Primary => {
                let base_c = to_color(tokens.colors.primary);
                let text_c = contrast_text_on(base_c);
                let (bg, tc) = match status {
                    button::Status::Active => (base_c, text_c),
                    button::Status::Hovered => (lighten(base_c, 0.06), text_c),
                    button::Status::Pressed => (darken(base_c, 0.08), text_c),
                    button::Status::Disabled => (base_c.scale_alpha(0.4), text_c.scale_alpha(0.65)),
                };
                button::Style {
                    background: Some(Background::Color(bg)),
                    text_color: tc,
                    border: pill_border(r),
                    shadow: Default::default(),
                    snap: false,
                }
            }
            ButtonVariant::Secondary => {
                let surf = to_color(tokens.colors.surface);
                let line = to_color(tokens.colors.text_muted).scale_alpha(0.38);
                let tc = to_color(tokens.colors.text);
                let (bg, line_a, txt_a) = match status {
                    button::Status::Active => (surf, line, 1.0f32),
                    button::Status::Hovered => (lighten(surf, 0.03), line.scale_alpha(1.25), 1.0),
                    button::Status::Pressed => (darken(surf, 0.04), line, 1.0),
                    button::Status::Disabled => {
                        (surf.scale_alpha(0.55), line.scale_alpha(0.5), 0.55)
                    }
                };
                button::Style {
                    background: Some(Background::Color(bg)),
                    text_color: tc.scale_alpha(txt_a),
                    border: border::rounded(r).width(1.0).color(line_a),
                    shadow: Default::default(),
                    snap: false,
                }
            }
            ButtonVariant::Ghost => {
                let prim = to_color(tokens.colors.primary);
                let (bg, tc) = match status {
                    button::Status::Active => (None, prim),
                    button::Status::Hovered => {
                        (Some(Background::Color(prim.scale_alpha(0.1))), prim)
                    }
                    button::Status::Pressed => {
                        (Some(Background::Color(prim.scale_alpha(0.16))), prim)
                    }
                    button::Status::Disabled => (None, prim.scale_alpha(0.45)),
                };
                button::Style {
                    background: bg,
                    text_color: tc,
                    border: pill_border(r),
                    shadow: Default::default(),
                    snap: false,
                }
            }
            ButtonVariant::Danger => {
                let d = to_color(tokens.colors.danger);
                let tw = contrast_text_on(d);
                let (bg, tc) = match status {
                    button::Status::Active => (d, tw),
                    button::Status::Hovered => (lighten(d, 0.06), tw),
                    button::Status::Pressed => (darken(d, 0.1), tw),
                    button::Status::Disabled => (d.scale_alpha(0.45), tw.scale_alpha(0.65)),
                };
                button::Style {
                    background: Some(Background::Color(bg)),
                    text_color: tc,
                    border: pill_border(r),
                    shadow: Default::default(),
                    snap: false,
                }
            }
        }
    }
}

/// Label with a color matching [`button_style`] so text stays visible inside nested layouts
/// (iced 0.13 often does not propagate `button::Style::text_color` to [`text`] under [`container`]).
pub fn button_label_for_variant<Message: 'static>(
    label: impl Into<String>,
    tokens: ThemeTokens,
    variant: ButtonVariant,
) -> Element<'static, Message> {
    let c = match variant {
        ButtonVariant::Primary => contrast_on_primary(tokens),
        ButtonVariant::Secondary => to_color(tokens.colors.text),
        ButtonVariant::Ghost => to_color(tokens.colors.primary),
        ButtonVariant::Danger => contrast_text_on(to_color(tokens.colors.danger)),
    };
    text(label.into()).color(c).into()
}

/// Vertically centers label inside fixed-height [`button`]s without forcing horizontal `Fill`
/// (a `Fill` width hint makes every button expand and can break text layout in iced 0.13).
pub fn button_content_centered<Message: Clone + 'static>(
    content: Element<'static, Message>,
) -> Element<'static, Message> {
    container(content)
        .center_y(Length::Fill)
        .into()
}

/// Grouped surface for settings / runtime pages (title + body with card chrome).
pub fn section_card<Message: 'static>(
    tokens: ThemeTokens,
    title: String,
    body: Element<'static, Message>,
) -> Element<'static, Message> {
    let ty = tokens.typography();
    let sp = tokens.space();
    let pad = tokens.card_padding_px();
    let inset = Padding::from([pad + 4.0, pad + 4.0]);
    let card_s = card_container_style(tokens, 1);
    container(
        column![
            text(title).size(ty.section),
            body,
        ]
        .spacing(sp.md as f32)
        .width(Length::Fill),
    )
    .padding(inset)
    .width(Length::Fill)
    .style(move |theme: &Theme| card_s(theme))
    .into()
}

/// Card surface: semantic `surface`, token radius & padding applied at call site.
pub fn card_container_style(
    tokens: ThemeTokens,
    elevation: u8,
) -> impl Fn(&Theme) -> container::Style {
    let r = tokens.card_corner_radius();
    let fill = to_color(tokens.colors.surface);
    let border_c = to_color(tokens.colors.text_muted).scale_alpha(0.22);
    let (blur, alpha, off) = match elevation.min(2) {
        2 => (
            tokens.shadow.blur_radius * 1.1,
            (tokens.shadow.color_alpha * 1.1).min(0.45),
            tokens.shadow.offset_y + 0.5,
        ),
        _ => (
            tokens.shadow.blur_radius * 0.75,
            tokens.shadow.color_alpha * 0.85,
            tokens.shadow.offset_y,
        ),
    };
    let shadow = iced::Shadow {
        color: Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: alpha,
        },
        offset: iced::Vector::new(0.0, off),
        blur_radius: blur,
    };
    move |_theme: &Theme| {
        container::Style::default()
            .background(fill)
            .border(border::rounded(r).color(border_c).width(1.0))
            .shadow(shadow)
    }
}

pub fn text_input_style(
    tokens: ThemeTokens,
) -> impl Fn(&Theme, text_input::Status) -> text_input::Style + Copy {
    move |_theme: &Theme, status: text_input::Status| {
        let bg = Background::Color(to_color(tokens.colors.surface));
        let muted = to_color(tokens.colors.text_muted);
        let txt = to_color(tokens.colors.text);
        let primary = to_color(tokens.colors.primary);
        let r = tokens.radius_sm;
        let base = text_input::Style {
            background: bg,
            border: border::rounded(r).color(muted.scale_alpha(0.45)).width(1.0),
            icon: muted,
            placeholder: muted,
            value: txt,
            selection: primary.scale_alpha(0.28),
        };
        match status {
            text_input::Status::Active => base,
            text_input::Status::Hovered => text_input::Style {
                border: border::rounded(r).color(muted.scale_alpha(0.72)).width(1.0),
                ..base
            },
            text_input::Status::Focused { .. } => text_input::Style {
                border: border::width(tokens.focus_ring_width_px())
                    .color(primary)
                    .rounded(r),
                ..base
            },
            text_input::Status::Disabled => text_input::Style {
                background: Background::Color(to_color(tokens.colors.background)),
                value: muted,
                ..base
            },
        }
    }
}
