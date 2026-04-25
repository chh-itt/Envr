mod index;
mod manager;
#[cfg(not(windows))]
mod unix;

pub use index::{
    PhpReleasesIndex, ReleaseLine, blocking_http_client, fetch_php_windows_releases_json,
    list_latest_stable_per_minor_line_for_build, list_remote_versions, parse_php_windows_index,
    resolve_php_version,
};
pub use manager::{
    PhpManager, PhpPaths, list_installed_versions, list_installed_versions_unfiltered,
    php_installation_valid, read_current, read_current_global_want_ts,
    resolve_global_php_current_target,
};

use envr_config::env_context::{load_settings_cached, runtime_root};

use envr_config::settings::{PhpWindowsBuildFlavor, php_windows_releases_json_url};
use envr_domain::installer::install_via_manager;
use envr_domain::runtime::{
    InstallRequest, RemoteFilter, ResolvedVersion, RuntimeKind, RuntimeProvider, RuntimeVersion,
    VersionSpec,
};
use envr_domain::runtime::{MajorVersionRecord, VersionListAdapter, VersionRecord};
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use envr_platform::remote_index_cache::{CacheMode, CachedRemoteIndex, RemoteIndexParser, RemoteSourceCache};
use std::path::PathBuf;
use std::time::Duration;

pub struct PhpRuntimeProvider {
    runtime_root_override: Option<std::path::PathBuf>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct PhpIndexItem {
    version: String,
    builds: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone)]
struct PhpIndexParser {
    want_ts: bool,
    arch: &'static str,
}

impl RemoteIndexParser for PhpIndexParser {
    type Item = PhpIndexItem;

    fn parse(&self, body: &str) -> EnvrResult<Vec<Self::Item>> {
        let idx = crate::index::parse_php_windows_index(body)?;
        Ok(idx
            .into_values()
            .map(|line| PhpIndexItem {
                version: line.version,
                builds: line.builds,
            })
            .collect())
    }

    fn version_label<'a>(&self, item: &'a Self::Item) -> &'a str {
        item.version.as_str()
    }

    fn is_installable_on_host(&self, item: &Self::Item) -> bool {
        let line = crate::index::ReleaseLine {
            version: item.version.clone(),
            builds: item.builds.clone(),
        };
        crate::index::pick_windows_zip(&line, Some(self.want_ts), self.arch).is_ok()
    }
}

impl PhpRuntimeProvider {
    pub fn new() -> Self {
        Self {
            runtime_root_override: None,
        }
    }

    pub fn with_runtime_root(mut self, root: std::path::PathBuf) -> Self {
        self.runtime_root_override = Some(root);
        self
    }

    fn runtime_root(&self) -> EnvrResult<std::path::PathBuf> {
        Ok(match &self.runtime_root_override {
            Some(p) => p.clone(),
            None => runtime_root()?,
        })
    }

    fn manager(&self) -> EnvrResult<PhpManager> {
        let (json_url, want_ts) = self.resolved_remote_settings()?;
        PhpManager::try_new(self.runtime_root()?, json_url, want_ts)
    }

    fn load_index(&self) -> EnvrResult<PhpReleasesIndex> {
        let client = blocking_http_client()?;
        let (json_url, _want_ts) = self.resolved_remote_settings()?;
        let body = fetch_php_windows_releases_json(&client, &json_url)?;
        parse_php_windows_index(&body)
    }

    fn resolved_remote_settings(&self) -> EnvrResult<(String, bool)> {
        let s = load_settings_cached().unwrap_or_default();
        let url = php_windows_releases_json_url(&s).to_string();
        let want_ts = matches!(s.runtime.php.windows_build, PhpWindowsBuildFlavor::Ts);
        Ok((url, want_ts))
    }

