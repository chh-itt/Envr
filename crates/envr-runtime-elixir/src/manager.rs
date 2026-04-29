use crate::index::{
    DEFAULT_BUILDS_BASE_URL, DEFAULT_OTP_SERIES, ElixirBuild, blocking_http_client,
    fetch_builds_index, parse_elixir_builds, pick_build_for_version, resolve_elixir_version,
    select_builds_prefer_otp,
};
use envr_domain::installer::{
    SpecDrivenInstaller, execute_install_pipeline, install_progress_handles,
};
use envr_domain::runtime::{InstallRequest, RuntimeVersion};
use envr_download::blocking::download_url_to_path_resumable;
use envr_download::extract;
use envr_error::{EnvrError, EnvrResult};
use envr_platform::links::{LinkType, ensure_link};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64};

#[derive(Debug, Clone)]
pub struct ElixirPaths {
    runtime_root: PathBuf,
}

impl ElixirPaths {
    pub fn new(runtime_root: PathBuf) -> Self {
        Self { runtime_root }
    }

    pub fn elixir_home(&self) -> PathBuf {
        self.runtime_root.join("runtimes").join("elixir")
    }

    pub fn versions_dir(&self) -> PathBuf {
        self.elixir_home().join("versions")
    }

    pub fn current_link(&self) -> PathBuf {
        self.elixir_home().join("current")
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.runtime_root.join("cache").join("elixir")
    }

    pub fn version_dir(&self, version_label: &str) -> PathBuf {
        self.versions_dir().join(version_label)
    }
}

fn elixir_executable(home: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        home.join("bin").join("elixir.bat")
    }
    #[cfg(not(windows))]
    {
        home.join("bin").join("elixir")
    }
}

fn mix_executable(home: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        home.join("bin").join("mix.bat")
    }
    #[cfg(not(windows))]
    {
        home.join("bin").join("mix")
    }
}

pub fn elixir_installation_valid(home: &Path) -> bool {
    elixir_executable(home).is_file() && mix_executable(home).is_file()
}

