use crate::manager::{RustManager, RustPaths, RustupMode};
use envr_config::settings::{Settings, settings_path_from_platform};
use envr_domain::runtime::{InstallRequest, VersionSpec};
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use envr_platform::paths::current_platform_paths;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RustChannel {
    Stable,
    Beta,
    Nightly,
}

impl RustChannel {
    pub fn as_str(self) -> &'static str {
        match self {
            RustChannel::Stable => "stable",
            RustChannel::Beta => "beta",
            RustChannel::Nightly => "nightly",
        }
    }
}

fn rustup_init_target_triple() -> &'static str {
    // Keep this conservative; rustup-init supports more targets but these cover our primary OSes.
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        "x86_64-pc-windows-msvc"
    }
    #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
    {
        "aarch64-pc-windows-msvc"
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        "x86_64-unknown-linux-gnu"
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        "aarch64-unknown-linux-gnu"
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        "x86_64-apple-darwin"
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        "aarch64-apple-darwin"
    }
    #[cfg(not(any(
        all(target_os = "windows", target_arch = "x86_64"),
        all(target_os = "windows", target_arch = "aarch64"),
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "aarch64"),
    )))]
    {
        // Fallback: best effort. Rustup-init may still work if upstream supports the target.
        "x86_64-pc-windows-msvc"
    }
}

fn rustup_init_filename() -> &'static str {
    #[cfg(windows)]
    {
        "rustup-init.exe"
    }
    #[cfg(not(windows))]
    {
        "rustup-init"
    }
}

fn rustup_init_url_from_settings(st: &Settings) -> String {
    // Keep rustup-init download source aligned with `RUSTUP_DIST_SERVER`.
    let base = envr_config::settings::rustup_dist_server_from_settings(st)
        .unwrap_or_else(|| "https://static.rust-lang.org".to_string());
    format!(
        "{}/rustup/dist/{}/{}",
        base.trim_end_matches('/'),
        rustup_init_target_triple(),
        rustup_init_filename()
    )
}

fn download_rustup_init_to(
    url: &str,
    dest: &Path,
    progress_downloaded: Option<&Arc<AtomicU64>>,
    progress_total: Option<&Arc<AtomicU64>>,
    cancel: Option<&Arc<AtomicBool>>,
) -> EnvrResult<()> {
    if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
        return Err(EnvrError::Download("download cancelled".to_string()));
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(90))
        .user_agent(concat!("envr-runtime-rust/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| EnvrError::with_source(ErrorCode::Download, "build rustup init http client", e))?;
    let mut resp = client
        .get(url)
        .send()
        .map_err(|e| EnvrError::with_source(ErrorCode::Download, format!("request failed for {url}"), e))?;
    if !resp.status().is_success() {
        return Err(EnvrError::Download(format!(
            "GET {url} -> {}",
            resp.status()
        )));
    }
    if let Some(p) = dest.parent() {
        fs::create_dir_all(p).map_err(EnvrError::from)?;
    }
    if let Some(t) = progress_total {
        t.store(resp.content_length().unwrap_or(0), Ordering::Relaxed);
    }
    if let Some(d) = progress_downloaded {
        d.store(0, Ordering::Relaxed);
    }
    let mut f = fs::File::create(dest).map_err(EnvrError::from)?;
    let mut buf = [0u8; 64 * 1024];
    loop {
        if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
            return Err(EnvrError::Download("download cancelled".to_string()));
        }
        let n = resp
            .read(&mut buf)
            .map_err(|e| {
                EnvrError::with_source(ErrorCode::Download, format!("read response body failed for {url}"), e)
            })?;
        if n == 0 {
            break;
        }
        f.write_all(&buf[..n]).map_err(EnvrError::from)?;
        if let Some(d) = progress_downloaded {
            d.fetch_add(n as u64, Ordering::Relaxed);
        }
    }
    Ok(())
}

fn load_settings_best_effort() -> Settings {
    let Ok(platform) = current_platform_paths() else {
        return Settings::default();
    };
    let path = settings_path_from_platform(&platform);
    Settings::load_or_default_from(&path).unwrap_or_default()
}

