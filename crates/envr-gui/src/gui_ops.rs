//! Async bridge: run blocking `RuntimeService` calls on the Tokio blocking pool.

use envr_core::shim_service::ShimService;
use envr_domain::runtime::{
    InstallRequest, MajorVersionRecord, RuntimeKind, RuntimeVersion, VersionRecord, VersionSpec,
    runtime_descriptor, runtime_kinds_all,
};
use envr_download::task::CancelToken;
use envr_error::{EnvrError, EnvrResult};
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::time::Instant;

use crate::app::Message;
use crate::runtime_exec::runtime;
use crate::service::open_runtime_service;
use crate::view::dashboard::DashboardMsg;
use crate::view::env_center::EnvCenterMsg;
use iced::Task;

use crate::view::env_center::RustStatus;
use envr_config::settings::resolve_runtime_root;
use envr_runtime_rust::{RustChannel, RustManager, RustupMode, install_rustup_managed};
use std::process::Command;

type RefreshRuntimesOk = (Vec<RuntimeVersion>, Option<RuntimeVersion>, Option<bool>);

fn check_jvm_runtime_java_compat_sync(
    kind: RuntimeKind,
    missing_java_msg: &'static str,
) -> Result<(), String> {
    let svc = open_runtime_service().map_err(|e| e.to_string())?;
    let Some(runtime_cur) = svc.current(kind).map_err(|e| e.to_string())? else {
        return Ok(());
    };
    let Some(java_cur) = svc.current(RuntimeKind::Java).map_err(|e| e.to_string())? else {
        return Err(missing_java_msg.into());
    };
    let key = runtime_descriptor(kind).key;
    if let Some(msg) =
        envr_domain::jvm_hosted::hosted_runtime_jdk_mismatch_message(key, &runtime_cur.0, &java_cur.0)
    {
        return Err(msg);
    }
    Ok(())
}

pub fn refresh_runtimes(kind: RuntimeKind) -> Task<Message> {
    let handle = runtime().handle().clone();
    Task::future(async move {
        let res = handle
            .spawn_blocking(move || -> Result<RefreshRuntimesOk, String> {
                let svc = open_runtime_service().map_err(|e: EnvrError| e.to_string())?;
                let installed = svc
                    .list_installed(kind)
                    .map_err(|e: EnvrError| e.to_string())?;
                let current = svc.current(kind).map_err(|e: EnvrError| e.to_string())?;
                let php_global_ts = if matches!(kind, RuntimeKind::Php) {
                    svc.php_global_current_want_ts()
                        .map_err(|e: EnvrError| e.to_string())?
                } else {
                    None
                };
                Ok((installed, current, php_global_ts))
            })
            .await;
        let msg = match res {
            Ok(Ok(data)) => EnvCenterMsg::DataLoaded(Ok(data)),
            Ok(Err(e)) => EnvCenterMsg::DataLoaded(Err(e)),
            Err(e) => EnvCenterMsg::DataLoaded(Err(e.to_string())),
        };
        Message::EnvCenter(msg)
    })
}

pub fn check_scala_java_compat() -> Task<Message> {
    let handle = runtime().handle().clone();
    Task::future(async move {
        let res = handle
            .spawn_blocking(move || {
                check_jvm_runtime_java_compat_sync(
                    RuntimeKind::Scala,
                    "Scala 需要已设置全局 **Java current**：请先在「Java」页安装并选择 JDK。",
                )
            })
            .await;

        Message::EnvCenter(EnvCenterMsg::ScalaJavaChecked(match res {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(e),
            Err(e) => Err(e.to_string()),
        }))
    })
}

pub fn check_kotlin_jdk_compat() -> Task<Message> {
    let handle = runtime().handle().clone();
    Task::future(async move {
        let res = handle
            .spawn_blocking(move || {
                check_jvm_runtime_java_compat_sync(
                    RuntimeKind::Kotlin,
                    "Kotlin 需要已设置全局 **Java current**：请先在「Java」页安装并选择 JDK。",
                )
            })
            .await;

        Message::EnvCenter(EnvCenterMsg::KotlinJdkChecked(match res {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(e),
            Err(e) => Err(e.to_string()),
        }))
    })
}

