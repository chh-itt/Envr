use envr_domain::runtime::{
    InstallRequest, MajorVersionRecord, RemoteFilter, ResolvedVersion, RuntimeKind, RuntimeProvider,
    RuntimeVersion, VersionRecord, VersionSpec, major_line_remote_install_blocked, runtime_descriptor,
    version_line_key_for_kind,
};
use envr_error::{EnvrError, EnvrResult};
use envr_platform::cache_recovery;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

fn attach_runtime_root<T>(
    runtime_root: &Option<PathBuf>,
    new: impl FnOnce() -> T,
    with_root: impl FnOnce(T, PathBuf) -> T,
) -> T {
    match runtime_root {
        None => new(),
        Some(r) => with_root(new(), r.clone()),
    }
}

fn default_provider_boxes(runtime_root: Option<PathBuf>) -> Vec<Box<dyn RuntimeProvider>> {
    vec![
        Box::new(attach_runtime_root(
            &runtime_root,
            envr_runtime_node::NodeRuntimeProvider::new,
            |p, r| p.with_runtime_root(r),
        )) as Box<dyn RuntimeProvider>,
        Box::new(attach_runtime_root(
            &runtime_root,
            envr_runtime_python::PythonRuntimeProvider::new,
            |p, r| p.with_runtime_root(r),
        )) as Box<dyn RuntimeProvider>,
        Box::new(attach_runtime_root(
            &runtime_root,
            envr_runtime_java::JavaRuntimeProvider::new,
            |p, r| p.with_runtime_root(r),
        )) as Box<dyn RuntimeProvider>,
        Box::new(attach_runtime_root(
            &runtime_root,
            envr_runtime_go::GoRuntimeProvider::new,
            |p, r| p.with_runtime_root(r),
        )) as Box<dyn RuntimeProvider>,
        Box::new(attach_runtime_root(
            &runtime_root,
            envr_runtime_rust::RustRuntimeProvider::new,
            |p, r| p.with_runtime_root(r),
        )) as Box<dyn RuntimeProvider>,
        Box::new(attach_runtime_root(
            &runtime_root,
            envr_runtime_ruby::RubyRuntimeProvider::new,
            |p, r| p.with_runtime_root(r),
        )) as Box<dyn RuntimeProvider>,
        Box::new(attach_runtime_root(
            &runtime_root,
            envr_runtime_elixir::ElixirRuntimeProvider::new,
            |p, r| p.with_runtime_root(r),
        )) as Box<dyn RuntimeProvider>,
        Box::new(attach_runtime_root(
            &runtime_root,
            envr_runtime_erlang::ErlangRuntimeProvider::new,
            |p, r| p.with_runtime_root(r),
        )) as Box<dyn RuntimeProvider>,
        Box::new(attach_runtime_root(
            &runtime_root,
            envr_runtime_php::PhpRuntimeProvider::new,
            |p, r| p.with_runtime_root(r),
        )) as Box<dyn RuntimeProvider>,
        Box::new(attach_runtime_root(
            &runtime_root,
            envr_runtime_deno::DenoRuntimeProvider::new,
            |p, r| p.with_runtime_root(r),
        )) as Box<dyn RuntimeProvider>,
        Box::new(attach_runtime_root(
            &runtime_root,
            envr_runtime_bun::BunRuntimeProvider::new,
            |p, r| p.with_runtime_root(r),
        )) as Box<dyn RuntimeProvider>,
        Box::new(attach_runtime_root(
            &runtime_root,
            envr_runtime_dotnet::DotnetRuntimeProvider::new,
            |p, r| p.with_runtime_root(r),
        )) as Box<dyn RuntimeProvider>,
        Box::new(attach_runtime_root(
            &runtime_root,
            envr_runtime_zig::ZigRuntimeProvider::new,
            |p, r| p.with_runtime_root(r),
        )) as Box<dyn RuntimeProvider>,
        Box::new(attach_runtime_root(
            &runtime_root,
            envr_runtime_julia::JuliaRuntimeProvider::new,
            |p, r| p.with_runtime_root(r),
        )) as Box<dyn RuntimeProvider>,
        Box::new(attach_runtime_root(
            &runtime_root,
            envr_runtime_lua::LuaRuntimeProvider::new,
            |p, r| p.with_runtime_root(r),
        )) as Box<dyn RuntimeProvider>,
        Box::new(attach_runtime_root(
            &runtime_root,
            envr_runtime_nim::NimRuntimeProvider::new,
            |p, r| p.with_runtime_root(r),
        )) as Box<dyn RuntimeProvider>,
        Box::new(attach_runtime_root(
            &runtime_root,
            envr_runtime_crystal::CrystalRuntimeProvider::new,
            |p, r| p.with_runtime_root(r),
        )) as Box<dyn RuntimeProvider>,
        Box::new(attach_runtime_root(
            &runtime_root,
            envr_runtime_rlang::RlangRuntimeProvider::new,
            |p, r| p.with_runtime_root(r),
        )) as Box<dyn RuntimeProvider>,
    ]
}

