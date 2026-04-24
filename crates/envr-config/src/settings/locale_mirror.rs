use super::Settings;

/// True when the primary language subtag of a BCP-47–style tag is `zh` (e.g. `zh-CN`, `zh_TW.UTF-8`).
pub(crate) fn bcp47_primary_language_is_zh(tag: &str) -> bool {
    let t = tag.trim();
    let first = t
        .split(['-', '_'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    first == "zh"
}

/// POSIX `LANG` / `LC_*` hints (Unix shells, CI, WSL); secondary to [`sys_locale::get_locale`].
fn env_locale_vars_suggest_chinese() -> bool {
    for key in ["LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE"] {
        if let Ok(v) = std::env::var(key) {
            let l = v.to_ascii_lowercase();
            if l.contains("zh_cn")
                || l.contains("zh-cn")
                || l.contains("zh_hans")
                || l.starts_with("zh.")
                || bcp47_primary_language_is_zh(&v)
            {
                return true;
            }
        }
    }
    false
}

/// Heuristic: OS or environment suggests a Chinese locale (used for i18n `follow_system`).
///
/// Order: [`sys_locale::get_locale`] (cross-platform OS API), then `LC_*` / `LANG` / `LANGUAGE`.
pub fn system_locale_suggests_chinese() -> bool {
    if let Some(tag) = sys_locale::get_locale()
        && bcp47_primary_language_is_zh(&tag)
    {
        return true;
    }
    env_locale_vars_suggest_chinese()
}

/// Global China mirror preference switch (explicit user-controlled behavior).
pub fn prefer_china_mirrors(settings: &Settings) -> bool {
    settings.mirror.prefer_china_mirrors
}

/// Backward-compatible alias; keep during transition to explicit naming.
pub fn prefer_china_mirror_locale(settings: &Settings) -> bool {
    prefer_china_mirrors(settings)
}
