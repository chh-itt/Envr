mod index;
mod manager;

pub use index::{
    DEFAULT_BUN_TAGS_API, Tag, blocking_http_client, fetch_all_tags, fetch_tags,
    list_latest_patch_per_major_from_tags, list_remote_versions, parse_tags, resolve_bun_version,
};
pub use manager::{
    BunManager, BunPaths, bun_installation_valid, list_installed_versions, read_current,
};

use envr_config::env_context::runtime_root;
use envr_domain::installer::install_via_manager;
use envr_domain::runtime::{
    InstallRequest, RemoteFilter, ResolvedVersion, RuntimeKind, RuntimeProvider, RuntimeVersion,
    VersionSpec,
};
use envr_domain::runtime::{MajorVersionRecord, VersionListAdapter, VersionRecord};
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use envr_mirror::resolver::{load_settings_cached, maybe_mirror_url};
use envr_platform::paths::{current_platform_paths, index_cache_dir_from_platform};
use envr_platform::remote_index_cache::{CacheMode, CachedRemoteIndex, RemoteIndexParser, RemoteSourceCache};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub struct BunRuntimeProvider {
    tags_api: String,
    runtime_root_override: Option<std::path::PathBuf>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct BunTagItem {
    name: String,
}

#[derive(Debug, Clone)]
struct BunTagParser;

impl RemoteIndexParser for BunTagParser {
    type Item = BunTagItem;

    fn parse(&self, body: &str) -> EnvrResult<Vec<Self::Item>> {
        let tags = index::parse_tags(body)?;
        Ok(tags
            .into_iter()
            .map(|t| BunTagItem { name: t.name })
            .collect())
    }

    fn version_label<'a>(&self, item: &'a Self::Item) -> &'a str {
        item.name.as_str()
    }

    fn is_installable_on_host(&self, item: &Self::Item) -> bool {
        index::normalize_bun_version(&item.name).is_some_and(|v| {
            v.split('.')
                .next()
                .and_then(|s| s.parse::<u64>().ok())
                .is_some_and(|m| m >= 1)
        })
    }
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
            None => runtime_root()?,
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
        let settings = load_settings_cached()?;
        let offline = settings.mirror.mode == envr_config::settings::MirrorMode::Offline;

        if (offline || Self::file_is_within_ttl(&cache_file, ttl_secs))
            && let Ok(body) = std::fs::read_to_string(&cache_file)
        {
            match parse_tags(&body) {
                Ok(tags) if !tags.is_empty() => return Ok(tags),
                _ => {
                    let _ = std::fs::remove_file(&cache_file);
                }
            }
        }

        if offline {
            return Err(EnvrError::Download(format!(
                "offline mode: missing cached bun tags at {} (run `envr cache index sync`)",
                cache_file.display()
            )));
        }

        let client = blocking_http_client()?;
        let url = maybe_mirror_url(&settings, &self.tags_api)?;
        let tags = fetch_all_tags(&client, &url)?;
        let _ = (|| -> EnvrResult<()> {
            std::fs::create_dir_all(&base)?;
            let s = serde_json::to_string(&tags).map_err(|e| {
                EnvrError::with_source(ErrorCode::Validation, "json encode bun tags cache", e)
            })?;
            envr_platform::fs_atomic::write_atomic(&cache_file, s.as_bytes())?;
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

    fn unified_list_dir(&self) -> EnvrResult<PathBuf> {
        let root = self.runtime_root()?;
        Ok(root.join("cache").join("bun").join("unified_version_list"))
    }

    fn cached_index(&self) -> EnvrResult<CachedRemoteIndex<BunTagParser>> {
        let unified_dir = self.unified_list_dir()?;
        Ok(CachedRemoteIndex::new(
            RuntimeKind::Bun,
            unified_dir.clone(),
            RemoteSourceCache::new(unified_dir, "bun_github_tags"),
            BunTagParser,
        ))
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
        let Some(list) =
            envr_platform::cache_recovery::read_json_string_list(&path, None, |xs| !xs.is_empty())
        else {
            return Vec::new();
        };
        list.into_iter().map(RuntimeVersion).collect()
    }

    fn list_remote_latest_per_major(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let ttl_secs = Self::remote_cache_ttl_secs();
        let cache_file = self.remote_latest_per_major_cache_path()?;
        if let Some(list) = envr_platform::cache_recovery::read_json_string_list(
            &cache_file,
            Some(ttl_secs),
            |xs| !xs.is_empty(),
        ) {
            return Ok(list.into_iter().map(RuntimeVersion).collect());
        }

        let tags = self.load_tags()?;
        let list = list_latest_patch_per_major_from_tags(&tags);

        let _ = (|| -> EnvrResult<()> {
            let paths = BunPaths::new(self.runtime_root()?);
            std::fs::create_dir_all(paths.cache_dir())?;
            let s = serde_json::to_string(&list).map_err(|e| {
                EnvrError::with_source(ErrorCode::Validation, "json encode bun latest cache", e)
            })?;
            envr_platform::fs_atomic::write_atomic(&cache_file, s.as_bytes())?;
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
        install_via_manager(self.manager(), request)
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

    fn version_list_adapter(&self) -> Option<&dyn VersionListAdapter> {
        Some(self)
    }
}

impl VersionListAdapter for BunRuntimeProvider {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Bun
    }

    fn load_major_rows_cached(&self) -> EnvrResult<Vec<MajorVersionRecord>> {
        self.cached_index()?.load_major_rows_cached()
    }

    fn refresh_major_rows_remote(&self) -> EnvrResult<Vec<MajorVersionRecord>> {
        let idx = self.cached_index()?;
        let st = load_settings_cached()?;
        let offline = st.mirror.mode == envr_config::settings::MirrorMode::Offline;
        let mode = if offline { CacheMode::Offline } else { CacheMode::StaleOk };
        let ttl = Duration::from_secs(Self::remote_cache_ttl_secs());
        let url = maybe_mirror_url(&st, &self.tags_api)?;
        idx.refresh_major_rows_remote(url.as_str(), ttl, mode, |u| {
            let client = index::blocking_http_client()?;
            let tags = index::fetch_all_tags(&client, u)?;
            serde_json::to_string(&tags).map_err(|e| {
                EnvrError::with_source(ErrorCode::Validation, "json encode bun tags", e)
            })
        })
    }

    fn load_children_cached(&self, major_key: &str) -> EnvrResult<Vec<VersionRecord>> {
        self.cached_index()?.load_children_cached(major_key)
    }

    fn refresh_children_remote(&self, major_key: &str) -> EnvrResult<Vec<VersionRecord>> {
        let idx = self.cached_index()?;
        let st = load_settings_cached()?;
        let offline = st.mirror.mode == envr_config::settings::MirrorMode::Offline;
        let mode = if offline { CacheMode::Offline } else { CacheMode::StaleOk };
        let ttl = Duration::from_secs(Self::remote_cache_ttl_secs());
        let url = maybe_mirror_url(&st, &self.tags_api)?;
        idx.refresh_children_remote(url.as_str(), ttl, mode, major_key, |u| {
            let client = index::blocking_http_client()?;
            let tags = index::fetch_all_tags(&client, u)?;
            serde_json::to_string(&tags).map_err(|e| {
                EnvrError::with_source(ErrorCode::Validation, "json encode bun tags", e)
            })
        })
    }

    fn is_installable_on_host(&self, _version: &VersionRecord) -> bool {
        true
    }
}
