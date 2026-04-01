use envr_config::settings::{LocaleMode, Settings};
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locale {
    ZhCn,
    EnUs,
}

impl Locale {
    pub fn label(self) -> &'static str {
        match self {
            Locale::ZhCn => "简体中文",
            Locale::EnUs => "English",
        }
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(1); // default en-US

pub fn current() -> Locale {
    match CURRENT.load(Ordering::Relaxed) {
        0 => Locale::ZhCn,
        _ => Locale::EnUs,
    }
}

pub fn set(locale: Locale) {
    let v = match locale {
        Locale::ZhCn => 0,
        Locale::EnUs => 1,
    };
    CURRENT.store(v, Ordering::Relaxed);
}

pub fn init_from_settings(settings: &Settings) {
    let loc = match settings.i18n.locale {
        LocaleMode::ZhCn => Locale::ZhCn,
        LocaleMode::EnUs => Locale::EnUs,
        LocaleMode::FollowSystem => detect_system_locale(),
    };
    set(loc);
}

pub fn detect_system_locale() -> Locale {
    // Heuristic: prefer env vars that exist across platforms.
    let env = std::env::var("LC_ALL")
        .or_else(|_| std::env::var("LANG"))
        .or_else(|_| std::env::var("LANGUAGE"))
        .unwrap_or_default()
        .to_ascii_lowercase();

    if env.contains("zh") || env.contains("cn") {
        Locale::ZhCn
    } else {
        Locale::EnUs
    }
}

/// Translate between two static strings.
pub fn tr(zh_cn: &'static str, en_us: &'static str) -> &'static str {
    match current() {
        Locale::ZhCn => zh_cn,
        Locale::EnUs => en_us,
    }
}
