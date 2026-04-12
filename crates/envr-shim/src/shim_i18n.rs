//! Load `settings.toml` locale once per process so `envr-shim` hints match CLI/GUI i18n.

use std::sync::OnceLock;

static INIT: OnceLock<()> = OnceLock::new();

pub fn bootstrap() {
    INIT.get_or_init(|| {
        if let Ok(paths) = envr_platform::paths::current_platform_paths() {
            let p = envr_config::settings::settings_path_from_platform(&paths);
            if let Ok(st) = envr_config::settings::Settings::load_or_default_from(&p) {
                envr_core::i18n::init_from_settings(&st);
            }
        }
    });
}
