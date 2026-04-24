use envr_error::{EnvrError, EnvrResult};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::{Duration, Instant};

#[derive(Debug)]
struct WindowState {
    window_start: Instant,
    window_bytes: u64,
}

#[derive(Debug)]
pub struct GlobalRateLimiter {
    max_bytes_per_sec: u64,
    state: Mutex<WindowState>,
}

impl GlobalRateLimiter {
    pub fn new(max_bytes_per_sec: u64) -> EnvrResult<Self> {
        if max_bytes_per_sec == 0 {
            return Err(EnvrError::Validation(
                "max_bytes_per_sec must be >= 1".to_string(),
            ));
        }
        Ok(Self {
            max_bytes_per_sec,
            state: Mutex::new(WindowState {
                window_start: Instant::now(),
                window_bytes: 0,
            }),
        })
    }

    pub fn max_bytes_per_sec(&self) -> u64 {
        self.max_bytes_per_sec
    }

    fn compute_sleep(&self, incoming: u64) -> EnvrResult<Option<Duration>> {
        if incoming == 0 {
            return Ok(None);
        }
        let mut g = self.state.lock().expect("global rate limiter mutex");
        let now = Instant::now();
        let elapsed = now.duration_since(g.window_start);
        if elapsed >= Duration::from_secs(1) {
            g.window_start = now;
            g.window_bytes = 0;
        }

        g.window_bytes = g.window_bytes.saturating_add(incoming);
        if g.window_bytes <= self.max_bytes_per_sec {
            return Ok(None);
        }

        let sleep_for = Duration::from_secs(1).saturating_sub(elapsed);
        // Reset window after the sleep (next caller starts a fresh window).
        g.window_start = now + sleep_for;
        g.window_bytes = 0;
        Ok(Some(sleep_for))
    }

    pub fn throttle_blocking(&self, incoming: u64) -> EnvrResult<()> {
        if let Some(d) = self.compute_sleep(incoming)? {
            std::thread::sleep(d);
        }
        Ok(())
    }

    pub async fn throttle_async(&self, incoming: u64) -> EnvrResult<()> {
        if let Some(d) = self.compute_sleep(incoming)? {
            tokio::time::sleep(d).await;
        }
        Ok(())
    }
}

static GLOBAL_LIMITER: OnceLock<RwLock<Option<Arc<GlobalRateLimiter>>>> = OnceLock::new();

fn global_cell() -> &'static RwLock<Option<Arc<GlobalRateLimiter>>> {
    GLOBAL_LIMITER.get_or_init(|| RwLock::new(None))
}

pub fn set_global_download_limit(max_bytes_per_sec: Option<u64>) -> EnvrResult<()> {
    let mut g = global_cell().write().expect("global limiter write lock");
    *g = match max_bytes_per_sec {
        None | Some(0) => None,
        Some(n) => Some(Arc::new(GlobalRateLimiter::new(n)?)),
    };
    Ok(())
}

pub fn global_download_limiter() -> Option<Arc<GlobalRateLimiter>> {
    global_cell().read().ok().and_then(|g| g.clone())
}
