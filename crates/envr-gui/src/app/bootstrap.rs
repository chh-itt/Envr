use envr_config::settings::{FontMode, Settings};
use envr_ui::font;
use envr_ui::theme::Srgb;
use iced::Font;
use iced::font::Family;

pub(crate) fn ui_text_scale_from_env() -> f32 {
    std::env::var("ENVR_UI_SCALE")
        .ok()
        .and_then(|s| s.trim().parse::<f32>().ok())
        .unwrap_or(1.0)
        .clamp(0.85, 1.35)
}

pub(crate) fn env_flag(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .map(|s| {
            let t = s.trim().to_ascii_lowercase();
            t == "1" || t == "true" || t == "yes" || t == "on"
        })
        .unwrap_or(false)
}

pub(crate) fn accent_from_settings(st: &Settings) -> Option<Srgb> {
    st.appearance.accent_color.as_deref().and_then(|s| {
        let t = s.trim();
        if t.is_empty() {
            None
        } else {
            Srgb::from_hex(t).ok()
        }
    })
}

/// Default Iced font from persisted [`Settings`] (used by [`crate::app::run`]).
pub(crate) fn configured_default_font(st: &Settings) -> Font {
    match st.appearance.font.mode {
        FontMode::Auto => Font::with_name(font::preferred_system_sans_family()),
        FontMode::Custom => {
            let fam = st
                .appearance
                .font
                .family
                .as_deref()
                .unwrap_or(font::preferred_system_sans_family())
                .to_string();
            let leaked: &'static str = Box::leak(fam.into_boxed_str());
            Font {
                family: Family::Name(leaked),
                ..Font::default()
            }
        }
    }
}
