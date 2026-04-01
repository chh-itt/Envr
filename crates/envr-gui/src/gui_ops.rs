//! Async bridge: run blocking `RuntimeService` calls on the Tokio blocking pool.

use envr_domain::runtime::{InstallRequest, RuntimeKind, RuntimeVersion, VersionSpec};
use envr_error::EnvrError;

use crate::app::Message;
use crate::runtime_exec::runtime;
use crate::service::open_runtime_service;
use crate::view::dashboard::DashboardMsg;
use crate::view::env_center::EnvCenterMsg;
use iced::Task;

pub fn refresh_runtimes(kind: RuntimeKind) -> Task<Message> {
    let handle = runtime().handle().clone();
    Task::future(async move {
        let res = handle
            .spawn_blocking(
                move || -> Result<(Vec<RuntimeVersion>, Option<RuntimeVersion>), String> {
                    let svc = open_runtime_service().map_err(|e: EnvrError| e.to_string())?;
                    let installed = svc
                        .list_installed(kind)
                        .map_err(|e: EnvrError| e.to_string())?;
                    let current = svc.current(kind).map_err(|e: EnvrError| e.to_string())?;
                    Ok((installed, current))
                },
            )
            .await;
        let msg = match res {
            Ok(Ok(data)) => EnvCenterMsg::DataLoaded(Ok(data)),
            Ok(Err(e)) => EnvCenterMsg::DataLoaded(Err(e)),
            Err(e) => EnvCenterMsg::DataLoaded(Err(e.to_string())),
        };
        Message::EnvCenter(msg)
    })
}

pub fn install_version(kind: RuntimeKind, spec: String) -> Task<Message> {
    let handle = runtime().handle().clone();
    Task::future(async move {
        let res = handle
            .spawn_blocking(move || -> Result<RuntimeVersion, String> {
                let svc = open_runtime_service().map_err(|e: EnvrError| e.to_string())?;
                svc.install(
                    kind,
                    &InstallRequest {
                        spec: VersionSpec(spec),
                    },
                )
                .map_err(|e: EnvrError| e.to_string())
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

pub fn install_then_use(kind: RuntimeKind, spec: String) -> Task<Message> {
    let handle = runtime().handle().clone();
    Task::future(async move {
        let res = handle
            .spawn_blocking(move || -> Result<RuntimeVersion, String> {
                let svc = open_runtime_service().map_err(|e: EnvrError| e.to_string())?;
                let installed = svc
                    .install(
                        kind,
                        &InstallRequest {
                            spec: VersionSpec(spec.clone()),
                        },
                    )
                    .map_err(|e: EnvrError| e.to_string())?;

                // Some providers set `current` during install; ensure current is set to resolved spec.
                let resolved = svc
                    .resolve(kind, &VersionSpec(spec))
                    .map_err(|e: EnvrError| e.to_string())?;
                svc.set_current(kind, &resolved.version)
                    .map_err(|e: EnvrError| e.to_string())?;

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
                let svc = open_runtime_service().map_err(|e: EnvrError| e.to_string())?;
                let spec = VersionSpec(version_label);
                let resolved = svc
                    .resolve(kind, &spec)
                    .map_err(|e: EnvrError| e.to_string())?;
                svc.set_current(kind, &resolved.version)
                    .map_err(|e: EnvrError| e.to_string())
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
                for kind in [
                    RuntimeKind::Node,
                    RuntimeKind::Python,
                    RuntimeKind::Java,
                    RuntimeKind::Go,
                    RuntimeKind::Rust,
                    RuntimeKind::Php,
                    RuntimeKind::Deno,
                    RuntimeKind::Bun,
                ] {
                    let installed = svc
                        .list_installed(kind)
                        .map_err(|e: EnvrError| e.to_string())?;
                    let current = svc.current(kind).map_err(|e: EnvrError| e.to_string())?;
                    rows.push(crate::view::dashboard::RuntimeRow {
                        kind,
                        installed: installed.len(),
                        current: current.map(|v| v.0),
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
