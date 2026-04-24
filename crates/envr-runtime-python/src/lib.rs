mod index;
mod manager;

pub use index::{
    DEFAULT_PYTHON_RELEASE_FILES_URL, DEFAULT_PYTHON_RELEASES_URL, PyRelease, PyReleaseFile,
    PythonIndex, blocking_http_client, fetch_json, list_latest_patch_per_major,
    list_remote_versions, load_python_index, normalize_python_version_label,
    parse_release_file_list, parse_release_list, pick_install_artifact,
    release_has_platform_assets, release_id_for_version_label, resolve_python_version,
};
pub use manager::{
    PythonManager, PythonPaths, list_installed_versions, python_executable,
    python_installation_valid, read_current,
};

use envr_config::settings::resolve_runtime_root;
use envr_domain::installer::SpecDrivenInstaller;
use envr_domain::runtime::{
    InstallRequest, RemoteFilter, ResolvedVersion, RuntimeKind, RuntimeProvider, RuntimeVersion,
    VersionSpec,
};
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use std::path::PathBuf;

pub struct PythonRuntimeProvider {
    releases_url: String,
    files_url: String,
    runtime_root_override: Option<std::path::PathBuf>,
}

impl PythonRuntimeProvider {
    pub fn new() -> Self {
        Self {
            releases_url: DEFAULT_PYTHON_RELEASES_URL.to_string(),
            files_url: DEFAULT_PYTHON_RELEASE_FILES_URL.to_string(),
            runtime_root_override: None,
        }
    }

    pub fn with_api_urls(releases_url: impl Into<String>, files_url: impl Into<String>) -> Self {
        Self {
            releases_url: releases_url.into(),
            files_url: files_url.into(),
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
            None => resolve_runtime_root()?,
        })
    }

    fn manager(&self) -> EnvrResult<PythonManager> {
        PythonManager::try_new(
            self.runtime_root()?,
            self.releases_url.clone(),
            self.files_url.clone(),
        )
    }

    fn load_index(&self) -> EnvrResult<PythonIndex> {
        let client = index::blocking_http_client()?;
        index::load_python_index(&client, &self.releases_url, &self.files_url)
    }

    fn remote_cache_ttl_secs() -> u64 {
        const DEFAULT: u64 = 24 * 60 * 60;
        std::env::var("ENVR_PYTHON_REMOTE_CACHE_TTL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT)
    }

    fn remote_latest_per_major_cache_file(
        &self,
        os: &str,
        arch: &str,
    ) -> EnvrResult<std::path::PathBuf> {
        let paths = PythonPaths::new(self.runtime_root()?);
        Ok(paths
            .cache_dir()
            .join(format!("remote_latest_per_major_{os}_{arch}.json")))
    }
}

impl Default for PythonRuntimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeProvider for PythonRuntimeProvider {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Python
    }

    fn list_installed(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let paths = PythonPaths::new(self.runtime_root()?);
        list_installed_versions(&paths)
    }

    fn current(&self) -> EnvrResult<Option<RuntimeVersion>> {
        let paths = PythonPaths::new(self.runtime_root()?);
        read_current(&paths)
    }

    fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        self.manager()?.set_current(version)
    }

    fn list_remote(&self, filter: &RemoteFilter) -> EnvrResult<Vec<RuntimeVersion>> {
        let idx = self.load_index()?;
        list_remote_versions(&idx, std::env::consts::OS, std::env::consts::ARCH, filter)
    }

    fn try_load_remote_latest_per_major_from_disk(&self) -> Vec<RuntimeVersion> {
        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;
        let Ok(cache_file) = self.remote_latest_per_major_cache_file(os, arch) else {
            return Vec::new();
        };
        let Some(list) =
            envr_platform::cache_recovery::read_json_string_list(&cache_file, None, |xs| {
                xs.len() >= 6
            })
        else {
            return Vec::new();
        };
        list.into_iter().map(RuntimeVersion).collect()
    }

    fn list_remote_latest_per_major(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;
        let ttl_secs = Self::remote_cache_ttl_secs();
        let cache_file = self.remote_latest_per_major_cache_file(os, arch)?;

        if let Some(list) = envr_platform::cache_recovery::read_json_string_list(
            &cache_file,
            Some(ttl_secs),
            |xs| xs.len() >= 6,
        ) {
            return Ok(list.into_iter().map(RuntimeVersion).collect());
        }

        let idx = self.load_index()?;
        let list = index::list_latest_patch_per_major(&idx, os, arch)?;

        // Best-effort cache write (don't fail the whole operation).
        let _ = (|| -> EnvrResult<()> {
            let paths = PythonPaths::new(self.runtime_root()?);
            std::fs::create_dir_all(paths.cache_dir())?;
            let strings: Vec<String> = list.iter().map(|v| v.0.clone()).collect();
            let s = serde_json::to_string(&strings)
                .map_err(|e| EnvrError::with_source(ErrorCode::Validation, "json encode python latest labels", e))?;
            envr_platform::fs_atomic::write_atomic(&cache_file, s.as_bytes())?;
            Ok(())
        })();

        Ok(list)
    }

    fn resolve(&self, spec: &VersionSpec) -> EnvrResult<ResolvedVersion> {
        let idx = self.load_index()?;
        let v =
            resolve_python_version(&idx, std::env::consts::OS, std::env::consts::ARCH, &spec.0)?;
        Ok(ResolvedVersion {
            version: RuntimeVersion(v),
        })
    }

    fn install(&self, request: &InstallRequest) -> EnvrResult<RuntimeVersion> {
        SpecDrivenInstaller::install_from_spec(&self.manager()?, request)
    }

    fn uninstall(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        self.manager()?.uninstall(version)
    }

    fn uninstall_dry_run_targets(
        &self,
        version: &RuntimeVersion,
    ) -> EnvrResult<(Vec<PathBuf>, Option<String>)> {
        let paths = PythonPaths::new(self.runtime_root()?);
        Ok((vec![paths.version_dir(&version.0)], None))
    }
}
