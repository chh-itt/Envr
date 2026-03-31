use crate::task::CancelToken;
use envr_error::{EnvrError, EnvrResult};
use futures::StreamExt;
use reqwest::{Client, StatusCode, Url, header};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use tokio::{
    fs::{self, OpenOptions},
    io::AsyncWriteExt,
};

#[derive(Debug, Clone)]
pub struct DownloadOptions {
    pub timeout: Duration,
    pub max_bytes_per_sec: Option<u64>,
}

impl Default for DownloadOptions {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(60),
            max_bytes_per_sec: None,
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
            .build()
            .map_err(|e| EnvrError::Download(format!("reqwest client build failed: {e}")))
    }

    pub async fn download_to_file(
        &self,
        url: Url,
        dest_path: impl AsRef<Path>,
        cancel: &CancelToken,
        options: &DownloadOptions,
    ) -> EnvrResult<DownloadOutcome> {
        let dest_path = dest_path.as_ref().to_path_buf();
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
            .map_err(|e| EnvrError::Download(format!("request failed: {e}")))?;

        let status = response.status();
        let (append, effective_resumed_from) = match status {
            StatusCode::OK => (false, 0),
            StatusCode::PARTIAL_CONTENT => (true, resumed_from),
            _ => {
                return Err(EnvrError::Download(format!(
                    "unexpected http status {status} for {url}"
                )));
            }
        };

        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .append(append)
            .truncate(!append)
            .open(&dest_path)
            .await
            .map_err(EnvrError::from)?;

        let mut limiter = RateLimiter::new(options.max_bytes_per_sec);
        let mut bytes_written = 0u64;

        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            if cancel.is_cancelled() {
                return Err(EnvrError::Download("download cancelled".to_string()));
            }
            let chunk =
                chunk.map_err(|e| EnvrError::Download(format!("read chunk failed: {e}")))?;

            limiter.throttle(chunk.len() as u64).await?;

            file.write_all(&chunk).await.map_err(EnvrError::from)?;
            bytes_written = bytes_written.saturating_add(chunk.len() as u64);
        }
        file.flush().await.map_err(EnvrError::from)?;

        Ok(DownloadOutcome {
            path: dest_path,
            bytes_written,
            resumed_from: effective_resumed_from,
        })
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
}
