mod index;
mod manager;

pub use index::{
    DEFAULT_NODE_INDEX_JSON_URL, NodeRelease, NodeRemoteRow, blocking_http_client,
    fetch_node_index, list_node_remote_rows, list_remote_versions, node_version_v_prefix,
    normalize_node_version, parse_node_index, release_has_platform, resolve_node_version,
};
pub use manager::{
    NodeManager, NodePaths, dist_root_from_index_json_url, list_installed_versions,
    node_installation_valid, parse_shasums256, pick_node_dist_artifact, promote_single_root_dir,
    read_current,
};

use envr_config::env_context::{load_settings_cached, runtime_root};

use envr_config::settings;
use envr_domain::installer::install_via_manager;
use envr_domain::runtime::{
    InstallRequest, RemoteFilter, ResolvedVersion, RuntimeIndex, RuntimeInstaller, RuntimeKind,
    RuntimeProvider, RuntimeVersion, VersionSpec,
};
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use envr_platform::paths::{current_platform_paths, index_cache_dir_from_platform};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Node.js runtime provider (remote index, install layout under envr data root).
pub struct NodeRuntimeProvider {
    /// When set, bypasses `settings.toml` [`settings::node_index_json_url`] (tests / advanced).
    index_json_override: Option<String>,
    /// When `None`, uses the effective runtime root ([`envr_config::env_context::runtime_root`]).
    runtime_root_override: Option<std::path::PathBuf>,
}

impl NodeRuntimeProvider {
    pub fn new() -> Self {
        Self {
            index_json_override: None,
            runtime_root_override: None,
        }
    }

