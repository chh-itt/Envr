//! Maps `envr_ui::theme::ThemeTokens` to `iced`.

use envr_ui::theme::{Srgb, ThemeTokens};
use iced::border;
use iced::theme::Palette;
use iced::widget::container;
use iced::{Color, Theme};

pub fn iced_palette(tokens: ThemeTokens) -> Palette {
    let c = tokens.colors;
    Palette {
        background: to_color(c.background),
        text: to_color(c.text),
        primary: to_color(c.primary),
        success: to_color(c.success),
        danger: to_color(c.danger),
    }
}

pub fn iced_theme(tokens: ThemeTokens) -> Theme {
    let name = format!("envr-{}", tokens.flavor);
    Theme::custom(name, iced_palette(tokens))
}

pub(crate) fn to_color(s: Srgb) -> Color {
    Color {
        r: s.r,
        g: s.g,
        b: s.b,
        a: s.a,
    }
}

/// Text/icon color on top of the primary brand color (sidebar selected item, etc.).
pub(crate) fn contrast_on_primary(tokens: ThemeTokens) -> Color {
    let bg = to_color(tokens.colors.primary);
    let lum = 0.2126 * bg.r + 0.7152 * bg.g + 0.0722 * bg.b;
    if lum > 0.55 {
        Color::from_rgb(0.15, 0.15, 0.16)
    } else {
        Color::from_rgb(0.98, 0.98, 0.99)
    }
}

/// Sidebar card: rounded rect using token radius + soft shadow.
pub fn panel_container_style(tokens: ThemeTokens) -> impl Fn(&Theme) -> container::Style {
    let r = tokens.radius_md;
    let panel = to_color(tokens.colors.surface_panel);
    let blur = tokens.shadow.blur_radius;
    let offset_y = tokens.shadow.offset_y;
    let shadow_alpha = tokens.shadow.color_alpha;
    let shadow_color = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: shadow_alpha,
    };
    let border_a = tokens.panel_border_alpha;
    move |_theme: &Theme| {
        container::Style::default()
            .background(panel)
            .border(border::rounded(r).color(Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: border_a,
            }))
            .shadow(iced::Shadow {
                color: shadow_color,
                offset: iced::Vector::new(0.0, offset_y),
                blur_radius: blur,
            })
    }
}

/// Error strip uses semantic danger with light fill.
pub fn error_banner_style(tokens: ThemeTokens) -> container::Style {
    let d = tokens.colors.danger;
    container::Style::default().background(Color {
        r: d.r,
        g: d.g,
        b: d.b,
        a: 0.12,
    })
}
