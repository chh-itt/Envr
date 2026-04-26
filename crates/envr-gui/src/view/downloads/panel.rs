use super::model::{DownloadJob, JobState};

#[derive(Debug, Clone)]
pub enum DownloadMsg {
    Tick,
    ToggleVisible,
    ToggleExpand,
    /// Pointer down on title / drag strip (long-press then drag — GUI-061).
    TitleBarPress,
    Event(iced::Event),
    EnqueueDemo,
    Finished {
        id: u64,
        result: Result<u64, String>,
    },
    Cancel(u64),
    Retry(u64),
}

pub fn format_job_state_line(job: &DownloadJob) -> String {
    match job.state {
        JobState::Queued => {
            envr_core::i18n::tr_key("gui.job.queued", "排队中", "Queued").to_string()
        }
        JobState::Running => {
            if job.cancel.is_cancelled() {
                return envr_core::i18n::tr_key("gui.job.cancelling", "取消中…", "Cancelling…")
                    .to_string();
            }
            if job.is_runtime_install_row() {
                let d = job.downloaded_display();
                let t = job.total_display();
                let phase = if t == 0 && d == 0 {
                    envr_core::i18n::tr_key(
                        "gui.downloads.install_preparing",
                        "准备中…",
                        "Preparing…",
                    )
                } else if t > 0 && d < t {
                    envr_core::i18n::tr_key(
                        "gui.downloads.install_downloading",
                        "下载中…",
                        "Downloading…",
                    )
                } else {
                    envr_core::i18n::tr_key(
                        "gui.downloads.install_finalizing",
                        "下载完成，正在安装…",
                        "Download complete, installing…",
                    )
                };
                let spd = format_speed(job.speed_bps);
                let sz = if t > 0 {
                    format!("{} / {}", format_transfer_size(d), format_transfer_size(t))
                } else {
                    format_transfer_size(d)
                };
                return format!(
                    "{} · {phase} · {sz} · {spd}",
                    envr_core::i18n::tr_key("gui.job.running", "进行中", "Running"),
                );
            }
            let spd = format_speed(job.speed_bps);
            let d = job.downloaded_display();
            let t = job.total_display();
            let eta = job
                .eta_secs()
                .map(|s| {
                    format!(
                        " · {} {s}s",
                        envr_core::i18n::tr_key("gui.downloads.eta_label", "约剩余", "ETA")
                    )
                })
                .unwrap_or_default();
            let sz = if t > 0 {
                format!("{} / {}", format_transfer_size(d), format_transfer_size(t))
            } else {
                format_transfer_size(d)
            };
            format!(
                "{} · {sz} · {spd}{eta}",
                envr_core::i18n::tr_key("gui.job.running", "进行中", "Running")
            )
        }
        JobState::Done => {
            if job.is_local_install_done_minimal() {
                return envr_core::i18n::tr_key("gui.job.done", "完成", "Done").to_string();
            }
            format!(
                "{} · {}",
                envr_core::i18n::tr_key("gui.job.done", "完成", "Done"),
                format_transfer_size(job.downloaded_display()),
            )
        }
        JobState::Failed => {
            let detail = job.last_error.clone().unwrap_or_else(|| {
                envr_core::i18n::tr_key("gui.downloads.unknown_error", "未知错误", "unknown error")
            });
            format!(
                "{}: {detail}",
                envr_core::i18n::tr_key("gui.job.failed", "失败", "Failed"),
            )
        }
        JobState::Cancelled => {
            if job.cancel_settled_by_timeout {
                envr_core::i18n::tr_key(
                    "gui.job.cancelled_timeout_settled",
                    "取消超时，界面已结束（后台可能仍在收尾）",
                    "Cancel timed out; ended in UI (background may still be finalizing)",
                )
            } else {
                envr_core::i18n::tr_key("gui.job.cancelled", "已取消", "Cancelled")
            }
        }
    }
}

fn format_speed(bps: f64) -> String {
    if bps >= 1_048_576.0 {
        format!("{:.1} MiB/s", bps / 1_048_576.0)
    } else if bps >= 1024.0 {
        format!("{:.1} KiB/s", bps / 1024.0)
    } else {
        format!("{:.0} B/s", bps)
    }
}

/// Human-readable size for progress text (binary units, consistent with [`format_speed`]).
fn format_transfer_size(n: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;
    let nf = n as f64;
    if nf >= MIB {
        format!("{:.1} MiB", nf / MIB)
    } else if nf >= KIB {
        format!("{:.1} KiB", nf / KIB)
    } else {
        format!(
            "{n} {}",
            envr_core::i18n::tr_key("gui.downloads.bytes", "字节", "bytes")
        )
    }
}