pub fn check_elixir_prereqs() -> Task<Message> {
    let handle = runtime().handle().clone();
    Task::future(async move {
        let res = handle
            .spawn_blocking(move || -> Result<(), String> {
                #[cfg(windows)]
                let mut cmd = {
                    let mut c = Command::new("erl.exe");
                    c.arg("-version");
                    c
                };
                #[cfg(not(windows))]
                let mut cmd = {
                    let mut c = Command::new("erl");
                    c.arg("-version");
                    c
                };
                match cmd.output() {
                    Ok(out) if out.status.success() => Ok(()),
                    Ok(_) => Err(
                        "Erlang/OTP 已安装但不可用：`erl` 无法正常执行。请修复 Erlang/OTP 后重试。"
                            .into(),
                    ),
                    Err(_) => Err(
                        "Elixir 需要 Erlang/OTP（`erl.exe`）。当前系统未检测到 `erl`，请先安装 Erlang/OTP。"
                            .into(),
                    ),
                }
            })
            .await;

        Message::EnvCenter(EnvCenterMsg::ElixirPrereqChecked(match res {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(e),
            Err(e) => Err(e.to_string()),
        }))
    })
}

pub fn rust_refresh() -> Task<Message> {
    let handle = runtime().handle().clone();
    Task::future(async move {
        let res = handle
            .spawn_blocking(move || -> Result<RustStatus, String> {
                let root = resolve_runtime_root().map_err(|e| e.to_string())?;
                let mgr = RustManager::try_new(root).map_err(|e| e.to_string())?;

                let system = RustManager::system_rustup_available();
                let managed = mgr.managed_rustup_installed();
                let mode = if system {
                    "system".to_string()
                } else if managed {
                    "managed".to_string()
                } else {
                    "none".to_string()
                };

                let active_toolchain = mgr.active_toolchain().ok().flatten().map(|v| v.0);

                let rustc_version = (|| -> Option<String> {
                    // Prefer managed rustc when we own the install; otherwise fall back to PATH.
                    if managed {
                        let paths = envr_runtime_rust::RustPaths::new(resolve_runtime_root().ok()?);
                        let rustc = paths.managed_rustc_exe();
                        let o = std::process::Command::new(rustc).arg("-V").output().ok()?;
                        if o.status.success() {
                            let s = String::from_utf8_lossy(&o.stdout);
                            return Some(
                                s.split_whitespace().nth(1).unwrap_or("").trim().to_string(),
                            );
                        }
                    }
                    let o = std::process::Command::new("rustc")
                        .arg("-V")
                        .output()
                        .ok()?;
                    if !o.status.success() {
                        return None;
                    }
                    let s = String::from_utf8_lossy(&o.stdout);
                    Some(s.split_whitespace().nth(1).unwrap_or("").trim().to_string())
                })();

                Ok(RustStatus {
                    mode,
                    active_toolchain,
                    rustc_version,
                    managed_install_available: !system && !managed,
                    managed_installed: managed,
                })
            })
            .await;
        Message::EnvCenter(EnvCenterMsg::RustStatusLoaded(match res {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(e)) => Err(e),
            Err(e) => Err(e.to_string()),
        }))
    })
}

pub fn rust_load_components() -> Task<Message> {
    let handle = runtime().handle().clone();
    Task::future(async move {
        let res = handle
            .spawn_blocking(move || -> Result<Vec<(String, bool, bool)>, String> {
                let root = resolve_runtime_root().map_err(|e| e.to_string())?;
                let mgr = RustManager::try_new(root).map_err(|e| e.to_string())?;
                if !mgr.rustup_available() {
                    return Ok(vec![]);
                }
                mgr.list_components(None).map_err(|e| e.to_string())
            })
            .await;
        Message::EnvCenter(EnvCenterMsg::RustComponentsLoaded(match res {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(e)) => Err(e),
            Err(e) => Err(e.to_string()),
        }))
    })
}

pub fn rust_load_targets() -> Task<Message> {
    let handle = runtime().handle().clone();
    Task::future(async move {
        let res = handle
            .spawn_blocking(move || -> Result<Vec<(String, bool, bool)>, String> {
                let root = resolve_runtime_root().map_err(|e| e.to_string())?;
                let mgr = RustManager::try_new(root).map_err(|e| e.to_string())?;
                if !mgr.rustup_available() {
                    return Ok(vec![]);
                }
                mgr.list_targets(None).map_err(|e| e.to_string())
            })
            .await;
        Message::EnvCenter(EnvCenterMsg::RustTargetsLoaded(match res {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(e)) => Err(e),
            Err(e) => Err(e.to_string()),
        }))
    })
}

