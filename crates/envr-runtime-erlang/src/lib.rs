mod index;
mod manager;

pub use index::{
    DEFAULT_GITHUB_TAGS_API, ErlangRelease, GithubTag, list_latest_per_major,
    normalize_otp_version, resolve_erlang_version,
};
pub use manager::{
    ErlangManager, ErlangPaths, erlang_installation_valid, list_installed_versions, read_current,
};

use envr_config::env_context::runtime_root;
use envr_domain::installer::SpecDrivenInstaller;
use envr_domain::runtime::{
    InstallRequest, RemoteFilter, ResolvedVersion, RuntimeKind, RuntimeProvider, RuntimeVersion,
    VersionSpec,
};
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use std::collections::BTreeSet;
use std::path::PathBuf;

pub struct ErlangRuntimeProvider {
    tags_api_url: String,
    runtime_root_override: Option<PathBuf>,
}

impl ErlangRuntimeProvider {
    pub fn new() -> Self {
        Self {
            tags_api_url: DEFAULT_GITHUB_TAGS_API.to_string(),
            runtime_root_override: None,
        }
    }

    pub fn with_tags_api_url(mut self, url: impl Into<String>) -> Self {
        self.tags_api_url = url.into();
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

    fn manager(&self) -> EnvrResult<ErlangManager> {
        ErlangManager::try_new(self.runtime_root()?, self.tags_api_url.clone())
    }

    fn remote_cache_ttl_secs() -> u64 {
        const DEFAULT: u64 = 24 * 60 * 60;
        std::env::var("ENVR_ERLANG_REMOTE_CACHE_TTL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT)
    }

    fn remote_latest_per_major_cache_file(&self) -> Option<PathBuf> {
        let root = self.runtime_root().ok()?;
        let paths = ErlangPaths::new(root);
        Some(paths.cache_dir().join("remote_latest_per_major.json"))
    }
}

impl Default for ErlangRuntimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeProvider for ErlangRuntimeProvider {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Erlang
    }

    fn list_installed(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let paths = ErlangPaths::new(self.runtime_root()?);
        list_installed_versions(&paths)
    }

    fn current(&self) -> EnvrResult<Option<RuntimeVersion>> {
        let paths = ErlangPaths::new(self.runtime_root()?);
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
        let releases = self.manager()?.load_releases()?;
        let list = index::list_latest_per_major(&releases)?;
        let _ = (|| -> EnvrResult<()> {
            let root = self.runtime_root()?;
            let paths = ErlangPaths::new(root);
            std::fs::create_dir_all(paths.cache_dir())?;
            let strings: Vec<String> = list.iter().map(|v| v.0.clone()).collect();
            let s = serde_json::to_string(&strings)
                .map_err(|e| EnvrError::with_source(ErrorCode::Validation, "json encode erlang latest labels", e))?;
            let cache_file = paths.cache_dir().join("remote_latest_per_major.json");
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
        self.manager()?.install_from_spec(request)
    }

    fn uninstall(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        self.manager()?.uninstall(version)
    }

    fn uninstall_dry_run_targets(
        &self,
        version: &RuntimeVersion,
    ) -> EnvrResult<(Vec<PathBuf>, Option<String>)> {
        let paths = ErlangPaths::new(self.runtime_root()?);
        Ok((vec![paths.version_dir(&version.0)], None))
    }
}
