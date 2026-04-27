use envr_domain::runtime::{
    MajorVersionRecord, RuntimeKind, RuntimeVersion, VersionRecord, VersionSpec,
    major_line_remote_install_blocked, numeric_version_segments, version_line_key_for_kind,
};
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheMode {
    Offline,
    StaleOk,
    ForceRefresh,
}

#[derive(Debug, Clone)]
pub struct RemoteSourceCache {
    pub dir: PathBuf,
    pub key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SourceMeta {
    url: String,
    fetched_at_unix_secs: u64,
}

impl RemoteSourceCache {
    pub fn new(dir: PathBuf, key: impl Into<String>) -> Self {
        Self {
            dir,
            key: key.into(),
        }
    }

    fn body_path(&self) -> PathBuf {
        self.dir.join("source").join(format!("{}.body", self.key))
    }

    fn meta_path(&self) -> PathBuf {
        self.dir
            .join("source")
            .join(format!("{}.meta.json", self.key))
    }

    fn read_meta(&self) -> Option<SourceMeta> {
        let p = self.meta_path();
        let s = fs::read_to_string(p).ok()?;
        serde_json::from_str(&s).ok()
    }

    fn write_meta(&self, meta: &SourceMeta) -> EnvrResult<()> {
        let p = self.meta_path();
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).map_err(EnvrError::from)?;
        }
        let s = serde_json::to_string(meta)
            .map_err(|e| EnvrError::with_source(ErrorCode::Validation, "json encode meta", e))?;
        crate::fs_atomic::write_atomic(&p, s.as_bytes())?;
        Ok(())
    }

    fn file_is_within_ttl(path: &Path, ttl_secs: u64) -> bool {
        if ttl_secs == 0 {
            return false;
        }
        let Ok(meta) = fs::metadata(path) else {
            return false;
        };
        let Ok(mtime) = meta.modified() else {
            return false;
        };
        let Ok(age) = SystemTime::now().duration_since(mtime) else {
            return false;
        };
        age.as_secs() <= ttl_secs
    }

    /// Returns cached body when fresh; otherwise uses `fetcher` to refresh the disk copy.
    pub fn get_body_cached(
        &self,
        url: &str,
        ttl: Duration,
        mode: CacheMode,
        fetcher: impl FnOnce(&str) -> EnvrResult<String>,
    ) -> EnvrResult<String> {
        let body_path = self.body_path();
        let ttl_secs = ttl.as_secs();

        let meta_ok = self
            .read_meta()
            .is_some_and(|m| m.url == url.trim().to_string());

        if mode != CacheMode::ForceRefresh
            && meta_ok
            && Self::file_is_within_ttl(&body_path, ttl_secs)
            && let Ok(body) = fs::read_to_string(&body_path)
            && !body.trim().is_empty()
        {
            return Ok(body);
        }

        // Stale OK: if network fails, allow returning cached body (even if expired).
        // Keep this behavior for `ForceRefresh` as well so SWR refresh doesn't blank.
        let stale_ok = matches!(mode, CacheMode::StaleOk | CacheMode::ForceRefresh);

        if matches!(mode, CacheMode::Offline) {
            if let Ok(body) = fs::read_to_string(&body_path)
                && !body.trim().is_empty()
            {
                return Ok(body);
            }
            return Err(EnvrError::Download(format!(
                "offline mode: missing cached body for {url} at {}",
                body_path.display()
            )));
        }

        let fetched = fetcher(url);
        match fetched {
            Ok(body) => {
                let _ = (|| -> EnvrResult<()> {
                    if let Some(parent) = body_path.parent() {
                        fs::create_dir_all(parent).map_err(EnvrError::from)?;
                    }
                    crate::fs_atomic::write_atomic(&body_path, body.as_bytes())?;
                    let now = SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    self.write_meta(&SourceMeta {
                        url: url.trim().to_string(),
                        fetched_at_unix_secs: now,
                    })?;
                    Ok(())
                })();
                Ok(body)
            }
            Err(e) if stale_ok => {
                if let Ok(body) = fs::read_to_string(&body_path)
                    && !body.trim().is_empty()
                {
                    Ok(body)
                } else {
                    Err(e)
                }
            }
            Err(e) => Err(e),
        }
    }
}