pub fn rust_channel_install_or_switch(channel: String) -> Task<Message> {
    let handle = runtime().handle().clone();
    Task::future(async move {
        let res = handle
            .spawn_blocking(move || -> Result<(), String> {
                let root = resolve_runtime_root().map_err(|e| e.to_string())?;
                let mgr = RustManager::try_new(root).map_err(|e| e.to_string())?;
                if !mgr.rustup_available() {
                    return Err("rustup not installed".into());
                }
                // Ensure toolchain exists (idempotent), then set default.
                let _ = mgr
                    .install_toolchain(&envr_domain::runtime::InstallRequest {
                        spec: envr_domain::runtime::VersionSpec(channel.clone()),
                        progress_downloaded: None,
                        progress_total: None,
                        cancel: None,
                    })
                    .map_err(|e| e.to_string())?;
                mgr.set_default(&RuntimeVersion(channel))
                    .map_err(|e| e.to_string())
            })
            .await;
        Message::EnvCenter(EnvCenterMsg::RustOpFinished(match res {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(e),
            Err(e) => Err(e.to_string()),
        }))
    })
}

pub fn rust_update_current() -> Task<Message> {
    let handle = runtime().handle().clone();
    Task::future(async move {
        let res = handle
            .spawn_blocking(move || -> Result<(), String> {
                let root = resolve_runtime_root().map_err(|e| e.to_string())?;
                let mgr = RustManager::try_new(root).map_err(|e| e.to_string())?;
                if !mgr.rustup_available() {
                    return Err("rustup not installed".into());
                }
                if let Some(tc) = mgr.active_toolchain().map_err(|e| e.to_string())? {
                    mgr.update_toolchain(&tc).map_err(|e| e.to_string())
                } else {
                    mgr.update_all().map_err(|e| e.to_string())
                }
            })
            .await;
        Message::EnvCenter(EnvCenterMsg::RustOpFinished(match res {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(e),
            Err(e) => Err(e.to_string()),
        }))
    })
}

pub fn rust_managed_install_stable(
    progress_downloaded: Arc<AtomicU64>,
    progress_total: Arc<AtomicU64>,
    cancel: CancelToken,
) -> Task<Message> {
    let handle = runtime().handle().clone();
    Task::future(async move {
        let res = handle
            .spawn_blocking(move || -> Result<(), String> {
                let root = resolve_runtime_root().map_err(|e| e.to_string())?;
                let cancel_flag = cancel.shared_atomic();
                let request = InstallRequest {
                    spec: VersionSpec("stable".into()),
                    progress_downloaded: Some(progress_downloaded),
                    progress_total: Some(progress_total),
                    cancel: Some(cancel_flag),
                };
                install_rustup_managed(root, RustChannel::Stable, Some(&request))
                    .map_err(|e| e.to_string())
            })
            .await;
        Message::EnvCenter(EnvCenterMsg::RustOpFinished(match res {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(e),
            Err(e) => Err(e.to_string()),
        }))
    })
}

pub fn rust_managed_uninstall() -> Task<Message> {
    let handle = runtime().handle().clone();
    Task::future(async move {
        let res = handle
            .spawn_blocking(move || -> Result<(), String> {
                let root = resolve_runtime_root().map_err(|e| e.to_string())?;
                let mgr = RustManager::try_new(root).map_err(|e| e.to_string())?;
                if mgr.mode() != RustupMode::Managed {
                    return Err("not using managed rustup".into());
                }
                mgr.managed_uninstall().map_err(|e| e.to_string())
            })
            .await;
        Message::EnvCenter(EnvCenterMsg::RustOpFinished(match res {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(e),
            Err(e) => Err(e.to_string()),
        }))
    })
}

pub fn rust_component_toggle(name: String, install: bool) -> Task<Message> {
    let handle = runtime().handle().clone();
    Task::future(async move {
        let res = handle
            .spawn_blocking(move || -> Result<(), String> {
                let root = resolve_runtime_root().map_err(|e| e.to_string())?;
                let mgr = RustManager::try_new(root).map_err(|e| e.to_string())?;
                if !mgr.rustup_available() {
                    return Err("rustup not installed".into());
                }
                if install {
                    mgr.component_add(&name, None).map_err(|e| e.to_string())
                } else {
                    mgr.component_remove(&name, None).map_err(|e| e.to_string())
                }
            })
            .await;
        Message::EnvCenter(EnvCenterMsg::RustOpFinished(match res {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(e),
            Err(e) => Err(e.to_string()),
        }))
    })
}

pub fn rust_target_toggle(name: String, install: bool) -> Task<Message> {
    let handle = runtime().handle().clone();
    Task::future(async move {
        let res = handle
            .spawn_blocking(move || -> Result<(), String> {
                let root = resolve_runtime_root().map_err(|e| e.to_string())?;
                let mgr = RustManager::try_new(root).map_err(|e| e.to_string())?;
                if !mgr.rustup_available() {
                    return Err("rustup not installed".into());
                }
                if install {
                    mgr.target_add(&name, None).map_err(|e| e.to_string())
                } else {
                    mgr.target_remove(&name, None).map_err(|e| e.to_string())
                }
            })
            .await;
        Message::EnvCenter(EnvCenterMsg::RustOpFinished(match res {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(e),
            Err(e) => Err(e.to_string()),
        }))
    })
}

