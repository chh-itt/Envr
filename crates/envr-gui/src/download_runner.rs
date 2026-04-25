//! Start HTTP downloads with [`envr_download::DownloadEngine`] for the GUI panel.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use envr_download::engine::{DownloadEngine, DownloadOptions};
use envr_download::task::CancelToken;
use envr_download::DownloadPriority;
use iced::Task;
use reqwest::Url;

use crate::app::Message;
use crate::view::downloads::DownloadMsg;

/// Small public asset suitable for repeated demo downloads.
pub const DEMO_URL: &str = "https://www.rust-lang.org/logos/rust-logo-blk.svg";

pub fn start_http_job(
    id: u64,
    url: Url,
    dest: PathBuf,
    cancel: CancelToken,
    downloaded: Arc<AtomicU64>,
    total: Arc<AtomicU64>,
) -> Task<Message> {
    Task::future(async move {
        let client = match DownloadEngine::default_client() {
            Ok(c) => c,
            Err(e) => {
                return Message::Download(DownloadMsg::Finished {
                    id,
                    result: Err(e.to_string()),
                });
            }
        };
        let engine = DownloadEngine::new(client);
        let opts = DownloadOptions {
            priority: DownloadPriority::Prefetch,
            ..DownloadOptions::default()
        };
        match engine
            .download_to_file(
                url,
                &dest,
                &cancel,
                &opts,
                Some(downloaded),
                Some(total),
                None,
            )
            .await
        {
            Ok(o) => Message::Download(DownloadMsg::Finished {
                id,
                result: Ok(o.resumed_from.saturating_add(o.bytes_written)),
            }),
            Err(e) => Message::Download(DownloadMsg::Finished {
                id,
                result: Err(e.to_string()),
            }),
        }
    })
}
