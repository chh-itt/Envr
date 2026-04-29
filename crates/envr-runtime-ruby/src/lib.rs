mod index;
mod manager;

pub use index::{
    DEFAULT_RUBY_RELEASES_URL, RubyRelease, list_latest_patch_per_major, parse_ruby_releases,
    resolve_ruby_version,
};
pub use manager::{
    RubyManager, RubyPaths, list_installed_versions, read_current, ruby_installation_valid,
};

use envr_config::env_context::runtime_root;
use envr_domain::installer::install_via_manager;
use envr_domain::runtime::{
    InstallRequest, RemoteFilter, ResolvedVersion, RuntimeKind, RuntimeProvider, RuntimeVersion,
    VersionSpec,
};
use envr_domain::runtime::{
    MajorVersionRecord, VersionListAdapter, VersionRecord, major_line_remote_install_blocked,
    version_line_key_for_kind,
};
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use std::collections::BTreeSet;
use std::path::PathBuf;

pub struct RubyRuntimeProvider {
    releases_url: String,
    runtime_root_override: Option<PathBuf>,
}

impl RubyRuntimeProvider {
    pub fn new() -> Self {
        Self {
            releases_url: DEFAULT_RUBY_RELEASES_URL.to_string(),
            runtime_root_override: None,
        }
    }

    pub fn with_releases_url(mut self, url: impl Into<String>) -> Self {
        self.releases_url = url.into();
        self
    }

    pub fn with_runtime_root(mut self, root: PathBuf) -> Self {
        self.runtime_root_override = Some(root);
        self
    }

    fn runtime_root(&self) -> EnvrResult<PathBuf> {
        Ok(match &self.runtime_root_override {
            Some(p) => p.clone(),
            None => runtime_root()?,
        })
    }

    fn manager(&self) -> EnvrResult<RubyManager> {
        RubyManager::try_new(self.runtime_root()?, self.releases_url.clone())
    }

    fn remote_cache_ttl_secs() -> u64 {
        const DEFAULT: u64 = 24 * 60 * 60;
        std::env::var("ENVR_RUBY_REMOTE_CACHE_TTL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT)
    }

    fn remote_latest_per_major_cache_file(&self) -> Option<PathBuf> {
        let root = self.runtime_root().ok()?;
        let paths = RubyPaths::new(root);
        Some(
            paths
                .cache_dir()
                .join("remote_latest_per_major_installer.json"),
        )
    }

    fn unified_list_dir(&self) -> EnvrResult<PathBuf> {
        let root = self.runtime_root()?;
        Ok(root.join("cache").join("ruby").join("unified_version_list"))
    }

    fn unified_major_rows_path(&self) -> EnvrResult<PathBuf> {
        Ok(self.unified_list_dir()?.join("major_rows.json"))
    }

    fn unified_children_path(&self, major_key: &str) -> EnvrResult<PathBuf> {
        Ok(self
            .unified_list_dir()?
            .join("children")
            .join(format!("{major_key}.json")))
    }
}

impl Default for RubyRuntimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeProvider for RubyRuntimeProvider {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Ruby
    }

    fn list_installed(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let paths = RubyPaths::new(self.runtime_root()?);
        list_installed_versions(&paths)
    }

    fn current(&self) -> EnvrResult<Option<RuntimeVersion>> {
        let paths = RubyPaths::new(self.runtime_root()?);
        read_current(&paths)
    }

    fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        self.manager()?.set_current(version)
    }

    fn list_remote(&self, filter: &RemoteFilter) -> EnvrResult<Vec<RuntimeVersion>> {
        let releases = self.manager()?.load_releases()?;
        index::list_remote_versions(&releases, filter)
    }

    fn list_remote_majors(&self) -> EnvrResult<Vec<String>> {
        let releases = self.manager()?.load_releases()?;
        let mut majors = BTreeSet::<String>::new();
        for release in releases {
            if let Some(major) = release.version.split('.').next()
                && !major.is_empty()
            {
                majors.insert(major.to_string());
            }
        }
        Ok(majors.into_iter().collect())
    }

    fn list_remote_latest_per_major(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let ttl_secs = Self::remote_cache_ttl_secs();
        if let Some(path) = self.remote_latest_per_major_cache_file()
            && let Some(list) =
                envr_platform::cache_recovery::read_json_string_list(&path, Some(ttl_secs), |xs| {
                    !xs.is_empty()
                })
        {
            return Ok(list.into_iter().map(RuntimeVersion).collect());
        }
        let releases = self.manager()?.load_releases()?;
        let list = index::list_latest_patch_per_major(&releases)?;
        let _ = (|| -> EnvrResult<()> {
            let root = self.runtime_root()?;
            let paths = RubyPaths::new(root);
            std::fs::create_dir_all(paths.cache_dir())?;
            let strings: Vec<String> = list.iter().map(|v| v.0.clone()).collect();
            let s = serde_json::to_string(&strings).map_err(|e| {
                EnvrError::with_source(ErrorCode::Validation, "json encode ruby latest labels", e)
            })?;
            let cache_file = paths
                .cache_dir()
                .join("remote_latest_per_major_installer.json");
            envr_platform::fs_atomic::write_atomic(&cache_file, s.as_bytes())?;
            Ok(())
        })();
        Ok(list)
    }

    fn try_load_remote_latest_per_major_from_disk(&self) -> Vec<RuntimeVersion> {
        let Some(path) = self.remote_latest_per_major_cache_file() else {
            return Vec::new();
        };
        let Some(list) =
            envr_platform::cache_recovery::read_json_string_list(&path, None, |xs| !xs.is_empty())
        else {
            return Vec::new();
        };
        list.into_iter().map(RuntimeVersion).collect()
    }

    fn resolve(&self, spec: &VersionSpec) -> EnvrResult<ResolvedVersion> {
        let version = self.manager()?.resolve_spec(&spec.0)?;
        Ok(ResolvedVersion { version })
    }

    fn install(&self, request: &InstallRequest) -> EnvrResult<RuntimeVersion> {
        install_via_manager(self.manager(), request)
    }

    fn uninstall(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        self.manager()?.uninstall(version)
    }

    fn uninstall_dry_run_targets(
        &self,
        version: &RuntimeVersion,
    ) -> EnvrResult<(Vec<PathBuf>, Option<String>)> {
        let paths = RubyPaths::new(self.runtime_root()?);
        Ok((vec![paths.version_dir(&version.0)], None))
    }

    fn version_list_adapter(&self) -> Option<&dyn VersionListAdapter> {
        Some(self)
    }
}

