//! Async bridge: run blocking `RuntimeService` calls on the Tokio blocking pool.

use envr_domain::runtime::{InstallRequest, RuntimeKind, RuntimeVersion, VersionSpec};
use envr_error::EnvrError;

use crate::app::Message;
use crate::runtime_exec::runtime;
use crate::service::open_runtime_service;
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
