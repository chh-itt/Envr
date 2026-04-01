use super::model::{DownloadJob, JobState};

#[derive(Debug, Clone)]
pub enum DownloadMsg {
    Tick,
    ToggleVisible,
    ToggleExpand,
    StartDrag,
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
                .map(|s| format!(" · {} {s}s", envr_core::i18n::tr("约剩余", "ETA")))
                .unwrap_or_default();
            let sz = if t > 0 {
                format!("{d} / {t} {}", envr_core::i18n::tr("字节", "bytes"))
            } else {
                format!("{d} {}", envr_core::i18n::tr("字节", "bytes"))
            };
            format!(
                "{} · {sz} · {spd}{eta}",
                envr_core::i18n::tr("进行中", "Running")
            )
        }
        JobState::Done => format!(
            "{} · {} {}",
            envr_core::i18n::tr("完成", "Done"),
            job.downloaded_display(),
            envr_core::i18n::tr("字节", "bytes")
        ),
        JobState::Failed => format!(
            "{}: {}",
            envr_core::i18n::tr("失败", "Failed"),
            job.last_error
                .as_deref()
                .unwrap_or(envr_core::i18n::tr("未知错误", "unknown error"))
        ),
        JobState::Cancelled => envr_core::i18n::tr("已取消", "Cancelled").to_string(),
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
