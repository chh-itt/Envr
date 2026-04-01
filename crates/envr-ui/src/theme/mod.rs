//! Platform-oriented visual tokens (Fluent / Liquid Glass / Material 3).
//!
//! Renderer mappings (e.g. `iced::Theme`) live in `envr-gui` to keep this crate free of GUI stacks.

mod color;
mod detect;
mod flavor;
mod presets;
mod scheme;
mod tokens;

pub use color::Srgb;
pub use detect::default_flavor_for_target;
pub use flavor::UiFlavor;
pub use presets::tokens_for_scheme;
pub use scheme::{UiScheme, scheme_for_mode, system_prefers_dark_cached};
pub use tokens::{MotionTokens, SemanticColors, ShadowTokens, ThemeTokens};

#[cfg(test)]
mod tests {
    use super::{UiFlavor, UiScheme, default_flavor_for_target, tokens_for_scheme};

    #[test]
    fn preset_geometry_differs_across_flavors() {
        let f = tokens_for_scheme(UiFlavor::Fluent, UiScheme::Light);
        let l = tokens_for_scheme(UiFlavor::LiquidGlass, UiScheme::Light);
        let m = tokens_for_scheme(UiFlavor::Material3, UiScheme::Light);
        assert!(f.radius_md < l.radius_md && l.radius_md < m.radius_md);
        assert!(f.shadow.blur_radius < m.shadow.blur_radius);
    }

    #[test]
    fn default_flavor_follows_target_os() {
        let d = default_flavor_for_target();
        #[cfg(target_os = "windows")]
        assert_eq!(d, UiFlavor::Fluent);
        #[cfg(target_os = "macos")]
        assert_eq!(d, UiFlavor::LiquidGlass);
        #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
        assert_eq!(d, UiFlavor::Material3);
    }
}
