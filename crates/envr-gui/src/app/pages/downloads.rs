use super::super::*;

use iced::Task;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use crate::download_runner;
use crate::view::downloads::{DownloadJob, DownloadJobPayload, DownloadMsg, JobState};

const CANCEL_SETTLE_TIMEOUT: Duration = Duration::from_secs(15);

pub(crate) fn handle_download(state: &mut AppState, msg: DownloadMsg) -> Task<Message> {
    match msg {
        DownloadMsg::Tick => {
            state.downloads.on_tick();
            settle_stuck_cancelling_jobs(state)
        }
        DownloadMsg::ToggleVisible => {
            let tokens = state.tokens();
            if state.downloads.visible && state.downloads.reveal_anim.is_none() {
                state.downloads.start_hide_anim(tokens);
            } else if !state.downloads.visible {
                state.downloads.start_show_anim(tokens);
                let _ = persist_download_panel_settings(state);
            }
            Task::none()
        }
        DownloadMsg::ToggleExpand => {
            state.downloads.expanded = !state.downloads.expanded;
            let _ = persist_download_panel_settings(state);
            Task::none()
        }
        DownloadMsg::TitleBarPress => {
            state.downloads.title_drag_armed_since = Some(std::time::Instant::now());
            state.downloads.last_drag_pointer = None;
            Task::none()
        }
        DownloadMsg::Event(e) => {
            use iced::Event;
            use iced::mouse;
            match e {
                Event::Mouse(mouse::Event::CursorMoved { position }) => {
                    let (cx, cy) = (position.x, position.y);
                    if state.downloads.title_drag_armed_since.is_some() && !state.downloads.dragging
                    {
                        state.downloads.last_drag_pointer = Some((cx, cy));
                    }
                    if !state.downloads.dragging {
                        return Task::none();
                    }
                    if state.downloads.drag_from_cursor.is_none() {
                        state.downloads.drag_from_cursor = Some((cx, cy));
                        return Task::none();
                    }
                    let (sx, sy) = state.downloads.drag_from_cursor.unwrap();
                    let (px, py) = state
                        .downloads
                        .drag_from_pos
                        .unwrap_or((state.downloads.x, state.downloads.y));
                    // Interpret y as bottom offset; moving cursor down decreases bottom offset.
                    let dx = cx - sx;
                    let dy = cy - sy;
                    state.downloads.x = (px + dx.round() as i32).max(0);
                    state.downloads.y = (py - dy.round() as i32).max(0);
                    super::super::clamp_download_panel_to_window(state);
                    Task::none()
                }
                Event::Mouse(mouse::Event::ButtonReleased(_btn)) => {
                    if state.downloads.dragging {
                        state.downloads.dragging = false;
                        state.downloads.drag_from_cursor = None;
                        state.downloads.drag_from_pos = None;
                        let _ = persist_download_panel_settings(state);
                    }
                    state.downloads.title_drag_armed_since = None;
                    Task::none()
                }
                _ => Task::none(),
            }
        }
        DownloadMsg::EnqueueDemo => enqueue_demo_download(state),
        DownloadMsg::Finished { id, result } => {
            if let Some(j) = state.downloads.jobs.iter_mut().find(|j| j.id == id) {
                if j.state == JobState::Cancelled && j.cancel.is_cancelled() {
                    return Task::none();
                }
                match &result {
                    Ok(_) => {
                        j.state = JobState::Done;
                        j.cancel_settled_by_timeout = false;
                        let d = j.downloaded.load(Ordering::Relaxed);
                        let t = j.total.load(Ordering::Relaxed);
                        if t == 0 || d < t {
                            j.total.store(d.max(t), Ordering::Relaxed);
                            j.downloaded.store(d.max(t), Ordering::Relaxed);
                        }
                    }
                    Err(e) => {
                        if looks_like_user_cancelled(e) {
                            j.state = JobState::Cancelled;
                            j.cancel_settled_by_timeout = false;
                            j.last_error = None;
                        } else {
                            j.state = JobState::Failed;
                            j.cancel_settled_by_timeout = false;
                            j.last_error = Some(e.clone());
                        }
                    }
                }
            }
            Task::none()
        }
        DownloadMsg::Cancel(id) => {
            if let Some(j) = state.downloads.jobs.iter_mut().find(|j| j.id == id) {
                if j.cancel.is_cancelled() {
                    return Task::none();
                }
                j.cancel.cancel();
                j.cancel_requested_at = Some(Instant::now());
                if j.state == JobState::Queued {
                    j.state = JobState::Cancelled;
                    j.cancel_settled_by_timeout = false;
                    j.last_error = None;
                }
            }
            maybe_start_queued_jobs(state)
        }
        DownloadMsg::Retry(id) => {
            let Some(failed) = state
                .downloads
                .jobs
                .iter()
                .find(|j| j.id == id && j.state == JobState::Failed)
                .map(|j| (j.url.clone(), j.label.clone()))
            else {
                return Task::none();
            };
            let (url_str, label) = failed;
            state.downloads.jobs.retain(|j| j.id != id);
            retry_download(
                state,
                &url_str,
                &format!(
                    "{label} {}",
                    envr_core::i18n::tr_key("gui.action.retry_suffix", "(重试)", "(retry)")
                ),
            )
        }
    }
}

