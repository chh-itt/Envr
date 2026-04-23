use crate::task::CancelToken;
use crate::{GlobalRateLimiter, global_download_limiter};
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use futures::StreamExt;
use reqwest::{Client, StatusCode, Url, header};
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

/// Optional `(downloaded_bytes, total_bytes)` reporter; `total_bytes` may be 0 until `Content-Length` is known.
pub type DownloadProgressFn = Arc<dyn Fn(u64, u64) + Send + Sync>;
use tokio::{
    fs::{self, OpenOptions},
    io::AsyncWriteExt,
};

/// Default TCP connect timeout for [`DownloadEngine::default_client`] (each single request can still set its own total timeout via [`DownloadOptions::timeout`]).
pub const DEFAULT_HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

fn http_connect_timeout_from_env() -> Duration {
    const MAX_SECS: u64 = 600;
    match std::env::var("ENVR_HTTP_CONNECT_TIMEOUT_SECS") {
        Ok(s) => match s.trim().parse::<u64>() {
            Ok(n) if (1..=MAX_SECS).contains(&n) => Duration::from_secs(n),
            _ => DEFAULT_HTTP_CONNECT_TIMEOUT,
        },
        Err(_) => DEFAULT_HTTP_CONNECT_TIMEOUT,
    }
}

#[derive(Debug, Clone)]
pub struct DownloadOptions {
    pub timeout: Duration,
    /// Per-request limiter (bytes/sec). Prefer [`crate::set_global_download_limit`] for a shared total limiter.
    pub max_bytes_per_sec: Option<u64>,
    /// Optional shared global limiter for a group of downloads.
    pub global_limiter: Option<Arc<GlobalRateLimiter>>,
}

impl Default for DownloadOptions {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(60),
            max_bytes_per_sec: None,
            global_limiter: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadOutcome {
    pub path: PathBuf,
    pub bytes_written: u64,
    pub resumed_from: u64,
}

#[derive(Clone)]
pub struct DownloadEngine {
    client: Client,
}

impl DownloadEngine {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    pub fn default_client() -> EnvrResult<Client> {
        Client::builder()
            .user_agent("envr/0.1")
            .connect_timeout(http_connect_timeout_from_env())
            .build()
            .map_err(|e| {
                EnvrError::with_source(
                    ErrorCode::Download,
                    "reqwest client build failed",
                    e,
                )
            })
    }

    /// Optional `progress_downloaded` / `progress_total` are updated for GUI observability.
    /// `progress_downloaded` starts at the resume offset (if any) and increases with each written chunk.
    /// `progress_total` is set from `Content-Length` when present (full file size ≈ resume + remainder).
    ///
    /// Optional `on_progress` is throttled (≈200ms or 256KiB) and invoked with current `(downloaded, total)`.
    #[allow(clippy::too_many_arguments)]
    pub async fn download_to_file(
        &self,
        url: Url,
        dest_path: impl AsRef<Path>,
        cancel: &CancelToken,
        options: &DownloadOptions,
        progress_downloaded: Option<Arc<AtomicU64>>,
        progress_total: Option<Arc<AtomicU64>>,
        on_progress: Option<DownloadProgressFn>,
    ) -> EnvrResult<DownloadOutcome> {
        let dest_path = dest_path.as_ref().to_path_buf();
        let mut range_recovery = 0u8;
        loop {
            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent).await.map_err(EnvrError::from)?;
            }

            let resumed_from = existing_file_len(&dest_path).await.unwrap_or(0);
            let mut request = self.client.get(url.clone()).timeout(options.timeout);
            if resumed_from > 0 {
                request = request.header(header::RANGE, format!("bytes={resumed_from}-"));
            }

            let response = request
                .send()
                .await
                .map_err(|e| {
                    EnvrError::with_source(
                        ErrorCode::Download,
                        format!("request failed for {url}"),
                        e,
                    )
                })?;