pub struct RuntimeService {
    providers: HashMap<RuntimeKind, Box<dyn RuntimeProvider>>,
    runtime_root_override: Option<PathBuf>,
}

impl RuntimeService {
    pub fn new(providers: Vec<Box<dyn RuntimeProvider>>) -> EnvrResult<Self> {
        let mut map = HashMap::new();
        for p in providers {
            if map.contains_key(&p.kind()) {
                return Err(EnvrError::Validation(format!(
                    "duplicate provider for {:?}",
                    p.kind()
                )));
            }
            map.insert(p.kind(), p);
        }
        Ok(Self {
            providers: map,
            runtime_root_override: None,
        })
    }

    #[cfg(test)]
    pub(crate) fn new_with_cache_root_for_tests(
        providers: Vec<Box<dyn RuntimeProvider>>,
        runtime_root: PathBuf,
    ) -> EnvrResult<Self> {
        let mut svc = Self::new(providers)?;
        svc.runtime_root_override = Some(runtime_root);
        Ok(svc)
    }

    pub fn with_defaults() -> EnvrResult<Self> {
        Self::new(default_provider_boxes(None))
    }

    /// Same as [`Self::with_defaults`], but all providers use this runtime root (e.g. from `ENVR_RUNTIME_ROOT`).
    pub fn with_runtime_root(root: PathBuf) -> EnvrResult<Self> {
        let root_override = root.clone();
        let mut svc = Self::new(default_provider_boxes(Some(root)))?;
        svc.runtime_root_override = Some(root_override);
        Ok(svc)
    }

    fn provider(&self, kind: RuntimeKind) -> EnvrResult<&dyn RuntimeProvider> {
        self.providers
            .get(&kind)
            .map(|b| b.as_ref())
            .ok_or_else(|| EnvrError::Validation(format!("provider not registered: {kind:?}")))
    }

    pub fn list_installed(&self, kind: RuntimeKind) -> EnvrResult<Vec<RuntimeVersion>> {
        self.provider(kind)?.list_installed()
    }

    /// Remote versions that [`RuntimeProvider::install`] is expected to satisfy (see
    /// [`RuntimeProvider::list_remote_installable`]).
    pub fn list_remote(
        &self,
        kind: RuntimeKind,
        filter: &RemoteFilter,
    ) -> EnvrResult<Vec<RuntimeVersion>> {
        self.provider(kind)?.list_remote_installable(filter)
    }

    pub fn list_remote_majors(&self, kind: RuntimeKind) -> EnvrResult<Vec<String>> {
        self.provider(kind)?.list_remote_majors()
    }

    pub fn list_remote_latest_per_major(
        &self,
        kind: RuntimeKind,
    ) -> EnvrResult<Vec<RuntimeVersion>> {
        self.provider(kind)?
            .list_remote_latest_installable_per_major()
    }

    pub fn try_load_remote_latest_per_major_from_disk(
        &self,
        kind: RuntimeKind,
    ) -> Vec<RuntimeVersion> {
        self.providers
            .get(&kind)
            .map(|p| p.try_load_remote_latest_installable_per_major_from_disk())
            .unwrap_or_default()
    }

