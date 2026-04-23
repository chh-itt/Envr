//! Text-mode install feedback: `InstallRequest` progress atomics plus a stderr line / live byte counter.
//! JS runtimes (node / deno / bun) download with blocking HTTP and already update these atomics.

use crate::CliUxPolicy;
use crate::cli::GlobalArgs;
use envr_domain::runtime::{InstallRequest, VersionSpec};
use indicatif::{ProgressBar, ProgressStyle};
use std::io::{IsTerminal, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

/// Human-oriented stderr messages (no TTY requirement).
pub fn wants_cli_text_feedback(g: &GlobalArgs) -> bool {
    CliUxPolicy::from_global(g).human_text_decorated()
}

/// Live download meter on stderr (TTY only so CI logs are not spammed with `\r`).
pub fn wants_cli_download_progress(g: &GlobalArgs) -> bool {
    wants_cli_text_feedback(g) && std::io::stderr().is_terminal()
}

pub struct CliInstallProgressGuard {
    done: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
    bar: Option<ProgressBar>,
}

impl CliInstallProgressGuard {
    pub fn disabled() -> Self {
        Self {
            done: Arc::new(AtomicBool::new(true)),
            join: None,
            bar: None,
        }
    }

    pub fn finish(mut self) {
        self.done.store(true, Ordering::SeqCst);
        if let Some(h) = self.join {
            let _ = h.join();
        }
        if let Some(bar) = self.bar.take() {
            bar.finish_and_clear();
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
    let bar = Arc::new(ProgressBar::new(0));
    let style = ProgressStyle::with_template(
        "{spinner:.cyan} [{bar:30.cyan/white}] {percent:>3}% {bytes:>9}/{total_bytes:<9} {bytes_per_sec:>10}",
    )
    .unwrap_or_else(|_| ProgressStyle::default_bar())
    .progress_chars("=> ");
    bar.set_style(style);
    bar.enable_steady_tick(Duration::from_millis(100));
    bar.set_message(headline.clone());
    let bar2 = Arc::clone(&bar);
    let handle = std::thread::spawn(move || {
        let _ = writeln!(std::io::stderr(), "{headline}");
        let _ = std::io::stderr().flush();
        while !done2.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(150));
            let dl = d2.load(Ordering::Relaxed);
            let tot = t2.load(Ordering::Relaxed);
            if tot > 0 {
                bar2.set_length(tot);
                bar2.set_position(dl.min(tot));
            } else if dl > 0 {
                bar2.set_position(dl);
            }
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
            bar: Some((*bar).clone()),
        },
    )
}
