use envr_download::task::CancelToken;
use envr_ui::theme::ThemeTokens;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

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
    /// HTTP(S) source, or empty for GUI runtime **install** tasks (no URL line; see panel UI).
    pub url: String,
    pub state: JobState,
    pub cancellable: bool,
    pub downloaded: Arc<AtomicU64>,
    pub total: Arc<AtomicU64>,
    pub cancel: CancelToken,
    pub last_error: Option<String>,
    pub tick_prev_bytes: u64,
    pub tick_prev_at: Option<Instant>,
    pub speed_bps: f64,
}

impl DownloadJob {
    fn has_no_transfer_stats(&self) -> bool {
        self.downloaded_display() == 0 && self.total_display() == 0
    }

    /// Runtime install row: no URL line in the panel (bytes still update via atomics).
    pub fn is_runtime_install_row(&self) -> bool {
        self.url.is_empty()
    }

    /// Resolve / connect / wait for first byte — before `Content-Length` or any chunk.
    pub fn is_install_warmup_phase(&self) -> bool {
        self.state == JobState::Running
            && self.is_runtime_install_row()
            && self.has_no_transfer_stats()
    }

    /// Done install that never reported transfer (edge case); show a short status line.
    pub fn is_local_install_done_minimal(&self) -> bool {
        self.state == JobState::Done
            && self.is_runtime_install_row()
            && self.has_no_transfer_stats()
    }

    pub fn progress_ratio(&self) -> f32 {
        if self.is_install_warmup_phase() {
            return 0.0;
        }
        if self.is_runtime_install_row() && self.has_no_transfer_stats() {
            return match self.state {
                JobState::Done => 100.0,
                JobState::Failed | JobState::Cancelled => 0.0,
                JobState::Running => 0.0,
            };
        }
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
        // Need a known total and a non-zero speed estimate (tick updates ~4 Hz).
        // Avoid a high floor (e.g. 256 B/s) so slow links and runtime installs still show ETA.
        if t == 0 || d >= t || self.speed_bps <= 0.0 {
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

/// Long-press duration before a title-bar drag arms (`tasks_gui.md` GUI-061).
pub const TITLE_DRAG_HOLD: Duration = Duration::from_millis(280);

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
    /// Left button down on the title / drag strip — drag starts after [`TITLE_DRAG_HOLD`].
    pub title_drag_armed_since: Option<Instant>,
    /// Latest pointer (window coords) while tracking a title drag.
    pub last_drag_pointer: Option<(f32, f32)>,
}

impl Default for DownloadPanelState {
    fn default() -> Self {
        Self {
            jobs: Vec::new(),
            next_id: 1,
            expanded: false,
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
            title_drag_armed_since: None,
            last_drag_pointer: None,
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
        if self.progress_throttle_frames.is_multiple_of(13) {
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
