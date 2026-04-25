mod index;
mod manager;

pub use index::{
    DEFAULT_BUILDS_BASE_URL, DEFAULT_BUILDS_INDEX_URL, DEFAULT_OTP_SERIES, ElixirBuild,
    list_latest_per_major, parse_elixir_builds, resolve_elixir_version,
};
pub use manager::{
    ElixirManager, ElixirPaths, elixir_installation_valid, list_installed_versions, read_current,
};

use envr_config::env_context::runtime_root;
use envr_domain::installer::install_via_manager;
use envr_domain::runtime::{
    InstallRequest, RemoteFilter, ResolvedVersion, RuntimeKind, RuntimeProvider, RuntimeVersion,
    VersionSpec,
};
use envr_domain::runtime::{MajorVersionRecord, VersionListAdapter, VersionRecord};
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Duration;

pub struct ElixirRuntimeProvider {
    builds_index_url: String,
    runtime_root_override: Option<PathBuf>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct ElixirBuildItem {
    version: String,
    otp: String,
}

#[derive(Debug, Clone)]
struct ElixirBuildParser {
    builds_base_url: String,
    preferred_otp: String,
}

impl envr_platform::remote_index_cache::RemoteIndexParser for ElixirBuildParser {
    type Item = ElixirBuildItem;

    fn parse(&self, body: &str) -> EnvrResult<Vec<Self::Item>> {
        let builds = index::parse_elixir_builds(body, &self.builds_base_url)?;
        let selected = index::select_builds_prefer_otp(&builds, &self.preferred_otp);
        Ok(selected
            .into_iter()
            .map(|b| ElixirBuildItem {
                version: b.version,
                otp: b.otp,
            })
            .collect())
    }

    fn version_label<'a>(&self, item: &'a Self::Item) -> &'a str {
        item.version.as_str()
    }
}

impl ElixirRuntimeProvider {
    pub fn new() -> Self {
        Self {
            builds_index_url: DEFAULT_BUILDS_INDEX_URL.to_string(),
            runtime_root_override: None,
        }
    }

    pub fn with_builds_index_url(mut self, url: impl Into<String>) -> Self {
        self.builds_index_url = url.into();
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

    fn manager(&self) -> EnvrResult<ElixirManager> {
        ElixirManager::try_new(self.runtime_root()?, self.builds_index_url.clone())
    }

    fn remote_cache_ttl_secs() -> u64 {
        const DEFAULT: u64 = 24 * 60 * 60;
        std::env::var("ENVR_ELIXIR_REMOTE_CACHE_TTL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT)
    }

    fn remote_latest_per_major_cache_file(&self) -> Option<PathBuf> {
        let root = self.runtime_root().ok()?;
        let paths = ElixirPaths::new(root);
        Some(paths.cache_dir().join("remote_latest_per_major.json"))
    }

    fn unified_list_dir(&self) -> EnvrResult<PathBuf> {
        let root = self.runtime_root()?;
        Ok(root.join("cache").join("elixir").join("unified_version_list"))
    }

    fn cached_index(
        &self,
    ) -> EnvrResult<envr_platform::remote_index_cache::CachedRemoteIndex<ElixirBuildParser>> {
        let unified_dir = self.unified_list_dir()?;
        Ok(envr_platform::remote_index_cache::CachedRemoteIndex::new(
            RuntimeKind::Elixir,
            unified_dir.clone(),
            envr_platform::remote_index_cache::RemoteSourceCache::new(unified_dir, "elixir_builds_index"),
            ElixirBuildParser {
                builds_base_url: DEFAULT_BUILDS_BASE_URL.to_string(),
                preferred_otp: DEFAULT_OTP_SERIES.to_string(),
            },
        ))
    }
}

impl Default for ElixirRuntimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeProvider for ElixirRuntimeProvider {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Elixir
    }

    fn list_installed(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let paths = ElixirPaths::new(self.runtime_root()?);
        list_installed_versions(&paths)
    }

    fn current(&self) -> EnvrResult<Option<RuntimeVersion>> {
        let paths = ElixirPaths::new(self.runtime_root()?);
        read_current(&paths)
    }

    fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        self.manager()?.set_current(version)
    }