pub(crate) fn enqueue_demo_download(state: &mut AppState) -> Task<Message> {
    retry_download(
        state,
        download_runner::DEMO_URL,
        &format!(
            "{} #{}",
            envr_core::i18n::tr_key("gui.label.demo", "演示", "Demo"),
            state.downloads.next_id
        ),
    )
}

pub(crate) fn retry_download(state: &mut AppState, url_str: &str, label: &str) -> Task<Message> {
    if url_str.trim().is_empty() {
        state.error = Some(envr_core::i18n::tr_key(
            "gui.error.retry_requires_url",
            "该任务没有可重试的下载 URL，请回到运行时页面重新安装。",
            "This task has no retryable download URL. Please retry install from runtime page.",
        ));
        return Task::none();
    }
    // Validate URL early so queued jobs don't fail later with a generic message.
    if let Err(e) = reqwest::Url::parse(url_str) {
        state.error = Some(format!(
            "{}: {e}",
            envr_core::i18n::tr_key(
                "gui.error.url_parse_failed",
                "URL 解析失败",
                "URL parse failed",
            )
        ));
        return Task::none();
    }
    let id = state.downloads.next_id;
    state.downloads.next_id += 1;
    let dest = std::env::temp_dir().join(format!("envr-gui-dl-{id}.tmp"));
    let downloaded = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let total = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let cancel = envr_download::task::CancelToken::new();
    state.downloads.jobs.push(DownloadJob {
        id,
        label: label.to_string(),
        url: url_str.to_string(),
        state: JobState::Queued,
        cancellable: true,
        downloaded: downloaded.clone(),
        total: total.clone(),
        cancel: cancel.clone(),
        cancel_requested_at: None,
        cancel_settled_by_timeout: false,
        last_error: None,
        tick_prev_bytes: 0,
        tick_prev_at: None,
        speed_bps: 0.0,
        payload: Some(DownloadJobPayload::HttpDownload {
            url: url_str.to_string(),
            dest,
        }),
    });
    maybe_start_queued_jobs(state)
}

pub(crate) fn enqueue_runtime_install_job(
    state: &mut AppState,
    label: String,
    cancellable: bool,
    payload: DownloadJobPayload,
) -> (
    u64,
    std::sync::Arc<std::sync::atomic::AtomicU64>,
    std::sync::Arc<std::sync::atomic::AtomicU64>,
    envr_download::task::CancelToken,
) {
    let id = state.downloads.next_id;
    state.downloads.next_id += 1;
    let downloaded = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let total = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let cancel = envr_download::task::CancelToken::new();
    // Empty `url` marks a local install task (see downloads panel / `format_job_state_line`).
    state.downloads.jobs.push(DownloadJob {
        id,
        label,
        url: String::new(),
        state: JobState::Queued,
        cancellable,
        downloaded: downloaded.clone(),
        total: total.clone(),
        cancel: cancel.clone(),
        cancel_requested_at: None,
        cancel_settled_by_timeout: false,
        last_error: None,
        tick_prev_bytes: 0,
        tick_prev_at: None,
        speed_bps: 0.0,
        payload: Some(payload),
    });
    let tokens = state.tokens();
    if !state.downloads.visible {
        state.downloads.start_show_anim(tokens);
    }
    (id, downloaded, total, cancel)
}

