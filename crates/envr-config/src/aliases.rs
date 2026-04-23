//! User-defined command aliases stored beside `settings.toml` (`config/aliases.toml`).

use envr_error::{EnvrError, EnvrResult, ErrorCode};
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
        let content = toml::to_string_pretty(self).map_err(|e| {
            EnvrError::with_source(ErrorCode::Runtime, "toml encode aliases", e)
        })?;
        envr_platform::fs_atomic::write_atomic(path, content.as_bytes())
            .map_err(EnvrError::from)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn load_missing_file_returns_default() {
        let tmp = TempDir::new().expect("tmp");
        let path = tmp.path().join("aliases.toml");
        let loaded = AliasesFile::load_or_default(&path).expect("load");
        assert!(loaded.aliases.is_empty());
    }

    #[test]
    fn save_and_load_roundtrip() {
        let tmp = TempDir::new().expect("tmp");
        let path = tmp.path().join("nested/aliases.toml");
        let mut f = AliasesFile::default();
        f.aliases.insert("ll".into(), "list --format json".into());
        f.aliases.insert("cu".into(), "current".into());

        f.save_to(&path).expect("save");
        let loaded = AliasesFile::load_or_default(&path).expect("load");
        assert_eq!(loaded, f);
    }

    #[test]
    fn malformed_aliases_file_is_error() {
        let tmp = TempDir::new().expect("tmp");
        let path = tmp.path().join("aliases.toml");
        fs::write(&path, "aliases = [").expect("write");
        let err = AliasesFile::load_or_default(&path).expect_err("must fail");
        assert!(err.to_string().contains("failed to parse"));
    }
}