    pub fn with_index_json_url(url: impl Into<String>) -> Self {
        Self {
            index_json_override: Some(url.into()),
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

    fn resolved_index_json_url(&self) -> EnvrResult<String> {
        if let Some(u) = &self.index_json_override {
            return Ok(u.clone());
        }
        let s = load_settings_cached()?;
        Ok(settings::node_index_json_url(&s))
    }

    fn manager(&self) -> EnvrResult<NodeManager> {
        NodeManager::try_new(self.runtime_root()?, self.resolved_index_json_url()?)
    }

    /// Seconds to reuse on-disk `index.json`, `remote_majors_*.json`, and
    /// `remote_latest_per_major_*.json` before re-fetching.
    /// Default: 24h. Set to `0` to disable cache reads (always download).  
    /// Variable: `ENVR_NODE_REMOTE_CACHE_TTL_SECS`.
    fn remote_cache_ttl_secs() -> u64 {
        const DEFAULT: u64 = 24 * 60 * 60;
        std::env::var("ENVR_NODE_REMOTE_CACHE_TTL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT)
    }

    fn index_body_cache_path(&self) -> EnvrResult<PathBuf> {
        let platform = current_platform_paths()?;
        let base = index_cache_dir_from_platform(&platform).join("node");
        Ok(base.join("index.json"))
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

    /// Load `index.json` text from disk when fresh, otherwise download and refresh the on-disk copy.
    fn load_index_body_cached(&self) -> EnvrResult<String> {
        let ttl_secs = Self::remote_cache_ttl_secs();
        let index_cache = self.index_body_cache_path()?;

        if Self::file_is_within_ttl(&index_cache, ttl_secs)
            && let Ok(body) = std::fs::read_to_string(&index_cache)
        {
            if body.trim_start().starts_with('[') && index::parse_node_index(&body).is_ok() {
                return Ok(body);
            }
            let _ = std::fs::remove_file(&index_cache);
        }

        // Offline mode: do not attempt network fetch; require a cached index.
        let st = load_settings_cached()?;
        if st.mirror.mode == settings::MirrorMode::Offline {
            return Err(EnvrError::Download(format!(
                "offline mode: missing cached node index at {} (run `envr cache index sync`)",
                index_cache.display()
            )));
        }

        let url = self.resolved_index_json_url()?;
        let client = index::blocking_http_client()?;
        let body = index::fetch_node_index(&client, &url)?;
        let _ = (|| -> EnvrResult<()> {
            if let Some(parent) = index_cache.parent() {
                std::fs::create_dir_all(parent)?;
            }
            envr_platform::fs_atomic::write_atomic(&index_cache, body.as_bytes())?;
            Ok(())
        })();
        Ok(body)
    }

    fn load_releases(&self) -> EnvrResult<Vec<NodeRelease>> {
        let body = self.load_index_body_cached()?;
        index::parse_node_index(&body)
    }

    fn fetch_index_body(&self) -> EnvrResult<String> {
        self.load_index_body_cached()
    }

    fn remote_latest_per_major_cache_file(&self) -> Option<PathBuf> {
        let root = self.runtime_root().ok()?;
        let paths = NodePaths::new(root);
        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;
        Some(
            paths
                .cache_dir()
                .join(format!("remote_latest_per_major_{os}_{arch}.json")),
        )
    }

    pub fn index_port(&self) -> &dyn RuntimeIndex {
        self
    }

    pub fn installer_port(&self) -> &dyn RuntimeInstaller {
        self
    }
}

impl Default for NodeRuntimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeProvider for NodeRuntimeProvider {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Node
    }

    fn list_installed(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let paths = NodePaths::new(self.runtime_root()?);
        list_installed_versions(&paths)
    }

    fn current(&self) -> EnvrResult<Option<RuntimeVersion>> {
        let paths = NodePaths::new(self.runtime_root()?);
        read_current(&paths)
    }

    fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        self.manager()?.set_current(version)
    }

    fn list_remote(&self, filter: &RemoteFilter) -> EnvrResult<Vec<RuntimeVersion>> {
        let releases = self.load_releases()?;
        list_remote_versions(
            &releases,
            std::env::consts::OS,
            std::env::consts::ARCH,
            filter,
        )
    }

    fn list_remote_majors(&self) -> EnvrResult<Vec<String>> {
        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;

        // Disk cache: `remote_majors_*.json` when fresh; otherwise derive from cached `index.json`
        // (see `load_index_body_cached`) so a bad majors file does not force a re-download.
        let ttl_secs = Self::remote_cache_ttl_secs();
        let paths = NodePaths::new(self.runtime_root()?);
        let cache_file = paths
            .cache_dir()
            .join(format!("remote_majors_{os}_{arch}.json"));

        if let Ok(meta) = std::fs::metadata(&cache_file)
            && let Ok(mtime) = meta.modified()
            && let Ok(age) = std::time::SystemTime::now().duration_since(mtime)
            && age.as_secs() <= ttl_secs
            && let Ok(s) = std::fs::read_to_string(&cache_file)
        {
            if let Ok(list) = serde_json::from_str::<Vec<String>>(&s) {
                return Ok(list);
            }
            // Cache exists but is not in the expected format (e.g. accidentally
            // contains `index.json` or other JSON). Remove it so we can rebuild.
            let _ = std::fs::remove_file(&cache_file);
        }

        let body = self.fetch_index_body()?;
        // Prefer the streaming parser to keep memory stable. If the upstream JSON is
        // unexpectedly shaped (or a proxy returns a different JSON schema), fall back
        // to full parsing and derive majors from releases.
        let majors = match index::parse_node_major_keys(&body, os, arch) {
            Ok(m) => m,
            Err(primary) => {
                let releases = index::parse_node_index(&body)?;
                let mut majors: HashSet<String> = HashSet::new();
                for r in releases {
                    if !index::release_has_platform(&r.files, os, arch) {
                        continue;
                    }
                    let t = r.version.trim_start_matches('v');
                    let major = t.split('.').next().unwrap_or("");
                    if !major.is_empty() && major.chars().all(|c| c.is_ascii_digit()) {
                        majors.insert(major.to_string());
                    }
                }
                let mut out: Vec<String> = majors.into_iter().collect();
                out.sort_by(|a, b| {
                    let na = a.parse::<u64>().unwrap_or(0);
                    let nb = b.parse::<u64>().unwrap_or(0);
                    nb.cmp(&na)
                });
                if out.is_empty() {
                    return Err(primary);
                }
                out
            }
        };

        // Best-effort cache write (don't fail the whole operation if disk write fails).
        let _ = (|| -> EnvrResult<()> {
            std::fs::create_dir_all(paths.cache_dir())?;
            let s = serde_json::to_string(&majors).map_err(|e| {
                EnvrError::with_source(ErrorCode::Validation, "json encode node major list", e)
            })?;
            envr_platform::fs_atomic::write_atomic(&cache_file, s.as_bytes())?;
            Ok(())
        })();

        Ok(majors)
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

    fn list_remote_latest_per_major(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;
        let ttl_secs = Self::remote_cache_ttl_secs();
        let paths = NodePaths::new(self.runtime_root()?);
        let cache_file = paths
            .cache_dir()
            .join(format!("remote_latest_per_major_{os}_{arch}.json"));

        if let Some(list) = envr_platform::cache_recovery::read_json_string_list(
            &cache_file,
            Some(ttl_secs),
            |xs| !xs.is_empty(),
        ) {
            return Ok(list.into_iter().map(RuntimeVersion).collect());
        }

        let body = self.load_index_body_cached()?;
        let releases = index::parse_node_index(&body)?;
        let list = index::list_latest_patch_per_major(&releases, os, arch)?;

        let _ = (|| -> EnvrResult<()> {
            std::fs::create_dir_all(paths.cache_dir())?;
            let strings: Vec<String> = list.iter().map(|v| v.0.clone()).collect();
            let s = serde_json::to_string(&strings).map_err(|e| {
                EnvrError::with_source(ErrorCode::Validation, "json encode node latest list", e)
            })?;
            envr_platform::fs_atomic::write_atomic(&cache_file, s.as_bytes())?;
            Ok(())
        })();

        Ok(list)
    }

    fn resolve(&self, spec: &VersionSpec) -> EnvrResult<ResolvedVersion> {
        let releases = self.load_releases()?;
        let v = resolve_node_version(
            &releases,
            std::env::consts::OS,
            std::env::consts::ARCH,
            &spec.0,
        )?;
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
        let paths = NodePaths::new(self.runtime_root()?);
        Ok((vec![paths.version_dir(&version.0)], None))
    }
}
