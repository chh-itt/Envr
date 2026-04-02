use envr_config::settings::{LocaleMode, Settings};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

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
    #[cfg(target_os = "windows")]
    if let Some(loc) = windows_locale_name_from_registry() {
        return locale_from_bcp47_name(&loc);
    }

    #[cfg(target_os = "macos")]
    if let Some(loc) = macos_preferred_language_tag() {
        return locale_from_bcp47_name(&loc);
    }

    // Unix / fallback: LC_* / LANG are usually set (especially on Linux).
    let env = std::env::var("LC_ALL")
        .or_else(|_| std::env::var("LANG"))
        .or_else(|_| std::env::var("LANGUAGE"))
        .unwrap_or_default()
        .to_ascii_lowercase();

    if env.contains("zh") {
        Locale::ZhCn
    } else {
        Locale::EnUs
    }
}

/// Map BCP-47-ish tags (e.g. `zh-CN`, `zh-Hans-CN`) to our coarse UI locale.
fn locale_from_bcp47_name(s: &str) -> Locale {
    let lower = s.trim().to_ascii_lowercase();
    if lower.starts_with("zh") {
        return Locale::ZhCn;
    }
    Locale::EnUs
}

#[cfg(target_os = "windows")]
fn windows_locale_name_from_registry() -> Option<String> {
    use std::process::Command;

    let out = Command::new("reg")
        .args([
            "query",
            r"HKCU\Control Panel\International",
            "/v",
            "LocaleName",
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout);
    parse_reg_sz_locale_name(&s)
}

/// Parse `reg query` stdout for a line like `LocaleName    REG_SZ    zh-CN`.
fn parse_reg_sz_locale_name(reg_stdout: &str) -> Option<String> {
    for line in reg_stdout.lines() {
        let line = line.trim();
        if !line.contains("LocaleName") || !line.contains("REG_SZ") {
            continue;
        }
        let mut parts = line.split_whitespace();
        while let Some(p) = parts.next() {
            if p == "REG_SZ" {
                return parts.next().map(|s| s.to_string());
            }
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn macos_preferred_language_tag() -> Option<String> {
    use std::process::Command;

    let out = Command::new("defaults")
        .args(["read", "-g", "AppleLanguages"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).to_ascii_lowercase();
    // Example: ( "zh-Hans-CN", "en-US", )
    if s.contains("zh-hans")
        || s.contains("zh-hant")
        || s.contains("zh-cn")
        || s.contains("zh-tw")
        || s.contains("zh-hk")
        || s.contains("\"zh-")
    {
        return Some("zh-CN".into());
    }
    None
}

/// Translate between two static strings.
pub fn tr(zh_cn: &'static str, en_us: &'static str) -> &'static str {
    match current() {
        Locale::ZhCn => zh_cn,
        Locale::EnUs => en_us,
    }
}

fn zh_messages() -> &'static HashMap<String, String> {
    static ZH: OnceLock<HashMap<String, String>> = OnceLock::new();
    ZH.get_or_init(|| load_messages(include_str!("../../../locales/zh-CN.toml")))
}

fn en_messages() -> &'static HashMap<String, String> {
    static EN: OnceLock<HashMap<String, String>> = OnceLock::new();
    EN.get_or_init(|| load_messages(include_str!("../../../locales/en-US.toml")))
}

fn load_messages(raw: &str) -> HashMap<String, String> {
    let parsed = match raw.parse::<toml::Value>() {
        Ok(v) => v,
        Err(_) => return HashMap::new(),
    };
    let Some(tbl) = parsed.get("messages").and_then(|v| v.as_table()) else {
        return HashMap::new();
    };
    tbl.iter()
        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
        .collect()
}

/// Translate by i18n key, fallback to inline zh/en literals when key is missing.
pub fn tr_key(key: &str, zh_cn_fallback: &'static str, en_us_fallback: &'static str) -> String {
    match current() {
        Locale::ZhCn => zh_messages()
            .get(key)
            .cloned()
            .unwrap_or_else(|| zh_cn_fallback.to_string()),
        Locale::EnUs => en_messages()
            .get(key)
            .cloned()
            .unwrap_or_else(|| en_us_fallback.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use envr_config::settings::{I18nSettings, Settings};

    #[test]
    fn parses_locale_name_from_reg_output() {
        let sample = r#"
HKEY_CURRENT_USER\Control Panel\International
    LocaleName    REG_SZ    zh-CN

"#;
        assert_eq!(parse_reg_sz_locale_name(sample).as_deref(), Some("zh-CN"));
    }

    #[test]
    fn set_and_tr_follow_current_locale() {
        set(Locale::ZhCn);
        assert_eq!(tr("中文", "English"), "中文");
        set(Locale::EnUs);
        assert_eq!(tr("中文", "English"), "English");
    }

    #[test]
    fn init_from_settings_uses_explicit_locale() {
        let mut s = Settings {
            i18n: I18nSettings {
                locale: LocaleMode::ZhCn,
            },
            ..Default::default()
        };
        init_from_settings(&s);
        assert_eq!(current(), Locale::ZhCn);

        s.i18n.locale = LocaleMode::EnUs;
        init_from_settings(&s);
        assert_eq!(current(), Locale::EnUs);
    }

    #[test]
    fn locale_label_is_stable() {
        assert_eq!(Locale::ZhCn.label(), "简体中文");
        assert_eq!(Locale::EnUs.label(), "English");
    }

    #[test]
    fn bcp47_name_mapping_handles_zh_and_non_zh() {
        assert_eq!(locale_from_bcp47_name("zh-Hans-CN"), Locale::ZhCn);
        assert_eq!(locale_from_bcp47_name("en-US"), Locale::EnUs);
    }

    #[test]
    fn tr_key_uses_locales_and_falls_back() {
        set(Locale::EnUs);
        assert_eq!(tr_key("gui.action.install", "安装", "Install"), "Install");
        assert_eq!(tr_key("missing.key", "中文默认", "English default"), "English default");
        set(Locale::ZhCn);
        assert_eq!(tr_key("gui.action.install", "安装", "Install"), "安装");
    }
}
