use envr_error::{EnvrError, EnvrResult};
use envr_platform::paths::EnvrPaths;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    time::SystemTime,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub download: DownloadSettings,

    #[serde(default)]
    pub mirror: MirrorSettings,
}

impl Settings {
    pub fn validate(&self) -> EnvrResult<()> {
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

    pub fn set_and_persist(&mut self, settings: Settings) -> EnvrResult<()> {
        settings.save_to(&self.path)?;
        self.cached = settings;
        self.last_modified = file_mtime(&self.path).ok();
        Ok(())
    }
}

pub fn settings_path_from_platform(paths: &EnvrPaths) -> PathBuf {
    paths.settings_file.clone()
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
    use super::MirrorMode;

    pub fn max_concurrent_downloads() -> u32 {
        4
    }

    pub fn retry_max() -> u32 {
        3
    }

    pub fn mirror_mode() -> MirrorMode {
        MirrorMode::Auto
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
            download: DownloadSettings {
                max_concurrent_downloads: 8,
                retry_max: 5,
            },
            mirror: MirrorSettings {
                mode: MirrorMode::Manual,
                manual_id: Some("cn-fast".to_string()),
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
}
