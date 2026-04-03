use envr_config::settings::{Settings, SettingsCache, settings_path_from_platform};
use envr_error::EnvrResult;

pub struct RuntimeSettingsState {
    pub expanded: bool,
    pub cache: SettingsCache,
    pub draft: Settings,
    pub go_goproxy_draft: String,
    pub bun_global_bin_dir_draft: String,
    pub last_message: Option<String>,
}

impl RuntimeSettingsState {
    pub fn new() -> Self {
        let paths =
            envr_platform::paths::current_platform_paths().expect("platform paths for settings");
        let path = settings_path_from_platform(&paths);
        let cache = SettingsCache::new(path).expect("settings cache");
        let mut s = Self {
            expanded: false,
            cache,
            draft: Settings::default(),
            go_goproxy_draft: String::new(),
            bun_global_bin_dir_draft: String::new(),
            last_message: None,
        };
        s.sync_from_cache().expect("initial sync");
        s
    }

    pub fn sync_from_cache(&mut self) -> EnvrResult<()> {
        let st = self.cache.snapshot().clone();
        self.draft = st.clone();
        self.go_goproxy_draft = st.runtime.go.goproxy.clone().unwrap_or_default();
        self.bun_global_bin_dir_draft = st.runtime.bun.global_bin_dir.clone().unwrap_or_default();
        Ok(())
    }

    pub fn build_settings(&self) -> EnvrResult<Settings> {
        let mut s = self.draft.clone();
        let gp = self.go_goproxy_draft.trim();
        s.runtime.go.goproxy = if gp.is_empty() {
            None
        } else {
            Some(gp.to_string())
        };

        let bb = self.bun_global_bin_dir_draft.trim();
        s.runtime.bun.global_bin_dir = if bb.is_empty() {
            None
        } else {
            Some(bb.to_string())
        };

        s.validate()?;
        Ok(s)
    }

    // Persist is performed off the UI thread in `app.rs`.
}

impl Default for RuntimeSettingsState {
    fn default() -> Self {
        Self::new()
    }
}