fn rustup_env_from_settings(st: &Settings) -> Vec<(String, String)> {
    let mut out = Vec::new();
    if let Some(v) = envr_config::settings::rustup_dist_server_from_settings(st) {
        out.push(("RUSTUP_DIST_SERVER".into(), v));
    }
    if let Some(v) = envr_config::settings::rustup_update_root_from_settings(st) {
        out.push(("RUSTUP_UPDATE_ROOT".into(), v));
    }
    out
}

/// Install envr-managed rustup (downloads `rustup-init`, runs it, then ensures default toolchain).
///
/// When `progress` is set (e.g. CLI `InstallRequest` or GUI download-panel atomics), `rustup-init`
/// download reports bytes via `progress_downloaded` / `progress_total` and honors `cancel`.
pub fn install_rustup_managed(
    runtime_root: PathBuf,
    default_channel: RustChannel,
    progress: Option<&InstallRequest>,
) -> EnvrResult<()> {
    if std::env::var("ENVR_CLI_TEST_MOCK_RUST_INSTALL_MANAGED")
        .ok()
        .as_deref()
        == Some("1")
    {
        let paths = RustPaths::new(runtime_root);
        fs::create_dir_all(paths.rust_root()).map_err(EnvrError::from)?;
        fs::create_dir_all(paths.rust_root().join("cache")).map_err(EnvrError::from)?;
        fs::create_dir_all(paths.rustup_home()).map_err(EnvrError::from)?;
        fs::create_dir_all(paths.cargo_home()).map_err(EnvrError::from)?;
        return Ok(());
    }

    // Rule B: if system rustup exists, do not install a managed one.
    if RustManager::system_rustup_available() {
        return Err(EnvrError::Validation(
            "system rustup is already installed; managed install is disabled".into(),
        ));
    }

    let paths = RustPaths::new(runtime_root);
    fs::create_dir_all(paths.rust_root()).map_err(EnvrError::from)?;

    let cache_dir = paths.rust_root().join("cache");
    fs::create_dir_all(&cache_dir).map_err(EnvrError::from)?;
    let installer = cache_dir.join(rustup_init_filename());
    let st = load_settings_best_effort();
    let url = rustup_init_url_from_settings(&st);
    download_rustup_init_to(
        &url,
        &installer,
        progress.and_then(|r| r.progress_downloaded.as_ref()),
        progress.and_then(|r| r.progress_total.as_ref()),
        progress.and_then(|r| r.cancel.as_ref()),
    )?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perm = fs::metadata(&installer)
            .map_err(EnvrError::from)?
            .permissions();
        perm.set_mode(0o755);
        let _ = fs::set_permissions(&installer, perm);
    }

    let mut cmd = std::process::Command::new(&installer);
    cmd.args([
        "-y",
        "--default-toolchain",
        default_channel.as_str(),
        "--no-modify-path",
    ]);
    cmd.env("RUSTUP_HOME", paths.rustup_home());
    cmd.env("CARGO_HOME", paths.cargo_home());
    for (k, v) in rustup_env_from_settings(&st) {
        cmd.env(k, v);
    }
    // Do not inherit envr PATH assumptions; rustup-init is a standalone binary.
    let out = cmd.output().map_err(EnvrError::from)?;
    if !out.status.success() {
        return Err(EnvrError::Runtime(format!(
            "rustup-init failed: {}",
            String::from_utf8_lossy(&out.stderr)
        )));
    }

    // Ensure the initial toolchain exists (rustup-init usually installs it, but keep this idempotent).
    let mgr = RustManager::try_new(paths.runtime_root())?;
    if mgr.mode() != RustupMode::Managed {
        // Managed rustup should now exist; if not, surface a clear error.
        return Err(EnvrError::Runtime(
            "managed rustup did not become available after install".into(),
        ));
    }
    let _ = mgr.install_toolchain(&InstallRequest {
        spec: VersionSpec(default_channel.as_str().to_string()),
        progress_downloaded: None,
        progress_total: None,
        cancel: None,
    });
    Ok(())
}
