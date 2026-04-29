use envr_domain::runtime::{
    InstallRequest, MajorVersionRecord, RemoteFilter, ResolvedVersion, RuntimeIndex,
    RuntimeInstaller, RuntimeKind, RuntimeProvider, RuntimeVersion, VersionRecord, VersionSpec,
    major_line_remote_install_blocked, runtime_descriptor, version_line_key_for_kind,
};
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use envr_platform::cache_recovery;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

struct ProviderIndexRef<'a> {
    inner: &'a dyn RuntimeProvider,
}

impl RuntimeIndex for ProviderIndexRef<'_> {
    fn kind(&self) -> RuntimeKind {
        self.inner.kind()
    }

    fn list_installed(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        self.inner.list_installed()
    }

    fn current(&self) -> EnvrResult<Option<RuntimeVersion>> {
        self.inner.current()
    }

    fn list_remote(&self, filter: &RemoteFilter) -> EnvrResult<Vec<RuntimeVersion>> {
        self.inner.list_remote(filter)
    }

    fn list_remote_installable(&self, filter: &RemoteFilter) -> EnvrResult<Vec<RuntimeVersion>> {
        self.inner.list_remote_installable(filter)
    }

    fn list_remote_majors(&self) -> EnvrResult<Vec<String>> {
        self.inner.list_remote_majors()
    }

    fn list_remote_latest_per_major(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        self.inner.list_remote_latest_per_major()
    }

    fn list_remote_latest_installable_per_major(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        self.inner.list_remote_latest_installable_per_major()
    }

    fn try_load_remote_latest_installable_per_major_from_disk(&self) -> Vec<RuntimeVersion> {
        self.inner
            .try_load_remote_latest_installable_per_major_from_disk()
    }

    fn try_load_remote_latest_per_major_from_disk(&self) -> Vec<RuntimeVersion> {
        self.inner.try_load_remote_latest_per_major_from_disk()
    }

    fn resolve(&self, spec: &VersionSpec) -> EnvrResult<ResolvedVersion> {
        self.inner.resolve(spec)
    }
}

struct ProviderInstallerRef<'a> {
    inner: &'a dyn RuntimeProvider,
}

impl RuntimeInstaller for ProviderInstallerRef<'_> {
    fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        self.inner.set_current(version)
    }

    fn install(&self, request: &InstallRequest) -> EnvrResult<RuntimeVersion> {
        self.inner.install(request)
    }

    fn uninstall(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        self.inner.uninstall(version)
    }

    fn uninstall_dry_run_targets(
        &self,
        version: &RuntimeVersion,
    ) -> EnvrResult<(Vec<PathBuf>, Option<String>)> {
        self.inner.uninstall_dry_run_targets(version)
    }
}

pub struct RuntimeService {
    providers: HashMap<RuntimeKind, Box<dyn RuntimeProvider>>,
    runtime_root_override: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct UnifiedCacheWarmupRuntimeReport {
    pub kind: RuntimeKind,
    pub major_rows: usize,
    pub children_rows: usize,
    pub ok: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct UnifiedCacheWarmupReport {
    pub runtimes: Vec<UnifiedCacheWarmupRuntimeReport>,
}

impl UnifiedCacheWarmupReport {
    pub fn success_count(&self) -> usize {
        self.runtimes.iter().filter(|r| r.ok).count()
    }

    pub fn failure_count(&self) -> usize {
        self.runtimes.iter().filter(|r| !r.ok).count()
    }
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
        Self::new(envr_runtime_registry::default_provider_boxes(None))
    }

    /// Same as [`Self::with_defaults`], but all providers use this runtime root (e.g. from `ENVR_RUNTIME_ROOT`).
    pub fn with_runtime_root(root: PathBuf) -> EnvrResult<Self> {
        let root_override = root.clone();
        let mut svc = Self::new(envr_runtime_registry::default_provider_boxes(Some(root)))?;
        svc.runtime_root_override = Some(root_override);
        Ok(svc)
    }

    fn provider(&self, kind: RuntimeKind) -> EnvrResult<&dyn RuntimeProvider> {
        self.providers
            .get(&kind)
            .map(|b| b.as_ref())
            .ok_or_else(|| EnvrError::Validation(format!("provider not registered: {kind:?}")))
    }

    fn index_provider(&self, kind: RuntimeKind) -> EnvrResult<ProviderIndexRef<'_>> {
        self.provider(kind).map(|p| ProviderIndexRef { inner: p })
    }