pub fn list_installed_versions(paths: &ElixirPaths) -> EnvrResult<Vec<RuntimeVersion>> {
    let dir = paths.versions_dir();
    if !dir.is_dir() {
        return Ok(vec![]);
    }
    let mut out = Vec::new();
    for e in fs::read_dir(&dir).map_err(EnvrError::from)? {
        let e = e.map_err(EnvrError::from)?;
        if !e.file_type().map_err(EnvrError::from)?.is_dir() {
            continue;
        }
        let p = e.path();
        if elixir_installation_valid(&p) {
            out.push(RuntimeVersion(e.file_name().to_string_lossy().into_owned()));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

pub fn read_current(paths: &ElixirPaths) -> EnvrResult<Option<RuntimeVersion>> {
    let cur = paths.current_link();
    if !cur.exists() {
        return Ok(None);
    }
    if let Ok(target) = fs::read_link(&cur) {
        let resolved = if target.is_relative() {
            cur.parent().map(|p| p.join(&target)).unwrap_or(target)
        } else {
            target
        };
        let name = resolved
            .file_name()
            .ok_or_else(|| EnvrError::Runtime("invalid elixir current link".into()))?
            .to_string_lossy()
            .into_owned();
        return Ok(Some(RuntimeVersion(name)));
    }
    let s = fs::read_to_string(&cur).map_err(EnvrError::from)?;
    let t = s.trim();
    if t.is_empty() {
        return Ok(None);
    }
    let target = PathBuf::from(t);
    let name = target
        .file_name()
        .ok_or_else(|| EnvrError::Runtime("invalid elixir current pointer".into()))?
        .to_string_lossy()
        .into_owned();
    Ok(Some(RuntimeVersion(name)))
}

fn set_current_pointer_file(cur: &Path, abs_target_dir: &Path) -> EnvrResult<()> {
    if cur.exists() {
        if cur.is_dir() {
            fs::remove_dir_all(cur).map_err(EnvrError::from)?;
        } else {
            fs::remove_file(cur).map_err(EnvrError::from)?;
        }
    }
    if let Some(parent) = cur.parent() {
        fs::create_dir_all(parent).map_err(EnvrError::from)?;
    }
    envr_platform::fs_atomic::write_atomic(
        cur,
        abs_target_dir.to_string_lossy().to_string().as_bytes(),
    )
    .map_err(EnvrError::from)?;
    Ok(())
}

fn remove_path_if_exists(path: &Path) {
    if fs::symlink_metadata(path).is_err() {
        return;
    }
    if fs::remove_file(path).is_ok() {
        return;
    }
    if fs::remove_dir(path).is_ok() {
        return;
    }
    let _ = fs::remove_dir_all(path);
}

fn ensure_erlang_runtime_available() -> EnvrResult<()> {
    #[cfg(windows)]
    let mut cmd = {
        let mut c = Command::new("cmd");
        c.arg("/C").arg("erl -noshell -eval halt().");
        c
    };
    #[cfg(not(windows))]
    let mut cmd = {
        let mut c = Command::new("erl");
        c.arg("-noshell").arg("-eval").arg("halt().");
        c
    };

    match cmd.output() {
        Ok(out) if out.status.success() => Ok(()),
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            if let Some(diag) = envr_platform::process::classify_exit_failure_message(
                Some(envr_domain::runtime::RuntimeKind::Elixir),
                "erlang runtime check",
                out.status,
                &stderr,
            ) {
                return Err(EnvrError::Runtime(diag));
            }
            Err(EnvrError::Runtime(
            "Erlang/OTP runtime check failed: `erl` is present but not runnable. Install or repair Erlang/OTP, then retry Elixir install.".into(),
            ))
        }
        Err(e) => Err(EnvrError::Runtime(
            envr_platform::process::classify_spawn_failure_message(
                Some(envr_domain::runtime::RuntimeKind::Elixir),
                "erlang runtime check",
                &e,
            ),
        )),
    }
}

/// Resolve envr-managed Erlang home from `runtimes/erlang/current` (symlink or pointer file).
#[cfg(windows)]
fn resolve_erlang_home(runtime_root: &Path) -> Option<PathBuf> {
    let cur = runtime_root.join("runtimes").join("erlang").join("current");
    if !cur.exists() {
        return None;
    }
    let resolved = if let Ok(target) = fs::read_link(&cur) {
        if target.is_relative() {
            cur.parent().map(|p| p.join(&target)).unwrap_or(target)
        } else {
            target
        }
    } else {
        let s = fs::read_to_string(&cur).ok()?;
        let t = s.trim();
        if t.is_empty() {
            return None;
        }
        PathBuf::from(t)
    };
    fs::canonicalize(&resolved).ok().or(Some(resolved))
}

fn validate_elixir_installation(home: &Path, runtime_root: &Path) -> EnvrResult<()> {
    if !elixir_installation_valid(home) {
        return Err(EnvrError::Validation(
            "elixir install did not produce a valid runtime layout".into(),
        ));
    }
    let exe = elixir_executable(home);
    let mut cmd = Command::new(&exe);
    cmd.arg("--version");
    #[cfg(windows)]
    if let Some(erlang_home) = resolve_erlang_home(runtime_root) {
        let eh = envr_platform::path_norm::normalize_fs_path_string_lossy(&erlang_home);
        let mut erts =
            envr_platform::path_norm::normalize_fs_path_string_lossy(&erlang_home.join("bin"));
        if !erts.ends_with('\\') {
            erts.push('\\');
        }
        cmd.env("ERLANG_HOME", &eh);
        cmd.env("ERTS_BIN", &erts);
    }
    let out = cmd.output().map_err(|e| {
        EnvrError::Runtime(envr_platform::process::classify_spawn_failure_message(
            Some(envr_domain::runtime::RuntimeKind::Elixir),
            "elixir --version",
            &e,
        ))
    })?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
        if let Some(diag) = envr_platform::process::classify_exit_failure_message(
            Some(envr_domain::runtime::RuntimeKind::Elixir),
            "elixir --version",
            out.status,
            &stderr,
        ) {
            return Err(EnvrError::Runtime(diag));
        }
        return Err(EnvrError::Runtime(format!(
            "elixir --version failed: {stderr}"
        )));
    }
    Ok(())
}

#[cfg(windows)]
fn patch_windows_elixir_batch(home: &Path) -> EnvrResult<()> {
    let bat = home.join("bin").join("elixir.bat");
    if !bat.is_file() {
        return Ok(());
    }
    let src = fs::read_to_string(&bat).map_err(EnvrError::from)?;
    let needle_crlf = "set ERTS_BIN=\r\nset ERTS_BIN=!ERTS_BIN!";
    let needle_lf = "set ERTS_BIN=\nset ERTS_BIN=!ERTS_BIN!";
    let repl_crlf = "if defined ERLANG_HOME set ERLANG_HOME=%ERLANG_HOME:\"=%\r\nif not defined ERTS_BIN if defined ERLANG_HOME set ERTS_BIN=%ERLANG_HOME%\\bin\\\r\nif defined ERTS_BIN set ERTS_BIN=%ERTS_BIN:\"=%\r\nif not defined ERTS_BIN set ERTS_BIN=\r\nset ERTS_BIN=!ERTS_BIN!";
    let repl_lf = "if defined ERLANG_HOME set ERLANG_HOME=%ERLANG_HOME:\"=%\nif not defined ERTS_BIN if defined ERLANG_HOME set ERTS_BIN=%ERLANG_HOME%\\bin\\\nif defined ERTS_BIN set ERTS_BIN=%ERTS_BIN:\"=%\nif not defined ERTS_BIN set ERTS_BIN=\nset ERTS_BIN=!ERTS_BIN!";
    let mut patched = if src.contains(needle_crlf) {
        src.replace(needle_crlf, repl_crlf)
    } else if src.contains(needle_lf) {
        src.replace(needle_lf, repl_lf)
    } else {
        src
    };
    let run_line_crlf =
        "\"%ERTS_BIN%erl.exe\" %ELIXIR_ERL_OPTIONS% %parsErlang% %beforeExtra% -extra %*";
    let run_repl_crlf = "set \"ERL_EXE=erl.exe\"\r\nif defined ERTS_BIN set \"ERL_EXE=%ERTS_BIN%\\erl.exe\"\r\n\"%ERL_EXE%\" %ELIXIR_ERL_OPTIONS% %parsErlang% %beforeExtra% -extra %*";
    let run_line_lf =
        "\"%ERTS_BIN%erl.exe\" %ELIXIR_ERL_OPTIONS% %parsErlang% %beforeExtra% -extra %*";
    let run_repl_lf = "set \"ERL_EXE=erl.exe\"\nif defined ERTS_BIN set \"ERL_EXE=%ERTS_BIN%\\erl.exe\"\n\"%ERL_EXE%\" %ELIXIR_ERL_OPTIONS% %parsErlang% %beforeExtra% -extra %*";
    if patched.contains(run_line_crlf) {
        patched = patched.replace(run_line_crlf, run_repl_crlf);
    } else if patched.contains(run_line_lf) {
        patched = patched.replace(run_line_lf, run_repl_lf);
    }
    fs::write(&bat, patched).map_err(EnvrError::from)
}

#[cfg(not(windows))]
fn patch_windows_elixir_batch(_home: &Path) -> EnvrResult<()> {
    Ok(())
}

pub struct ElixirManager {
    pub paths: ElixirPaths,
    builds_index_url: String,
    builds_base_url: String,
    otp_series: String,
    client: reqwest::blocking::Client,
}

impl ElixirManager {
    pub fn try_new(runtime_root: PathBuf, builds_index_url: String) -> EnvrResult<Self> {
        Ok(Self {
            paths: ElixirPaths::new(runtime_root),
            builds_index_url,
            builds_base_url: DEFAULT_BUILDS_BASE_URL.to_string(),
            otp_series: DEFAULT_OTP_SERIES.to_string(),
            client: blocking_http_client()?,
        })
    }

    pub fn load_builds(&self) -> EnvrResult<Vec<ElixirBuild>> {
        let text = fetch_builds_index(&self.client, &self.builds_index_url)?;
        let all = parse_elixir_builds(&text, &self.builds_base_url)?;
        let builds = select_builds_prefer_otp(&all, &self.otp_series);
        if builds.is_empty() {
            return Err(EnvrError::Validation(format!(
                "no elixir builds found in index (preferred otp {})",
                self.otp_series
            )));
        }
        Ok(builds)
    }

    pub fn resolve_spec(&self, spec: &str) -> EnvrResult<RuntimeVersion> {
        let builds = self.load_builds()?;
        Ok(RuntimeVersion(resolve_elixir_version(&builds, spec)?))
    }

    fn install_resolved_version(
        &self,
        version: &RuntimeVersion,
        progress_downloaded: Option<&Arc<AtomicU64>>,
        progress_total: Option<&Arc<AtomicU64>>,
        cancel: Option<&Arc<AtomicBool>>,
    ) -> EnvrResult<RuntimeVersion> {
        ensure_erlang_runtime_available()?;
        let builds = self.load_builds()?;
        let build = pick_build_for_version(&builds, &version.0)?;
        let file_name = build
            .url
            .rsplit('/')
            .next()
            .ok_or_else(|| EnvrError::Validation("elixir build url missing filename".into()))?;
        let cache_file = self.paths.cache_dir().join(&version.0).join(file_name);
        let final_dir = self.paths.version_dir(&version.0);
        execute_install_pipeline(
            cancel,
            || fs::create_dir_all(self.paths.cache_dir()).map_err(EnvrError::from),
            || {
                download_url_to_path_resumable(
                    &self.client,
                    &build.url,
                    &cache_file,
                    progress_downloaded,
                    progress_total,
                    cancel,
                )
            },
            || Ok(()),
            || {
                use envr_platform::install_layout;
                install_layout::ensure_final_parent(&final_dir)?;
                let staging_final = install_layout::sibling_staging_path(&final_dir)?;
                install_layout::remove_if_exists(&staging_final)?;
                fs::create_dir_all(&staging_final).map_err(EnvrError::from)?;
                extract::extract_archive(&cache_file, &staging_final)?;
                patch_windows_elixir_batch(&staging_final)?;
                if let Err(e) =
                    validate_elixir_installation(&staging_final, &self.paths.runtime_root)
                {
                    let _ = fs::remove_dir_all(&staging_final);
                    return Err(e);
                }
                install_layout::commit_staging_dir(&staging_final, &final_dir)
            },
            || {
                self.set_current(version)?;
                Ok(RuntimeVersion(version.0.clone()))
            },
        )
    }

    pub fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        let dir = self.paths.version_dir(&version.0);
        if !elixir_installation_valid(&dir) {
            return Err(EnvrError::Validation(format!(
                "elixir {} is not installed",
                version.0
            )));
        }
        let abs = fs::canonicalize(&dir).map_err(EnvrError::from)?;
        let cur = self.paths.current_link();
        match ensure_link(LinkType::Soft, &abs, &cur) {
            Ok(()) => Ok(()),
            Err(EnvrError::Io(e)) if e.raw_os_error() == Some(1314) => {
                set_current_pointer_file(&cur, &abs)?;
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    pub fn uninstall(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        let current = read_current(&self.paths)?;
        let dir = self.paths.version_dir(&version.0);
        if fs::symlink_metadata(&dir).is_err() {
            return Err(EnvrError::Validation(format!(
                "elixir {} is not installed",
                version.0
            )));
        }
        fs::remove_dir_all(&dir).map_err(EnvrError::from)?;
        if current.as_ref().is_some_and(|v| v.0 == version.0) {
            remove_path_if_exists(&self.paths.current_link());
        }
        Ok(())
    }
}

impl SpecDrivenInstaller for ElixirManager {
    fn install_from_spec(&self, request: &InstallRequest) -> EnvrResult<RuntimeVersion> {
        let builds = self.load_builds()?;
        let version = resolve_elixir_version(&builds, &request.spec.0)?;
        let (downloaded, total, cancel) = install_progress_handles(request);
        self.install_resolved_version(&RuntimeVersion(version), downloaded, total, cancel)
    }
}
