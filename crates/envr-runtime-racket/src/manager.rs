use crate::index::{
    RacketInstallableRow, blocking_http_client, fetch_racket_installable_rows,
    list_remote_latest_per_major_lines, list_remote_versions, resolve_racket_version,
};
use envr_domain::installer::{
    SpecDrivenInstaller, execute_install_pipeline, install_progress_handles,
};
use envr_domain::runtime::{InstallRequest, RemoteFilter, RuntimeVersion};
use envr_download::extract;
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use envr_platform::links::ensure_runtime_current_symlink_or_pointer;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct RacketPaths {
    runtime_root: PathBuf,
}
impl RacketPaths {
    pub fn new(runtime_root: PathBuf) -> Self {
        Self { runtime_root }
    }
    pub fn home(&self) -> PathBuf {
        self.runtime_root.join("runtimes").join("racket")
    }
    pub fn versions_dir(&self) -> PathBuf {
        self.home().join("versions")
    }
    pub fn current_link(&self) -> PathBuf {
        self.home().join("current")
    }
    pub fn cache_dir(&self) -> PathBuf {
        self.runtime_root.join("cache").join("racket")
    }
    pub fn version_dir(&self, version: &str) -> PathBuf {
        self.versions_dir().join(version)
    }
    pub fn releases_cache_path(&self) -> PathBuf {
        self.cache_dir().join("releases.json")
    }
    pub fn latest_cache_path(&self) -> PathBuf {
        self.cache_dir().join("latest_per_major.json")
    }
    pub fn archive_cache_path(&self, version: &str) -> PathBuf {
        self.cache_dir()
            .join("archives")
            .join(format!("racket-minimal-{version}-x86_64-win32-cs.tgz"))
    }
}

fn first_existing(cands: &[PathBuf]) -> Option<PathBuf> {
    cands.iter().find(|p| p.is_file()).cloned()
}
pub fn racket_tool_candidate(home: &Path) -> Option<PathBuf> {
    first_existing(&[
        home.join("racket.exe"),
        home.join("racket"),
        home.join("bin").join("racket.exe"),
        home.join("bin").join("racket"),
    ])
}
pub fn racket_installation_valid(home: &Path) -> bool {
    racket_tool_candidate(home).is_some()
}

fn download_file_atomic(
    client: &reqwest::blocking::Client,
    url: &str,
    dst: &Path,
) -> EnvrResult<()> {
    let mut response = client
        .get(url)
        // Keep archive bytes as-is; do not let HTTP layer transparently decompress.
        .header(reqwest::header::ACCEPT_ENCODING, "identity")
        .send()
        .map_err(|e| {
            EnvrError::with_source(ErrorCode::Download, format!("request failed for {url}"), e)
        })?;
    if !response.status().is_success() {
        return Err(EnvrError::Download(format!(
            "GET {url} -> {}",
            response.status()
        )));
    }
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent).map_err(EnvrError::from)?;
    }
    let part = dst.with_extension("part");
    let mut file = fs::File::create(&part).map_err(EnvrError::from)?;
    let copied = std::io::copy(&mut response, &mut file).map_err(EnvrError::from)?;
    file.flush().map_err(EnvrError::from)?;
    if copied < 1024 * 1024 {
        let _ = fs::remove_file(&part);
        return Err(EnvrError::Download(format!(
            "downloaded installer looks too small ({copied} bytes)"
        )));
    }
    if dst.exists() {
        fs::remove_file(dst).map_err(EnvrError::from)?;
    }
    fs::rename(&part, dst).map_err(EnvrError::from)?;
    Ok(())
}

fn file_starts_with_gzip_magic(path: &Path) -> EnvrResult<bool> {
    let mut f = fs::File::open(path).map_err(EnvrError::from)?;
    let mut header = [0u8; 2];
    let n = std::io::Read::read(&mut f, &mut header).map_err(EnvrError::from)?;
    if n < 2 {
        return Ok(false);
    }
    Ok(header == [0x1F, 0x8B])
}

fn mirror_url_for_release_asset(version: &str, url: &str) -> Option<String> {
    let name = url.split('/').next_back()?.trim();
    if name.is_empty() {
        return None;
    }
    Some(format!(
        "https://mirror.racket-lang.org/installers/{version}/{name}"
    ))
}

