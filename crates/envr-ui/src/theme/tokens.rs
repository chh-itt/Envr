use super::color::Srgb;
use super::flavor::UiFlavor;

/// Elevation / shadow tuning (for panels, cards). Renderer may approximate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShadowTokens {
    pub blur_radius: f32,
    pub offset_y: f32,
    pub color_alpha: f32,
}

/// Durations for motion; widgets can interpolate later.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MotionTokens {
    pub standard_ms: u16,
    pub emphasized_ms: u16,
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
    pub shadow: ShadowTokens,
    pub motion: MotionTokens,
    /// Hint for future acrylic / blur passes (0 = none).
    pub backdrop_blur_hint: f32,
}

impl ThemeTokens {
    pub fn content_spacing(&self) -> f32 {
        match self.flavor {
            UiFlavor::Fluent => 10.0,
            UiFlavor::LiquidGlass => 12.0,
            UiFlavor::Material3 => 14.0,
        }
    }

    pub fn sidebar_width(&self) -> f32 {
        match self.flavor {
            UiFlavor::Fluent => 188.0,
            UiFlavor::LiquidGlass => 200.0,
            UiFlavor::Material3 => 196.0,
        }
    }
}