    fn installer_provider(&self, kind: RuntimeKind) -> EnvrResult<ProviderInstallerRef<'_>> {
        self.provider(kind)
            .map(|p| ProviderInstallerRef { inner: p })
    }

    pub fn index_port(&self, kind: RuntimeKind) -> EnvrResult<Box<dyn RuntimeIndex + '_>> {
        self.index_provider(kind)
            .map(|p| Box::new(p) as Box<dyn RuntimeIndex>)
    }

    pub fn installer_port(&self, kind: RuntimeKind) -> EnvrResult<Box<dyn RuntimeInstaller + '_>> {
        self.installer_provider(kind)
            .map(|p| Box::new(p) as Box<dyn RuntimeInstaller>)
    }

    /// Refresh unified remote cache snapshots for all runtimes that expose remote lists.
    ///
    /// This eagerly refreshes:
    /// - major rows (`major_rows.json`)
    /// - per-major children (`children/<major>.json`)
    ///
    /// The method is best-effort: failures are captured per runtime in the returned report.
    pub fn refresh_all_unified_cache(&self) -> UnifiedCacheWarmupReport {
        let mut report = UnifiedCacheWarmupReport::default();
        for kind in envr_domain::runtime::runtime_kinds_all() {
            if !runtime_descriptor(kind).supports_remote_latest {
                continue;
            }
            report
                .runtimes
                .push(self.refresh_unified_cache_for_kind_for_report(kind));
        }
        report
    }

    /// Refresh unified remote cache snapshots only for runtimes with stale full snapshot files.
    pub fn refresh_all_unified_cache_if_stale(&self) -> UnifiedCacheWarmupReport {
        let mut report = UnifiedCacheWarmupReport::default();
        for kind in envr_domain::runtime::runtime_kinds_all() {
            if !runtime_descriptor(kind).supports_remote_latest {
                continue;
            }
            if !self.unified_full_snapshot_is_stale(kind) {
                continue;
            }
            report
                .runtimes
                .push(self.refresh_unified_cache_for_kind_for_report(kind));
        }
        report
    }

    pub fn refresh_unified_cache_for_kind_for_report(
        &self,
        kind: RuntimeKind,
    ) -> UnifiedCacheWarmupRuntimeReport {
        let mut row = UnifiedCacheWarmupRuntimeReport {
            kind,
            major_rows: 0,
            children_rows: 0,
            ok: true,
            error: None,
        };
        match self.refresh_major_rows_remote(kind) {
            Ok(major_rows) => {
                row.major_rows = major_rows.len();
                for major in major_rows.iter().map(|r| r.major_key.as_str()) {
                    match self.refresh_children_remote(kind, major) {
                        Ok(children) => {
                            row.children_rows += children.len();
                        }
                        Err(e) => {
                            row.ok = false;
                            row.error = Some(e.to_string());
                            break;
                        }
                    }
                }
            }
            Err(e) => {
                row.ok = false;
                row.error = Some(e.to_string());
            }
        }
        row
    }

    fn unified_full_snapshot_is_stale(&self, kind: RuntimeKind) -> bool {
        let path = match self.unified_full_remote_installable_cache_file(kind) {
            Ok(p) => p,
            Err(_) => return true,
        };
        let ttl = Duration::from_secs(Self::unified_list_full_remote_ttl_secs());
        let modified = match fs::metadata(&path).and_then(|m| m.modified()) {
            Ok(m) => m,
            Err(_) => return true,
        };
        match SystemTime::now().duration_since(modified) {
            Ok(age) => age > ttl,
            Err(_) => true,
        }
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
        if let Ok(p) = self.provider(kind)
            && let Some(ad) = p.version_list_adapter()
        {
            let rows = ad.load_major_rows_cached()?;
            return Ok(Self::filter_major_rows_remote_install_lines(kind, rows));
        }
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
        if let Ok(p) = self.provider(kind)
            && let Some(ad) = p.version_list_adapter()
        {
            let rows = ad.refresh_major_rows_remote()?;
            return Ok(Self::filter_major_rows_remote_install_lines(kind, rows));
        }
        let latest = self
            .index_provider(kind)?
            .list_remote_latest_installable_per_major()?;
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
        if let Ok(p) = self.provider(kind)
            && let Some(ad) = p.version_list_adapter()
        {
            let rows = ad.load_children_cached(major_key)?;
            if !rows.is_empty() {
                return Ok(rows);
            }
            // Adapter-specific per-major cache may be missing even when the full unified snapshot
            // is already present locally (e.g. after remote -u). Fall through to generic full-cache
            // projection so expand can render immediately without waiting for remote refresh.
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
        if let Ok(p) = self.provider(kind)
            && let Some(ad) = p.version_list_adapter()
        {
            return ad.refresh_children_remote(major_key);
        }
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

    fn read_cached_string_list_fresh_or_stale(
        path: &Path,
        fresh_ttl: Option<u64>,
    ) -> Option<Vec<String>> {
        if let Some(ttl) = fresh_ttl {
            if let Some(xs) =
                cache_recovery::read_json_string_list(path, Some(ttl), |l| !l.is_empty())
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

    /// Full installable remote list (same source as `RuntimeIndex::list_remote_installable` with no prefix), with disk cache
    /// so expanding multiple major lines does not re-fetch the upstream index for each expand.
    fn load_full_remote_installable_cached(
        &self,
        kind: RuntimeKind,
    ) -> EnvrResult<Vec<RuntimeVersion>> {
        let ttl = Self::unified_list_full_remote_ttl_secs();
        let path = self.unified_full_remote_installable_cache_file(kind)?;
        if let Some(list) =
            cache_recovery::read_json_string_list(&path, Some(ttl), |xs| !xs.is_empty())
        {
            return Ok(list.into_iter().map(RuntimeVersion).collect());
        }
        let all = self
            .index_provider(kind)?
            .list_remote_installable(&RemoteFilter::default())?;
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
    pub fn try_load_full_remote_installable_from_disk(
        &self,
        kind: RuntimeKind,
    ) -> Vec<RuntimeVersion> {
        self.try_read_full_remote_installable_stale_ok(kind)
            .unwrap_or_default()
    }

    /// Persist the full installable remote list for [`Self::try_load_full_remote_installable_from_disk`]
    /// (used by `envr remote` without `-u` so the CLI can show the same rows as a fresh `list_remote`).
    pub fn persist_full_remote_installable_snapshot(
        &self,
        kind: RuntimeKind,
        versions: &[RuntimeVersion],
    ) -> EnvrResult<()> {
        if versions.is_empty() {
            return Ok(());
        }
        self.write_full_remote_installable(kind, versions)
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
        let s = serde_json::to_string(&data)
            .map_err(|e| EnvrError::with_source(ErrorCode::Validation, "json encode", e))?;
        envr_platform::fs_atomic::write_atomic(&path, s.as_bytes())?;
        Ok(())
    }

    /// `Some(true)` = global active PHP is TS, `Some(false)` = NTS/legacy; `None` = no global current.
    pub fn php_global_current_want_ts(&self) -> EnvrResult<Option<bool>> {
        #[cfg(not(windows))]
        {
            return Ok(None);
        }
        #[cfg(windows)]
        {
            let root = self.cache_runtime_root()?;
            let php_home = root.join("runtimes").join("php");
            for link in [
                php_home.join("current"),
                php_home.join("current-ts"),
                php_home.join("current-nts"),
            ] {
                if let Some(target) = Self::resolve_current_link_target(&link)? {
                    let Some(name) = target.file_name().and_then(|s| s.to_str()) else {
                        return Ok(None);
                    };
                    if name.is_empty() {
                        return Ok(None);
                    }
                    return Ok(Some(envr_config::php_layout::dir_flavor_is_ts(name)));
                }
            }
        }
        Ok(None)
    }

    #[cfg(windows)]
    fn resolve_current_link_target(link: &Path) -> EnvrResult<Option<PathBuf>> {
        if !link.exists() {
            return Ok(None);
        }
        if link.is_file() {
            let s = fs::read_to_string(link).map_err(EnvrError::from)?;
            let t = s.trim();
            let target = PathBuf::from(t);
            return Ok(Some(fs::canonicalize(&target).map_err(EnvrError::from)?));
        }
        if let Ok(t) = fs::read_link(link) {
            let resolved = if t.is_relative() {
                link.parent().map(|p| p.join(&t)).unwrap_or(t)
            } else {
                t
            };
            return Ok(Some(fs::canonicalize(&resolved).map_err(EnvrError::from)?));
        }
        if link.is_dir() {
            return Ok(Some(fs::canonicalize(link).map_err(EnvrError::from)?));
        }
        Ok(None)
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
        Ok(root.join("cache").join(key).join("unified_version_list"))
    }

    fn unified_major_cache_file(&self, kind: RuntimeKind) -> EnvrResult<PathBuf> {
        Ok(self.unified_list_cache_dir(kind)?.join("major_rows.json"))
    }

    fn unified_children_cache_file(
        &self,
        kind: RuntimeKind,
        major_key: &str,
    ) -> EnvrResult<PathBuf> {
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
        let s = serde_json::to_string(&data)
            .map_err(|e| EnvrError::with_source(ErrorCode::Validation, "json encode", e))?;
        envr_platform::fs_atomic::write_atomic(&path, s.as_bytes())?;
        Ok(())
    }

    fn read_cached_children(
        &self,
        kind: RuntimeKind,
        major_key: &str,
    ) -> EnvrResult<Vec<RuntimeVersion>> {
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
        let s = serde_json::to_string(&data)
            .map_err(|e| EnvrError::with_source(ErrorCode::Validation, "json encode", e))?;
        envr_platform::fs_atomic::write_atomic(&path, s.as_bytes())?;
        Ok(())
    }
}