pub fn load_unified_major_rows_cached(kind: RuntimeKind) -> Task<Message> {
    let handle = runtime().handle().clone();
    Task::future(async move {
        let res = handle
            .spawn_blocking(move || -> Result<Vec<MajorVersionRecord>, String> {
                let svc = open_runtime_service().map_err(|e: EnvrError| e.to_string())?;
                svc.list_major_rows_cached(kind)
                    .map_err(|e: EnvrError| e.to_string())
            })
            .await;
        let msg = match res {
            Ok(Ok(rows)) => EnvCenterMsg::UnifiedMajorRowsCached(kind, Ok(rows)),
            Ok(Err(e)) => EnvCenterMsg::UnifiedMajorRowsCached(kind, Err(e)),
            Err(e) => EnvCenterMsg::UnifiedMajorRowsCached(kind, Err(e.to_string())),
        };
        Message::EnvCenter(msg)
    })
}

pub fn refresh_unified_major_rows(kind: RuntimeKind) -> Task<Message> {
    let handle = runtime().handle().clone();
    Task::future(async move {
        let res = handle
            .spawn_blocking(move || -> Result<Vec<MajorVersionRecord>, String> {
                let svc = open_runtime_service().map_err(|e: EnvrError| e.to_string())?;
                svc.refresh_major_rows_remote(kind)
                    .map_err(|e: EnvrError| e.to_string())
            })
            .await;
        let msg = match res {
            Ok(Ok(rows)) => EnvCenterMsg::UnifiedMajorRowsRefreshed(kind, Ok(rows)),
            Ok(Err(e)) => EnvCenterMsg::UnifiedMajorRowsRefreshed(kind, Err(e)),
            Err(e) => EnvCenterMsg::UnifiedMajorRowsRefreshed(kind, Err(e.to_string())),
        };
        Message::EnvCenter(msg)
    })
}

pub fn load_unified_children_cached(kind: RuntimeKind, major_key: String) -> Task<Message> {
    let handle = runtime().handle().clone();
    Task::future(async move {
        let major_for_msg = major_key.clone();
        let major_for_call = major_key.clone();
        let res = handle
            .spawn_blocking(move || -> Result<Vec<VersionRecord>, String> {
                let svc = open_runtime_service().map_err(|e: EnvrError| e.to_string())?;
                svc.list_children_cached(kind, &major_for_call)
                    .map_err(|e: EnvrError| e.to_string())
            })
            .await;
        let msg = match res {
            Ok(Ok(rows)) => EnvCenterMsg::UnifiedChildrenCached(
                kind,
                major_for_msg,
                Ok(rows.into_iter().map(|r| r.version).collect()),
            ),
            Ok(Err(e)) => EnvCenterMsg::UnifiedChildrenCached(kind, major_key, Err(e)),
            Err(e) => EnvCenterMsg::UnifiedChildrenCached(kind, major_key, Err(e.to_string())),
        };
        Message::EnvCenter(msg)
    })
}

pub fn refresh_unified_children(kind: RuntimeKind, major_key: String) -> Task<Message> {
    let handle = runtime().handle().clone();
    Task::future(async move {
        let major_for_msg = major_key.clone();
        let major_for_call = major_key.clone();
        let res = handle
            .spawn_blocking(move || -> Result<Vec<VersionRecord>, String> {
                let svc = open_runtime_service().map_err(|e: EnvrError| e.to_string())?;
                svc.refresh_children_remote(kind, &major_for_call)
                    .map_err(|e: EnvrError| e.to_string())
            })
            .await;
        let msg = match res {
            Ok(Ok(rows)) => EnvCenterMsg::UnifiedChildrenRefreshed(
                kind,
                major_for_msg,
                Ok(rows.into_iter().map(|r| r.version).collect()),
            ),
            Ok(Err(e)) => EnvCenterMsg::UnifiedChildrenRefreshed(kind, major_key, Err(e)),
            Err(e) => EnvCenterMsg::UnifiedChildrenRefreshed(kind, major_key, Err(e.to_string())),
        };
        Message::EnvCenter(msg)
    })
}

/// Best-effort: delete unified list on-disk cache for `kind` (major rows, full remote snapshot, children).
pub fn invalidate_unified_list_disk_cache(kind: RuntimeKind) -> Task<Message> {
    let handle = runtime().handle().clone();
    Task::future(async move {
        let _ = handle
            .spawn_blocking(move || {
                if let Ok(svc) = open_runtime_service() {
                    let _ = svc.remove_unified_version_list_cache_dir(kind);
                }
            })
            .await;
        Message::Idle
    })
}

