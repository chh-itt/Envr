//! Text-mode install feedback: `InstallRequest` progress atomics plus a stderr line / live byte counter.
//! JS runtimes (node / deno / bun) download with blocking HTTP and already update these atomics.

use crate::cli::{GlobalArgs, OutputFormat};
use envr_domain::runtime::{InstallRequest, VersionSpec};
use std::io::{IsTerminal, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

/// Human-oriented stderr messages (no TTY requirement).
pub fn wants_cli_text_feedback(g: &GlobalArgs) -> bool {
    !g.quiet
        && !g.porcelain
        && matches!(
            g.output_format.unwrap_or(OutputFormat::Text),
            OutputFormat::Text
        )
}

/// Live download meter on stderr (TTY only so CI logs are not spammed with `\r`).
pub fn wants_cli_download_progress(g: &GlobalArgs) -> bool {
    wants_cli_text_feedback(g) && std::io::stderr().is_terminal()
}

pub struct CliInstallProgressGuard {
    done: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl CliInstallProgressGuard {
    pub fn disabled() -> Self {
        Self {
            done: Arc::new(AtomicBool::new(true)),
            join: None,
        }
    }

    pub fn finish(self) {
        self.done.store(true, Ordering::SeqCst);
        if let Some(h) = self.join {
            let _ = h.join();
        }
    }
}

pub fn install_request_with_progress(
    g: &GlobalArgs,
    spec: VersionSpec,
    headline: String,
) -> (InstallRequest, CliInstallProgressGuard) {
    if !wants_cli_download_progress(g) {
        if wants_cli_text_feedback(g) {
            let _ = writeln!(std::io::stderr(), "{headline}");
            let _ = std::io::stderr().flush();
        }
        return (
            InstallRequest {
                spec,
                progress_downloaded: None,
                progress_total: None,
                cancel: None,
            },
            CliInstallProgressGuard::disabled(),
        );
    }

    let downloaded = Arc::new(AtomicU64::new(0));
    let total = Arc::new(AtomicU64::new(0));
    let done = Arc::new(AtomicBool::new(false));
    let d2 = downloaded.clone();
    let t2 = total.clone();
    let done2 = done.clone();
    let handle = std::thread::spawn(move || {
        let _ = writeln!(std::io::stderr(), "{headline}");
        let _ = std::io::stderr().flush();
        while !done2.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(150));
            let dl = d2.load(Ordering::Relaxed);
            let tot = t2.load(Ordering::Relaxed);
            if tot > 0 {
                let pct = (dl.saturating_mul(100) / tot.max(1)).min(100);
                let _ = write!(std::io::stderr(), "\r    {pct}%  {dl} / {tot} bytes  ");
            } else if dl > 0 {
                let _ = write!(std::io::stderr(), "\r    {dl} bytes…  ");
            } else {
                let _ = write!(std::io::stderr(), "\r    …  ");
            }
            let _ = std::io::stderr().flush();
        }
        let dl = d2.load(Ordering::Relaxed);
        let tot = t2.load(Ordering::Relaxed);
        if tot > 0 {
            let _ = writeln!(
                std::io::stderr(),
                "\r    100%  {dl} / {tot} bytes    "
            );
        } else if dl > 0 {
            let _ = writeln!(std::io::stderr(), "\r    {dl} bytes    ");
        } else {
            let _ = writeln!(std::io::stderr());
        }
    });

    (
        InstallRequest {
            spec,
            progress_downloaded: Some(downloaded),
            progress_total: Some(total),
            cancel: None,
        },
        CliInstallProgressGuard {
            done,
            join: Some(handle),
        },
    )
}
