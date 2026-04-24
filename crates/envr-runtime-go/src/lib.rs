mod index;
mod manager;

pub use index::{
    DEFAULT_GO_DL_JSON_URL, GoRelease, blocking_http_client, fetch_go_index, go_dl_arch_for_rust,
    go_dl_os_for_rust, go_release_has_installable_archive, list_latest_stable_per_minor_line,
    list_remote_versions, normalize_go_version, parse_go_index, resolve_go_version,
};
pub use manager::{
    GoManager, GoPaths, go_installation_valid, list_installed_versions, read_current,
};

use envr_config::env_context::{load_settings_cached, runtime_root};
use envr_config::settings::{GoDownloadSource, prefer_china_mirrors};

use envr_domain::installer::install_via_manager;
use envr_domain::runtime::{
    InstallRequest, RemoteFilter, ResolvedVersion, RuntimeKind, RuntimeProvider, RuntimeVersion,
    VersionSpec,
};
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use std::path::PathBuf;

pub struct GoRuntimeProvider {
    runtime_root_override: Option<std::path::PathBuf>,
}

impl GoRuntimeProvider {
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

    fn manager(&self) -> EnvrResult<GoManager> {
        let (json_url, base_url) = self.resolved_dl_urls()?;
        GoManager::try_new(self.runtime_root()?, json_url, base_url)
    }

    fn load_releases(&self) -> EnvrResult<Vec<GoRelease>> {
        let client = blocking_http_client()?;
        let (json_url, _base_url) = self.resolved_dl_urls()?;
        let body = fetch_go_index(&client, &json_url)?;
        parse_go_index(&body)
    }

    fn remote_cache_ttl_secs() -> u64 {
        const DEFAULT: u64 = 24 * 60 * 60;
        std::env::var("ENVR_GO_REMOTE_CACHE_TTL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT)
    }

    fn remote_latest_per_major_cache_path(&self) -> EnvrResult<PathBuf> {
        let paths = GoPaths::new(self.runtime_root()?);
        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;
        Ok(paths
            .cache_dir()
            .join(format!("remote_latest_per_major_{os}_{arch}.json")))
    }

    fn resolved_dl_urls(&self) -> EnvrResult<(String, String)> {
        let s = load_settings_cached().unwrap_or_default();
        let src = match s.runtime.go.download_source {
            GoDownloadSource::Auto => {
                if prefer_china_mirrors(&s) {
                    GoDownloadSource::Domestic
                } else {
                    GoDownloadSource::Official
                }
            }
            other => other,
        };
        let base = match src {
            GoDownloadSource::Domestic => "https://golang.google.cn",
            GoDownloadSource::Official | GoDownloadSource::Auto => "https://go.dev",
        };
        // Keep the include=all behavior aligned with the existing index default.
        let json = format!("{base}/dl/?mode=json&include=all");
        Ok((json, base.to_string()))
    }
}

impl Default for GoRuntimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeProvider for GoRuntimeProvider {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Go
    }

    fn list_installed(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let paths = GoPaths::new(self.runtime_root()?);
        list_installed_versions(&paths)
    }

    fn current(&self) -> EnvrResult<Option<RuntimeVersion>> {
        let paths = GoPaths::new(self.runtime_root()?);
        read_current(&paths)
    }

    fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        self.manager()?.set_current(version)
    }

    fn list_remote(&self, filter: &RemoteFilter) -> EnvrResult<Vec<RuntimeVersion>> {
        let releases = self.load_releases()?;
        list_remote_versions(&releases, filter)
    }

    fn try_load_remote_latest_per_major_from_disk(&self) -> Vec<RuntimeVersion> {
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
        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;
        let ttl_secs = Self::remote_cache_ttl_secs();
        let cache_file = self.remote_latest_per_major_cache_path()?;

        if let Some(list) = envr_platform::cache_recovery::read_json_string_list(
            &cache_file,
            Some(ttl_secs),
            |xs| !xs.is_empty(),
        ) {
            return Ok(list.into_iter().map(RuntimeVersion).collect());
        }

        let releases = self.load_releases()?;
        let list = list_latest_stable_per_minor_line(&releases, os, arch)?;

        let _ = (|| -> EnvrResult<()> {
            let paths = GoPaths::new(self.runtime_root()?);
            std::fs::create_dir_all(paths.cache_dir())?;
            let strings: Vec<String> = list.iter().map(|v| v.0.clone()).collect();
            let s = serde_json::to_string(&strings).map_err(|e| {
                EnvrError::with_source(ErrorCode::Validation, "json encode go latest labels", e)
            })?;
            envr_platform::fs_atomic::write_atomic(&cache_file, s.as_bytes())?;
            Ok(())
        })();

        Ok(list)
    }

    fn resolve(&self, spec: &VersionSpec) -> EnvrResult<ResolvedVersion> {
        let releases = self.load_releases()?;
        let v = resolve_go_version(&releases, &spec.0)?;
        Ok(ResolvedVersion {
            version: RuntimeVersion(v),
        })
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
        let paths = GoPaths::new(self.runtime_root()?);
        Ok((vec![paths.version_dir(&version.0)], None))
    }
}