/// Like [`install_version`], but runs [`RuntimeService::resolve`] first so invalid specs fail before download.
pub fn install_version_with_resolve_precheck(
    kind: RuntimeKind,
    spec: String,
    progress_downloaded: Arc<AtomicU64>,
    progress_total: Arc<AtomicU64>,
    cancel: CancelToken,
) -> Task<Message> {
    let handle = runtime().handle().clone();
    Task::future(async move {
        let res = handle
            .spawn_blocking(move || -> Result<RuntimeVersion, String> {
                let svc = open_runtime_service().map_err(|e: EnvrError| e.to_string())?;
                let vs = VersionSpec(spec.clone());
                svc.resolve(kind, &vs)
                    .map_err(|e: EnvrError| e.to_string())?;
                let prev_current = svc.current(kind).map_err(|e: EnvrError| e.to_string())?;
                let cancel_flag = cancel.shared_atomic();
                svc.install(
                    kind,
                    &InstallRequest {
                        spec: VersionSpec(spec),
                        progress_downloaded: Some(progress_downloaded),
                        progress_total: Some(progress_total),
                        cancel: Some(cancel_flag),
                    },
                )
                .map_err(|e: EnvrError| e.to_string())
                .inspect(|_| {
                    if let Some(prev) = prev_current.as_ref() {
                        let _ = svc.set_current(kind, prev);
                    }
                })
            })
            .await;
        let msg = match res {
            Ok(Ok(v)) => EnvCenterMsg::InstallFinished(Ok(v)),
            Ok(Err(e)) => EnvCenterMsg::InstallFinished(Err(e)),
            Err(e) => EnvCenterMsg::InstallFinished(Err(e.to_string())),
        };
        Message::EnvCenter(msg)
    })
}

pub fn install_version(
    kind: RuntimeKind,
    spec: String,
    progress_downloaded: Arc<AtomicU64>,
    progress_total: Arc<AtomicU64>,
    cancel: CancelToken,
) -> Task<Message> {
    let handle = runtime().handle().clone();
    Task::future(async move {
        let res = handle
            .spawn_blocking(move || -> Result<RuntimeVersion, String> {
                let svc = open_runtime_service().map_err(|e: EnvrError| e.to_string())?;
                let prev_current = svc.current(kind).map_err(|e: EnvrError| e.to_string())?;
                let cancel_flag = cancel.shared_atomic();
                svc.install(
                    kind,
                    &InstallRequest {
                        spec: VersionSpec(spec),
                        progress_downloaded: Some(progress_downloaded),
                        progress_total: Some(progress_total),
                        cancel: Some(cancel_flag),
                    },
                )
                .map_err(|e: EnvrError| e.to_string())
                .inspect(|_| {
                    if let Some(prev) = prev_current.as_ref() {
                        let _ = svc.set_current(kind, prev);
                    }
                })
            })
            .await;
        let msg = match res {
            Ok(Ok(v)) => EnvCenterMsg::InstallFinished(Ok(v)),
            Ok(Err(e)) => EnvCenterMsg::InstallFinished(Err(e)),
            Err(e) => EnvCenterMsg::InstallFinished(Err(e.to_string())),
        };
        Message::EnvCenter(msg)
    })
}

/// Like [`install_then_use`], but resolves the spec before install (fail fast on bad input).
pub fn install_then_use_with_resolve_precheck(
    kind: RuntimeKind,
    spec: String,
    progress_downloaded: Arc<AtomicU64>,
    progress_total: Arc<AtomicU64>,
    cancel: CancelToken,
) -> Task<Message> {
    let handle = runtime().handle().clone();
    Task::future(async move {
        let res = handle
            .spawn_blocking(move || -> Result<RuntimeVersion, String> {
                let svc = open_runtime_service().map_err(|e: EnvrError| e.to_string())?;
                let vs = VersionSpec(spec.clone());
                svc.resolve(kind, &vs)
                    .map_err(|e: EnvrError| e.to_string())?;
                let cancel_flag = cancel.shared_atomic();
                let installed = svc
                    .install(
                        kind,
                        &InstallRequest {
                            spec: VersionSpec(spec),
                            progress_downloaded: Some(progress_downloaded),
                            progress_total: Some(progress_total),
                            cancel: Some(cancel_flag),
                        },
                    )
                    .map_err(|e: EnvrError| e.to_string())?;
                svc.set_current(kind, &installed)
                    .map_err(|e: EnvrError| e.to_string())?;
                ensure_core_shims_for_kind(kind).map_err(|e| e.to_string())?;
                Ok(installed)
            })
            .await;
        let msg = match res {
            Ok(Ok(v)) => EnvCenterMsg::InstallFinished(Ok(v)),
            Ok(Err(e)) => EnvCenterMsg::InstallFinished(Err(e)),
            Err(e) => EnvCenterMsg::InstallFinished(Err(e.to_string())),
        };
        Message::EnvCenter(msg)
    })
}

