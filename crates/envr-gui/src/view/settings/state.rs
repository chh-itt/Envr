use envr_config::settings::{FontMode, LocaleMode, MirrorMode, Settings, SettingsCache};
use envr_error::EnvrResult;
use envr_ui::theme::Srgb;

/// In-memory editor bound to `settings.toml` via [`SettingsCache`].
pub struct SettingsViewState {
    pub cache: SettingsCache,
    pub draft: Settings,
    pub runtime_root_draft: String,
    pub manual_id_draft: String,
    pub max_conc_text: String,
    pub retry_text: String,
    pub font_family_draft: String,
    pub accent_color_draft: String,
    pub locale_mode_draft: LocaleMode,
    pub last_message: Option<String>,
}

impl SettingsViewState {
    pub fn new() -> Self {
        let paths =
            envr_platform::paths::current_platform_paths().expect("platform paths for settings");
        let path = envr_config::settings::settings_path_from_platform(&paths);
        let cache = SettingsCache::new(path).expect("settings cache");
        let mut s = Self {
            cache,
            draft: Settings::default(),
            runtime_root_draft: String::new(),
            manual_id_draft: String::new(),
            max_conc_text: String::new(),
            retry_text: String::new(),
            font_family_draft: String::new(),
            accent_color_draft: String::new(),
            locale_mode_draft: LocaleMode::FollowSystem,
            last_message: None,
        };
        s.sync_from_cache().expect("initial settings sync");
        s
    }

    pub fn sync_from_cache(&mut self) -> EnvrResult<()> {
        let st = self.cache.snapshot().clone();
        self.draft = st.clone();
        self.runtime_root_draft = st.paths.runtime_root.clone().unwrap_or_default();
        self.manual_id_draft = st.mirror.manual_id.clone().unwrap_or_default();
        self.max_conc_text = st.download.max_concurrent_downloads.to_string();
        self.retry_text = st.download.retry_max.to_string();
        self.font_family_draft = st.appearance.font.family.clone().unwrap_or_default();
        self.accent_color_draft = st.appearance.accent_color.clone().unwrap_or_default();
        self.locale_mode_draft = st.i18n.locale;
        Ok(())
    }

    pub fn build_settings(&self) -> EnvrResult<Settings> {
        let mut s = self.draft.clone();
        let rr = self.runtime_root_draft.trim();
        s.paths.runtime_root = if rr.is_empty() {
            None
        } else {
            Some(rr.to_string())
        };
        let mid = self.manual_id_draft.trim();
        s.mirror.manual_id = if mid.is_empty() {
            None
        } else {
            Some(mid.to_string())
        };
        s.download.max_concurrent_downloads = self.max_conc_text.trim().parse().map_err(|_| {
            envr_error::EnvrError::Validation(envr_core::i18n::tr_key(
                "gui.settings.err.max_conc",
                "download.max_concurrent_downloads 必须是正整数",
                "download.max_concurrent_downloads must be a positive integer",
            ))
        })?;
        s.download.retry_max = self.retry_text.trim().parse().map_err(|_| {
            envr_error::EnvrError::Validation(envr_core::i18n::tr_key(
                "gui.settings.err.retry",
                "download.retry_max 必须是整数",
                "download.retry_max must be an integer",
            ))
        })?;

        if s.appearance.font.mode == FontMode::Custom {
            let fam = self.font_family_draft.trim();
            s.appearance.font.family = if fam.is_empty() {
                None
            } else {
                Some(fam.to_string())
            };
        } else {
            s.appearance.font.family = None;
        }

        s.i18n.locale = self.locale_mode_draft;

        let ac = self.accent_color_draft.trim();
        s.appearance.accent_color = if ac.is_empty() {
            None
        } else {
            Srgb::from_hex(ac).map_err(|_| {
                envr_error::EnvrError::Validation(envr_core::i18n::tr_key(
                    "gui.settings.err.accent_hex",
                    "appearance.accent_color 须为 #RGB 或 #RRGGBB",
                    "appearance.accent_color must be #RGB or #RRGGBB",
                ))
            })?;
            Some(ac.to_string())
        };

        s.validate()?;
        Ok(s)
    }

    pub fn env_overrides_runtime_root() -> bool {
        std::env::var("ENVR_RUNTIME_ROOT")
            .map(|p| !p.trim().is_empty())
            .unwrap_or(false)
    }

    pub fn mirror_mode_label(m: MirrorMode) -> String {
        match m {
            MirrorMode::Official => envr_core::i18n::tr_key(
                "gui.settings.mirror.official",
                "official（仅官方）",
                "official (upstream only)",
            ),
            MirrorMode::Auto => envr_core::i18n::tr_key(
                "gui.settings.mirror.auto",
                "auto（自动测速）",
                "auto (probe fastest mirror)",
            ),
            MirrorMode::Manual => envr_core::i18n::tr_key(
                "gui.settings.mirror.manual",
                "manual（指定镜像 ID）",
                "manual (specific mirror ID)",
            ),
            MirrorMode::Offline => {
                envr_core::i18n::tr_key("gui.settings.mirror.offline", "offline", "offline")
            }
        }
    }
}

impl Default for SettingsViewState {
    fn default() -> Self {
        Self::new()
    }
}