            let status = response.status();
            let (append, effective_resumed_from) = match status {
                StatusCode::OK => (false, 0),
                StatusCode::PARTIAL_CONTENT => (true, resumed_from),
                StatusCode::RANGE_NOT_SATISFIABLE if resumed_from > 0 => {
                    range_recovery = range_recovery.saturating_add(1);
                    if range_recovery > 3 {
                        return Err(EnvrError::Download(format!(
                            "GET {url} -> {status} (range recovery limit)"
                        )));
                    }
                    let _ = fs::remove_file(&dest_path).await;
                    tokio::time::sleep(Duration::from_millis(200)).await;
                    continue;
                }
                _ => {
                    return Err(EnvrError::Download(format!(
                        "unexpected http status {status} for {url}"
                    )));
                }
            };

            if let (Some(total_atomic), Some(remainder)) =
                (progress_total.as_ref(), response.content_length())
            {
                total_atomic.store(
                    effective_resumed_from.saturating_add(remainder),
                    Ordering::Relaxed,
                );
            }

            if let Some(dl) = progress_downloaded.as_ref() {
                dl.store(effective_resumed_from, Ordering::Relaxed);
            }

            let mut file = OpenOptions::new()
                .create(true)
                .write(true)
                .append(append)
                .truncate(!append)
                .open(&dest_path)
                .await
                .map_err(EnvrError::from)?;

            let mut limiter = RateLimiter::new(options.max_bytes_per_sec);
            let global_limiter = options
                .global_limiter
                .clone()
                .or_else(global_download_limiter);
            let mut bytes_written = 0u64;

            let emit_state: Option<Arc<Mutex<(Instant, u64)>>> = on_progress
                .as_ref()
                .map(|_| Arc::new(Mutex::new((Instant::now(), 0u64))));

            let maybe_emit = |downloaded: u64, total: u64, force: bool| {
                let Some(cb) = on_progress.as_ref() else {
                    return;
                };
                let Some(st) = emit_state.as_ref() else {
                    return;
                };
                let now = Instant::now();
                let mut g = st.lock().expect("progress mutex");
                let byte_delta = downloaded.saturating_sub(g.1);
                if force
                    || now.duration_since(g.0) >= Duration::from_millis(200)
                    || byte_delta >= 256 * 1024
                {
                    cb(downloaded, total);
                    g.0 = now;
                    g.1 = downloaded;
                }
            };

            let mut stream = response.bytes_stream();
            while let Some(chunk) = stream.next().await {
                if cancel.is_cancelled() {
                    return Err(EnvrError::Download("download cancelled".to_string()));
                }
                let chunk =
                    chunk.map_err(|e| {
                        EnvrError::with_source(
                            ErrorCode::Download,
                            format!("read chunk failed for {url}"),
                            e,
                        )
                    })?;

                if let Some(gl) = global_limiter.as_ref() {
                    gl.throttle_async(chunk.len() as u64).await?;
                }
                limiter.throttle(chunk.len() as u64).await?;

                file.write_all(&chunk).await.map_err(EnvrError::from)?;
                bytes_written = bytes_written.saturating_add(chunk.len() as u64);
                if let Some(dl) = progress_downloaded.as_ref() {
                    dl.fetch_add(chunk.len() as u64, Ordering::Relaxed);
                }
                let downloaded = progress_downloaded
                    .as_ref()
                    .map(|d| d.load(Ordering::Relaxed))
                    .unwrap_or(effective_resumed_from.saturating_add(bytes_written));
                let total = progress_total
                    .as_ref()
                    .map(|t| t.load(Ordering::Relaxed))
                    .unwrap_or(0);
                maybe_emit(downloaded, total, false);
            }
            file.flush().await.map_err(EnvrError::from)?;

            let downloaded_final = progress_downloaded
                .as_ref()
                .map(|d| d.load(Ordering::Relaxed))
                .unwrap_or(effective_resumed_from.saturating_add(bytes_written));
            let total_final = progress_total
                .as_ref()
                .map(|t| t.load(Ordering::Relaxed))
                .unwrap_or(0);
            maybe_emit(downloaded_final, total_final, true);

            return Ok(DownloadOutcome {
                path: dest_path,
                bytes_written,
                resumed_from: effective_resumed_from,
            });
        }
    }
}