fn racket_candidate_dirs(staging: &Path) -> EnvrResult<Vec<PathBuf>> {
    let mut out = Vec::new();
    if staging.is_dir() {
        out.push(staging.to_path_buf());
    }
    for e in fs::read_dir(staging).map_err(EnvrError::from)? {
        let e = e.map_err(EnvrError::from)?;
        if !e.file_type().map_err(EnvrError::from)?.is_dir() {
            continue;
        }
        let p = e.path();
        out.push(p.clone());
        for se in fs::read_dir(&p).map_err(EnvrError::from)? {
            let se = se.map_err(EnvrError::from)?;
            if se.file_type().map_err(EnvrError::from)?.is_dir() {
                out.push(se.path());
            }
        }
    }
    Ok(out)
}

fn depth_key(staging: &Path, p: &Path) -> (usize, String) {
    let depth = p
        .strip_prefix(staging)
        .map(|r| r.components().count())
        .unwrap_or(usize::MAX);
    (depth, p.display().to_string())
}

fn pick_racket_home_from_candidates(staging: &Path, candidates: &[PathBuf]) -> EnvrResult<PathBuf> {
    let mut val: Vec<PathBuf> = candidates
        .iter()
        .filter(|p| racket_installation_valid(p))
        .cloned()
        .collect();
    match val.len() {
        0 => Err(EnvrError::Validation(
            "extracted racket layout missing racket executable".into(),
        )),
        1 => Ok(val[0].clone()),
        _ => {
            val.sort_by_key(|a| depth_key(staging, a));
            Ok(val[0].clone())
        }
    }
}

fn promote_racket_extract(staging: &Path, final_dir: &Path) -> EnvrResult<()> {
    use envr_platform::install_layout;

    install_layout::ensure_final_parent(final_dir)?;
    let staging_final = install_layout::sibling_staging_path(final_dir)?;
    install_layout::remove_if_exists(&staging_final)?;

    let commit_if_valid = |home: &Path| -> EnvrResult<()> {
        if !racket_installation_valid(home) {
            return Err(EnvrError::Validation(
                "extracted racket layout missing racket executable".into(),
            ));
        }
        install_layout::commit_staging_dir(home, final_dir)
    };

    if racket_installation_valid(staging) {
        fs::create_dir_all(&staging_final).map_err(EnvrError::from)?;
        install_layout::hoist_directory_children(staging, &staging_final)?;
        return commit_if_valid(&staging_final);
    }

    let cands = racket_candidate_dirs(staging)?;
    let inner = pick_racket_home_from_candidates(staging, &cands)?;
    if inner == staging {
        fs::create_dir_all(&staging_final).map_err(EnvrError::from)?;
        install_layout::hoist_directory_children(staging, &staging_final)?;
        return commit_if_valid(&staging_final);
    }
    fs::rename(&inner, &staging_final).map_err(EnvrError::from)?;
    commit_if_valid(&staging_final)
}

