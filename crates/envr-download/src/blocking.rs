//! Blocking HTTP download with optional resume (`Range`) and cooperative cancel.
//!
//! On `416 Range Not Satisfiable` with a non-zero resume offset, the partial file is removed and
//! the download is retried without `Range` (shared policy with async [`crate::engine::DownloadEngine`] where applicable).

use envr_error::{EnvrError, EnvrResult};
use reqwest::blocking::Client;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

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

/// GET `url` to `path`, optionally resuming from existing partial file length, with bounded retries.
pub fn download_url_to_path_resumable(
    client: &Client,
    url: &str,
    path: &Path,
    progress_downloaded: Option<&Arc<AtomicU64>>,
    progress_total: Option<&Arc<AtomicU64>>,
    cancel: Option<&Arc<AtomicBool>>,
) -> EnvrResult<()> {
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
        if resumed_from > 0 {
            request = request.header(reqwest::header::RANGE, format!("bytes={}-", resumed_from));
        }

        let response = match request.send() {
            Ok(r) => r,
            Err(e) => {
                last_err = Some(EnvrError::Download(e.to_string()));
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
        loop {
            if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
                return Err(EnvrError::Download("download cancelled".to_string()));
            }
            let n = match response.read(&mut buf) {
                Ok(n) => n,
                Err(e) => {
                    read_error = Some(EnvrError::Download(e.to_string()));
                    break;
                }
            };
            if n == 0 {
                break;
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
