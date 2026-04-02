//! Iced styles for buttons, inputs, and cards (`tasks_gui.md` GUI-020).

use envr_ui::theme::ThemeTokens;
use iced::border;
use iced::widget::button;
use iced::widget::container;
use iced::widget::text_input;
use iced::{Background, Color, Theme};

use crate::theme::to_color;

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
                }
            }
        }
    }
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
            text_input::Status::Focused => text_input::Style {
                border: border::width(2.0).color(primary).rounded(r),
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