    pub fn list_major_rows_cached(&self, kind: RuntimeKind) -> EnvrResult<Vec<MajorVersionRecord>> {
        let rows = if let Some(rows) = self.try_read_cached_major_rows(kind)? {
            rows
        } else {
            self.try_load_remote_latest_per_major_from_disk(kind)
                .into_iter()
                .filter_map(|v| {
                    let major = version_line_key_for_kind(kind, &v.0)?;
                    Some(MajorVersionRecord {
                        major_key: major,
                        latest_installable: Some(v),
                    })
                })
                .collect()
        };
        Ok(Self::filter_major_rows_remote_install_lines(kind, rows))
    }

    pub fn refresh_major_rows_remote(
        &self,
        kind: RuntimeKind,
    ) -> EnvrResult<Vec<MajorVersionRecord>> {
        let latest = self.list_remote_latest_per_major(kind)?;
        let rows = latest
            .into_iter()
            .filter_map(|v| {
                let major = version_line_key_for_kind(kind, &v.0)?;
                Some(MajorVersionRecord {
                    major_key: major,
                    latest_installable: Some(v),
                })
            })
            .collect::<Vec<_>>();
        let rows = Self::filter_major_rows_remote_install_lines(kind, rows);
        self.write_cached_major_rows(kind, &rows)?;
        Ok(rows)
    }

    fn filter_major_rows_remote_install_lines(
        kind: RuntimeKind,
        rows: Vec<MajorVersionRecord>,
    ) -> Vec<MajorVersionRecord> {
        rows.into_iter()
            .filter(|r| !major_line_remote_install_blocked(kind, &r.major_key))
            .collect()
    }

    pub fn list_children_cached(
        &self,
        kind: RuntimeKind,
        major_key: &str,
    ) -> EnvrResult<Vec<VersionRecord>> {
        if major_line_remote_install_blocked(kind, major_key) {
            return Ok(Vec::new());
        }
        let per_major = self.read_cached_children(kind, major_key)?;
        if !per_major.is_empty() {
            return Ok(per_major
                .into_iter()
                .map(|v| VersionRecord { version: v })
                .collect());
        }
        if let Some(full) = self.try_read_full_remote_installable_stale_ok(kind) {
            let filtered: Vec<RuntimeVersion> = full
                .into_iter()
                .filter(|v| version_line_key_for_kind(kind, &v.0).as_deref() == Some(major_key))
                .collect();
            if !filtered.is_empty() {
                return Ok(filtered
                    .into_iter()
                    .map(|v| VersionRecord { version: v })
                    .collect());
            }
        }
        Ok(Vec::new())
    }

    pub fn refresh_children_remote(
        &self,
        kind: RuntimeKind,
        major_key: &str,
    ) -> EnvrResult<Vec<VersionRecord>> {
        if major_line_remote_install_blocked(kind, major_key) {
            return Ok(Vec::new());
        }
        let all = self.load_full_remote_installable_cached(kind)?;
        let filtered = all
            .into_iter()
            .filter(|v| version_line_key_for_kind(kind, &v.0).as_deref() == Some(major_key))
            .collect::<Vec<_>>();
        self.write_cached_children(kind, major_key, &filtered)?;
        Ok(filtered
            .into_iter()
            .map(|v| VersionRecord { version: v })
            .collect())
    }

    /// Remove all unified list cache files for `kind` (major rows, full remote snapshot, per-major children).
    pub fn remove_unified_version_list_cache_dir(&self, kind: RuntimeKind) -> EnvrResult<()> {
        let dir = self.unified_list_cache_dir(kind)?;
        if dir.is_dir() {
            fs::remove_dir_all(&dir).map_err(EnvrError::from)?;
        }
        Ok(())
    }

