//! Blocking HTTP download with optional resume (`Range`) and cooperative cancel.
//!
//! On `416 Range Not Satisfiable` with a non-zero resume offset, the partial file is removed and
//! the download is retried without `Range` (shared policy with async [`crate::engine::DownloadEngine`] where applicable).

use envr_error::{EnvrError, EnvrResult, ErrorCode};
use reqwest::blocking::Client;
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, OnceLock, RwLock};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use crate::global_limit::{
    DownloadPriority, global_download_concurrency_limiter, global_download_limiter,
};
use crate::stats::{record_blocking_pool_hit, record_blocking_pool_miss};

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

/// Shared blocking HTTP client builder for runtime index fetches and other sync HTTP paths.
pub fn blocking_http_client_builder(
    user_agent: &str,
    request_timeout: Option<Duration>,
) -> reqwest::blocking::ClientBuilder {
    const DEFAULT_TIMEOUT_SECS: u64 = 60;
    const DEFAULT_CONNECT_TIMEOUT_SECS: u64 = 30;
    let timeout = request_timeout.unwrap_or(Duration::from_secs(DEFAULT_TIMEOUT_SECS));
    let connect_timeout = std::env::var("ENVR_HTTP_CONNECT_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .filter(|n| *n > 0)
        .map(Duration::from_secs)
        .unwrap_or(Duration::from_secs(DEFAULT_CONNECT_TIMEOUT_SECS));
    reqwest::blocking::Client::builder()
        .timeout(timeout)
        .connect_timeout(connect_timeout)
        .user_agent(user_agent)
}

/// Shared blocking HTTP client builder for runtime index fetches and other sync HTTP paths.
pub fn build_blocking_http_client(
    user_agent: &str,
    request_timeout: Option<Duration>,
) -> EnvrResult<Client> {
    static POOL: OnceLock<RwLock<HashMap<String, Client>>> = OnceLock::new();
    let pool = POOL.get_or_init(|| RwLock::new(HashMap::new()));
    let connect_timeout = std::env::var("ENVR_HTTP_CONNECT_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(30);
    let timeout = request_timeout.unwrap_or(Duration::from_secs(60)).as_secs();
    let key = format!("ua={user_agent}|timeout={timeout}|connect={connect_timeout}|profile=default");

    if let Ok(g) = pool.read()
        && let Some(client) = g.get(&key)
    {
        record_blocking_pool_hit();
        return Ok(client.clone());
    }
    record_blocking_pool_miss();

    let built = blocking_http_client_builder(user_agent, request_timeout)
        .build()
        .map_err(|e| {
            EnvrError::with_source(
                ErrorCode::Download,
                "reqwest blocking client build failed",
                e,
            )
        })?;
    if let Ok(mut g) = pool.write() {
        g.insert(key, built.clone());
    }
    Ok(built)
}

/// Shared blocking HTTP client builder for runtime index fetches that require HTTP/1 only.
pub fn build_blocking_http1_only_client(
    user_agent: &str,
    request_timeout: Option<Duration>,
) -> EnvrResult<Client> {
    static POOL: OnceLock<RwLock<HashMap<String, Client>>> = OnceLock::new();
    let pool = POOL.get_or_init(|| RwLock::new(HashMap::new()));
    let connect_timeout = std::env::var("ENVR_HTTP_CONNECT_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(30);
    let timeout = request_timeout.unwrap_or(Duration::from_secs(60)).as_secs();
    let key = format!("ua={user_agent}|timeout={timeout}|connect={connect_timeout}|profile=http1");

    if let Ok(g) = pool.read()
        && let Some(client) = g.get(&key)
    {
        record_blocking_pool_hit();
        return Ok(client.clone());
    }
    record_blocking_pool_miss();

    let built = blocking_http_client_builder(user_agent, request_timeout)
        .http1_only()
        .build()
        .map_err(|e| {
            EnvrError::with_source(
                ErrorCode::Download,
                "reqwest blocking client build failed",
                e,
            )
        })?;
    if let Ok(mut g) = pool.write() {
        g.insert(key, built.clone());
    }
    Ok(built)
}

/// GET `url` to `path`, optionally resuming from existing partial file length, with bounded retries.
pub fn download_url_to_path_resumable(
    client: &Client,
    url: &str,
    path: &Path,
    progress_downloaded: Option<&Arc<AtomicU64>>,
    progress_total: Option<&Arc<AtomicU64>>,
    cancel: Option<&Arc<AtomicBool>>,
) -> EnvrResult<()> {
    download_url_to_path_resumable_with_headers(
        client,
        url,
        path,
        progress_downloaded,
        progress_total,
        cancel,
        None,
    )
}

/// Same as [`download_url_to_path_resumable`] but allows additional fixed request headers.
pub fn download_url_to_path_resumable_with_headers(
    client: &Client,
    url: &str,
    path: &Path,
    progress_downloaded: Option<&Arc<AtomicU64>>,
    progress_total: Option<&Arc<AtomicU64>>,
    cancel: Option<&Arc<AtomicBool>>,
    headers: Option<&reqwest::header::HeaderMap>,
) -> EnvrResult<()> {
    let _permit = global_download_concurrency_limiter()
        .map(|lim| lim.acquire_blocking(DownloadPriority::Artifact));
    let mut last_err: Option<EnvrError> = None;
    let mut range_recovery = 0u8;
    for attempt in 1..=3 {
        if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
            return Err(EnvrError::Download("download cancelled".to_string()));
        }

        let resumed_from = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(EnvrError::from)?;
        }

        let mut request = client.get(url);
        if let Some(extra) = headers {
            for (name, value) in extra {
                request = request.header(name, value);
            }
        }
        if resumed_from > 0 {
            request = request.header(reqwest::header::RANGE, format!("bytes={}-", resumed_from));
        }

        let response = match request.send() {
            Ok(r) => r,
            Err(e) => {
                last_err = Some(EnvrError::with_source(
                    ErrorCode::Download,
                    format!("request failed for {url}"),
                    e,
                ));
                if attempt < 3 {
                    let delay = match attempt {
                        1 => Duration::from_secs(1),
                        2 => Duration::from_secs(2),
                        _ => Duration::from_secs(4),
                    };
                    std::thread::sleep(delay);
                    continue;
                }
                break;
            }
        };

        let status = response.status();
        if !status.is_success() {
            let err = EnvrError::Download(format!("GET {} -> {}", url, status));
            if resumed_from > 0 && status == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
                drop(response);
                range_recovery = range_recovery.saturating_add(1);
                if range_recovery > 3 {
                    return Err(err);
                }
                remove_path_if_exists(path);
                std::thread::sleep(Duration::from_millis(200));
                continue;
            }
            if status.is_server_error() && attempt < 3 {
                drop(response);
                last_err = Some(err);
                std::thread::sleep(Duration::from_secs(attempt as u64));
                continue;
            }
            return Err(err);
        }

        let restart = resumed_from > 0 && status == reqwest::StatusCode::OK;
        let content_len = response.content_length().unwrap_or(0);
        let total_bytes = if restart {
            content_len
        } else {
            resumed_from.saturating_add(content_len)
        };

        if let Some(t) = progress_total {
            t.store(total_bytes, Ordering::Relaxed);
        }
        if let Some(d) = progress_downloaded {
            d.store(if restart { 0 } else { resumed_from }, Ordering::Relaxed);
        }

        let mut file = if restart {
            fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(path)
        } else if resumed_from > 0 {
            fs::OpenOptions::new()
                .create(true)
                .write(true)
                .append(true)
                .open(path)
        } else {
            fs::File::create(path)
        }
        .map_err(EnvrError::from)?;

        let mut response = response;
        let mut buf = [0u8; 64 * 1024];
        let mut read_error: Option<EnvrError> = None;
        let global = global_download_limiter();
        loop {
            if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
                return Err(EnvrError::Download("download cancelled".to_string()));
            }
            let n = match response.read(&mut buf) {
                Ok(n) => n,
                Err(e) => {
                    read_error = Some(EnvrError::with_source(
                        ErrorCode::Download,
                        format!("read failed for {url}"),
                        e,
                    ));
                    break;
                }
            };
            if n == 0 {
                break;
            }
            if let Some(gl) = global.as_ref() {
                gl.throttle_blocking(n as u64)?;
            }
            if let Err(e) = file.write_all(&buf[..n]) {
                read_error = Some(EnvrError::from(e));
                break;
            }
            if let Some(d) = progress_downloaded {
                d.fetch_add(n as u64, Ordering::Relaxed);
            }
        }

        if let Some(e) = read_error {
            last_err = Some(e);
            if attempt < 3 {
                std::thread::sleep(Duration::from_secs(1 << attempt.min(3)));
                continue;
            }
            break;
        }

        return Ok(());
    }

    Err(last_err
        .unwrap_or_else(|| EnvrError::Download("download failed (unknown error)".to_string())))
}

/// Execute a blocking network section under the global download concurrency scheduler.
///
/// Useful for index/tag fetches that should use `Index` priority while reusing the same
/// process-wide queue as artifact downloads.
pub fn with_download_priority_blocking<T, F>(
    priority: DownloadPriority,
    f: F,
) -> EnvrResult<T>
where
    F: FnOnce() -> EnvrResult<T>,
{
    let _permit = global_download_concurrency_limiter().map(|lim| lim.acquire_blocking(priority));
    f()
}