pub fn install_then_use(
    kind: RuntimeKind,
    spec: String,
    progress_downloaded: Arc<AtomicU64>,
    progress_total: Arc<AtomicU64>,
    cancel: CancelToken,
) -> Task<Message> {
    let handle = runtime().handle().clone();
    Task::future(async move {
        let res = handle
            .spawn_blocking(move || -> Result<RuntimeVersion, String> {
                let svc = open_runtime_service().map_err(|e: EnvrError| e.to_string())?;
                let cancel_flag = cancel.shared_atomic();
                let installed = svc
                    .install(
                        kind,
                        &InstallRequest {
                            spec: VersionSpec(spec.clone()),
                            progress_downloaded: Some(progress_downloaded),
                            progress_total: Some(progress_total),
                            cancel: Some(cancel_flag),
                        },
                    )
                    .map_err(|e: EnvrError| e.to_string())?;

                // Some providers set `current` during install; ensure current is set to resolved spec.
                let resolved = svc
                    .resolve(kind, &VersionSpec(spec))
                    .map_err(|e: EnvrError| e.to_string())?;
                svc.set_current(kind, &resolved.version)
                    .map_err(|e: EnvrError| e.to_string())?;
                ensure_core_shims_for_kind(kind).map_err(|e| e.to_string())?;

                Ok(installed)
            })
            .await;
        let msg = match res {
            Ok(Ok(v)) => EnvCenterMsg::InstallFinished(Ok(v)),
            Ok(Err(e)) => EnvCenterMsg::InstallFinished(Err(e)),
            Err(e) => EnvCenterMsg::InstallFinished(Err(e.to_string())),
        };
        Message::EnvCenter(msg)
    })
}

pub fn use_version(kind: RuntimeKind, version_label: String) -> Task<Message> {
    let handle = runtime().handle().clone();
    Task::future(async move {
        let res = handle
            .spawn_blocking(move || -> Result<(), String> {
                let t_total = Instant::now();
                let svc = open_runtime_service().map_err(|e: EnvrError| e.to_string())?;
                // "Use/Switch" always passes an installed exact version label from GUI state;
                // avoid `resolve()` (can hit remote index/network and add multi-second latency).
                let target = RuntimeVersion(version_label);
                let t_set = Instant::now();
                svc.set_current(kind, &target)
                    .map_err(|e: EnvrError| e.to_string())?;
                let set_ms = t_set.elapsed().as_millis();

                let t_shim = Instant::now();
                ensure_core_shims_for_kind(kind).map_err(|e| e.to_string())?;
                let shim_ms = t_shim.elapsed().as_millis();
                let total_ms = t_total.elapsed().as_millis();
                tracing::info!(
                    kind = ?kind,
                    version = %target.0,
                    set_current_ms = set_ms,
                    ensure_shims_ms = shim_ms,
                    total_ms = total_ms,
                    "gui use_version timing"
                );
                Ok(())
            })
            .await;
        let msg = match res {
            Ok(Ok(())) => EnvCenterMsg::UseFinished(Ok(())),
            Ok(Err(e)) => EnvCenterMsg::UseFinished(Err(e)),
            Err(e) => EnvCenterMsg::UseFinished(Err(e.to_string())),
        };
        Message::EnvCenter(msg)
    })
}

pub fn uninstall_version(kind: RuntimeKind, version_label: String) -> Task<Message> {
    let handle = runtime().handle().clone();
    Task::future(async move {
        let res = handle
            .spawn_blocking(move || -> Result<(), String> {
                let svc = open_runtime_service().map_err(|e: EnvrError| e.to_string())?;
                svc.uninstall(kind, &RuntimeVersion(version_label))
                    .map_err(|e: EnvrError| e.to_string())
            })
            .await;
        let msg = match res {
            Ok(Ok(())) => EnvCenterMsg::UninstallFinished(Ok(())),
            Ok(Err(e)) => EnvCenterMsg::UninstallFinished(Err(e)),
            Err(e) => EnvCenterMsg::UninstallFinished(Err(e.to_string())),
        };
        Message::EnvCenter(msg)
    })
}