pub(crate) fn enqueue_op_job_running(
    state: &mut AppState,
    label: String,
    cancellable: bool,
) -> (
    u64,
    std::sync::Arc<std::sync::atomic::AtomicU64>,
    std::sync::Arc<std::sync::atomic::AtomicU64>,
    envr_download::task::CancelToken,
) {
    let id = state.downloads.next_id;
    state.downloads.next_id += 1;
    let downloaded = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let total = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let cancel = envr_download::task::CancelToken::new();
    state.downloads.jobs.push(DownloadJob {
        id,
        label,
        url: String::new(),
        state: JobState::Running,
        cancellable,
        downloaded: downloaded.clone(),
        total: total.clone(),
        cancel: cancel.clone(),
        cancel_requested_at: None,
        cancel_settled_by_timeout: false,
        last_error: None,
        tick_prev_bytes: 0,
        tick_prev_at: None,
        speed_bps: 0.0,
        payload: None,
    });
    let tokens = state.tokens();
    if !state.downloads.visible {
        state.downloads.start_show_anim(tokens);
    }
    (id, downloaded, total, cancel)
}

pub(crate) fn maybe_start_queued_jobs(state: &mut AppState) -> Task<Message> {
    let max = state
        .settings
        .cache
        .snapshot()
        .download
        .max_concurrent_downloads
        .max(1) as usize;
    let running = state
        .downloads
        .jobs
        .iter()
        .filter(|j| j.state == JobState::Running)
        .count();
    if running >= max {
        return Task::none();
    }
    let available = max - running;
    let mut tasks: Vec<Task<Message>> = Vec::new();
    for _ in 0..available {
        let Some(j) = state
            .downloads
            .jobs
            .iter_mut()
            .find(|j| j.state == JobState::Queued)
        else {
            break;
        };
        if j.cancel.is_cancelled() {
            j.state = JobState::Cancelled;
            j.cancel_settled_by_timeout = false;
            j.last_error = None;
            continue;
        }
        let Some(payload) = j.payload.clone() else {
            j.state = JobState::Failed;
            j.last_error = Some("internal: missing job payload".into());
            continue;
        };
        j.state = JobState::Running;
        let id = j.id;
        match payload {
            DownloadJobPayload::HttpDownload { url, dest } => {
                let Ok(url) = reqwest::Url::parse(&url) else {
                    j.state = JobState::Failed;
                    j.last_error = Some("invalid url".into());
                    continue;
                };
                tasks.push(download_runner::start_http_job(
                    id,
                    url,
                    dest,
                    j.cancel.clone(),
                    j.downloaded.clone(),
                    j.total.clone(),
                ));
            }
            DownloadJobPayload::RuntimeInstall {
                kind,
                spec,
                resolve_precheck,
                install_and_use,
            } => {
                let cancel = j.cancel.clone();
                let downloaded = j.downloaded.clone();
                let total = j.total.clone();
                let task = if resolve_precheck && install_and_use {
                    gui_ops::install_then_use_with_resolve_precheck(
                        id, kind, spec, downloaded, total, cancel,
                    )
                } else if resolve_precheck {
                    gui_ops::install_version_with_resolve_precheck(
                        id, kind, spec, downloaded, total, cancel,
                    )
                } else if install_and_use {
                    gui_ops::install_then_use(id, kind, spec, downloaded, total, cancel)
                } else {
                    gui_ops::install_version(id, kind, spec, downloaded, total, cancel)
                };
                tasks.push(task);
            }
        }
    }
    if tasks.is_empty() {
        Task::none()
    } else {
        Task::batch(tasks)
    }
}

fn settle_stuck_cancelling_jobs(state: &mut AppState) -> Task<Message> {
    let now = Instant::now();
    let mut touched = false;
    let mut released_active_installs = Vec::new();
    let mut clear_rust_op = false;

    for j in &mut state.downloads.jobs {
        if j.state != JobState::Running || !j.cancel.is_cancelled() {
            continue;
        }
        let Some(requested_at) = j.cancel_requested_at else {
            continue;
        };
        if now.duration_since(requested_at) < CANCEL_SETTLE_TIMEOUT {
            continue;
        }
        j.state = JobState::Cancelled;
        j.cancel_settled_by_timeout = true;
        j.last_error = None;
        touched = true;
        released_active_installs.push(j.id);
        if state.env_center.op_job_id == Some(j.id) {
            clear_rust_op = true;
        }
    }

    if !touched {
        return Task::none();
    }

    for id in released_active_installs {
        state.env_center.active_install_job_ids.remove(&id);
    }
    if clear_rust_op {
        state.env_center.op_job_id = None;
    }
    if state.env_center.op_job_id.is_none() && state.env_center.active_install_job_ids.is_empty() {
        state.env_center.busy = false;
    }
    maybe_start_queued_jobs(state)
}