pub fn list_installed_versions(paths: &RacketPaths) -> EnvrResult<Vec<RuntimeVersion>> {
    let dir = paths.versions_dir();
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for e in fs::read_dir(&dir).map_err(EnvrError::from)? {
        let e = e.map_err(EnvrError::from)?;
        if !e.file_type().map_err(EnvrError::from)?.is_dir() {
            continue;
        }
        let p = e.path();
        if racket_installation_valid(&p) {
            out.push(RuntimeVersion(e.file_name().to_string_lossy().into_owned()));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

pub fn read_current(paths: &RacketPaths) -> EnvrResult<Option<RuntimeVersion>> {
    let cur = paths.current_link();
    if !cur.exists() {
        return Ok(None);
    }
    if cur.is_file() {
        let s = fs::read_to_string(&cur).map_err(EnvrError::from)?;
        let t = s.trim();
        if t.is_empty() {
            return Ok(None);
        }
        let target = PathBuf::from(t);
        let resolved = fs::canonicalize(&target).unwrap_or(target);
        let name = resolved
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        return if name.is_empty() {
            Ok(None)
        } else {
            Ok(Some(RuntimeVersion(name)))
        };
    }
    let Ok(target) = fs::read_link(&cur) else {
        return Ok(None);
    };
    let resolved = if target.is_relative() {
        cur.parent().map(|p| p.join(&target)).unwrap_or(target)
    } else {
        target
    };
    let resolved = fs::canonicalize(&resolved).unwrap_or(resolved);
    let name = resolved
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    if name.is_empty() {
        Ok(None)
    } else {
        Ok(Some(RuntimeVersion(name)))
    }
}

#[derive(Debug, Clone)]
pub struct RacketManager {
    paths: RacketPaths,
    all_versions_url: String,
}
impl RacketManager {
    pub fn try_new(runtime_root: PathBuf, all_versions_url: String) -> EnvrResult<Self> {
        Ok(Self {
            paths: RacketPaths::new(runtime_root),
            all_versions_url,
        })
    }
    fn index_cache_ttl_secs() -> u64 {
        std::env::var("ENVR_RACKET_INDEX_CACHE_TTL_SECS")
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or(3600)
    }
    fn latest_cache_ttl_secs() -> u64 {
        std::env::var("ENVR_RACKET_REMOTE_CACHE_TTL_SECS")
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or(86400)
    }
    fn load_cached_rows(&self) -> Option<Vec<RacketInstallableRow>> {
        let path = self.paths.releases_cache_path();
        let meta = fs::metadata(&path).ok()?;
        let age = SystemTime::now()
            .duration_since(meta.modified().ok()?)
            .ok()?
            .as_secs();
        if age > Self::index_cache_ttl_secs() {
            return None;
        }
        let text = fs::read_to_string(&path).ok()?;
        serde_json::from_str::<Vec<RacketInstallableRow>>(&text).ok()
    }
    fn save_cached_rows(&self, rows: &[RacketInstallableRow]) -> EnvrResult<()> {
        fs::create_dir_all(self.paths.cache_dir()).map_err(EnvrError::from)?;
        let text = serde_json::to_string_pretty(rows).map_err(|e| {
            EnvrError::with_source(ErrorCode::Download, "serialize racket rows cache", e)
        })?;
        fs::write(self.paths.releases_cache_path(), text).map_err(EnvrError::from)?;
        Ok(())
    }
    fn fetch_rows(&self, force_refresh: bool) -> EnvrResult<Vec<RacketInstallableRow>> {
        if !force_refresh && let Some(rows) = self.load_cached_rows() {
            return Ok(rows);
        }
        let client = blocking_http_client()?;
        let rows = fetch_racket_installable_rows(&client, &self.all_versions_url)?;
        self.save_cached_rows(&rows)?;
        Ok(rows)
    }
    pub fn list_remote(&self, filter: &RemoteFilter) -> EnvrResult<Vec<RuntimeVersion>> {
        Ok(list_remote_versions(&self.fetch_rows(false)?, filter))
    }
    pub fn list_remote_latest_per_major(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let path = self.paths.latest_cache_path();
        if let Ok(meta) = fs::metadata(&path)
            && let Ok(age) =
                SystemTime::now().duration_since(meta.modified().map_err(EnvrError::from)?)
            && age.as_secs() <= Self::latest_cache_ttl_secs()
            && let Ok(text) = fs::read_to_string(&path)
            && let Ok(v) = serde_json::from_str::<Vec<String>>(&text)
        {
            return Ok(v.into_iter().map(RuntimeVersion).collect());
        }
        let latest = list_remote_latest_per_major_lines(&self.fetch_rows(false)?);
        fs::create_dir_all(self.paths.cache_dir()).map_err(EnvrError::from)?;
        let labels: Vec<String> = latest.iter().map(|v| v.0.clone()).collect();
        let text = serde_json::to_string_pretty(&labels).map_err(|e| {
            EnvrError::with_source(ErrorCode::Download, "serialize racket latest cache", e)
        })?;
        fs::write(&path, text).map_err(EnvrError::from)?;
        Ok(latest)
    }
    pub fn resolve_label(&self, spec: &str) -> EnvrResult<String> {
        resolve_racket_version(&self.fetch_rows(false)?, spec)
            .ok_or_else(|| EnvrError::Validation(format!("unknown racket version spec: {spec}")))
    }
    pub fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        let dir = self.paths.version_dir(&version.0);
        if !dir.is_dir() || !racket_installation_valid(&dir) {
            return Err(EnvrError::Validation(format!(
                "racket version not installed: {}",
                version.0
            )));
        }
        ensure_runtime_current_symlink_or_pointer(&dir, &self.paths.current_link())
    }
    pub fn uninstall(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        let dir = self.paths.version_dir(&version.0);
        if dir.exists() {
            fs::remove_dir_all(&dir).map_err(EnvrError::from)?;
        }
        Ok(())
    }
}

impl SpecDrivenInstaller for RacketManager {
    fn install_from_spec(&self, request: &InstallRequest) -> EnvrResult<RuntimeVersion> {
        let label = self.resolve_label(&request.spec.0)?;
        let rows = self.fetch_rows(false)?;
        let row = rows.iter().find(|r| r.version == label).ok_or_else(|| {
            EnvrError::Validation(format!("racket version not found in index: {label}"))
        })?;
        let final_dir = self.paths.version_dir(&label);
        if final_dir.is_dir() && racket_installation_valid(&final_dir) {
            return Ok(RuntimeVersion(label));
        }
        let client = blocking_http_client()?;
        let dl_dir = self.paths.cache_dir().join("downloads");
        fs::create_dir_all(&dl_dir).map_err(EnvrError::from)?;
        let dl_tmp = tempfile::tempdir_in(&dl_dir).map_err(EnvrError::from)?;
        let archive = dl_tmp.path().join(format!("racket-minimal-{label}.tgz"));
        let fallback = row.url.replace("-cs.tgz", ".tgz");
        let mirror_primary = mirror_url_for_release_asset(&label, &row.url);
        let mirror_fallback = mirror_url_for_release_asset(&label, &fallback);
        let label_for_cache = label.clone();
        let resolved_label = label.clone();
        let mut candidates = vec![row.url.clone()];
        if fallback != row.url {
            candidates.push(fallback.clone());
        }
        if let Some(m) = mirror_primary {
            candidates.push(m);
        }
        if let Some(mf) = mirror_fallback
            && !candidates.iter().any(|u| u == &mf)
        {
            candidates.push(mf);
        }
        let (_, _, cancel) = install_progress_handles(request);
        execute_install_pipeline(
            cancel,
            || {
                fs::create_dir_all(&final_dir).map_err(EnvrError::from)?;
                fs::create_dir_all(self.paths.cache_dir()).map_err(EnvrError::from)?;
                fs::create_dir_all(&dl_dir).map_err(EnvrError::from)?;
                Ok(())
            },
            || {
                let mut errs = Vec::new();
                for url in &candidates {
                    match download_file_atomic(&client, url, &archive) {
                        Ok(()) => {
                            if file_starts_with_gzip_magic(&archive)? {
                                errs.clear();
                                break;
                            }
                            errs.push(format!("{url} (downloaded file is not gzip data)"));
                        }
                        Err(e) => {
                            errs.push(format!("{url} ({e})"));
                        }
                    }
                }
                if !archive.is_file() || !file_starts_with_gzip_magic(&archive)? {
                    return Err(EnvrError::Download(format!(
                        "failed to download racket archive; tried: {}",
                        errs.join("; ")
                    )));
                }
                // Keep a copy in cache for operator debugging; ignore failures (install can proceed from temp file).
                let _ = fs::copy(&archive, self.paths.archive_cache_path(&label_for_cache));
                Ok(())
            },
            || Ok(()),
            || {
                let staging_parent = self.paths.cache_dir().join("extract_staging");
                fs::create_dir_all(&staging_parent).map_err(EnvrError::from)?;
                let staging = tempfile::tempdir_in(&staging_parent).map_err(EnvrError::from)?;
                extract::extract_archive(&archive, staging.path())?;
                promote_racket_extract(staging.path(), &final_dir)
            },
            || {
                if !racket_installation_valid(&final_dir) {
                    return Err(EnvrError::Validation(
                        "racket install validation failed".into(),
                    ));
                }
                Ok(RuntimeVersion(resolved_label))
            },
        )
    }
}
