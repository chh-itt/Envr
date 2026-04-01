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
    use super::{
        MotionTokens, SemanticColors, ShadowTokens, ThemeTokens, UiFlavor, UiScheme,
        default_flavor_for_target, scheme_for_mode, tokens_for_scheme,
    };
    use envr_config::settings::ThemeMode;

    #[test]
    fn ui_flavor_keys_and_labels_cover_all_variants() {
        assert_eq!(UiFlavor::ALL.len(), 3);
        for f in UiFlavor::ALL {
            assert!(!f.as_str().is_empty());
            assert!(!f.label_en().is_empty());
            assert!(!f.label_zh().is_empty());
            assert_eq!(format!("{f}"), f.as_str());
        }
    }

    #[test]
    fn scheme_for_mode_light_and_dark_are_deterministic() {
        assert_eq!(scheme_for_mode(ThemeMode::Light), UiScheme::Light);
        assert_eq!(scheme_for_mode(ThemeMode::Dark), UiScheme::Dark);
    }

    #[test]
    fn scheme_for_mode_follow_system_returns_a_scheme() {
        let s = scheme_for_mode(ThemeMode::FollowSystem);
        assert!(matches!(s, UiScheme::Light | UiScheme::Dark));
    }

    #[test]
    fn theme_tokens_spacing_matches_flavor() {
        let c = SemanticColors {
            background: super::Srgb::new(0.0, 0.0, 0.0),
            surface: super::Srgb::new(0.1, 0.1, 0.1),
            surface_panel: super::Srgb::new(0.12, 0.12, 0.12),
            text: super::Srgb::new(1.0, 1.0, 1.0),
            text_muted: super::Srgb::new(0.7, 0.7, 0.7),
            primary: super::Srgb::new(0.2, 0.5, 1.0),
            success: super::Srgb::new(0.2, 0.8, 0.3),
            danger: super::Srgb::new(0.9, 0.2, 0.2),
        };
        let shadow = ShadowTokens {
            blur_radius: 1.0,
            offset_y: 1.0,
            color_alpha: 0.5,
        };
        let motion = MotionTokens {
            standard_ms: 200,
            emphasized_ms: 300,
        };
        let t = ThemeTokens {
            flavor: UiFlavor::Fluent,
            colors: c,
            radius_sm: 4.0,
            radius_md: 8.0,
            radius_lg: 12.0,
            shadow,
            motion,
            backdrop_blur_hint: 0.0,
        };
        assert_eq!(t.content_spacing(), 10.0);
        assert_eq!(t.sidebar_width(), 188.0);
        let t2 = ThemeTokens {
            flavor: UiFlavor::Material3,
            ..t
        };
        assert_eq!(t2.content_spacing(), 14.0);
    }

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