pub fn sync_shims_for_kind(kind: RuntimeKind) -> Task<Message> {
    let handle = runtime().handle().clone();
    Task::future(async move {
        let res = handle
            .spawn_blocking(move || -> Result<(), String> {
                ensure_core_shims_for_kind(kind).map_err(|e| e.to_string())
            })
            .await;
        let msg = match res {
            Ok(Ok(())) => EnvCenterMsg::SyncShimsFinished(Ok(())),
            Ok(Err(e)) => EnvCenterMsg::SyncShimsFinished(Err(e)),
            Err(e) => EnvCenterMsg::SyncShimsFinished(Err(e.to_string())),
        };
        Message::EnvCenter(msg)
    })
}

pub fn refresh_dashboard() -> Task<Message> {
    let handle = runtime().handle().clone();
    Task::future(async move {
        let res = handle
            .spawn_blocking(move || -> Result<crate::view::dashboard::DashboardData, String> {
                let svc = open_runtime_service().map_err(|e: EnvrError| e.to_string())?;
                let root = envr_config::settings::resolve_runtime_root()
                    .map_err(|e: EnvrError| e.to_string())?;
                let shims = root.join("shims");

                let shims_empty = if shims.is_dir() {
                    std::fs::read_dir(&shims)
                        .map(|mut d| d.next().is_none())
                        .unwrap_or(true)
                } else {
                    true
                };

                let mut issues = Vec::new();
                let mut recs = Vec::new();

                if !root.exists() {
                    issues.push("runtime root does not exist".to_string());
                    recs.push(format!("create {}", root.display()));
                } else if !runtime_root_writable(&root) {
                    issues.push("runtime root is not writable".to_string());
                    recs.push("fix directory permissions or choose another runtime root".to_string());
                }
                if shims_empty {
                    recs.push("shims directory is empty; run `envr shim sync --globals` and add shims to PATH".to_string());
                }

                let mut rows = Vec::new();
                for kind in runtime_kinds_all() {
                    // Lazy-load Rust: probing Rust calls `rustup`, which creates
                    // `{runtime_root}/runtimes/rust/rustup/settings.toml` on first run.
                    // We only want that side-effect when the user actually opens the Rust page.
                    let (installed, current) = if kind == RuntimeKind::Rust {
                        (0usize, None)
                    } else {
                    let installed = svc
                        .list_installed(kind)
                        .map_err(|e: EnvrError| e.to_string())?;
                        let current =
                            svc.current(kind).map_err(|e: EnvrError| e.to_string())?;
                        (installed.len(), current.map(|v| v.0))
                    };
                    rows.push(crate::view::dashboard::RuntimeRow {
                        kind,
                        installed,
                        current,
                    });
                }

                Ok(crate::view::dashboard::DashboardData {
                    runtime_root: root.to_string_lossy().to_string(),
                    shims_dir: shims.to_string_lossy().to_string(),
                    shims_empty,
                    rows,
                    issues,
                    recommendations: recs,
                })
            })
            .await;

        let msg = match res {
            Ok(Ok(data)) => DashboardMsg::DataLoaded(Ok(data)),
            Ok(Err(e)) => DashboardMsg::DataLoaded(Err(e)),
            Err(e) => DashboardMsg::DataLoaded(Err(e.to_string())),
        };
        Message::Dashboard(msg)
    })
}