pub trait RemoteIndexParser: Send + Sync + 'static {
    type Item: Clone + Serialize + for<'de> Deserialize<'de>;

    fn parse(&self, body: &str) -> EnvrResult<Vec<Self::Item>>;

    fn version_label<'a>(&self, item: &'a Self::Item) -> &'a str;

    /// Return true if this item is installable on the host (OS/ARCH, flavor, etc).
    fn is_installable_on_host(&self, _item: &Self::Item) -> bool {
        true
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ParsedMeta {
    source_mtime_unix_secs: Option<u64>,
}

/// Unified cached remote index helper: SourceCache + ParsedCache + DerivedCache.
#[derive(Debug, Clone)]
pub struct CachedRemoteIndex<P: RemoteIndexParser> {
    pub kind: RuntimeKind,
    pub unified_dir: PathBuf,
    pub source: RemoteSourceCache,
    pub parser: P,
}

impl<P: RemoteIndexParser> CachedRemoteIndex<P> {
    pub fn new(
        kind: RuntimeKind,
        unified_dir: PathBuf,
        source: RemoteSourceCache,
        parser: P,
    ) -> Self {
        Self {
            kind,
            unified_dir,
            source,
            parser,
        }
    }

    fn parsed_dir(&self) -> PathBuf {
        self.unified_dir.join("parsed")
    }

    fn parsed_cache_path(&self) -> PathBuf {
        self.parsed_dir().join(format!("{}.json", self.source.key))
    }

    fn parsed_meta_path(&self) -> PathBuf {
        self.parsed_dir()
            .join(format!("{}.meta.json", self.source.key))
    }

    fn major_rows_path(&self) -> PathBuf {
        self.unified_dir.join("major_rows.json")
    }

    fn children_path(&self, major_key: &str) -> PathBuf {
        self.unified_dir
            .join("children")
            .join(format!("{major_key}.json"))
    }

    fn full_installable_path(&self) -> PathBuf {
        self.unified_dir.join("full_installable_versions.json")
    }

    fn unified_list_full_remote_ttl_secs() -> u64 {
        const DEFAULT: u64 = 5 * 60;
        std::env::var("ENVR_UNIFIED_LIST_FULL_REMOTE_TTL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT)
    }

    fn unified_major_disk_ttl_secs() -> u64 {
        const DEFAULT: u64 = 10 * 60;
        std::env::var("ENVR_UNIFIED_LIST_MAJOR_DISK_TTL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT)
    }

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
                crate::cache_recovery::read_json_string_list(path, Some(ttl), |l| !l.is_empty())
            {
                return Some(xs);
            }
        }
        crate::cache_recovery::read_json_string_list(path, None, |l| !l.is_empty())
    }

    fn parsed_meta_source_mtime_secs(source_body_path: &Path) -> Option<u64> {
        let meta = fs::metadata(source_body_path).ok()?;
        if !meta.is_file() {
            return None;
        }
        meta.modified()
            .ok()?
            .duration_since(SystemTime::UNIX_EPOCH)
            .ok()
            .map(|d| d.as_secs())
    }

    fn read_parsed_meta(&self) -> Option<ParsedMeta> {
        let s = fs::read_to_string(self.parsed_meta_path()).ok()?;
        serde_json::from_str(&s).ok()
    }

    fn write_parsed_meta(&self, meta: &ParsedMeta) -> EnvrResult<()> {
        let p = self.parsed_meta_path();
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).map_err(EnvrError::from)?;
        }
        let s = serde_json::to_string(meta).map_err(|e| {
            EnvrError::with_source(ErrorCode::Validation, "json encode parsed meta", e)
        })?;
        crate::fs_atomic::write_atomic(&p, s.as_bytes())?;
        Ok(())
    }

    fn load_parsed_cached(&self, source_mtime_unix_secs: Option<u64>) -> Option<Vec<P::Item>> {
        let meta = self.read_parsed_meta()?;
        if meta.source_mtime_unix_secs != source_mtime_unix_secs {
            return None;
        }
        let s = fs::read_to_string(self.parsed_cache_path()).ok()?;
        serde_json::from_str::<Vec<P::Item>>(&s).ok()
    }

    fn write_parsed(
        &self,
        items: &[P::Item],
        source_mtime_unix_secs: Option<u64>,
    ) -> EnvrResult<()> {
        let cache_path = self.parsed_cache_path();
        if let Some(parent) = cache_path.parent() {
            fs::create_dir_all(parent).map_err(EnvrError::from)?;
        }
        let s = serde_json::to_string(items)
            .map_err(|e| EnvrError::with_source(ErrorCode::Validation, "json encode parsed", e))?;
        crate::fs_atomic::write_atomic(&cache_path, s.as_bytes())?;
        self.write_parsed_meta(&ParsedMeta {
            source_mtime_unix_secs,
        })?;
        Ok(())
    }

    pub fn load_items(
        &self,
        url: &str,
        source_ttl: Duration,
        mode: CacheMode,
        fetcher: impl FnOnce(&str) -> EnvrResult<String>,
    ) -> EnvrResult<Vec<P::Item>> {
        let body = self
            .source
            .get_body_cached(url, source_ttl, mode, fetcher)?;
        let source_mtime = Self::parsed_meta_source_mtime_secs(&self.source.body_path());
        if let Some(items) = self.load_parsed_cached(source_mtime) {
            return Ok(items);
        }
        let items = self.parser.parse(&body)?;
        let _ = self.write_parsed(&items, source_mtime);
        Ok(items)
    }

    fn sort_versions_desc(labels: &mut [String]) {
        labels.sort_by(|a, b| {
            let ka = numeric_version_segments(a);
            let kb = numeric_version_segments(b);
            match (ka, kb) {
                (Some(aa), Some(bb)) => bb.cmp(&aa),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => b.cmp(a),
            }
        });
    }

    fn build_full_installable_labels(&self, items: &[P::Item]) -> Vec<String> {
        let mut out: Vec<String> = items
            .iter()
            .filter(|it| self.parser.is_installable_on_host(it))
            .map(|it| self.parser.version_label(it).to_string())
            .collect();
        out.retain(|s| !s.trim().is_empty());
        Self::sort_versions_desc(&mut out);
        out.dedup();
        out
    }

    pub fn installable_labels(&self, items: &[P::Item]) -> Vec<String> {
        self.build_full_installable_labels(items)
    }

    /// Latest installable patch per major line, optionally restricted to items matching `include`.
    pub fn latest_installable_per_major_labels(
        &self,
        items: &[P::Item],
        include: impl Fn(&P::Item) -> bool,
    ) -> Vec<String> {
        let mut labels: Vec<String> = items
            .iter()
            .filter(|it| self.parser.is_installable_on_host(it) && include(it))
            .map(|it| self.parser.version_label(it).to_string())
            .collect();
        labels.retain(|s| !s.trim().is_empty());
        Self::sort_versions_desc(&mut labels);
        labels.dedup();

        let mut best: HashMap<String, String> = HashMap::new();
        for v in labels {
            let Some(major) = version_line_key_for_kind(self.kind, &v) else {
                continue;
            };
            if major_line_remote_install_blocked(self.kind, &major) {
                continue;
            }
            if !best.contains_key(&major) {
                best.insert(major, v);
            }
        }
        let mut majors: Vec<String> = best.keys().cloned().collect();
        majors.sort_by(|a, b| {
            let pa = numeric_version_segments(a);
            let pb = numeric_version_segments(b);
            match (pa, pb) {
                (Some(aa), Some(bb)) => bb.cmp(&aa),
                _ => b.cmp(a),
            }
        });
        majors.into_iter().filter_map(|m| best.remove(&m)).collect()
    }

    fn write_string_list(&self, path: &Path, strings: &[String]) -> EnvrResult<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(EnvrError::from)?;
        }
        let s = serde_json::to_string(strings)
            .map_err(|e| EnvrError::with_source(ErrorCode::Validation, "json encode list", e))?;
        crate::fs_atomic::write_atomic(path, s.as_bytes())?;
        Ok(())
    }

    fn load_full_installable_cached(
        &self,
        items: &[P::Item],
        mode: CacheMode,
    ) -> EnvrResult<Vec<String>> {
        let ttl = Self::unified_list_full_remote_ttl_secs();
        let path = self.full_installable_path();
        if mode != CacheMode::ForceRefresh {
            if let Some(list) =
                crate::cache_recovery::read_json_string_list(&path, Some(ttl), |xs| !xs.is_empty())
            {
                return Ok(list);
            }
        }
        let list = self.build_full_installable_labels(items);
        if !list.is_empty() {
            let _ = self.write_string_list(&path, &list);
        }
        Ok(list)
    }

    pub fn load_major_rows_cached(&self) -> EnvrResult<Vec<MajorVersionRecord>> {
        let path = self.major_rows_path();
        let Some(raw) = Self::read_cached_string_list_fresh_or_stale(
            &path,
            Some(Self::unified_major_disk_ttl_secs()),
        ) else {
            return Ok(Vec::new());
        };
        let rows = raw
            .into_iter()
            .filter_map(|v| {
                let major = version_line_key_for_kind(self.kind, &v)?;
                Some(MajorVersionRecord {
                    major_key: major,
                    latest_installable: Some(RuntimeVersion(v)),
                })
            })
            .collect::<Vec<_>>();
        Ok(rows
            .into_iter()
            .filter(|r| !major_line_remote_install_blocked(self.kind, &r.major_key))
            .collect())
    }

    pub fn refresh_major_rows_remote(
        &self,
        url: &str,
        source_ttl: Duration,
        mode: CacheMode,
        fetcher: impl FnOnce(&str) -> EnvrResult<String>,
    ) -> EnvrResult<Vec<MajorVersionRecord>> {
        let items = self.load_items(url, source_ttl, mode, fetcher)?;
        let full = self.load_full_installable_cached(&items, mode)?;
        let mut best: HashMap<String, String> = HashMap::new();
        for v in full {
            let Some(major) = version_line_key_for_kind(self.kind, &v) else {
                continue;
            };
            if major_line_remote_install_blocked(self.kind, &major) {
                continue;
            }
            match best.get(&major) {
                None => {
                    best.insert(major, v);
                }
                Some(prev) => {
                    // full list is already sorted desc, so first occurrence wins.
                    let _ = prev;
                }
            }
        }
        let mut majors: Vec<String> = best.keys().cloned().collect();
        majors.sort_by(|a, b| {
            let pa = numeric_version_segments(a);
            let pb = numeric_version_segments(b);
            match (pa, pb) {
                (Some(aa), Some(bb)) => bb.cmp(&aa),
                _ => b.cmp(a),
            }
        });
        let rows: Vec<MajorVersionRecord> = majors
            .into_iter()
            .map(|m| MajorVersionRecord {
                major_key: m.clone(),
                latest_installable: best.remove(&m).map(RuntimeVersion),
            })
            .collect();
        let data: Vec<String> = rows
            .iter()
            .filter_map(|r| r.latest_installable.as_ref().map(|v| v.0.clone()))
            .collect();
        let _ = self.write_string_list(&self.major_rows_path(), &data);
        Ok(rows)
    }

    pub fn load_children_cached(&self, major_key: &str) -> EnvrResult<Vec<VersionRecord>> {
        if major_line_remote_install_blocked(self.kind, major_key) {
            return Ok(Vec::new());
        }
        let path = self.children_path(major_key);
        let Some(raw) = Self::read_cached_string_list_fresh_or_stale(
            &path,
            Some(Self::unified_children_disk_ttl_secs()),
        ) else {
            return Ok(Vec::new());
        };
        Ok(raw
            .into_iter()
            .map(|v| VersionRecord {
                version: RuntimeVersion(v),
            })
            .collect())
    }

    pub fn refresh_children_remote(
        &self,
        url: &str,
        source_ttl: Duration,
        mode: CacheMode,
        major_key: &str,
        fetcher: impl FnOnce(&str) -> EnvrResult<String>,
    ) -> EnvrResult<Vec<VersionRecord>> {
        if major_line_remote_install_blocked(self.kind, major_key) {
            return Ok(Vec::new());
        }
        let items = self.load_items(url, source_ttl, mode, fetcher)?;
        let full = self.load_full_installable_cached(&items, mode)?;
        let mut filtered: Vec<String> = full
            .into_iter()
            .filter(|v| version_line_key_for_kind(self.kind, v).as_deref() == Some(major_key))
            .collect();
        filtered.retain(|s| !s.trim().is_empty());
        Self::sort_versions_desc(&mut filtered);
        filtered.dedup();
        let _ = self.write_string_list(&self.children_path(major_key), &filtered);
        Ok(filtered
            .into_iter()
            .map(|v| VersionRecord {
                version: RuntimeVersion(v),
            })
            .collect())
    }

    pub fn resolve_version_from_cached_items(
        &self,
        items: &[P::Item],
        spec: &VersionSpec,
    ) -> EnvrResult<RuntimeVersion> {
        let wanted = spec.0.trim();
        if wanted.is_empty() {
            return Err(EnvrError::Validation("empty version spec".into()));
        }
        let labels = self.build_full_installable_labels(items);
        if labels.is_empty() {
            return Err(EnvrError::Validation("no remote versions in index".into()));
        }
        // Exact match
        if let Some(v) = labels.iter().find(|v| {
            v.eq_ignore_ascii_case(wanted) || v.eq_ignore_ascii_case(wanted.trim_start_matches('v'))
        }) {
            return Ok(RuntimeVersion(v.clone()));
        }
        // Prefix match (major / major.minor)
        let p = wanted.trim_start_matches('v');
        if p.chars().all(|c| c.is_ascii_digit() || c == '.')
            && let Some(v) = labels.iter().find(|v| v.starts_with(p))
        {
            return Ok(RuntimeVersion(v.clone()));
        }
        Err(EnvrError::Validation(format!(
            "no remote version matches spec {wanted:?}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct Item {
        v: String,
    }

    #[derive(Debug, Clone)]
    struct Parser;

    impl RemoteIndexParser for Parser {
        type Item = Item;

        fn parse(&self, body: &str) -> EnvrResult<Vec<Self::Item>> {
            let mut out = Vec::new();
            for line in body.lines() {
                let t = line.trim();
                if t.is_empty() {
                    continue;
                }
                out.push(Item { v: t.to_string() });
            }
            Ok(out)
        }

        fn version_label<'a>(&self, item: &'a Self::Item) -> &'a str {
            item.v.as_str()
        }
    }

    #[test]
    fn source_cache_offline_requires_existing_file() {
        let td = tempdir().expect("tmp");
        let unified = td
            .path()
            .join("cache")
            .join("node")
            .join("unified_version_list");
        let src = RemoteSourceCache::new(unified, "k");
        let err = src
            .get_body_cached(
                "https://example.invalid/index.txt",
                Duration::from_secs(3600),
                CacheMode::Offline,
                |_u| Ok("never".into()),
            )
            .expect_err("offline missing should error");
        assert!(format!("{err}").to_ascii_lowercase().contains("offline"));
    }

    #[test]
    fn stale_ok_returns_cached_body_on_refresh_failure() {
        let td = tempdir().expect("tmp");
        let unified = td
            .path()
            .join("cache")
            .join("node")
            .join("unified_version_list");
        let src = RemoteSourceCache::new(unified, "k");
        // Seed cache with some body
        let seeded = src
            .get_body_cached(
                "https://example.invalid/index.txt",
                Duration::from_secs(0),
                CacheMode::StaleOk,
                |_u| Ok("a\nb\n".into()),
            )
            .expect("seed");
        assert!(seeded.contains('a'));
        // Force refresh but fetcher fails; stale_ok should return existing cached body.
        let got = src
            .get_body_cached(
                "https://example.invalid/index.txt",
                Duration::from_secs(0),
                CacheMode::ForceRefresh,
                |_u| Err(EnvrError::Download("boom".into())),
            )
            .expect("stale_ok fallback");
        assert!(got.contains('b'));
    }

    #[test]
    fn derived_cache_writes_major_rows_and_children() {
        let td = tempdir().expect("tmp");
        let unified = td
            .path()
            .join("cache")
            .join("node")
            .join("unified_version_list");
        let idx = CachedRemoteIndex::new(
            RuntimeKind::Node,
            unified.clone(),
            RemoteSourceCache::new(unified, "k"),
            Parser,
        );
        let url = "https://example.invalid/index.txt";
        let rows = idx
            .refresh_major_rows_remote(url, Duration::from_secs(3600), CacheMode::StaleOk, |_u| {
                Ok("3.2.1\n3.1.0\n2.9.9\n".into())
            })
            .expect("majors");
        assert!(!rows.is_empty());
        assert!(idx.major_rows_path().is_file());

        let children = idx
            .refresh_children_remote(
                url,
                Duration::from_secs(3600),
                CacheMode::StaleOk,
                "3",
                |_u| Ok("3.2.1\n3.1.0\n2.9.9\n".into()),
            )
            .expect("children");
        assert_eq!(children.len(), 2);
        assert!(idx.children_path("3").is_file());
    }
}
