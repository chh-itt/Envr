use serde::{Deserialize, Serialize};

use super::defaults;

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

    /// Global total download bandwidth cap in bytes/sec. `0` means unlimited.
    #[serde(default = "defaults::max_bytes_per_sec")]
    pub max_bytes_per_sec: u64,

    #[serde(default = "defaults::retry_max")]
    pub retry_max: u32,
}

impl Default for DownloadSettings {
    fn default() -> Self {
        Self {
            max_concurrent_downloads: defaults::max_concurrent_downloads(),
            max_bytes_per_sec: defaults::max_bytes_per_sec(),
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

    /// Global preference switch: when true, runtimes that support a China mirror
    /// choose domestic sources for `auto` modes; unsupported runtimes remain official.
    #[serde(default = "defaults::prefer_china_mirrors")]
    pub prefer_china_mirrors: bool,
}

impl Default for MirrorSettings {
    fn default() -> Self {
        Self {
            mode: defaults::mirror_mode(),
            manual_id: None,
            prefer_china_mirrors: defaults::prefer_china_mirrors(),
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
    #[serde(default = "defaults::auto_sync_shims_on_use")]
    pub auto_sync_shims_on_use: bool,
    #[serde(default = "defaults::auto_sync_globals_on_use")]
    pub auto_sync_globals_on_use: bool,
    #[serde(default = "defaults::auto_sync_windows_path_mirror_on_use")]
    pub auto_sync_windows_path_mirror_on_use: bool,
    #[serde(default = "defaults::cache_artifact_ttl_days")]
    pub cache_artifact_ttl_days: u32,
    #[serde(default = "defaults::cache_max_size_mb")]
    pub cache_max_size_mb: u64,
    #[serde(default = "defaults::cache_auto_prune_on_start")]
    pub cache_auto_prune_on_start: bool,
}
