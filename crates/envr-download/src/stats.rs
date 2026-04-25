use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DownloadControlPlaneStats {
    pub blocking_pool_hits: u64,
    pub blocking_pool_misses: u64,
    pub async_pool_hits: u64,
    pub async_pool_misses: u64,
    pub retry_scheduled: u64,
    pub blocking_queue_wait_events: u64,
    pub blocking_queue_wait_total_micros: u64,
    pub async_queue_wait_events: u64,
    pub async_queue_wait_total_micros: u64,
    pub blocking_in_flight: usize,
    pub blocking_in_flight_peak: usize,
    pub async_in_flight: usize,
    pub async_in_flight_peak: usize,
}

static BLOCKING_POOL_HITS: AtomicU64 = AtomicU64::new(0);
static BLOCKING_POOL_MISSES: AtomicU64 = AtomicU64::new(0);
static ASYNC_POOL_HITS: AtomicU64 = AtomicU64::new(0);
static ASYNC_POOL_MISSES: AtomicU64 = AtomicU64::new(0);
static RETRY_SCHEDULED: AtomicU64 = AtomicU64::new(0);

static BLOCKING_QUEUE_WAIT_EVENTS: AtomicU64 = AtomicU64::new(0);
static BLOCKING_QUEUE_WAIT_TOTAL_MICROS: AtomicU64 = AtomicU64::new(0);
static ASYNC_QUEUE_WAIT_EVENTS: AtomicU64 = AtomicU64::new(0);
static ASYNC_QUEUE_WAIT_TOTAL_MICROS: AtomicU64 = AtomicU64::new(0);

static BLOCKING_IN_FLIGHT: AtomicUsize = AtomicUsize::new(0);
static BLOCKING_IN_FLIGHT_PEAK: AtomicUsize = AtomicUsize::new(0);
static ASYNC_IN_FLIGHT: AtomicUsize = AtomicUsize::new(0);
static ASYNC_IN_FLIGHT_PEAK: AtomicUsize = AtomicUsize::new(0);

fn update_peak(peak: &AtomicUsize, cur: usize) {
    let mut old = peak.load(Ordering::Relaxed);
    while cur > old {
        match peak.compare_exchange_weak(old, cur, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => break,
            Err(next) => old = next,
        }
    }
}

pub fn record_blocking_pool_hit() {
    BLOCKING_POOL_HITS.fetch_add(1, Ordering::Relaxed);
}
pub fn record_blocking_pool_miss() {
    BLOCKING_POOL_MISSES.fetch_add(1, Ordering::Relaxed);
}
pub fn record_async_pool_hit() {
    ASYNC_POOL_HITS.fetch_add(1, Ordering::Relaxed);
}
pub fn record_async_pool_miss() {
    ASYNC_POOL_MISSES.fetch_add(1, Ordering::Relaxed);
}
pub fn record_retry_scheduled() {
    RETRY_SCHEDULED.fetch_add(1, Ordering::Relaxed);
}

pub fn record_blocking_queue_wait_micros(micros: u64) {
    BLOCKING_QUEUE_WAIT_EVENTS.fetch_add(1, Ordering::Relaxed);
    BLOCKING_QUEUE_WAIT_TOTAL_MICROS.fetch_add(micros, Ordering::Relaxed);
}
pub fn record_async_queue_wait_micros(micros: u64) {
    ASYNC_QUEUE_WAIT_EVENTS.fetch_add(1, Ordering::Relaxed);
    ASYNC_QUEUE_WAIT_TOTAL_MICROS.fetch_add(micros, Ordering::Relaxed);
}

pub fn inc_blocking_in_flight() {
    let cur = BLOCKING_IN_FLIGHT.fetch_add(1, Ordering::Relaxed) + 1;
    update_peak(&BLOCKING_IN_FLIGHT_PEAK, cur);
}
pub fn dec_blocking_in_flight() {
    BLOCKING_IN_FLIGHT.fetch_sub(1, Ordering::Relaxed);
}
pub fn inc_async_in_flight() {
    let cur = ASYNC_IN_FLIGHT.fetch_add(1, Ordering::Relaxed) + 1;
    update_peak(&ASYNC_IN_FLIGHT_PEAK, cur);
}
pub fn dec_async_in_flight() {
    ASYNC_IN_FLIGHT.fetch_sub(1, Ordering::Relaxed);
}

pub fn snapshot_download_control_plane_stats() -> DownloadControlPlaneStats {
    DownloadControlPlaneStats {
        blocking_pool_hits: BLOCKING_POOL_HITS.load(Ordering::Relaxed),
        blocking_pool_misses: BLOCKING_POOL_MISSES.load(Ordering::Relaxed),
        async_pool_hits: ASYNC_POOL_HITS.load(Ordering::Relaxed),
        async_pool_misses: ASYNC_POOL_MISSES.load(Ordering::Relaxed),
        retry_scheduled: RETRY_SCHEDULED.load(Ordering::Relaxed),
        blocking_queue_wait_events: BLOCKING_QUEUE_WAIT_EVENTS.load(Ordering::Relaxed),
        blocking_queue_wait_total_micros: BLOCKING_QUEUE_WAIT_TOTAL_MICROS.load(Ordering::Relaxed),
        async_queue_wait_events: ASYNC_QUEUE_WAIT_EVENTS.load(Ordering::Relaxed),
        async_queue_wait_total_micros: ASYNC_QUEUE_WAIT_TOTAL_MICROS.load(Ordering::Relaxed),
        blocking_in_flight: BLOCKING_IN_FLIGHT.load(Ordering::Relaxed),
        blocking_in_flight_peak: BLOCKING_IN_FLIGHT_PEAK.load(Ordering::Relaxed),
        async_in_flight: ASYNC_IN_FLIGHT.load(Ordering::Relaxed),
        async_in_flight_peak: ASYNC_IN_FLIGHT_PEAK.load(Ordering::Relaxed),
    }
}
