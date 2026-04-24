//! Load `settings.toml` locale once per process so `envr-shim` hints match CLI/GUI i18n.

use std::sync::OnceLock;

use envr_config::env_context::load_settings_cached;
use envr_config::settings::LocaleMode;

static PREFER_ZH: OnceLock<bool> = OnceLock::new();

pub fn bootstrap() {
    let _ = prefer_zh();
}

pub fn bootstrap_with_locale(locale: LocaleMode) {
    let _ = PREFER_ZH.get_or_init(|| match locale {
        LocaleMode::ZhCn => true,
        LocaleMode::EnUs => false,
        LocaleMode::FollowSystem => envr_config::settings::system_locale_suggests_chinese(),
    });
}

fn prefer_zh() -> bool {
    *PREFER_ZH.get_or_init(|| {
        let Ok(st) = load_settings_cached() else {
            return false;
        };
        match st.i18n.locale {
            LocaleMode::ZhCn => true,
            LocaleMode::EnUs => false,
            LocaleMode::FollowSystem => envr_config::settings::system_locale_suggests_chinese(),
        }
    })
}

pub fn node_engines_hint(spec: &str, active: &str) -> String {
    if prefer_zh() {
        format!(
            "envr 提示：package.json 中 engines.node（{spec}）不满足当前 Node（{active}）。可用 `envr project add node@…` 对齐版本，或修改 package.json。"
        )
    } else {
        format!(
            "envr hint: package.json engines.node ({spec}) does not include the active Node ({active}). Align with: envr project add node@… or adjust engines in package.json."
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_hint_formats_placeholders() {
        let msg = node_engines_hint("^20", "18.19.0");
        assert!(msg.contains("^20"));
        assert!(msg.contains("18.19.0"));
    }
}
