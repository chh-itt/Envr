//! Managed **Perl** runtime: Windows **Strawberry Perl** portable zips, Unix **skaji/relocatable-perl** tarballs.

mod index;
mod manager;

pub use index::{
    DEFAULT_RELOCATABLE_RELEASES_URL, DEFAULT_STRAWBERRY_RELEASES_URL, PerlReleaseRow,
    PerlUpstream, blocking_http_client, fetch_all_perl_release_rows, fetch_text,
    list_remote_latest_per_major_lines, list_remote_versions, parse_cached_install_rows,
    perl_upstream, relocatable_archive_stem, resolve_perl_version,
};
pub use manager::{
    PerlManager, PerlPaths, list_installed_versions, promote_perl_extract, read_current,
};

use envr_domain::runtime::{
    InstallRequest, RemoteFilter, ResolvedVersion, RuntimeKind, RuntimeProvider, RuntimeVersion,
    VersionSpec,
};
use envr_error::EnvrResult;
use envr_platform::paths::current_platform_paths;
use std::path::PathBuf;

pub struct PerlRuntimeProvider {
    releases_url: String,
    runtime_root_override: Option<PathBuf>,
}

impl PerlRuntimeProvider {
    pub fn new() -> Self {
        let default_url = match perl_upstream() {
            Ok(PerlUpstream::StrawberryWindows64) => DEFAULT_STRAWBERRY_RELEASES_URL,
            Ok(PerlUpstream::RelocatableUnix) => DEFAULT_RELOCATABLE_RELEASES_URL,
            Err(_) => DEFAULT_RELOCATABLE_RELEASES_URL,
        };
        Self {
            releases_url: std::env::var("ENVR_PERL_GITHUB_RELEASES_URL")
                .ok()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| default_url.to_string()),
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
            None => current_platform_paths()?.runtime_root,
        })
    }

    fn manager(&self) -> EnvrResult<PerlManager> {
        PerlManager::try_new(self.runtime_root()?, self.releases_url.clone())
    }

    fn remote_latest_per_major_cache_path(&self) -> EnvrResult<PathBuf> {
        let paths = PerlPaths::new(self.runtime_root()?);
        let slug = match perl_upstream()? {
            PerlUpstream::StrawberryWindows64 => "strawberry_win64",
            PerlUpstream::RelocatableUnix => {
                return Ok(paths
                    .cache_dir()
                    .join(format!("remote_latest_per_major_{}.json", relocatable_archive_stem()?)));
            }
        };
        Ok(paths
            .cache_dir()
            .join(format!("remote_latest_per_major_{slug}.json")))
    }

    fn remote_cache_ttl_secs() -> u64 {
        const DEFAULT: u64 = 24 * 60 * 60;
        std::env::var("ENVR_PERL_REMOTE_CACHE_TTL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT)
    }
}

impl Default for PerlRuntimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeProvider for PerlRuntimeProvider {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Perl
    }

    fn list_installed(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let paths = PerlPaths::new(self.runtime_root()?);
        list_installed_versions(&paths)
    }

    fn current(&self) -> EnvrResult<Option<RuntimeVersion>> {
        let paths = PerlPaths::new(self.runtime_root()?);
        read_current(&paths)
    }

    fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        self.manager()?.set_current(version)
    }

    fn list_remote(&self, filter: &RemoteFilter) -> EnvrResult<Vec<RuntimeVersion>> {
        self.manager()?.list_remote(filter)
    }

    fn try_load_remote_latest_installable_per_major_from_disk(&self) -> Vec<RuntimeVersion> {
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

    fn list_remote_latest_installable_per_major(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let ttl_secs = Self::remote_cache_ttl_secs();
        let cache_file = self.remote_latest_per_major_cache_path()?;

        if let Some(list) = envr_platform::cache_recovery::read_json_string_list(
            &cache_file,
            Some(ttl_secs),
            |xs| !xs.is_empty(),
        ) {
            return Ok(list.into_iter().map(RuntimeVersion).collect());
        }

        let list = self.manager()?.list_remote_latest_per_major()?;

        let _ = (|| -> EnvrResult<()> {
            let paths = PerlPaths::new(self.runtime_root()?);
            std::fs::create_dir_all(paths.cache_dir())?;
            let strings: Vec<String> = list.iter().map(|v| v.0.clone()).collect();
            let s = serde_json::to_string(&strings)
                .map_err(|e| envr_error::EnvrError::Validation(e.to_string()))?;
            envr_platform::fs_atomic::write_atomic(&cache_file, s.as_bytes())?;
            Ok(())
        })();

        Ok(list)
    }

    fn resolve(&self, spec: &VersionSpec) -> EnvrResult<ResolvedVersion> {
        let v = self.manager()?.resolve_label(&spec.0)?;
        Ok(ResolvedVersion {
            version: RuntimeVersion(v),
        })
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
        let paths = PerlPaths::new(self.runtime_root()?);
        Ok((vec![paths.version_dir(&version.0)], None))
    }
}