async fn existing_file_len(path: &Path) -> EnvrResult<u64> {
    match fs::metadata(path).await {
        Ok(m) => Ok(m.len()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(0),
        Err(e) => Err(EnvrError::from(e)),
    }
}

struct RateLimiter {
    max_bytes_per_sec: Option<u64>,
    window_start: Instant,
    window_bytes: u64,
}

impl RateLimiter {
    fn new(max_bytes_per_sec: Option<u64>) -> Self {
        Self {
            max_bytes_per_sec,
            window_start: Instant::now(),
            window_bytes: 0,
        }
    }

    async fn throttle(&mut self, incoming: u64) -> EnvrResult<()> {
        let Some(limit) = self.max_bytes_per_sec else {
            return Ok(());
        };
        if limit == 0 {
            return Err(EnvrError::Validation(
                "max_bytes_per_sec must be >= 1".to_string(),
            ));
        }

        let now = Instant::now();
        let elapsed = now.duration_since(self.window_start);
        if elapsed >= Duration::from_secs(1) {
            self.window_start = now;
            self.window_bytes = 0;
        }

        self.window_bytes = self.window_bytes.saturating_add(incoming);
        if self.window_bytes <= limit {
            return Ok(());
        }

        let sleep_for = Duration::from_secs(1).saturating_sub(elapsed);
        tokio::time::sleep(sleep_for).await;
        self.window_start = Instant::now();
        self.window_bytes = 0;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::CancelToken;
    use std::sync::atomic::AtomicU64;
    use tempfile::TempDir;
    use wiremock::matchers::{header, method, path as path_matcher};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn range_header_is_built_correctly() {
        let resumed_from = 123u64;
        let value = format!("bytes={resumed_from}-");
        assert_eq!(value, "bytes=123-");
    }

    #[tokio::test]
    async fn rate_limiter_rejects_zero_limit() {
        let mut lim = RateLimiter::new(Some(0));
        let err = lim.throttle(1).await.expect_err("should error");
        assert!(matches!(err, EnvrError::Validation(_)));
    }

    #[tokio::test]
    async fn existing_file_len_handles_present_and_missing() {
        let tmp = TempDir::new().expect("tmp");
        let missing = tmp.path().join("missing.bin");
        let n0 = existing_file_len(&missing).await.expect("len");
        assert_eq!(n0, 0);

        let present = tmp.path().join("present.bin");
        tokio::fs::write(&present, b"abcd").await.expect("write");
        let n1 = existing_file_len(&present).await.expect("len");
        assert_eq!(n1, 4);
    }

    #[test]
    fn default_client_can_be_built() {
        let _ = DownloadEngine::default_client().expect("client");
    }

    #[test]
    fn default_http_connect_timeout_is_thirty_seconds() {
        assert_eq!(DEFAULT_HTTP_CONNECT_TIMEOUT, Duration::from_secs(30));
    }

    #[test]
    fn default_download_options_are_sane() {
        let d = DownloadOptions::default();
        assert_eq!(d.timeout, Duration::from_secs(60));
        assert_eq!(d.max_bytes_per_sec, None);
    }

    #[tokio::test]
    async fn rate_limiter_no_limit_is_noop() {
        let mut lim = RateLimiter::new(None);
        lim.throttle(10_000).await.expect("noop");
    }

    #[tokio::test]
    async fn rate_limiter_under_limit_does_not_error() {
        let mut lim = RateLimiter::new(Some(1024));
        lim.throttle(128).await.expect("ok");
    }

    #[tokio::test]
    async fn download_to_file_writes_full_body_on_200() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_matcher("/file"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-length", "5")
                    .set_body_bytes(b"hello"),
            )
            .mount(&server)
            .await;

        let tmp = TempDir::new().expect("tmp");
        let dest = tmp.path().join("out.bin");
        let url = Url::parse(&format!("{}/file", server.uri())).expect("url");
        let cancel = CancelToken::new();
        let opts = DownloadOptions {
            timeout: Duration::from_secs(10),
            max_bytes_per_sec: None,
            global_limiter: None,
        };
        let out = DownloadEngine::new(Client::new())
            .download_to_file(url, &dest, &cancel, &opts, None, None, None)
            .await
            .expect("dl");
        assert_eq!(out.bytes_written, 5);
        assert_eq!(out.resumed_from, 0);
        let bytes = tokio::fs::read(&dest).await.expect("read");
        assert_eq!(bytes, b"hello");
    }

    #[tokio::test]
    async fn download_to_file_appends_on_206() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path_matcher("/resume"))
            .and(header("range", "bytes=2-"))
            .respond_with(
                ResponseTemplate::new(206)
                    .insert_header("content-length", "3")
                    .insert_header("content-range", "bytes 2-4/5")
                    .set_body_bytes(b"cde"),
            )
            .mount(&server)
            .await;

        let tmp = TempDir::new().expect("tmp");
        let dest = tmp.path().join("out.bin");
        tokio::fs::write(&dest, b"ab").await.expect("seed");
        let url = Url::parse(&format!("{}/resume", server.uri())).expect("url");
        let cancel = CancelToken::new();
        let opts = DownloadOptions {
            timeout: Duration::from_secs(10),
            max_bytes_per_sec: None,
            global_limiter: None,
        };
        let out = DownloadEngine::new(Client::new())
            .download_to_file(url, &dest, &cancel, &opts, None, None, None)
            .await
            .expect("dl");
        assert_eq!(out.bytes_written, 3);
        assert_eq!(out.resumed_from, 2);
        let bytes = tokio::fs::read(&dest).await.expect("read");
        assert_eq!(bytes, b"abcde");
    }

    #[tokio::test]
    async fn download_to_file_unexpected_status_errors() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let tmp = TempDir::new().expect("tmp");
        let dest = tmp.path().join("missing.bin");
        let url = Url::parse(&format!("{}/nope", server.uri())).expect("url");
        let err = DownloadEngine::new(Client::new())
            .download_to_file(
                url,
                &dest,
                &CancelToken::new(),
                &DownloadOptions::default(),
                None,
                None,
                None,
            )
            .await
            .expect_err("404");
        assert!(matches!(err, EnvrError::Download(_)));
    }

    #[tokio::test]
    async fn download_to_file_sets_progress_atomics() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-length", "4")
                    .set_body_bytes(b"abcd"),
            )
            .mount(&server)
            .await;

        let tmp = TempDir::new().expect("tmp");
        let dest = tmp.path().join("p.bin");
        let total = Arc::new(AtomicU64::new(0));
        let dl = Arc::new(AtomicU64::new(999));
        let url = Url::parse(&format!("{}/f", server.uri())).expect("url");
        DownloadEngine::new(Client::new())
            .download_to_file(
                url,
                &dest,
                &CancelToken::new(),
                &DownloadOptions::default(),
                Some(dl.clone()),
                Some(total.clone()),
                None,
            )
            .await
            .expect("dl");
        assert_eq!(total.load(Ordering::Relaxed), 4);
        assert_eq!(dl.load(Ordering::Relaxed), 4);
    }

    #[tokio::test]
    async fn download_to_file_invokes_on_progress() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-length", "4")
                    .set_body_bytes(b"abcd"),
            )
            .mount(&server)
            .await;

        let tmp = TempDir::new().expect("tmp");
        let dest = tmp.path().join("cb.bin");
        let hits = Arc::new(AtomicU64::new(0));
        let h2 = hits.clone();
        let cb: DownloadProgressFn = Arc::new(move |_, _| {
            h2.fetch_add(1, Ordering::Relaxed);
        });
        let url = Url::parse(&format!("{}/cb", server.uri())).expect("url");
        DownloadEngine::new(Client::new())
            .download_to_file(
                url,
                &dest,
                &CancelToken::new(),
                &DownloadOptions::default(),
                None,
                None,
                Some(cb),
            )
            .await
            .expect("dl");
        assert!(hits.load(Ordering::Relaxed) >= 1);
    }

    #[tokio::test]
    async fn download_to_file_precancel_errors_before_write() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"x"))
            .mount(&server)
            .await;

        let tmp = TempDir::new().expect("tmp");
        let dest = tmp.path().join("out.bin");
        let cancel = CancelToken::new();
        cancel.cancel();
        let url = Url::parse(&format!("{}/x", server.uri())).expect("url");
        let err = DownloadEngine::new(Client::new())
            .download_to_file(
                url,
                &dest,
                &cancel,
                &DownloadOptions::default(),
                None,
                None,
                None,
            )
            .await
            .expect_err("cancelled");
        let EnvrError::Download(msg) = err else {
            panic!("expected Download error");
        };
        assert!(msg.contains("cancelled"));
    }
}
