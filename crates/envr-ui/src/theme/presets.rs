//! Light-mode presets. Values are tuned to read distinctly in `iced` while staying on-brand per platform doc.

use super::color::Srgb;
use super::flavor::UiFlavor;
use super::tokens::{MotionTokens, SemanticColors, ShadowTokens, ThemeTokens};

fn fluent_colors() -> SemanticColors {
    SemanticColors {
        background: Srgb::new(0.96, 0.97, 0.98),
        surface: Srgb::new(1.0, 1.0, 1.0),
        surface_panel: Srgb::new(0.98, 0.99, 1.0),
        text: Srgb::new(0.11, 0.13, 0.15),
        text_muted: Srgb::new(0.35, 0.38, 0.42),
        primary: Srgb::new(0.0, 0.47, 0.83),
        success: Srgb::new(0.11, 0.62, 0.35),
        danger: Srgb::new(0.77, 0.2, 0.18),
    }
}

fn liquid_colors() -> SemanticColors {
    SemanticColors {
        background: Srgb::new(0.93, 0.94, 0.96),
        surface: Srgb::new(0.97, 0.97, 0.99),
        surface_panel: Srgb::new(0.94, 0.95, 0.98),
        text: Srgb::new(0.09, 0.10, 0.12),
        text_muted: Srgb::new(0.36, 0.38, 0.44),
        primary: Srgb::new(0.04, 0.52, 1.0),
        success: Srgb::new(0.13, 0.65, 0.38),
        danger: Srgb::new(0.75, 0.22, 0.19),
    }
}

fn material_colors() -> SemanticColors {
    SemanticColors {
        background: Srgb::new(0.95, 0.94, 0.98),
        surface: Srgb::new(0.99, 0.98, 1.0),
        surface_panel: Srgb::new(0.96, 0.94, 0.99),
        text: Srgb::new(0.11, 0.11, 0.13),
        text_muted: Srgb::new(0.38, 0.37, 0.42),
        primary: Srgb::new(0.40, 0.32, 0.64),
        success: Srgb::new(0.11, 0.55, 0.41),
        danger: Srgb::new(0.73, 0.18, 0.15),
    }
}

/// Resolved tokens for a flavor (light appearance).
pub fn tokens_for(flavor: UiFlavor) -> ThemeTokens {
    match flavor {
        UiFlavor::Fluent => ThemeTokens {
            flavor,
            colors: fluent_colors(),
            radius_sm: 3.0,
            radius_md: 6.0,
            radius_lg: 10.0,
            shadow: ShadowTokens {
                blur_radius: 10.0,
                offset_y: 2.0,
                color_alpha: 0.14,
            },
            motion: MotionTokens {
                standard_ms: 120,
                emphasized_ms: 220,
            },
            backdrop_blur_hint: 0.35,
        },
        UiFlavor::LiquidGlass => ThemeTokens {
            flavor,
            colors: liquid_colors(),
            radius_sm: 6.0,
            radius_md: 12.0,
            radius_lg: 18.0,
            shadow: ShadowTokens {
                blur_radius: 18.0,
                offset_y: 4.0,
                color_alpha: 0.10,
            },
            motion: MotionTokens {
                standard_ms: 180,
                emphasized_ms: 320,
            },
            backdrop_blur_hint: 0.55,
        },
        UiFlavor::Material3 => ThemeTokens {
            flavor,
            colors: material_colors(),
            radius_sm: 8.0,
            radius_md: 14.0,
            radius_lg: 22.0,
            shadow: ShadowTokens {
                blur_radius: 12.0,
                offset_y: 3.0,
                color_alpha: 0.18,
            },
            motion: MotionTokens {
                standard_ms: 140,
                emphasized_ms: 280,
            },
            backdrop_blur_hint: 0.15,
        },
    }
}
