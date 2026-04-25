use envr_error::{EnvrError, EnvrResult};
use std::sync::{Arc, Condvar, Mutex, OnceLock, RwLock};
use std::time::{Duration, Instant};
use crate::stats::{
    dec_async_in_flight, dec_blocking_in_flight, inc_async_in_flight, inc_blocking_in_flight,
    record_async_queue_wait_micros, record_blocking_queue_wait_micros,
};

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

#[derive(Debug)]
struct ConcurrencyState {
    in_flight: usize,
    waiting_index: usize,
    waiting_artifact: usize,
    waiting_prefetch: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadPriority {
    Index,
    Artifact,
    Prefetch,
}

#[derive(Debug)]
pub struct GlobalDownloadConcurrencyLimiter {
    max_in_flight: usize,
    state: Mutex<ConcurrencyState>,
    cv: Condvar,
}

impl GlobalDownloadConcurrencyLimiter {
    pub fn new(max_in_flight: usize) -> EnvrResult<Self> {
        if max_in_flight == 0 {
            return Err(EnvrError::Validation(
                "max_in_flight must be >= 1".to_string(),
            ));
        }
        Ok(Self {
            max_in_flight,
            state: Mutex::new(ConcurrencyState {
                in_flight: 0,
                waiting_index: 0,
                waiting_artifact: 0,
                waiting_prefetch: 0,
            }),
            cv: Condvar::new(),
        })
    }

    pub fn max_in_flight(&self) -> usize {
        self.max_in_flight
    }

    fn can_acquire(state: &ConcurrencyState, max_in_flight: usize, priority: DownloadPriority) -> bool {
        if state.in_flight >= max_in_flight {
            return false;
        }
        match priority {
            DownloadPriority::Index => true,
            DownloadPriority::Artifact => state.waiting_index == 0,
            DownloadPriority::Prefetch => state.waiting_index == 0 && state.waiting_artifact == 0,
        }
    }

    fn waiting_slot_mut(state: &mut ConcurrencyState, priority: DownloadPriority) -> &mut usize {
        match priority {
            DownloadPriority::Index => &mut state.waiting_index,
            DownloadPriority::Artifact => &mut state.waiting_artifact,
            DownloadPriority::Prefetch => &mut state.waiting_prefetch,
        }
    }

    fn acquire_inner(&self, priority: DownloadPriority) -> u64 {
        let t0 = Instant::now();
        let mut g = self.state.lock().expect("download concurrency mutex");
        *Self::waiting_slot_mut(&mut g, priority) += 1;
        while !Self::can_acquire(&g, self.max_in_flight, priority) {
            g = self.cv.wait(g).expect("download concurrency cv");
        }
        {
            let slot = Self::waiting_slot_mut(&mut g, priority);
            *slot = slot.saturating_sub(1);
        }
        g.in_flight += 1;
        drop(g);
        t0.elapsed().as_micros() as u64
    }

    #[cfg(test)]
    fn waiting_snapshot(&self) -> (usize, usize, usize) {
        let g = self.state.lock().expect("download concurrency mutex");
        (g.waiting_index, g.waiting_artifact, g.waiting_prefetch)
    }

    pub fn acquire_blocking(self: &Arc<Self>, priority: DownloadPriority) -> GlobalDownloadPermit {
        let waited_micros = self.acquire_inner(priority);
        record_blocking_queue_wait_micros(waited_micros);
        inc_blocking_in_flight();
        GlobalDownloadPermit {
            limiter: Arc::clone(self),
            mode: PermitMode::Blocking,
        }
    }

    pub async fn acquire_async(
        self: &Arc<Self>,
        priority: DownloadPriority,
    ) -> EnvrResult<GlobalDownloadPermit> {
        let this = Arc::clone(self);
        let waited_micros = tokio::task::spawn_blocking(move || this.acquire_inner(priority))
            .await
            .map_err(|e| {
                EnvrError::with_source(
                    envr_error::ErrorCode::Download,
                    "async download queue worker join failed",
                    e,
                )
            })?;
        record_async_queue_wait_micros(waited_micros);
        inc_async_in_flight();
        Ok(GlobalDownloadPermit {
            limiter: Arc::clone(self),
            mode: PermitMode::Async,
        })
    }

    fn release_blocking(&self) {
        let mut g = self.state.lock().expect("download concurrency mutex");
        g.in_flight = g.in_flight.saturating_sub(1);
        drop(g);
        self.cv.notify_one();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PermitMode {
    Blocking,
    Async,
}

pub struct GlobalDownloadPermit {
    limiter: Arc<GlobalDownloadConcurrencyLimiter>,
    mode: PermitMode,
}

impl Drop for GlobalDownloadPermit {
    fn drop(&mut self) {
        match self.mode {
            PermitMode::Blocking => dec_blocking_in_flight(),
            PermitMode::Async => dec_async_in_flight(),
        }
        self.limiter.release_blocking();
    }
}

static GLOBAL_CONCURRENCY: OnceLock<RwLock<Option<Arc<GlobalDownloadConcurrencyLimiter>>>> =
    OnceLock::new();

fn global_concurrency_cell() -> &'static RwLock<Option<Arc<GlobalDownloadConcurrencyLimiter>>> {
    GLOBAL_CONCURRENCY.get_or_init(|| RwLock::new(None))
}

pub fn set_global_download_concurrency_limit(max_in_flight: Option<usize>) -> EnvrResult<()> {
    let mut g = global_concurrency_cell()
        .write()
        .expect("global concurrency write lock");
    *g = match max_in_flight {
        None | Some(0) => None,
        Some(n) => Some(Arc::new(GlobalDownloadConcurrencyLimiter::new(n)?)),
    };
    Ok(())
}

pub fn global_download_concurrency_limiter() -> Option<Arc<GlobalDownloadConcurrencyLimiter>> {
    global_concurrency_cell()
        .read()
        .ok()
        .and_then(|g| g.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_acquire_respects_priority_under_contention() {
        let st = ConcurrencyState {
            in_flight: 0,
            waiting_index: 1,
            waiting_artifact: 1,
            waiting_prefetch: 1,
        };
        assert!(GlobalDownloadConcurrencyLimiter::can_acquire(
            &st,
            4,
            DownloadPriority::Index
        ));
        assert!(!GlobalDownloadConcurrencyLimiter::can_acquire(
            &st,
            4,
            DownloadPriority::Artifact
        ));
        assert!(!GlobalDownloadConcurrencyLimiter::can_acquire(
            &st,
            4,
            DownloadPriority::Prefetch
        ));
    }

    #[tokio::test]
    async fn acquire_async_and_blocking_roundtrip() {
        let lim = Arc::new(GlobalDownloadConcurrencyLimiter::new(1).expect("limiter"));
        let p1 = lim.acquire_async(DownloadPriority::Index).await.expect("async");
        drop(p1);
        let p2 = lim.acquire_blocking(DownloadPriority::Artifact);
        drop(p2);
        let (_wi, _wa, _wp) = lim.waiting_snapshot();
    }
}