    /// TTL for the unified **full installable** version list snapshot (drives child rows; keep short
    /// so upstream index changes propagate without stale installs for too long).
    fn unified_list_full_remote_ttl_secs() -> u64 {
        const DEFAULT: u64 = 5 * 60;
        std::env::var("ENVR_UNIFIED_LIST_FULL_REMOTE_TTL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT)
    }

    /// TTL for `major_rows.json` **mtime** when treating cache as “fresh” for the first read attempt;
    /// after expiry we still return **stale** data (second read) so UI never blanks on refresh failure.
    fn unified_major_disk_ttl_secs() -> u64 {
        const DEFAULT: u64 = 10 * 60;
        std::env::var("ENVR_UNIFIED_LIST_MAJOR_DISK_TTL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT)
    }

    /// TTL for per-major `children/<key>.json` “fresh” read; stale file still used for paint.
    fn unified_children_disk_ttl_secs() -> u64 {
        const DEFAULT: u64 = 5 * 60;
        std::env::var("ENVR_UNIFIED_LIST_CHILDREN_DISK_TTL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT)
    }

    fn read_cached_string_list_fresh_or_stale(path: &Path, fresh_ttl: Option<u64>) -> Option<Vec<String>> {
        if let Some(ttl) = fresh_ttl {
            if let Some(xs) = cache_recovery::read_json_string_list(path, Some(ttl), |l| !l.is_empty())
            {
                return Some(xs);
            }
        }
        cache_recovery::read_json_string_list(path, None, |l| !l.is_empty())
    }

    fn unified_full_remote_installable_cache_file(&self, kind: RuntimeKind) -> EnvrResult<PathBuf> {
        Ok(self
            .unified_list_cache_dir(kind)?
            .join("full_installable_versions.json"))
    }

    /// Full installable remote list (same source as [`Self::list_remote`] with no prefix), with disk cache
    /// so expanding multiple major lines does not re-fetch the upstream index for each expand.
    fn load_full_remote_installable_cached(&self, kind: RuntimeKind) -> EnvrResult<Vec<RuntimeVersion>> {
        let ttl = Self::unified_list_full_remote_ttl_secs();
        let path = self.unified_full_remote_installable_cache_file(kind)?;
        if let Some(list) = cache_recovery::read_json_string_list(&path, Some(ttl), |xs| !xs.is_empty())
        {
            return Ok(list.into_iter().map(RuntimeVersion).collect());
        }
        let all = self.list_remote(kind, &RemoteFilter { prefix: None })?;
        self.write_full_remote_installable(kind, &all)?;
        Ok(all)
    }

    fn try_read_full_remote_installable_stale_ok(
        &self,
        kind: RuntimeKind,
    ) -> Option<Vec<RuntimeVersion>> {
        let path = self.unified_full_remote_installable_cache_file(kind).ok()?;
        cache_recovery::read_json_string_list(&path, None, |xs| !xs.is_empty())
            .map(|xs| xs.into_iter().map(RuntimeVersion).collect())
    }

    /// Read unified full installable remote snapshot from disk (stale allowed, no network).
    pub fn try_load_full_remote_installable_from_disk(&self, kind: RuntimeKind) -> Vec<RuntimeVersion> {
        self.try_read_full_remote_installable_stale_ok(kind)
            .unwrap_or_default()
    }

    fn write_full_remote_installable(
        &self,
        kind: RuntimeKind,
        children: &[RuntimeVersion],
    ) -> EnvrResult<()> {
        let path = self.unified_full_remote_installable_cache_file(kind)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(EnvrError::from)?;
        }
        let data: Vec<String> = children.iter().map(|v| v.0.clone()).collect();
        let s = serde_json::to_string(&data).map_err(|e| EnvrError::Validation(e.to_string()))?;
        envr_platform::fs_atomic::write_atomic(&path, s.as_bytes())?;
        Ok(())
    }

    pub fn resolve(&self, kind: RuntimeKind, spec: &VersionSpec) -> EnvrResult<ResolvedVersion> {
        self.provider(kind)?.resolve(spec)
    }

    pub fn install(
        &self,
        kind: RuntimeKind,
        request: &InstallRequest,
    ) -> EnvrResult<RuntimeVersion> {
        self.provider(kind)?.install(request)
    }

    pub fn uninstall(&self, kind: RuntimeKind, version: &RuntimeVersion) -> EnvrResult<()> {
        self.provider(kind)?.uninstall(version)
    }

    pub fn uninstall_dry_run_targets(
        &self,
        kind: RuntimeKind,
        version: &RuntimeVersion,
    ) -> EnvrResult<(Vec<PathBuf>, Option<String>)> {
        self.provider(kind)?.uninstall_dry_run_targets(version)
    }

    pub fn current(&self, kind: RuntimeKind) -> EnvrResult<Option<RuntimeVersion>> {
        self.provider(kind)?.current()
    }

    /// `Some(true)` = global active PHP is TS, `Some(false)` = NTS/legacy; `None` = no global current.
    pub fn php_global_current_want_ts(&self) -> EnvrResult<Option<bool>> {
        let root = self.cache_runtime_root()?;
        let paths = envr_runtime_php::PhpPaths::new(root);
        envr_runtime_php::read_current_global_want_ts(&paths)
    }

    pub fn set_current(&self, kind: RuntimeKind, version: &RuntimeVersion) -> EnvrResult<()> {
        self.provider(kind)?.set_current(version)
    }

    fn cache_runtime_root(&self) -> EnvrResult<PathBuf> {
        if let Some(root) = self.runtime_root_override.as_ref() {
            return Ok(root.clone());
        }
        envr_config::settings::resolve_runtime_root()
    }

    fn unified_list_cache_dir(&self, kind: RuntimeKind) -> EnvrResult<PathBuf> {
        let root = self.cache_runtime_root()?;
        let key = runtime_descriptor(kind).key;
        Ok(root
            .join("cache")
            .join(key)
            .join("unified_version_list"))
    }

    fn unified_major_cache_file(&self, kind: RuntimeKind) -> EnvrResult<PathBuf> {
        Ok(self.unified_list_cache_dir(kind)?.join("major_rows.json"))
    }

    fn unified_children_cache_file(&self, kind: RuntimeKind, major_key: &str) -> EnvrResult<PathBuf> {
        Ok(self
            .unified_list_cache_dir(kind)?
            .join("children")
            .join(format!("{major_key}.json")))
    }

    fn try_read_cached_major_rows(
        &self,
        kind: RuntimeKind,
    ) -> EnvrResult<Option<Vec<MajorVersionRecord>>> {
        let path = self.unified_major_cache_file(kind)?;
        if !path.is_file() {
            return Ok(None);
        }
        let Some(raw) = Self::read_cached_string_list_fresh_or_stale(
            &path,
            Some(Self::unified_major_disk_ttl_secs()),
        ) else {
            return Ok(None);
        };
        let rows = raw
            .into_iter()
            .filter_map(|v| {
                let major = version_line_key_for_kind(kind, &v)?;
                Some(MajorVersionRecord {
                    major_key: major,
                    latest_installable: Some(RuntimeVersion(v)),
                })
            })
            .collect();
        Ok(Some(rows))
    }

    fn write_cached_major_rows(
        &self,
        kind: RuntimeKind,
        rows: &[MajorVersionRecord],
    ) -> EnvrResult<()> {
        let path = self.unified_major_cache_file(kind)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(EnvrError::from)?;
        }
        let data: Vec<String> = rows
            .iter()
            .filter_map(|r| r.latest_installable.as_ref().map(|v| v.0.clone()))
            .collect();
        let s = serde_json::to_string(&data).map_err(|e| EnvrError::Validation(e.to_string()))?;
        envr_platform::fs_atomic::write_atomic(&path, s.as_bytes())?;
        Ok(())
    }

    fn read_cached_children(&self, kind: RuntimeKind, major_key: &str) -> EnvrResult<Vec<RuntimeVersion>> {
        let path = self.unified_children_cache_file(kind, major_key)?;
        if !path.is_file() {
            return Ok(Vec::new());
        }
        let Some(raw) = Self::read_cached_string_list_fresh_or_stale(
            &path,
            Some(Self::unified_children_disk_ttl_secs()),
        ) else {
            return Ok(Vec::new());
        };
        Ok(raw.into_iter().map(RuntimeVersion).collect())
    }

    fn write_cached_children(
        &self,
        kind: RuntimeKind,
        major_key: &str,
        children: &[RuntimeVersion],
    ) -> EnvrResult<()> {
        let path = self.unified_children_cache_file(kind, major_key)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(EnvrError::from)?;
        }
        let data: Vec<String> = children.iter().map(|v| v.0.clone()).collect();
        let s = serde_json::to_string(&data).map_err(|e| EnvrError::Validation(e.to_string()))?;
        envr_platform::fs_atomic::write_atomic(&path, s.as_bytes())?;
        Ok(())
    }
}
