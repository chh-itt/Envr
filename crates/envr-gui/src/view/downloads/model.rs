use envr_download::task::CancelToken;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobState {
    Running,
    Done,
    Failed,
    Cancelled,
}

pub struct DownloadJob {
    pub id: u64,
    pub label: String,
    pub url: String,
    pub state: JobState,
    pub downloaded: Arc<AtomicU64>,
    pub total: Arc<AtomicU64>,
    pub cancel: CancelToken,
    pub last_error: Option<String>,
    pub tick_prev_bytes: u64,
    pub tick_prev_at: Option<Instant>,
    pub speed_bps: f64,
}

impl DownloadJob {
    pub fn progress_ratio(&self) -> f32 {
        let d = self.downloaded.load(Ordering::Relaxed);
        let t = self.total.load(Ordering::Relaxed);
        if t == 0 {
            return 0.0;
        }
        ((d as f64 / t as f64) * 100.0).min(100.0) as f32
    }

    pub fn downloaded_display(&self) -> u64 {
        self.downloaded.load(Ordering::Relaxed)
    }

    pub fn total_display(&self) -> u64 {
        self.total.load(Ordering::Relaxed)
    }

    pub fn eta_secs(&self) -> Option<u64> {
        let d = self.downloaded_display();
        let t = self.total_display();
        if t <= d || self.speed_bps < 256.0 {
            return None;
        }
        let remain = (t - d) as f64;
        Some((remain / self.speed_bps).ceil() as u64)
    }
}

pub struct DownloadPanelState {
    pub jobs: Vec<DownloadJob>,
    pub next_id: u64,
    pub expanded: bool,
}

impl Default for DownloadPanelState {
    fn default() -> Self {
        Self {
            jobs: Vec::new(),
            next_id: 1,
            expanded: true,
        }
    }
}

impl DownloadPanelState {
    pub fn on_tick(&mut self) {
        let now = Instant::now();
        for j in &mut self.jobs {
            if j.state != JobState::Running {
                continue;
            }
            let b = j.downloaded.load(Ordering::Relaxed);
            match j.tick_prev_at {
                None => {
                    j.tick_prev_at = Some(now);
                    j.tick_prev_bytes = b;
                }
                Some(t0) => {
                    let dt = now.duration_since(t0).as_secs_f64();
                    if dt >= 0.25 {
                        j.speed_bps = if b >= j.tick_prev_bytes {
                            (b - j.tick_prev_bytes) as f64 / dt
                        } else {
                            0.0
                        };
                        j.tick_prev_bytes = b;
                        j.tick_prev_at = Some(now);
                    }
                }
            }
        }
    }
}
