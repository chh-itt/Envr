use envr_download::blocking::with_download_priority_blocking;
use envr_download::engine::{DownloadEngine, DownloadOptions};
use envr_download::{
    DownloadPriority, global_download_concurrency_limiter, set_global_download_concurrency_limit,
};
use reqwest::Url;
use std::sync::{Mutex, OnceLock, mpsc};
use std::time::Duration;
use tempfile::TempDir;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

// These tests mutate process-global download gating state, so they must not overlap.
// We only hold the lock while preparing global state and spawning the task/thread under test;
// the guard is dropped before any await points to avoid holding a std::sync::Mutex across await.
fn global_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[test]
fn blocking_index_job_waits_until_global_permit_released() {
    let _guard = global_test_lock().lock().expect("test lock");
    set_global_download_concurrency_limit(Some(1)).expect("set limit");
    let lim = global_download_concurrency_limiter().expect("limiter");
    let holder = lim.acquire_blocking(DownloadPriority::Artifact);

    let (tx, rx) = mpsc::channel::<()>();
    let worker = std::thread::spawn(move || {
        with_download_priority_blocking(DownloadPriority::Index, || {
            tx.send(()).expect("send done");
            Ok(())
        })
        .expect("index work");
    });

    // Should still be blocked while holder is alive.
    assert!(rx.recv_timeout(Duration::from_millis(120)).is_err());

    drop(holder);
    rx.recv_timeout(Duration::from_secs(2))
        .expect("index work completed after release");
    worker.join().expect("worker join");
    set_global_download_concurrency_limit(None).expect("clear limit");
}

#[test]
fn async_prefetch_download_waits_then_completes_under_global_gate() {
    let _guard = global_test_lock().lock().expect("test lock");
    set_global_download_concurrency_limit(Some(1)).expect("set limit");
    let holder = {
        let lim = global_download_concurrency_limiter().expect("limiter");
        lim.acquire_blocking(DownloadPriority::Artifact)
    };

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("runtime");

    let server = rt.block_on(async {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-length", "5")
                    .set_body_bytes(b"hello"),
            )
            .mount(&server)
            .await;
        server
    });

    let tmp = TempDir::new().expect("tmp");
    let dest = tmp.path().join("prefetch.bin");
    let url = Url::parse(&server.uri()).expect("url");
    let engine = DownloadEngine::new(DownloadEngine::default_client().expect("client"));
    let opts = DownloadOptions {
        priority: DownloadPriority::Prefetch,
        ..DownloadOptions::default()
    };

    let handle = rt.spawn({
        let engine = engine.clone();
        let dest = dest.clone();
        async move {
            engine
                .download_to_file(
                    url,
                    &dest,
                    &envr_download::task::CancelToken::new(),
                    &opts,
                    None,
                    None,
                    None,
                )
                .await
        }
    });

    std::thread::sleep(Duration::from_millis(120));
    assert!(!handle.is_finished(), "prefetch should wait on gate");

    drop(holder);
    let out = rt
        .block_on(async { tokio::time::timeout(Duration::from_secs(5), handle).await })
        .expect("join timeout")
        .expect("join")
        .expect("download");
    assert_eq!(out.bytes_written, 5);
    set_global_download_concurrency_limit(None).expect("clear limit");
}
