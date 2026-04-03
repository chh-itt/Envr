//! Async bridge: run blocking `RuntimeService` calls on the Tokio blocking pool.

use envr_domain::runtime::{InstallRequest, RuntimeKind, RuntimeVersion, VersionSpec};
use envr_core::shim_service::ShimService;
use envr_error::{EnvrError, EnvrResult};

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

pub fn load_remote_latest_disk_snapshot(kind: RuntimeKind) -> Task<Message> {
    let handle = runtime().handle().clone();
    Task::future(async move {
        let res = handle
            .spawn_blocking(move || -> Vec<RuntimeVersion> {
                let Ok(svc) = open_runtime_service() else {
                    return Vec::new();
                };
                svc.try_load_remote_latest_per_major_from_disk(kind)
            })
            .await;

        Message::EnvCenter(EnvCenterMsg::RemoteLatestDiskSnapshot(
            res.unwrap_or_default(),
        ))
    })
}

pub fn refresh_remote_latest_per_major(kind: RuntimeKind) -> Task<Message> {
    let handle = runtime().handle().clone();
    Task::future(async move {
        let res = handle
            .spawn_blocking(move || -> Result<Vec<RuntimeVersion>, String> {
                if kind != RuntimeKind::Node {
                    return Ok(Vec::new());
                }
                let svc = open_runtime_service().map_err(|e: EnvrError| e.to_string())?;
                svc.list_remote_latest_per_major(kind)
                    .map_err(|e: EnvrError| e.to_string())
            })
            .await;

        let msg = match res {
            Ok(Ok(list)) => EnvCenterMsg::RemoteLatestRefreshed(Ok(list)),
            Ok(Err(e)) => EnvCenterMsg::RemoteLatestRefreshed(Err(e)),
            Err(e) => EnvCenterMsg::RemoteLatestRefreshed(Err(e.to_string())),
        };
        Message::EnvCenter(msg)
    })
}

/// Like [`install_version`], but runs [`RuntimeService::resolve`] first so invalid specs fail before download.
pub fn install_version_with_resolve_precheck(kind: RuntimeKind, spec: String) -> Task<Message> {
    let handle = runtime().handle().clone();
    Task::future(async move {
        let res = handle
            .spawn_blocking(move || -> Result<RuntimeVersion, String> {
                let svc = open_runtime_service().map_err(|e: EnvrError| e.to_string())?;
                let vs = VersionSpec(spec.clone());
                svc.resolve(kind, &vs)
                    .map_err(|e: EnvrError| e.to_string())?;
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

/// Like [`install_then_use`], but resolves the spec before install (fail fast on bad input).
pub fn install_then_use_with_resolve_precheck(kind: RuntimeKind, spec: String) -> Task<Message> {
    let handle = runtime().handle().clone();
    Task::future(async move {
        let res = handle
            .spawn_blocking(move || -> Result<RuntimeVersion, String> {
                let svc = open_runtime_service().map_err(|e: EnvrError| e.to_string())?;
                let vs = VersionSpec(spec.clone());
                svc.resolve(kind, &vs)
                    .map_err(|e: EnvrError| e.to_string())?;
                let installed = svc
                    .install(
                        kind,
                        &InstallRequest {
                            spec: VersionSpec(spec),
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
                let svc = open_runtime_service().map_err(|e: EnvrError| e.to_string())?;
                let spec = VersionSpec(version_label);
                let resolved = svc
                    .resolve(kind, &spec)
                    .map_err(|e: EnvrError| e.to_string())?;
                svc.set_current(kind, &resolved.version)
                    .map_err(|e: EnvrError| e.to_string())?;

                ensure_core_shims_for_kind(kind).map_err(|e| e.to_string())?;
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
    for root in &roots {
        let svc = ShimService::new(root.clone(), shim_exe.clone());
        svc.ensure_shims(kind)?;
    }

    // 2) For Node: sync global package forwards from the active runtime_root,
    // then copy non-core forward stubs into other PATH-visible shim dirs.
    if kind == RuntimeKind::Node {
        let runtime_svc = ShimService::new(runtime_root.clone(), shim_exe.clone());
        let _ = runtime_svc.sync_all_global_package_shims();

        let from_dir = runtime_root.join("shims");
        let to_core_stems = [
            "node", "npm", "npx", "python", "python3", "pip", "pip3", "java", "javac", "bun",
            "bunx",
        ];

        for to_root in roots {
            if to_root == runtime_root {
                continue;
            }
            let to_dir = to_root.join("shims");
            let _ = std::fs::create_dir_all(&to_dir);
            let Ok(entries) = std::fs::read_dir(&from_dir) else {
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
                let _ = std::fs::copy(&path, &dst);
            }
        }
    }

    Ok(())
}
