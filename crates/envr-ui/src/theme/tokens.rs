//! Single source of truth for design tokens (`tasks_gui.md` GUI-001 / §4).
//!
//! - **Surfaces & text** (`base`): shared light/dark roles; flavor presets compose these with
//!   platform **brand primaries** (GUI-002).
//! - **Spacing**: 8px base grid (`SpacingScale`).
//! - **Typography**: pixel sizes for shell pages (maps to iced `text().size(...)`).
//! - **Motion**: standard duration (~200ms) and Material-style easing coordinates for documentation
//!   or future animation bridges.

use super::color::Srgb;
use super::flavor::UiFlavor;

// --- Documented sRGB anchors (tasks_gui.md §4 + platform brand rows) -----------------------------

/// Light / dark shared surfaces and text. Flavor-specific presets start here, then swap `primary`.
pub mod base {
    use super::Srgb;

    // Surfaces
    /// Page background — `#F9F9F9`
    pub const SURFACE_PAGE_LIGHT: Srgb = Srgb::new(249.0 / 255.0, 249.0 / 255.0, 249.0 / 255.0);
    /// Page background — `#121212`
    pub const SURFACE_PAGE_DARK: Srgb = Srgb::new(18.0 / 255.0, 18.0 / 255.0, 18.0 / 255.0);

    /// Card / elevated surface — `#FFFFFF`
    pub const SURFACE_CARD_LIGHT: Srgb = Srgb::new(1.0, 1.0, 1.0);
    /// Card / elevated surface — `#1E1E1E`
    pub const SURFACE_CARD_DARK: Srgb = Srgb::new(30.0 / 255.0, 30.0 / 255.0, 30.0 / 255.0);

    /// Sidebar / panel tint (slightly off page) — light
    pub const SURFACE_PANEL_LIGHT: Srgb = Srgb::new(0.992, 0.992, 0.996);
    /// Sidebar / panel tint — dark
    pub const SURFACE_PANEL_DARK: Srgb = Srgb::new(0.11, 0.11, 0.12);

    /// Primary text — `#1E1E1E` (light scheme)
    pub const TEXT_PRIMARY_LIGHT: Srgb = Srgb::new(30.0 / 255.0, 30.0 / 255.0, 30.0 / 255.0);
    /// Primary text — `#E0E0E0` (dark scheme)
    pub const TEXT_PRIMARY_DARK: Srgb = Srgb::new(224.0 / 255.0, 224.0 / 255.0, 224.0 / 255.0);

    /// Secondary text — `#6C6C6C` (light)
    pub const TEXT_MUTED_LIGHT: Srgb = Srgb::new(108.0 / 255.0, 108.0 / 255.0, 108.0 / 255.0);
    /// Secondary text — `#9E9E9E` (dark)
    pub const TEXT_MUTED_DARK: Srgb = Srgb::new(158.0 / 255.0, 158.0 / 255.0, 158.0 / 255.0);

    // Brand primaries (GUI-002)
    /// Windows Fluent accent — `#0078D4`
    pub const BRAND_PRIMARY_FLUENT: Srgb = Srgb::new(0.0, 120.0 / 255.0, 212.0 / 255.0);
    /// macOS Liquid Glass accent — `#0A84FF`
    pub const BRAND_PRIMARY_LIQUID: Srgb = Srgb::new(10.0 / 255.0, 132.0 / 255.0, 1.0);
    /// Linux Material 3 fallback (matches Windows brand when dynamic seed unavailable)
    pub const BRAND_PRIMARY_MATERIAL_FALLBACK: Srgb = BRAND_PRIMARY_FLUENT;

    /// Fluent primary on dark surfaces (readable on `#121212`).
    pub const BRAND_PRIMARY_FLUENT_DARK: Srgb = Srgb::new(0.35, 0.72, 1.0);
    /// Liquid primary on dark surfaces.
    pub const BRAND_PRIMARY_LIQUID_DARK: Srgb = Srgb::new(0.40, 0.76, 1.0);
    /// Material fallback primary on dark surfaces.
    pub const BRAND_PRIMARY_MATERIAL_DARK: Srgb = Srgb::new(0.78, 0.70, 0.95);

    // Semantic (global)
    /// Error — `#D32F2F`
    pub const SEMANTIC_ERROR: Srgb = Srgb::new(211.0 / 255.0, 47.0 / 255.0, 47.0 / 255.0);
    /// Success — `#2E7D32`
    pub const SEMANTIC_SUCCESS: Srgb = Srgb::new(46.0 / 255.0, 125.0 / 255.0, 50.0 / 255.0);
    /// Warning — `#FBC02D`
    pub const SEMANTIC_WARNING: Srgb = Srgb::new(251.0 / 255.0, 192.0 / 255.0, 45.0 / 255.0);
}