impl VersionListAdapter for RubyRuntimeProvider {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Ruby
    }

    fn load_major_rows_cached(&self) -> EnvrResult<Vec<MajorVersionRecord>> {
        fn unified_major_disk_ttl_secs() -> u64 {
            const DEFAULT: u64 = 10 * 60;
            std::env::var("ENVR_UNIFIED_LIST_MAJOR_DISK_TTL_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT)
        }
        let path = self.unified_major_rows_path()?;
        let raw = envr_platform::cache_recovery::read_json_string_list(
            &path,
            Some(unified_major_disk_ttl_secs()),
            |xs| !xs.is_empty(),
        )
        .or_else(|| {
            envr_platform::cache_recovery::read_json_string_list(&path, None, |xs| !xs.is_empty())
        })
        .unwrap_or_default();
        Ok(raw
            .into_iter()
            .filter_map(|v| {
                let major = version_line_key_for_kind(RuntimeKind::Ruby, &v)?;
                Some(MajorVersionRecord {
                    major_key: major,
                    latest_installable: Some(RuntimeVersion(v)),
                })
            })
            .filter(|r| !major_line_remote_install_blocked(RuntimeKind::Ruby, &r.major_key))
            .collect())
    }

    fn refresh_major_rows_remote(&self) -> EnvrResult<Vec<MajorVersionRecord>> {
        let latest = RuntimeProvider::list_remote_latest_installable_per_major(self)?;
        let rows: Vec<MajorVersionRecord> = latest
            .into_iter()
            .filter_map(|v| {
                let major = version_line_key_for_kind(RuntimeKind::Ruby, &v.0)?;
                Some(MajorVersionRecord {
                    major_key: major,
                    latest_installable: Some(v),
                })
            })
            .filter(|r| !major_line_remote_install_blocked(RuntimeKind::Ruby, &r.major_key))
            .collect();
        let labels: Vec<String> = rows
            .iter()
            .filter_map(|r| r.latest_installable.as_ref().map(|v| v.0.clone()))
            .collect();
        let _ = (|| -> EnvrResult<()> {
            let p = self.unified_major_rows_path()?;
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent).map_err(EnvrError::from)?;
            }
            let s = serde_json::to_string(&labels).map_err(|e| {
                EnvrError::with_source(
                    ErrorCode::Validation,
                    "json encode ruby unified major rows",
                    e,
                )
            })?;
            envr_platform::fs_atomic::write_atomic(&p, s.as_bytes())?;
            Ok(())
        })();
        Ok(rows)
    }

    fn load_children_cached(&self, major_key: &str) -> EnvrResult<Vec<VersionRecord>> {
        if major_line_remote_install_blocked(RuntimeKind::Ruby, major_key) {
            return Ok(Vec::new());
        }
        let path = self.unified_children_path(major_key)?;
        let raw =
            envr_platform::cache_recovery::read_json_string_list(&path, None, |xs| !xs.is_empty())
                .unwrap_or_default();
        Ok(raw
            .into_iter()
            .map(|v| VersionRecord {
                version: RuntimeVersion(v),
            })
            .collect())
    }

    fn refresh_children_remote(&self, major_key: &str) -> EnvrResult<Vec<VersionRecord>> {
        if major_line_remote_install_blocked(RuntimeKind::Ruby, major_key) {
            return Ok(Vec::new());
        }
        let all = RuntimeProvider::list_remote_installable(self, &RemoteFilter::default())?;
        let filtered: Vec<RuntimeVersion> = all
            .into_iter()
            .filter(|v| {
                version_line_key_for_kind(RuntimeKind::Ruby, &v.0).as_deref() == Some(major_key)
            })
            .collect();
        let labels: Vec<String> = filtered.iter().map(|v| v.0.clone()).collect();
        let _ = (|| -> EnvrResult<()> {
            let p = self.unified_children_path(major_key)?;
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent).map_err(EnvrError::from)?;
            }
            let s = serde_json::to_string(&labels).map_err(|e| {
                EnvrError::with_source(
                    ErrorCode::Validation,
                    "json encode ruby unified children",
                    e,
                )
            })?;
            envr_platform::fs_atomic::write_atomic(&p, s.as_bytes())?;
            Ok(())
        })();
        Ok(filtered
            .into_iter()
            .map(|v| VersionRecord { version: v })
            .collect())
    }

    fn is_installable_on_host(&self, _version: &VersionRecord) -> bool {
        true
    }
}
