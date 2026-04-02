use envr_download::task::CancelToken;
use envr_ui::theme::ThemeTokens;
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

#[derive(Debug, Clone)]
pub struct PanelRevealAnim {
    pub from: f32,
    pub to: f32,
    pub started: Instant,
    pub duration_ms: u16,
}

pub struct DownloadPanelState {
    pub jobs: Vec<DownloadJob>,
    pub next_id: u64,
    pub expanded: bool,
    /// User preference: panel should be open (persisted).
    pub visible: bool,
    /// Visual progress 0..1 for fade / slide (`tasks_gui.md` GUI-042).
    pub reveal: f32,
    pub reveal_anim: Option<PanelRevealAnim>,
    pub persist_after_hide_anim: bool,
    /// Frames counted during fast ticks (~32ms) to throttle job progress refresh (~400ms).
    pub progress_throttle_frames: u32,
    /// Left offset (px) from window left edge.
    pub x: i32,
    /// Bottom offset (px) from window bottom edge.
    pub y: i32,
    pub dragging: bool,
    pub drag_from_cursor: Option<(f32, f32)>,
    pub drag_from_pos: Option<(i32, i32)>,
}

impl Default for DownloadPanelState {
    fn default() -> Self {
        Self {
            jobs: Vec::new(),
            next_id: 1,
            expanded: true,
            visible: true,
            reveal: 1.0,
            reveal_anim: None,
            persist_after_hide_anim: false,
            progress_throttle_frames: 0,
            x: 12,
            y: 12,
            dragging: false,
            drag_from_cursor: None,
            drag_from_pos: None,
        }
    }
}

impl DownloadPanelState {
    pub fn has_running_jobs(&self) -> bool {
        self.jobs.iter().any(|j| j.state == JobState::Running)
    }

    /// Fast UI tick: reveal animation or throttled progress while motion subscription runs.
    pub fn needs_motion_tick(&self) -> bool {
        self.reveal_anim.is_some()
    }

    pub fn needs_tick(&self) -> bool {
        // Tick exists to refresh progress/speed UI. When panel is hidden, we can
        // stop re-rendering to reduce CPU usage; the final Finished event will
        // still update job state.
        self.visible && self.has_running_jobs()
    }

    pub fn start_show_anim(&mut self, tokens: ThemeTokens) {
        self.visible = true;
        if (self.reveal - 1.0).abs() < 0.002 && self.reveal_anim.is_none() {
            self.reveal = 1.0;
            return;
        }
        self.reveal_anim = Some(PanelRevealAnim {
            from: self.reveal,
            to: 1.0,
            started: Instant::now(),
            duration_ms: tokens.motion.standard_ms,
        });
    }

    pub fn start_hide_anim(&mut self, tokens: ThemeTokens) {
        if !self.visible && self.reveal_anim.is_none() {
            return;
        }
        self.reveal_anim = Some(PanelRevealAnim {
            from: self.reveal,
            to: 0.0,
            started: Instant::now(),
            duration_ms: tokens.motion.standard_ms,
        });
    }

    pub fn advance_reveal(&mut self, tokens: ThemeTokens) {
        let anim = match self.reveal_anim.clone() {
            Some(a) => a,
            None => return,
        };
        if anim.duration_ms == 0 {
            self.reveal = anim.to;
            let was_hide = anim.to < 0.05;
            self.reveal_anim = None;
            if was_hide {
                self.visible = false;
                self.persist_after_hide_anim = true;
            }
            return;
        }
        let elapsed_ms = anim.started.elapsed().as_secs_f32() * 1000.0;
        let t = (elapsed_ms / anim.duration_ms as f32).min(1.0);
        let k = tokens.ease_standard(t);
        self.reveal = anim.from + (anim.to - anim.from) * k;
        if t >= 1.0 - 1e-5 {
            self.reveal = anim.to;
            let was_hide = anim.to < 0.05;
            self.reveal_anim = None;
            if was_hide {
                self.visible = false;
                self.persist_after_hide_anim = true;
            }
        }
    }

    pub fn take_persist_after_hide(&mut self) -> bool {
        std::mem::take(&mut self.persist_after_hide_anim)
    }

    /// Call from ~32ms motion tick when [`needs_motion_tick`] or throttling is active.
    pub fn maybe_progress_tick_on_motion_frame(&mut self) {
        if !self.visible || !self.has_running_jobs() {
            self.progress_throttle_frames = 0;
            return;
        }
        self.progress_throttle_frames = self.progress_throttle_frames.wrapping_add(1);
        if self.progress_throttle_frames % 13 == 0 {
            self.on_tick();
        }
    }

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
