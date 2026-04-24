use std::{fs, path::Path};

use envr_error::{EnvrError, EnvrResult, ErrorCode};

use super::Settings;
use super::runtime_root_cache::{
    runtime_root_cache_clear, settings_file_cache_get, settings_file_cache_insert,
    settings_file_cache_remove,
};
use super::settings_io::format_toml_settings_deser_error;
use super::storage_utils::backup_corrupted_file;

impl Settings {
    pub fn load_from(path: impl AsRef<Path>) -> EnvrResult<Self> {
        let path = path.as_ref();
        let content = fs::read_to_string(path).map_err(EnvrError::from)?;
        let settings: Settings = toml::from_str(&content).map_err(|err| {
            EnvrError::Config(format!(
                "failed to parse {}: {}",
                path.display(),
                format_toml_settings_deser_error(&content, &err)
            ))
        })?;
        settings.validate()?;
        Ok(settings)
    }

    pub fn load_or_default_from(path: impl AsRef<Path>) -> EnvrResult<Self> {
        let path = path.as_ref().to_path_buf();
        let mtime = fs::metadata(&path).ok().and_then(|m| m.modified().ok());

        if let Some(s) = settings_file_cache_get(&path, mtime) {
            return Ok(s);
        }

        let loaded: Settings = match Self::load_from(&path) {
            Ok(v) => v,
            Err(_err) => {
                if path.exists() {
                    let _ = backup_corrupted_file(&path);
                }
                let defaults = Settings::default();
                defaults.validate()?;
                defaults
            }
        };

        settings_file_cache_insert(path, mtime, loaded.clone());
        Ok(loaded)
    }

    pub fn save_to(&self, path: impl AsRef<Path>) -> EnvrResult<()> {
        self.validate()?;

        let path = path.as_ref();
        let content = toml::to_string_pretty(self)
            .map_err(|e| EnvrError::with_source(ErrorCode::Runtime, "toml encode settings", e))?;
        envr_platform::fs_atomic::write_atomic_with_backup(path, content.as_bytes(), "bak")
            .map_err(EnvrError::from)?;
        let pb = path.to_path_buf();
        settings_file_cache_remove(&pb);
        runtime_root_cache_clear();
        Ok(())
    }
}
