use envr_config::settings::{MirrorMode, Settings, SettingsCache};
use envr_error::EnvrResult;

/// In-memory editor bound to `settings.toml` via [`SettingsCache`].
pub struct SettingsViewState {
    pub cache: SettingsCache,
    pub draft: Settings,
    pub runtime_root_draft: String,
    pub manual_id_draft: String,
    pub max_conc_text: String,
    pub retry_text: String,
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
            last_message: None,
        };
        s.sync_from_cache().expect("initial settings sync");
        s
    }

    pub fn sync_from_cache(&mut self) -> EnvrResult<()> {
        let st = self.cache.get()?.clone();
        self.draft = st.clone();
        self.runtime_root_draft = st.paths.runtime_root.clone().unwrap_or_default();
        self.manual_id_draft = st.mirror.manual_id.clone().unwrap_or_default();
        self.max_conc_text = st.download.max_concurrent_downloads.to_string();
        self.retry_text = st.download.retry_max.to_string();
        Ok(())
    }

    pub fn reload_from_disk(&mut self) -> EnvrResult<()> {
        self.cache.reload()?;
        self.sync_from_cache()
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
            envr_error::EnvrError::Validation(
                "download.max_concurrent_downloads 必须是正整数".to_string(),
            )
        })?;
        s.download.retry_max = self.retry_text.trim().parse().map_err(|_| {
            envr_error::EnvrError::Validation("download.retry_max 必须是整数".to_string())
        })?;
        s.validate()?;
        Ok(s)
    }

    pub fn save(&mut self) -> EnvrResult<()> {
        let next = self.build_settings()?;
        self.cache.set_and_persist(next)?;
        self.sync_from_cache()?;
        Ok(())
    }

    pub fn env_overrides_runtime_root() -> bool {
        std::env::var("ENVR_RUNTIME_ROOT")
            .map(|p| !p.trim().is_empty())
            .unwrap_or(false)
    }

    pub fn mirror_mode_label(m: MirrorMode) -> &'static str {
        match m {
            MirrorMode::Official => "official（仅官方）",
            MirrorMode::Auto => "auto（自动测速）",
            MirrorMode::Manual => "manual（指定镜像 ID）",
            MirrorMode::Offline => "offline",
        }
    }
}

impl Default for SettingsViewState {
    fn default() -> Self {
        Self::new()
    }
}
