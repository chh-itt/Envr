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
            let bytes = envr_core::i18n::tr_key("gui.downloads.bytes", "字节", "bytes");
            let sz = if t > 0 {
                format!("{d} / {t} {bytes}")
            } else {
                format!("{d} {bytes}")
            };
            format!(
                "{} · {sz} · {spd}{eta}",
                envr_core::i18n::tr_key("gui.job.running", "进行中", "Running")
            )
        }
        JobState::Done => format!(
            "{} · {} {}",
            envr_core::i18n::tr_key("gui.job.done", "完成", "Done"),
            job.downloaded_display(),
            envr_core::i18n::tr_key("gui.downloads.bytes", "字节", "bytes")
        ),
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