/// 8px grid spacing (`tasks_gui.md` GUI-010). Use via [`ThemeTokens::space`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpacingScale {
    pub xs: u16,
    pub sm: u16,
    pub md: u16,
    pub lg: u16,
    pub xl: u16,
    pub xxl: u16,
}

/// Typography sizes (px) for routed pages and settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TypographyScale {
    pub page_title: f32,
    pub section: f32,
    pub subsection: f32,
    pub body: f32,
    pub body_small: f32,
    pub caption: f32,
    pub micro: f32,
    /// Dense secondary line (e.g. job status, footnotes).
    pub tiny: f32,
}

/// Elevation / shadow tuning (cards, panels). Renderer may approximate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShadowTokens {
    pub blur_radius: f32,
    pub offset_y: f32,
    pub color_alpha: f32,
}

/// Durations and easing metadata (`tasks_gui.md` GUI-040: ~200ms standard).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MotionTokens {
    /// Navigation, buttons, small transitions.
    pub standard_ms: u16,
    /// Emphasized panels / larger surfaces.
    pub emphasized_ms: u16,
    /// Material standard easing as cubic-bezier *P1x,P1y,P2x,P2y* (for docs / future CSS).
    pub easing_standard: [f32; 4],
}

/// Semantic palette independent of any single renderer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SemanticColors {
    pub background: Srgb,
    pub surface: Srgb,
    pub surface_panel: Srgb,
    pub text: Srgb,
    pub text_muted: Srgb,
    pub primary: Srgb,
    pub success: Srgb,
    pub warning: Srgb,
    pub danger: Srgb,
}

/// Full skin parameters for `envr` shells (iced or other).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeTokens {
    pub flavor: UiFlavor,
    pub colors: SemanticColors,
    pub radius_sm: f32,
    pub radius_md: f32,
    pub radius_lg: f32,
    /// Primary control height hint (Fluent ~36px, secondary ~32px — GUI-020).
    pub control_height_primary: f32,
    pub control_height_secondary: f32,
    pub shadow: ShadowTokens,
    pub motion: MotionTokens,
    /// Subtle hairline on panels (maps to iced border alpha).
    pub panel_border_alpha: f32,
    /// Hint for acrylic / blur passes (0 = none).
    pub backdrop_blur_hint: f32,
}

pub static SPACING_8PT: SpacingScale = SpacingScale {
    xs: 4,
    sm: 8,
    md: 12,
    lg: 16,
    xl: 24,
    xxl: 32,
};

/// Shell / window layout (`tasks_gui.md` GUI-010). Values are logical px.
pub mod shell {
    pub const WINDOW_DEFAULT_W: f32 = 1200.0;
    pub const WINDOW_DEFAULT_H: f32 = 720.0;
    pub const WINDOW_MIN_W: f32 = 960.0;
    pub const WINDOW_MIN_H: f32 = 600.0;
    /// Main reading column cap (12 × 80px grid units); keeps lines from stretching on ultra-wide windows.
    pub const CONTENT_MAX_WIDTH: f32 = 960.0;
}

impl ThemeTokens {
    /// Application shell: suggested sidebar width (`tasks_gui.md` GUI-010: 240px).
    pub fn sidebar_width(&self) -> f32 {
        240.0
    }

    /// Max width for the primary content column (centered when window is wider).
    pub fn content_max_width(&self) -> f32 {
        shell::CONTENT_MAX_WIDTH
    }

    /// Vertical gap after the page title before toolbar or first block (`tasks_gui.md` GUI-011).
    pub fn page_title_gap(&self) -> u16 {
        SPACING_8PT.lg
    }

    /// Default gap between major regions (content padding baseline).
    pub fn content_spacing(&self) -> f32 {
        SPACING_8PT.md as f32
    }

    /// Global spacing scale (8px grid).
    pub fn space(&self) -> &'static SpacingScale {
        &SPACING_8PT
    }

    /// Typography ramp; slightly larger on Liquid Glass (capsule / airy layout).
    pub fn typography(&self) -> TypographyScale {
        match self.flavor {
            UiFlavor::Fluent => TypographyScale {
                page_title: 22.0,
                section: 20.0,
                subsection: 17.0,
                body: 15.0,
                body_small: 14.0,
                caption: 13.0,
                micro: 12.0,
                tiny: 11.0,
            },
            UiFlavor::LiquidGlass => TypographyScale {
                page_title: 22.0,
                section: 20.0,
                subsection: 17.0,
                body: 15.0,
                body_small: 14.0,
                caption: 13.0,
                micro: 12.0,
                tiny: 11.0,
            },
            UiFlavor::Material3 => TypographyScale {
                page_title: 22.0,
                section: 20.0,
                subsection: 17.0,
                body: 15.0,
                body_small: 14.0,
                caption: 13.0,
                micro: 12.0,
                tiny: 11.0,
            },
        }
    }
}