    fn remote_cache_ttl_secs() -> u64 {
        const DEFAULT: u64 = 24 * 60 * 60;
        std::env::var("ENVR_PHP_REMOTE_CACHE_TTL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT)
    }

    fn remote_latest_per_major_cache_path(&self) -> EnvrResult<PathBuf> {
        let paths = PhpPaths::new(self.runtime_root()?);
        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;
        let (_, want_ts) = self.resolved_remote_settings()?;
        let flavor = if want_ts { "ts" } else { "nts" };
        Ok(paths
            .cache_dir()
            .join(format!("remote_latest_per_major_{os}_{arch}_{flavor}.json")))
    }

    fn unified_list_dir(&self) -> EnvrResult<std::path::PathBuf> {
        let root = self.runtime_root()?;
        Ok(root.join("cache").join("php").join("unified_version_list"))
    }

    fn cached_index(&self) -> EnvrResult<CachedRemoteIndex<PhpIndexParser>> {
        let (_, want_ts) = self.resolved_remote_settings()?;
        let arch = std::env::consts::ARCH;
        let unified_dir = self.unified_list_dir()?;
        Ok(CachedRemoteIndex::new(
            RuntimeKind::Php,
            unified_dir.clone(),
            RemoteSourceCache::new(unified_dir, if want_ts { "php_releases_ts" } else { "php_releases_nts" }),
            PhpIndexParser { want_ts, arch },
        ))
    }
}

impl Default for PhpRuntimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeProvider for PhpRuntimeProvider {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Php
    }

    fn list_installed(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let paths = PhpPaths::new(self.runtime_root()?);
        #[cfg(windows)]
        {
            let (_, want_ts) = self.resolved_remote_settings()?;
            list_installed_versions(&paths, want_ts)
        }
        #[cfg(not(windows))]
        {
            unix::sync_registered_versions(&paths)?;
            list_installed_versions_unfiltered(&paths)
        }
    }

    fn current(&self) -> EnvrResult<Option<RuntimeVersion>> {
        let paths = PhpPaths::new(self.runtime_root()?);
        read_current(&paths)
    }

    fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        #[cfg(windows)]
        {
            self.manager()?.set_current(version)
        }
        #[cfg(not(windows))]
        {
            let paths = PhpPaths::new(self.runtime_root()?);
            unix::sync_registered_versions(&paths)?;
            unix::set_global_current(&paths, version)
        }
    }

    fn list_remote(&self, filter: &RemoteFilter) -> EnvrResult<Vec<RuntimeVersion>> {
        if !cfg!(windows) {
            let _ = filter;
            return Ok(Vec::new());
        }
        let idx = self.load_index()?;
        list_remote_versions(&idx, filter)
    }

    fn try_load_remote_latest_per_major_from_disk(&self) -> Vec<RuntimeVersion> {
        if !cfg!(windows) {
            return Vec::new();
        }
        let Ok(path) = self.remote_latest_per_major_cache_path() else {
            return Vec::new();
        };
        let Some(list) =
            envr_platform::cache_recovery::read_json_string_list(&path, None, |xs| !xs.is_empty())
        else {
            return Vec::new();
        };
        list.into_iter().map(RuntimeVersion).collect()
    }

    fn list_remote_latest_per_major(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        if !cfg!(windows) {
            return Ok(Vec::new());
        }
        let ttl_secs = Self::remote_cache_ttl_secs();
        let cache_file = self.remote_latest_per_major_cache_path()?;
        if let Some(list) = envr_platform::cache_recovery::read_json_string_list(
            &cache_file,
            Some(ttl_secs),
            |xs| !xs.is_empty(),
        ) {
            return Ok(list.into_iter().map(RuntimeVersion).collect());
        }

        let idx = self.load_index()?;
        let (_, want_ts) = self.resolved_remote_settings()?;
        let arch = std::env::consts::ARCH;
        let list = list_latest_stable_per_minor_line_for_build(&idx, want_ts, arch)?;
        let _ = (|| -> EnvrResult<()> {
            let paths = PhpPaths::new(self.runtime_root()?);
            std::fs::create_dir_all(paths.cache_dir())?;
            let strings: Vec<String> = list.iter().map(|v| v.0.clone()).collect();
            let s = serde_json::to_string(&strings).map_err(|e| {
                EnvrError::with_source(ErrorCode::Validation, "json encode php latest labels", e)
            })?;
            envr_platform::fs_atomic::write_atomic(&cache_file, s.as_bytes())?;
            Ok(())
        })();
        Ok(list)
    }

    fn resolve(&self, spec: &VersionSpec) -> EnvrResult<ResolvedVersion> {
        if !cfg!(windows) {
            return Err(EnvrError::Platform(
                "php resolve is currently supported on Windows only".into(),
            ));
        }
        let idx = self.load_index()?;
        let v = resolve_php_version(&idx, &spec.0)?;
        Ok(ResolvedVersion {
            version: RuntimeVersion(v),
        })
    }

    fn install(&self, request: &InstallRequest) -> EnvrResult<RuntimeVersion> {
        #[cfg(windows)]
        {
            install_via_manager(self.manager(), request)
        }
        #[cfg(not(windows))]
        {
            let _ = request;
            Err(EnvrError::Platform(
                "php install from envr is not supported on this platform; install PHP with your system package manager or Homebrew, then refresh"
                    .into(),
            ))
        }
    }

    fn uninstall(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        #[cfg(windows)]
        {
            self.manager()?.uninstall(version)
        }
        #[cfg(not(windows))]
        {
            let paths = PhpPaths::new(self.runtime_root()?);
            unix::uninstall_registration(&paths, version)
        }
    }

    fn uninstall_dry_run_targets(
        &self,
        version: &RuntimeVersion,
    ) -> EnvrResult<(Vec<PathBuf>, Option<String>)> {
        let paths = PhpPaths::new(self.runtime_root()?);
        Ok((vec![paths.versions_dir().join(&version.0)], None))
    }

    fn version_list_adapter(&self) -> Option<&dyn VersionListAdapter> {
        if cfg!(windows) {
            Some(self)
        } else {
            None
        }
    }
}

