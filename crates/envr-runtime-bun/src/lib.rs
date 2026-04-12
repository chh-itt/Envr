mod index;
mod manager;
mod mirror;

pub use index::{
    DEFAULT_BUN_TAGS_API, Tag, blocking_http_client, fetch_all_tags, fetch_tags,
    list_latest_patch_per_major_from_tags, list_remote_versions, parse_tags, resolve_bun_version,
};
pub use manager::{
    BunManager, BunPaths, bun_installation_valid, list_installed_versions, read_current,
};

use envr_domain::runtime::{
    InstallRequest, RemoteFilter, ResolvedVersion, RuntimeKind, RuntimeProvider, RuntimeVersion,
    VersionSpec,
};
use envr_error::{EnvrError, EnvrResult};
use envr_platform::paths::{current_platform_paths, index_cache_dir_from_platform};
use std::path::{Path, PathBuf};

pub struct BunRuntimeProvider {
    tags_api: String,
    runtime_root_override: Option<std::path::PathBuf>,
}

impl BunRuntimeProvider {
    pub fn new() -> Self {
        Self {
            tags_api: DEFAULT_BUN_TAGS_API.to_string(),
            runtime_root_override: None,
        }
    }

    pub fn with_tags_api(mut self, url: impl Into<String>) -> Self {
        self.tags_api = url.into();
        self
    }

    pub fn with_runtime_root(mut self, root: std::path::PathBuf) -> Self {
        self.runtime_root_override = Some(root);
        self
    }

    fn runtime_root(&self) -> EnvrResult<std::path::PathBuf> {
        Ok(match &self.runtime_root_override {
            Some(p) => p.clone(),
            None => current_platform_paths()?.runtime_root,
        })
    }

    fn manager(&self) -> EnvrResult<BunManager> {
        BunManager::try_new(self.runtime_root()?, self.tags_api.clone())
    }

    fn load_tags(&self) -> EnvrResult<Vec<Tag>> {
        let platform = current_platform_paths()?;
        let base = index_cache_dir_from_platform(&platform).join("bun");
        let cache_file = base.join("tags.json");

        let ttl_secs = Self::remote_cache_ttl_secs();
        let settings = mirror::load_settings()?;
        let offline = settings.mirror.mode == envr_config::settings::MirrorMode::Offline;

        if (offline || Self::file_is_within_ttl(&cache_file, ttl_secs))
            && let Ok(body) = std::fs::read_to_string(&cache_file)
            && let Ok(tags) = parse_tags(&body)
            && !tags.is_empty()
        {
            return Ok(tags);
        }

        if offline {
            return Err(EnvrError::Download(format!(
                "offline mode: missing cached bun tags at {} (run `envr cache index sync`)",
                cache_file.display()
            )));
        }

        let client = blocking_http_client()?;
        let url = mirror::maybe_mirror_url(&settings, &self.tags_api)?;
        let tags = fetch_all_tags(&client, &url)?;
        let _ = (|| -> EnvrResult<()> {
            std::fs::create_dir_all(&base)?;
            let s = serde_json::to_string(&tags)
                .map_err(|e| envr_error::EnvrError::Validation(e.to_string()))?;
            std::fs::write(&cache_file, s)?;
            Ok(())
        })();
        Ok(tags)
    }

    fn remote_cache_ttl_secs() -> u64 {
        const DEFAULT: u64 = 24 * 60 * 60;
        std::env::var("ENVR_BUN_REMOTE_CACHE_TTL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT)
    }

    fn file_is_within_ttl(path: &Path, ttl_secs: u64) -> bool {
        if ttl_secs == 0 {
            return false;
        }
        let Ok(meta) = std::fs::metadata(path) else {
            return false;
        };
        let Ok(mtime) = meta.modified() else {
            return false;
        };
        let Ok(age) = std::time::SystemTime::now().duration_since(mtime) else {
            return false;
        };
        age.as_secs() <= ttl_secs
    }

    fn remote_latest_per_major_cache_path(&self) -> EnvrResult<PathBuf> {
        let paths = BunPaths::new(self.runtime_root()?);
        Ok(paths.cache_dir().join("remote_latest_per_major.json"))
    }

    fn exact_semver_spec(spec: &str) -> Option<String> {
        let s = spec.trim().trim_start_matches('v');
        let mut it = s.split('.');
        let a = it.next()?;
        let b = it.next()?;
        let c = it.next()?;
        if it.next().is_some() {
            return None;
        }
        if [a, b, c]
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|ch| ch.is_ascii_digit()))
            && a.parse::<u64>().ok()? >= 1
        {
            return Some(format!("{a}.{b}.{c}"));
        }
        None
    }
}

impl Default for BunRuntimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeProvider for BunRuntimeProvider {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Bun
    }

    fn list_installed(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let paths = BunPaths::new(self.runtime_root()?);
        list_installed_versions(&paths)
    }

    fn current(&self) -> EnvrResult<Option<RuntimeVersion>> {
        let paths = BunPaths::new(self.runtime_root()?);
        read_current(&paths)
    }

    fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        self.manager()?.set_current(version)
    }

    fn list_remote(&self, filter: &RemoteFilter) -> EnvrResult<Vec<RuntimeVersion>> {
        let tags = self.load_tags()?;
        list_remote_versions(&tags, filter)
    }

    fn try_load_remote_latest_per_major_from_disk(&self) -> Vec<RuntimeVersion> {
        let Ok(path) = self.remote_latest_per_major_cache_path() else {
            return Vec::new();
        };
        let Ok(s) = std::fs::read_to_string(&path) else {
            return Vec::new();
        };
        let Ok(list) = serde_json::from_str::<Vec<String>>(&s) else {
            return Vec::new();
        };
        list.into_iter().map(RuntimeVersion).collect()
    }

    fn list_remote_latest_per_major(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let ttl_secs = Self::remote_cache_ttl_secs();
        let cache_file = self.remote_latest_per_major_cache_path()?;
        if Self::file_is_within_ttl(&cache_file, ttl_secs)
            && let Ok(s) = std::fs::read_to_string(&cache_file)
            && let Ok(list) = serde_json::from_str::<Vec<String>>(&s)
            && !list.is_empty()
        {
            return Ok(list.into_iter().map(RuntimeVersion).collect());
        }

        let tags = self.load_tags()?;
        let list = list_latest_patch_per_major_from_tags(&tags);

        let _ = (|| -> EnvrResult<()> {
            let paths = BunPaths::new(self.runtime_root()?);
            std::fs::create_dir_all(paths.cache_dir())?;
            let s = serde_json::to_string(&list)
                .map_err(|e| envr_error::EnvrError::Validation(e.to_string()))?;
            std::fs::write(&cache_file, s)?;
            Ok(())
        })();

        Ok(list.into_iter().map(RuntimeVersion).collect())
    }

    fn resolve(&self, spec: &VersionSpec) -> EnvrResult<ResolvedVersion> {
        if let Some(v) = Self::exact_semver_spec(&spec.0) {
            return Ok(ResolvedVersion {
                version: RuntimeVersion(v),
            });
        }
        let tags = self.load_tags()?;
        let v = resolve_bun_version(&tags, &spec.0)?;
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
        let paths = BunPaths::new(self.runtime_root()?);
        Ok((vec![paths.version_dir(&version.0)], None))
    }
}
