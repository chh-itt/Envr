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
        JobState::Running => {
            if job.is_install_warmup_phase() {
                return format!(
                    "{} · {}",
                    envr_core::i18n::tr_key("gui.job.running", "进行中", "Running"),
                    envr_core::i18n::tr_key(
                        "gui.downloads.install_working",
                        "正在安装处理中…",
                        "Installing…",
                    ),
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
        JobState::Cancelled => envr_core::i18n::tr_key("gui.job.cancelled", "已取消", "Cancelled"),
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
