//! Composes [`super::tokens::base`] into full [`ThemeTokens`] per flavor + scheme.
//! User accent and Linux Material seed are applied in [`tokens_for_appearance`].

use super::flavor::UiFlavor;
use super::material_seed;
use super::scheme::UiScheme;
use super::tokens::base;
use super::tokens::{MotionTokens, SemanticColors, ShadowTokens, ThemeTokens};

const MOTION_STANDARD: MotionTokens = MotionTokens {
    standard_ms: 200,
    emphasized_ms: 300,
    easing_standard: [0.2, 0.0, 0.0, 1.0],
};

fn semantic_fluent_light() -> SemanticColors {
    SemanticColors {
        background: base::SURFACE_PAGE_LIGHT,
        surface: base::SURFACE_CARD_LIGHT,
        surface_panel: base::SURFACE_PANEL_LIGHT,
        text: base::TEXT_PRIMARY_LIGHT,
        text_muted: base::TEXT_MUTED_LIGHT,
        primary: base::BRAND_PRIMARY_FLUENT,
        success: base::SEMANTIC_SUCCESS,
        warning: base::SEMANTIC_WARNING,
        danger: base::SEMANTIC_ERROR,
    }
}

fn semantic_fluent_dark() -> SemanticColors {
    SemanticColors {
        background: base::SURFACE_PAGE_DARK,
        surface: base::SURFACE_CARD_DARK,
        surface_panel: base::SURFACE_PANEL_DARK,
        text: base::TEXT_PRIMARY_DARK,
        text_muted: base::TEXT_MUTED_DARK,
        primary: base::BRAND_PRIMARY_FLUENT_DARK,
        success: base::SEMANTIC_SUCCESS,
        warning: base::SEMANTIC_WARNING,
        danger: base::SEMANTIC_ERROR,
    }
}

fn semantic_liquid_light() -> SemanticColors {
    let mut c = semantic_fluent_light();
    c.surface_panel = liquid_panel_light();
    c.primary = base::BRAND_PRIMARY_LIQUID;
    c
}

fn semantic_liquid_dark() -> SemanticColors {
    let mut c = semantic_fluent_dark();
    c.primary = base::BRAND_PRIMARY_LIQUID_DARK;
    c
}

fn liquid_panel_light() -> super::color::Srgb {
    // Slightly cooler panel than generic (Liquid Glass)
    super::color::Srgb::new(0.94, 0.95, 0.98)
}

fn semantic_material_light() -> SemanticColors {
    let mut c = semantic_fluent_light();
    c.surface = base::SURFACE_CARD_LIGHT;
    c.surface_panel = super::color::Srgb::new(0.96, 0.94, 0.99);
    c.primary = base::BRAND_PRIMARY_MATERIAL_FALLBACK;
    c
}

fn semantic_material_dark() -> SemanticColors {
    let mut c = semantic_fluent_dark();
    c.surface_panel = super::color::Srgb::new(0.12, 0.11, 0.14);
    c.primary = base::BRAND_PRIMARY_MATERIAL_DARK;
    c
}

/// Resolved tokens for a flavor + scheme, then optional accent / Linux seed.
pub fn tokens_for_appearance(
    flavor: UiFlavor,
    scheme: UiScheme,
    accent: Option<super::color::Srgb>,
) -> ThemeTokens {
    let mut t = raw_tokens(flavor, scheme);
    if let Some(p) = accent {
        t.colors.primary = p;
    } else if flavor == UiFlavor::Material3
        && let Some(p) = material_seed::linux_material_primary_seed()
    {
        t.colors.primary = p;
    }
    t
}

/// Preset only (no accent / OS seed).
pub fn tokens_for_scheme(flavor: UiFlavor, scheme: UiScheme) -> ThemeTokens {
    tokens_for_appearance(flavor, scheme, None)
}

