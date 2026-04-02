use envr_error::{EnvrError, EnvrResult};
use envr_platform::paths::EnvrPaths;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MirrorMode {
    Official,
    Auto,
    Manual,
    Offline,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DownloadSettings {
    #[serde(default = "defaults::max_concurrent_downloads")]
    pub max_concurrent_downloads: u32,

    #[serde(default = "defaults::retry_max")]
    pub retry_max: u32,
}

impl Default for DownloadSettings {
    fn default() -> Self {
        Self {
            max_concurrent_downloads: defaults::max_concurrent_downloads(),
            retry_max: defaults::retry_max(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MirrorSettings {
    #[serde(default = "defaults::mirror_mode")]
    pub mode: MirrorMode,

    #[serde(default)]
    pub manual_id: Option<String>,
}

impl Default for MirrorSettings {
    fn default() -> Self {
        Self {
            mode: defaults::mirror_mode(),
            manual_id: None,
        }
    }
}

/// Persistent overrides for install layout (GUI + CLI read the same file).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PathSettings {
    /// If set (non-empty after trim), used as runtime root unless `ENVR_RUNTIME_ROOT` is set.
    #[serde(default)]
    pub runtime_root: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BehaviorSettings {
    /// Remove staging/temp artifacts after a successful install (providers may adopt later).
    #[serde(default)]
    pub cleanup_downloads_after_install: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FontMode {
    Auto,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    FollowSystem,
    Light,
    Dark,
}

impl Default for ThemeMode {
    fn default() -> Self {
        defaults::theme_mode()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocaleMode {
    FollowSystem,
    ZhCn,
    EnUs,
}

impl Default for LocaleMode {
    fn default() -> Self {
        defaults::locale_mode()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FontSettings {
    #[serde(default = "defaults::font_mode")]
    pub mode: FontMode,

    /// Used only when `mode = "custom"`.
    #[serde(default)]
    pub family: Option<String>,
}

impl Default for FontSettings {
    fn default() -> Self {
        Self {
            mode: defaults::font_mode(),
            family: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AppearanceSettings {
    #[serde(default)]
    pub font: FontSettings,

    #[serde(default = "defaults::theme_mode")]
    pub theme_mode: ThemeMode,

    /// Optional brand accent `#RGB` / `#RRGGBB`; merged into theme primary when valid (GUI-003).
    #[serde(default)]
    pub accent_color: Option<String>,
}

/// GUI-only state persisted in `settings.toml` so window layout/UX preferences survive restarts.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct GuiSettings {
    #[serde(default)]
    pub downloads_panel: DownloadsPanelSettings,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DownloadsPanelSettings {
    /// Whether the floating downloads panel is visible.
    #[serde(default = "defaults::downloads_panel_visible")]
    pub visible: bool,
    /// Whether the panel is expanded (shows job list).
    #[serde(default = "defaults::downloads_panel_expanded")]
    pub expanded: bool,
    /// Left offset in pixels from the window's left edge.
    #[serde(default = "defaults::downloads_panel_x")]
    pub x: i32,
    /// Bottom offset in pixels from the window's bottom edge.
    #[serde(default = "defaults::downloads_panel_y")]
    pub y: i32,
}

impl Default for DownloadsPanelSettings {
    fn default() -> Self {
        Self {
            visible: defaults::downloads_panel_visible(),
            expanded: defaults::downloads_panel_expanded(),
            x: defaults::downloads_panel_x(),
            y: defaults::downloads_panel_y(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RuntimeSettings {
    #[serde(default)]
    pub go: GoRuntimeSettings,
    #[serde(default)]
    pub bun: BunRuntimeSettings,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct GoRuntimeSettings {
    /// Optional `GOPROXY` value to inject into `envr env`/`run`/`exec` when Go is in scope.
    #[serde(default)]
    pub goproxy: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BunRuntimeSettings {
    /// Optional override for Bun global bin directory (defaults to `bun pm bin -g`).
    ///
    /// This affects shim sync for global Bun executables.
    #[serde(default)]
    pub global_bin_dir: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub paths: PathSettings,

    #[serde(default)]
    pub behavior: BehaviorSettings,

    #[serde(default)]
    pub appearance: AppearanceSettings,

    #[serde(default)]
    pub gui: GuiSettings,

    #[serde(default)]
    pub download: DownloadSettings,

    #[serde(default)]
    pub mirror: MirrorSettings,

    #[serde(default)]
    pub i18n: I18nSettings,

    #[serde(default)]
    pub runtime: RuntimeSettings,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct I18nSettings {
    #[serde(default = "defaults::locale_mode")]
    pub locale: LocaleMode,
}

impl Settings {
    pub fn validate(&self) -> EnvrResult<()> {
        if let Some(ref root) = self.paths.runtime_root
            && root.trim().is_empty()
        {
            return Err(EnvrError::Validation(
                "paths.runtime_root must not be whitespace-only".to_string(),
            ));
        }

        if self.download.max_concurrent_downloads == 0 {
            return Err(EnvrError::Validation(
                "download.max_concurrent_downloads must be >= 1".to_string(),
            ));
        }

        if self.mirror.mode == MirrorMode::Manual {
            let id_ok = self
                .mirror
                .manual_id
                .as_deref()
                .is_some_and(|s| !s.trim().is_empty());
            if !id_ok {
                return Err(EnvrError::Validation(
                    "mirror.manual_id is required when mirror.mode = manual".to_string(),
                ));
            }
        }

        if self.appearance.font.mode == FontMode::Custom {
            let ok = self
                .appearance
                .font
                .family
                .as_deref()
                .is_some_and(|s| !s.trim().is_empty());
            if !ok {
                return Err(EnvrError::Validation(
                    "appearance.font.family is required when appearance.font.mode = custom"
                        .to_string(),
                ));
            }
        }

        if let Some(ref gp) = self.runtime.go.goproxy
            && gp.trim().is_empty()
        {
            return Err(EnvrError::Validation(
                "runtime.go.goproxy must not be whitespace-only".to_string(),
            ));
        }

        if let Some(ref dir) = self.runtime.bun.global_bin_dir
            && dir.trim().is_empty()
        {
            return Err(EnvrError::Validation(
                "runtime.bun.global_bin_dir must not be whitespace-only".to_string(),
            ));
        }

        if self.gui.downloads_panel.x < 0 || self.gui.downloads_panel.y < 0 {
            return Err(EnvrError::Validation(
                "gui.downloads_panel x/y must be >= 0".to_string(),
            ));
        }

        Ok(())
    }

    pub fn load_from(path: impl AsRef<Path>) -> EnvrResult<Self> {
        let path = path.as_ref();
        let content = fs::read_to_string(path).map_err(EnvrError::from)?;
        let settings: Settings = toml::from_str(&content).map_err(|err| {
            EnvrError::Config(format!("failed to parse {}: {err}", path.display()))
        })?;
        settings.validate()?;
        Ok(settings)
    }

    pub fn load_or_default_from(path: impl AsRef<Path>) -> EnvrResult<Self> {
        let path = path.as_ref();
        match Self::load_from(path) {
            Ok(v) => Ok(v),
            Err(_err) => {
                if path.exists() {
                    let _ = backup_corrupted_file(path);
                }
                let defaults = Settings::default();
                defaults.validate()?;
                Ok(defaults)
            }
        }
    }

    pub fn save_to(&self, path: impl AsRef<Path>) -> EnvrResult<()> {
        self.validate()?;

        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(EnvrError::from)?;
        }

        let tmp_path = tmp_path_for(path);
        let content = toml::to_string_pretty(self)
            .map_err(|e| EnvrError::Runtime(format!("toml encode: {e}")))?;

        fs::write(&tmp_path, content).map_err(EnvrError::from)?;
        replace_file(&tmp_path, path)?;
        Ok(())
    }
}

pub struct SettingsCache {
    path: PathBuf,
    cached: Settings,
    last_modified: Option<SystemTime>,
}

impl SettingsCache {
    pub fn new(path: impl Into<PathBuf>) -> EnvrResult<Self> {
        let path = path.into();
        let cached = Settings::load_or_default_from(&path)?;
        let last_modified = file_mtime(&path).ok();
        Ok(Self {
            path,
            cached,
            last_modified,
        })
    }

    pub fn get(&mut self) -> EnvrResult<&Settings> {
        let mtime = file_mtime(&self.path).ok();
        if mtime != self.last_modified {
            self.cached = Settings::load_or_default_from(&self.path)?;
            self.last_modified = mtime;
        }
        Ok(&self.cached)
    }

    /// Reread `settings.toml` from disk even when mtime is unchanged (e.g. after external CLI edit in same second).
    pub fn reload(&mut self) -> EnvrResult<&Settings> {
        self.cached = Settings::load_or_default_from(&self.path)?;
        self.last_modified = file_mtime(&self.path).ok();
        Ok(&self.cached)
    }

    pub fn set_and_persist(&mut self, settings: Settings) -> EnvrResult<()> {
        settings.save_to(&self.path)?;
        self.cached = settings;
        self.last_modified = file_mtime(&self.path).ok();
        Ok(())
    }

    /// Replace cached settings without any disk I/O.
    ///
    /// Useful for GUI async flows where the settings were already loaded/saved
    /// off the UI thread.
    pub fn set_cached(&mut self, settings: Settings) {
        self.cached = settings;
        // Keep mtime tracking consistent so `get()` can stay in-memory unless disk changed.
        self.last_modified = file_mtime(&self.path).ok();
    }
}

pub fn settings_path_from_platform(paths: &EnvrPaths) -> PathBuf {
    paths.settings_file.clone()
}

/// Effective runtime data root: `ENVR_RUNTIME_ROOT` wins, then `settings.toml` `paths.runtime_root`,
/// then the platform default (`EnvrPaths::runtime_root`).
pub fn resolve_runtime_root() -> EnvrResult<PathBuf> {
    if let Ok(p) = std::env::var("ENVR_RUNTIME_ROOT") {
        let t = p.trim();
        if !t.is_empty() {
            return Ok(PathBuf::from(t));
        }
    }

    let platform = envr_platform::paths::current_platform_paths()?;
    let settings_path = settings_path_from_platform(&platform);
    let settings = Settings::load_or_default_from(&settings_path)?;
    if let Some(ref r) = settings.paths.runtime_root {
        let t = r.trim();
        if !t.is_empty() {
            return Ok(PathBuf::from(t));
        }
    }

    Ok(platform.runtime_root.clone())
}

fn file_mtime(path: &Path) -> EnvrResult<SystemTime> {
    let meta = fs::metadata(path).map_err(EnvrError::from)?;
    meta.modified()
        .map_err(|e| EnvrError::Io(std::io::Error::other(e)))
}

fn tmp_path_for(path: &Path) -> PathBuf {
    let mut tmp = path.to_path_buf();
    let ext = match path.extension().and_then(|s| s.to_str()) {
        Some(e) if !e.is_empty() => format!("{e}.tmp"),
        _ => "tmp".to_string(),
    };
    tmp.set_extension(ext);
    tmp
}

fn replace_file(tmp_path: &Path, final_path: &Path) -> EnvrResult<()> {
    if final_path.exists() {
        let bak = final_path.with_extension("bak");
        let _ = fs::remove_file(&bak);
        fs::rename(final_path, &bak).map_err(EnvrError::from)?;
    }
    fs::rename(tmp_path, final_path).map_err(EnvrError::from)?;
    Ok(())
}

fn backup_corrupted_file(path: &Path) -> EnvrResult<()> {
    let ts = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|e| EnvrError::Runtime(format!("time error: {e}")))?
        .as_secs();
    let bad = path.with_extension(format!("toml.bad.{ts}"));
    let _ = fs::rename(path, bad);
    Ok(())
}

mod defaults {
    use super::{FontMode, LocaleMode, MirrorMode, ThemeMode};

    pub fn max_concurrent_downloads() -> u32 {
        4
    }

    pub fn retry_max() -> u32 {
        3
    }

    pub fn mirror_mode() -> MirrorMode {
        MirrorMode::Auto
    }

    pub fn font_mode() -> FontMode {
        FontMode::Auto
    }

    pub fn theme_mode() -> ThemeMode {
        ThemeMode::FollowSystem
    }

    pub fn locale_mode() -> LocaleMode {
        LocaleMode::FollowSystem
    }

    pub fn downloads_panel_visible() -> bool {
        true
    }

    pub fn downloads_panel_expanded() -> bool {
        true
    }

    pub fn downloads_panel_x() -> i32 {
        12
    }

    pub fn downloads_panel_y() -> i32 {
        12
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn read_write_roundtrip_is_consistent() {
        let tmp = TempDir::new().expect("tmp");
        let path = tmp.path().join("settings.toml");

        let settings = Settings {
            paths: PathSettings {
                runtime_root: Some("/tmp/envr-rt".to_string()),
            },
            behavior: BehaviorSettings {
                cleanup_downloads_after_install: true,
            },
            appearance: AppearanceSettings {
                font: FontSettings {
                    mode: FontMode::Custom,
                    family: Some("Microsoft YaHei UI".to_string()),
                },
                theme_mode: ThemeMode::Dark,
                accent_color: None,
            },
            gui: GuiSettings {
                downloads_panel: DownloadsPanelSettings {
                    visible: true,
                    expanded: false,
                    x: 24,
                    y: 18,
                },
            },
            download: DownloadSettings {
                max_concurrent_downloads: 8,
                retry_max: 5,
            },
            mirror: MirrorSettings {
                mode: MirrorMode::Manual,
                manual_id: Some("cn-fast".to_string()),
            },
            i18n: I18nSettings {
                locale: LocaleMode::EnUs,
            },
            runtime: RuntimeSettings {
                go: GoRuntimeSettings {
                    goproxy: Some("https://proxy.golang.org,direct".to_string()),
                },
                bun: BunRuntimeSettings {
                    global_bin_dir: Some("/tmp/.bun/bin".to_string()),
                },
            },
        };

        settings.save_to(&path).expect("save");
        let loaded = Settings::load_from(&path).expect("load");
        assert_eq!(settings, loaded);
    }

    #[test]
    fn corrupted_file_recovers_defaults() {
        let tmp = TempDir::new().expect("tmp");
        let path = tmp.path().join("settings.toml");

        fs::write(&path, "not = toml = =").expect("write");
        let loaded = Settings::load_or_default_from(&path).expect("load_or_default");
        assert_eq!(loaded, Settings::default());
    }

    #[test]
    fn invalid_manual_mode_is_rejected() {
        let tmp = TempDir::new().expect("tmp");
        let path = tmp.path().join("settings.toml");

        fs::write(
            &path,
            r#"
[mirror]
mode = "manual"
"#,
        )
        .expect("write");

        let loaded = Settings::load_or_default_from(&path).expect("load_or_default");
        assert_eq!(loaded, Settings::default());
    }

    #[test]
    fn invalid_download_limits_recover_defaults() {
        let tmp = TempDir::new().expect("tmp");
        let path = tmp.path().join("settings.toml");

        fs::write(
            &path,
            r#"
[download]
max_concurrent_downloads = 0
retry_max = -1
"#,
        )
        .expect("write");

        let loaded = Settings::load_or_default_from(&path).expect("load_or_default");
        assert_eq!(loaded, Settings::default());
    }

    #[test]
    fn cache_set_cached_updates_in_memory_without_disk_write() {
        let tmp = TempDir::new().expect("tmp");
        let path = tmp.path().join("settings.toml");
        Settings::default().save_to(&path).expect("save default");

        let mut cache = SettingsCache::new(&path).expect("cache");
        let mut in_mem = Settings::default();
        in_mem.mirror.mode = MirrorMode::Offline;
        cache.set_cached(in_mem.clone());

        let got = cache.get().expect("get").clone();
        assert_eq!(got.mirror.mode, MirrorMode::Offline);

        // Disk content remains unchanged until explicitly persisted.
        let from_disk = Settings::load_from(&path).expect("load disk");
        assert_eq!(from_disk.mirror.mode, Settings::default().mirror.mode);
    }
}