fn runtime_root_writable(root: &std::path::Path) -> bool {
    let probe = root.join(".envr-gui-probe");
    match std::fs::write(&probe, b"ok") {
        Ok(()) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

#[cfg(windows)]
fn path_shim_roots() -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let p = std::env::var_os("Path").unwrap_or_default();
    let s = p.to_string_lossy();
    for seg in s.split(';') {
        let mut seg = seg.trim();
        if seg.is_empty() {
            continue;
        }
        // Normalize trailing separators to make matching stable.
        while seg.ends_with('\\') || seg.ends_with('/') {
            seg = seg.trim_end_matches(['\\', '/']).trim();
        }

        let lower = seg.to_ascii_lowercase();
        if lower.contains("envr") && lower.ends_with(r"\shims") {
            let p = std::path::Path::new(seg);
            if let Some(root) = p.parent() {
                out.push(root.to_path_buf());
            }
        }
    }
    out
}

#[cfg(not(windows))]
fn path_shim_roots() -> Vec<std::path::PathBuf> {
    Vec::new()
}

fn find_envr_shim_executable() -> EnvrResult<std::path::PathBuf> {
    let exe = std::env::current_exe()?;
    let dir = exe.parent().ok_or_else(|| {
        EnvrError::Runtime(envr_core::i18n::tr_key(
            "cli.err.shim_exe_no_parent",
            "current_exe 没有父目录",
            "current_exe has no parent directory",
        ))
    })?;

    #[cfg(windows)]
    let candidates = ["envr-shim.exe", "envr-shim"];
    #[cfg(not(windows))]
    let candidates = ["envr-shim"];

    for name in candidates {
        let p = dir.join(name);
        if p.is_file() {
            return Ok(p);
        }
    }

    Err(EnvrError::Runtime(envr_core::i18n::tr_key(
        "cli.err.shim_exe_not_found",
        "在 {path} 旁未找到 envr-shim 可执行文件",
        "envr-shim executable not found next to {path}",
    )))
}

fn ensure_core_shims_for_kind(kind: RuntimeKind) -> EnvrResult<()> {
    let t_total = Instant::now();
    let runtime_root = envr_config::settings::resolve_runtime_root()?;
    let shim_exe = find_envr_shim_executable()?;

    // Always ensure shims under the active `runtime_root`, but also ensure they're
    // written into any `...\\envr\\shims` directories already present in PATH.
    // This makes `node -v` work in a new `cmd.exe` immediately after GUI "切换".
    let mut roots = path_shim_roots();
    roots.push(runtime_root.clone());
    roots.sort();
    roots.dedup();

    // 1) Ensure core shims in all PATH-visible shim dirs.
    let t_core = Instant::now();
    for root in &roots {
        let svc = ShimService::new(root.clone(), shim_exe.clone());
        svc.ensure_shims(kind)?;
    }
    let core_ms = t_core.elapsed().as_millis();

    // 2) For Node/Python/Java: sync global package forwards from the active runtime_root,
    // then copy non-core forward stubs into other PATH-visible shim dirs.
    if matches!(
        kind,
        RuntimeKind::Node | RuntimeKind::Python | RuntimeKind::Java
    ) {
        let runtime_svc = ShimService::new(runtime_root.clone(), shim_exe.clone());
        let t_sync = Instant::now();
        if kind == RuntimeKind::Python {
            if let Err(err) = runtime_svc.sync_python_global_package_shims_fast() {
                tracing::warn!(kind = ?kind, error = %err, "best-effort python global shim sync failed");
            }
        } else if kind == RuntimeKind::Java {
            if let Err(err) = runtime_svc.sync_java_global_package_shims_fast() {
                tracing::warn!(kind = ?kind, error = %err, "best-effort java global shim sync failed");
            }
        } else {
            if let Err(err) = runtime_svc.sync_all_global_package_shims() {
                tracing::warn!(kind = ?kind, error = %err, "best-effort global shim sync failed");
            }
        }
        let sync_ms = t_sync.elapsed().as_millis();
        tracing::info!(kind = ?kind, sync_globals_ms = sync_ms, "shim global sync timing");

        if kind == RuntimeKind::Node {
            let t_copy = Instant::now();
            let from_dir = runtime_root.join("shims");
            let to_core_stems = [
                "node",
                "npm",
                "npx",
                "python",
                "python3",
                "pip",
                "pip3",
                "java",
                "javac",
                "bun",
                "bunx",
                "crystal",
                "lua",
                "luac",
                "r",
                "rscript",
            ];

            for to_root in roots {
                if to_root == runtime_root {
                    continue;
                }
                let to_dir = to_root.join("shims");
                if let Err(err) = std::fs::create_dir_all(&to_dir) {
                    tracing::warn!(to_dir = %to_dir.display(), error = %err, "failed to create shim mirror directory");
                    continue;
                }
                let Ok(entries) = std::fs::read_dir(&from_dir) else {
                    tracing::warn!(from_dir = %from_dir.display(), "failed to read source shim directory");
                    continue;
                };
                for e in entries.flatten() {
                    let path = e.path();
                    if !path.is_file() {
                        continue;
                    }
                    let stem = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_ascii_lowercase();
                    if to_core_stems.contains(&stem.as_str()) {
                        continue;
                    }
                    let dst = to_dir.join(path.file_name().unwrap_or_default());
                    if let Err(err) = std::fs::copy(&path, &dst) {
                        tracing::warn!(
                            src = %path.display(),
                            dst = %dst.display(),
                            error = %err,
                            "failed to copy non-core shim"
                        );
                    }
                }
            }
            let copy_ms = t_copy.elapsed().as_millis();
            tracing::info!(copy_non_core_ms = copy_ms, "shim non-core copy timing");
        }
    }

    tracing::info!(
        kind = ?kind,
        ensure_core_ms = core_ms,
        total_ms = t_total.elapsed().as_millis(),
        "ensure_core_shims_for_kind timing"
    );

    Ok(())
}