fn raw_tokens(flavor: UiFlavor, scheme: UiScheme) -> ThemeTokens {
    match (flavor, scheme) {
        (UiFlavor::Fluent, UiScheme::Light) => ThemeTokens {
            flavor,
            colors: semantic_fluent_light(),
            radius_sm: 4.0,
            radius_md: 8.0,
            radius_lg: 12.0,
            control_height_primary: 36.0,
            control_height_secondary: 32.0,
            shadow: ShadowTokens {
                blur_radius: 10.0,
                offset_y: 2.0,
                color_alpha: 0.14,
            },
            motion: MOTION_STANDARD,
            panel_border_alpha: 0.06,
            backdrop_blur_hint: 0.35,
            list_virtualize_min_rows: 28,
            min_interactive_size: 44.0,
            content_text_scale: 1.0,
        },
        (UiFlavor::Fluent, UiScheme::Dark) => ThemeTokens {
            flavor,
            colors: semantic_fluent_dark(),
            radius_sm: 4.0,
            radius_md: 8.0,
            radius_lg: 12.0,
            control_height_primary: 36.0,
            control_height_secondary: 32.0,
            shadow: ShadowTokens {
                blur_radius: 14.0,
                offset_y: 2.0,
                color_alpha: 0.30,
            },
            motion: MOTION_STANDARD,
            panel_border_alpha: 0.08,
            backdrop_blur_hint: 0.35,
            list_virtualize_min_rows: 28,
            min_interactive_size: 44.0,
            content_text_scale: 1.0,
        },
        (UiFlavor::LiquidGlass, UiScheme::Light) => ThemeTokens {
            flavor,
            colors: semantic_liquid_light(),
            radius_sm: 8.0,
            radius_md: 14.0,
            radius_lg: 20.0,
            control_height_primary: 36.0,
            control_height_secondary: 32.0,
            shadow: ShadowTokens {
                blur_radius: 18.0,
                offset_y: 4.0,
                color_alpha: 0.10,
            },
            motion: MOTION_STANDARD,
            panel_border_alpha: 0.06,
            backdrop_blur_hint: 0.55,
            list_virtualize_min_rows: 28,
            min_interactive_size: 44.0,
            content_text_scale: 1.0,
        },
        (UiFlavor::LiquidGlass, UiScheme::Dark) => ThemeTokens {
            flavor,
            colors: semantic_liquid_dark(),
            radius_sm: 8.0,
            radius_md: 14.0,
            radius_lg: 20.0,
            control_height_primary: 36.0,
            control_height_secondary: 32.0,
            shadow: ShadowTokens {
                blur_radius: 22.0,
                offset_y: 5.0,
                color_alpha: 0.26,
            },
            motion: MOTION_STANDARD,
            panel_border_alpha: 0.08,
            backdrop_blur_hint: 0.55,
            list_virtualize_min_rows: 28,
            min_interactive_size: 44.0,
            content_text_scale: 1.0,
        },
        (UiFlavor::Material3, UiScheme::Light) => ThemeTokens {
            flavor,
            colors: semantic_material_light(),
            radius_sm: 10.0,
            radius_md: 16.0,
            radius_lg: 24.0,
            control_height_primary: 36.0,
            control_height_secondary: 32.0,
            shadow: ShadowTokens {
                blur_radius: 12.0,
                offset_y: 3.0,
                color_alpha: 0.18,
            },
            motion: MOTION_STANDARD,
            panel_border_alpha: 0.06,
            backdrop_blur_hint: 0.15,
            list_virtualize_min_rows: 28,
            min_interactive_size: 44.0,
            content_text_scale: 1.0,
        },
        (UiFlavor::Material3, UiScheme::Dark) => ThemeTokens {
            flavor,
            colors: semantic_material_dark(),
            radius_sm: 10.0,
            radius_md: 16.0,
            radius_lg: 24.0,
            control_height_primary: 36.0,
            control_height_secondary: 32.0,
            shadow: ShadowTokens {
                blur_radius: 16.0,
                offset_y: 3.0,
                color_alpha: 0.32,
            },
            motion: MOTION_STANDARD,
            panel_border_alpha: 0.08,
            backdrop_blur_hint: 0.15,
            list_virtualize_min_rows: 28,
            min_interactive_size: 44.0,
            content_text_scale: 1.0,
        },
    }
}
