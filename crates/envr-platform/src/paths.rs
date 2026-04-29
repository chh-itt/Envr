use envr_error::{EnvrError, EnvrResult};
use std::{
    collections::HashMap,
    env,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetOs {
    Windows,
    Macos,
    Linux,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvrPaths {
    pub runtime_root: PathBuf,
    pub config_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub log_dir: PathBuf,
    pub settings_file: PathBuf,
}

impl EnvrPaths {
    pub fn new(base: PathBuf) -> Self {
        let config_dir = base.join("config");
        let cache_dir = base.join("cache");
        let log_dir = base.join("logs");
        let settings_file = config_dir.join("settings.toml");
        Self {
            runtime_root: base,
            config_dir,
            cache_dir,
            log_dir,
            settings_file,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct EnvSnapshot {
    pub vars: HashMap<String, String>,
    pub home_dir: Option<PathBuf>,
}

impl EnvSnapshot {
    pub fn capture_current() -> EnvrResult<Self> {
        let vars = env::vars().collect::<HashMap<_, _>>();
        let home_dir = match vars.get("HOME") {
            Some(home) if !home.is_empty() => Some(PathBuf::from(home)),
            _ => {
                #[cfg(windows)]
                {
                    vars.get("USERPROFILE").map(PathBuf::from)
                }
                #[cfg(not(windows))]
                {
                    None
                }
            }
        };

        Ok(Self { vars, home_dir })
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.vars.get(key).map(|s| s.as_str())
    }
}

fn join_home(home: &Path, suffix: &str) -> PathBuf {
    suffix
        .split('/')
        .fold(home.to_path_buf(), |acc, seg| acc.join(seg))
}

fn base_dir_for(os: TargetOs, envs: &EnvSnapshot) -> EnvrResult<PathBuf> {
    if let Some(root) = envs.get("ENVR_ROOT")
        && !root.is_empty()
    {
        return Ok(PathBuf::from(root));
    }

    match os {
        TargetOs::Windows => {
            if let Some(appdata) = envs.get("APPDATA")
                && !appdata.is_empty()
            {
                return Ok(PathBuf::from(appdata).join("envr"));
            }
            if let Some(localappdata) = envs.get("LOCALAPPDATA")
                && !localappdata.is_empty()
            {
                return Ok(PathBuf::from(localappdata).join("envr"));
            }
            let home = envs
                .home_dir
                .as_ref()
                .ok_or_else(|| EnvrError::Platform("missing home directory".to_string()))?;
            Ok(home.join(".envr"))
        }
        TargetOs::Macos => {
            let home = envs
                .home_dir
                .as_ref()
                .ok_or_else(|| EnvrError::Platform("missing home directory".to_string()))?;
            Ok(join_home(home, "Library/Application Support/envr"))
        }
        TargetOs::Linux => {
            if let Some(xdg) = envs.get("XDG_DATA_HOME")
                && !xdg.is_empty()
            {
                return Ok(PathBuf::from(xdg).join("envr"));
            }
            let home = envs
                .home_dir
                .as_ref()
                .ok_or_else(|| EnvrError::Platform("missing home directory".to_string()))?;
            Ok(home.join(".local").join("share").join("envr"))
        }
    }
}

pub fn compute_paths(os: TargetOs, envs: &EnvSnapshot) -> EnvrResult<EnvrPaths> {
    let base = base_dir_for(os, envs)?;
    Ok(EnvrPaths::new(base))
}

pub fn current_platform_paths() -> EnvrResult<EnvrPaths> {
    let envs = EnvSnapshot::capture_current()?;
    let os = if cfg!(target_os = "windows") {
        TargetOs::Windows
    } else if cfg!(target_os = "macos") {
        TargetOs::Macos
    } else {
        TargetOs::Linux
    };
    compute_paths(os, &envs)
}

/// Directory used to store offline-capable remote indexes (shared across machines via env var).
///
/// Resolution:
/// - `ENVR_INDEX_CACHE_DIR` (when set and non-empty after trim)
/// - `{paths.cache_dir}/indexes` (default)
pub fn index_cache_dir_from_platform(paths: &EnvrPaths) -> PathBuf {
    if let Ok(v) = std::env::var("ENVR_INDEX_CACHE_DIR") {
        let t = v.trim();
        if !t.is_empty() {
            return PathBuf::from(t);
        }
    }
    paths.cache_dir.join("indexes")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn windows_prefers_appdata() {
        let mut envs = EnvSnapshot::default();
        envs.vars.insert(
            "APPDATA".to_string(),
            r"C:\Users\me\AppData\Roaming".to_string(),
        );

        let paths = compute_paths(TargetOs::Windows, &envs).expect("paths");
        assert!(paths.runtime_root.ends_with(r"AppData\Roaming\envr"));
        assert!(paths.settings_file.ends_with(r"config\settings.toml"));
    }

    #[test]
    fn macos_uses_application_support() {
        let envs = EnvSnapshot {
            home_dir: Some(PathBuf::from("/Users/me")),
            ..Default::default()
        };

        let paths = compute_paths(TargetOs::Macos, &envs).expect("paths");
        assert_eq!(
            paths.runtime_root,
            PathBuf::from("/Users/me/Library/Application Support/envr")
        );
    }

    #[test]
    fn linux_prefers_xdg_data_home() {
        let mut envs = EnvSnapshot::default();
        envs.vars
            .insert("XDG_DATA_HOME".to_string(), "/home/me/.data".to_string());
        envs.home_dir = Some(PathBuf::from("/home/me"));

        let paths = compute_paths(TargetOs::Linux, &envs).expect("paths");
        assert_eq!(paths.runtime_root, PathBuf::from("/home/me/.data/envr"));
    }

    #[test]
    fn linux_falls_back_to_local_share() {
        let envs = EnvSnapshot {
            home_dir: Some(PathBuf::from("/home/me")),
            ..Default::default()
        };

        let paths = compute_paths(TargetOs::Linux, &envs).expect("paths");
        assert_eq!(
            paths.runtime_root,
            PathBuf::from("/home/me/.local/share/envr")
        );
    }
}