    fn list_remote(&self, filter: &RemoteFilter) -> EnvrResult<Vec<RuntimeVersion>> {
        let builds = self.manager()?.load_builds()?;
        index::list_remote_versions(&builds, filter)
    }

    fn list_remote_installable(&self, filter: &RemoteFilter) -> EnvrResult<Vec<RuntimeVersion>> {
        self.list_remote(filter)
    }

    fn list_remote_majors(&self) -> EnvrResult<Vec<String>> {
        let mut majors = BTreeSet::<String>::new();
        for v in self.list_remote(&RemoteFilter::default())? {
            if let Some(m) = v.0.split('.').next()
                && !m.is_empty()
            {
                majors.insert(m.to_string());
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
        let builds = self.manager()?.load_builds()?;
        let list = index::list_latest_per_major(&builds)?;
        let _ = (|| -> EnvrResult<()> {
            let root = self.runtime_root()?;
            let paths = ElixirPaths::new(root);
            std::fs::create_dir_all(paths.cache_dir())?;
            let strings: Vec<String> = list.iter().map(|v| v.0.clone()).collect();
            let s = serde_json::to_string(&strings).map_err(|e| {
                EnvrError::with_source(ErrorCode::Validation, "json encode elixir latest labels", e)
            })?;
            let cache_file = paths.cache_dir().join("remote_latest_per_major.json");
            envr_platform::fs_atomic::write_atomic(&cache_file, s.as_bytes())?;
            Ok(())
        })();
        Ok(list)
    }

    fn list_remote_latest_installable_per_major(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        self.list_remote_latest_per_major()
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

    fn try_load_remote_latest_installable_per_major_from_disk(&self) -> Vec<RuntimeVersion> {
        self.try_load_remote_latest_per_major_from_disk()
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
        let paths = ElixirPaths::new(self.runtime_root()?);
        Ok((vec![paths.version_dir(&version.0)], None))
    }

    fn version_list_adapter(&self) -> Option<&dyn VersionListAdapter> {
        Some(self)
    }
}

impl VersionListAdapter for ElixirRuntimeProvider {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Elixir
    }

    fn load_major_rows_cached(&self) -> EnvrResult<Vec<MajorVersionRecord>> {
        self.cached_index()?.load_major_rows_cached()
    }

    fn refresh_major_rows_remote(&self) -> EnvrResult<Vec<MajorVersionRecord>> {
        let idx = self.cached_index()?;
        let ttl = Duration::from_secs(Self::remote_cache_ttl_secs());
        idx.refresh_major_rows_remote(&self.builds_index_url, ttl, envr_platform::remote_index_cache::CacheMode::StaleOk, |u| {
            let client = index::blocking_http_client()?;
            index::fetch_builds_index(&client, u)
        })
    }

    fn load_children_cached(&self, major_key: &str) -> EnvrResult<Vec<VersionRecord>> {
        self.cached_index()?.load_children_cached(major_key)
    }

    fn refresh_children_remote(&self, major_key: &str) -> EnvrResult<Vec<VersionRecord>> {
        let idx = self.cached_index()?;
        let ttl = Duration::from_secs(Self::remote_cache_ttl_secs());
        idx.refresh_children_remote(
            &self.builds_index_url,
            ttl,
            envr_platform::remote_index_cache::CacheMode::StaleOk,
            major_key,
            |u| {
                let client = index::blocking_http_client()?;
                index::fetch_builds_index(&client, u)
            },
        )
    }

    fn is_installable_on_host(&self, _version: &VersionRecord) -> bool {
        true
    }
}
