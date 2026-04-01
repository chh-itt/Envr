//! User-defined command aliases stored beside `settings.toml` (`config/aliases.toml`).

use envr_error::{EnvrError, EnvrResult};
use envr_platform::paths::EnvrPaths;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AliasesFile {
    #[serde(default)]
    pub aliases: BTreeMap<String, String>,
}

impl AliasesFile {
    pub fn path_from(paths: &EnvrPaths) -> PathBuf {
        paths.config_dir.join("aliases.toml")
    }

    pub fn load_or_default(path: impl AsRef<Path>) -> EnvrResult<Self> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(path).map_err(EnvrError::from)?;
        let file: AliasesFile = toml::from_str(&content).map_err(|err| {
            EnvrError::Config(format!("failed to parse {}: {err}", path.display()))
        })?;
        Ok(file)
    }

    pub fn save_to(&self, path: impl AsRef<Path>) -> EnvrResult<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(EnvrError::from)?;
        }
        let tmp = path.with_extension("toml.tmp");
        let content = toml::to_string_pretty(self)
            .map_err(|e| EnvrError::Runtime(format!("toml encode: {e}")))?;
        fs::write(&tmp, content).map_err(EnvrError::from)?;
        fs::rename(&tmp, path).map_err(EnvrError::from)?;
        Ok(())
    }
}