impl VersionListAdapter for PhpRuntimeProvider {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Php
    }

    fn load_major_rows_cached(&self) -> EnvrResult<Vec<MajorVersionRecord>> {
        self.cached_index()?.load_major_rows_cached()
    }

    fn refresh_major_rows_remote(&self) -> EnvrResult<Vec<MajorVersionRecord>> {
        let idx = self.cached_index()?;
        let (url, _want_ts) = self.resolved_remote_settings()?;
        let ttl = Duration::from_secs(Self::remote_cache_ttl_secs());
        let st = load_settings_cached().unwrap_or_default();
        let mode = if st.mirror.mode == envr_config::settings::MirrorMode::Offline {
            CacheMode::Offline
        } else {
            CacheMode::StaleOk
        };

        let items = idx.load_items(&url, ttl, mode, |u| {
            let client = crate::index::blocking_http_client()?;
            crate::index::fetch_php_windows_releases_json(&client, u)
        })?;
        // Major rows should be stable-only for PHP (matches current provider behavior).
        let latest = idx.latest_installable_per_major_labels(&items, |it| {
            let t = it.version.trim().trim_start_matches('v');
            !t.contains('-')
        });
        let rows: Vec<MajorVersionRecord> = latest
            .into_iter()
            .filter_map(|v| {
                let major = envr_domain::runtime::version_line_key_for_kind(RuntimeKind::Php, &v)?;
                Some(MajorVersionRecord {
                    major_key: major,
                    latest_installable: Some(RuntimeVersion(v)),
                })
            })
            .collect();
        let data: Vec<String> = rows
            .iter()
            .filter_map(|r| r.latest_installable.as_ref().map(|v| v.0.clone()))
            .collect();
        let _ = (|| -> EnvrResult<()> {
            std::fs::create_dir_all(&idx.unified_dir).map_err(EnvrError::from)?;
            let s = serde_json::to_string(&data).map_err(|e| {
                EnvrError::with_source(ErrorCode::Validation, "json encode php major rows", e)
            })?;
            envr_platform::fs_atomic::write_atomic(&idx.unified_dir.join("major_rows.json"), s.as_bytes())?;
            Ok(())
        })();
        Ok(rows)
    }

    fn load_children_cached(&self, major_key: &str) -> EnvrResult<Vec<VersionRecord>> {
        self.cached_index()?.load_children_cached(major_key)
    }

    fn refresh_children_remote(&self, major_key: &str) -> EnvrResult<Vec<VersionRecord>> {
        let idx = self.cached_index()?;
        let (url, _want_ts) = self.resolved_remote_settings()?;
        let ttl = Duration::from_secs(Self::remote_cache_ttl_secs());
        let st = load_settings_cached().unwrap_or_default();
        let mode = if st.mirror.mode == envr_config::settings::MirrorMode::Offline {
            CacheMode::Offline
        } else {
            CacheMode::StaleOk
        };
        idx.refresh_children_remote(&url, ttl, mode, major_key, |u| {
            let client = crate::index::blocking_http_client()?;
            crate::index::fetch_php_windows_releases_json(&client, u)
        })
    }

    fn is_installable_on_host(&self, version: &VersionRecord) -> bool {
        let _ = version;
        true
    }
}
