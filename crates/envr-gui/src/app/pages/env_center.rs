use super::super::*;
use crate::view::env_center::EnvCenterMsg;
use iced::Task;
use std::sync::atomic::Ordering;

use super::downloads::{
    enqueue_op_job_running, enqueue_runtime_install_job, maybe_start_queued_jobs,
};
use crate::view::downloads::JobState;

pub(crate) fn persist_jvm_path_proxy_toggle(
    state: &mut AppState,
    kind: envr_domain::runtime::RuntimeKind,
    on: bool,
) -> Task<Message> {
    match kind {
        envr_domain::runtime::RuntimeKind::Kotlin => {
            persist_path_proxy_toggle(state, kind, on, |st, on| {
                st.runtime.kotlin.path_proxy_enabled = on
            })
        }
        envr_domain::runtime::RuntimeKind::Scala => {
            persist_path_proxy_toggle(state, kind, on, |st, on| {
                st.runtime.scala.path_proxy_enabled = on
            })
        }
        envr_domain::runtime::RuntimeKind::Clojure => {
            persist_path_proxy_toggle(state, kind, on, |st, on| {
                st.runtime.clojure.path_proxy_enabled = on
            })
        }
        envr_domain::runtime::RuntimeKind::Groovy => {
            persist_path_proxy_toggle(state, kind, on, |st, on| {
                st.runtime.groovy.path_proxy_enabled = on
            })
        }
        _ => Task::none(),
    }
}

pub(crate) fn handle_env_center(state: &mut AppState, msg: EnvCenterMsg) -> Task<Message> {
    // Moved verbatim from `app.rs` (route-page update extraction).
    match msg {
        EnvCenterMsg::PickKind(k) => {
            if state.env_center.kind == k {
                return Task::none();
            }
            state.env_center.kind = k;
            state.env_center.jvm_java_hints.clear();
            state.env_center.remote_error = None;
            state.env_center.rust_status = None;
            state.env_center.rust_components.clear();
            state.env_center.rust_targets.clear();
            state.env_center.install_input.clear();
            state.env_center.direct_install_input.clear();
            state.env_center.unified_expanded_major_keys.clear();
            state.env_center.active_install_job_ids.clear();
            if k == envr_domain::runtime::RuntimeKind::Elixir {
                state.env_center.elixir_prereq_error = None;
            }
            if k == envr_domain::runtime::RuntimeKind::Go {
                sync_go_env_center_drafts_from_settings(state);
            }
            if k == envr_domain::runtime::RuntimeKind::Bun {
                sync_bun_env_center_drafts_from_settings(state);
            }
            if k == envr_domain::runtime::RuntimeKind::Rust {
                state.env_center.busy = true;
                recompute_env_center_derived(state);
                return Task::batch([
                    gui_ops::rust_refresh(),
                    gui_ops::rust_load_components(),
                    gui_ops::rust_load_targets(),
                ]);
            }
            if envr_domain::runtime::runtime_descriptor(k).supports_remote_latest {
                recompute_env_center_derived(state);
                return Task::batch([
                    gui_ops::refresh_runtimes(k),
                    envr_domain::runtime::unified_major_list_rollout_enabled(k)
                        .then_some(gui_ops::load_unified_major_rows_cached(k))
                        .unwrap_or_else(Task::none),
                    envr_domain::runtime::unified_major_list_rollout_enabled(k)
                        .then_some(gui_ops::refresh_unified_major_rows(k))
                        .unwrap_or_else(Task::none),
                    (k == envr_domain::runtime::RuntimeKind::Elixir)
                        .then_some(gui_ops::check_elixir_prereqs())
                        .unwrap_or_else(Task::none),
                    env_center_jvm_check_task(k),
                ]);
            }
            recompute_env_center_derived(state);
            Task::batch([gui_ops::refresh_runtimes(k)])
        }
        EnvCenterMsg::ElixirPrereqChecked(res) => {
            state.env_center.elixir_prereq_error = res.err();
            Task::none()
        }
        EnvCenterMsg::JvmJavaChecked(kind, res) => {
            set_jvm_java_hint(state, kind, res);
            Task::none()
        }
        EnvCenterMsg::InstallInput(s) => {
            state.env_center.install_input =
                sanitize_runtime_filter_input(state.env_center.kind, &s);
            recompute_env_center_derived(state);
            Task::none()
        }
        EnvCenterMsg::DirectInstallInput(s) => {
            state.env_center.direct_install_input = s;
            Task::none()
        }
        EnvCenterMsg::DataLoaded(res) => {
            state.env_center.busy = false;
            match res {
                Ok((list, cur, php_global_ts)) => {
                    state.env_center.installed = list;
                    state.env_center.current = cur;
                    state.env_center.php_global_current_want_ts = php_global_ts;
                }
                Err(e) => state.error = Some(e),
            }
            recompute_env_center_derived(state);
            Task::none()
        }
        EnvCenterMsg::UnifiedMajorRowsCached(kind, res) => {
            if !envr_domain::runtime::unified_major_list_rollout_enabled(kind) {
                return Task::none();
            }
            match res {
                Ok(rows) => {
                    state.env_center.remote_error = None;
                    state
                        .env_center
                        .unified_major_rows_by_kind
                        .insert(kind, rows);
                }
                Err(e) => {
                    state.env_center.remote_error = Some(e);
                }
            }
            Task::none()
        }
        EnvCenterMsg::UnifiedMajorRowsRefreshed(kind, res) => {
            if !envr_domain::runtime::unified_major_list_rollout_enabled(kind) {
                return Task::none();
            }
            match res {
                Ok(rows) => {
                    state.env_center.remote_error = None;
                    state
                        .env_center
                        .unified_major_rows_by_kind
                        .insert(kind, rows);
                }
                Err(e) => {
                    state.env_center.remote_error = Some(e);
                }
            }
            Task::none()
        }
        EnvCenterMsg::UnifiedChildrenCached(kind, major_key, res) => {
            if !envr_domain::runtime::unified_major_list_rollout_enabled(kind) {
                return Task::none();
            }
            match res {
                Ok(rows) => {
                    state.env_center.remote_error = None;
                    state
                        .env_center
                        .unified_children_rows_by_kind_major
                        .insert((kind, major_key), rows);
                }
                Err(e) => {
                    state.env_center.remote_error = Some(e);
                }
            }
            Task::none()
        }
        EnvCenterMsg::UnifiedChildrenRefreshed(kind, major_key, res) => {
            if !envr_domain::runtime::unified_major_list_rollout_enabled(kind) {
                return Task::none();
            }
            match res {
                Ok(rows) => {
                    state.env_center.remote_error = None;
                    state
                        .env_center
                        .unified_children_rows_by_kind_major
                        .insert((kind, major_key), rows);
                }
                Err(e) => {
                    state.env_center.remote_error = Some(e);
                }
            }
            Task::none()
        }
        EnvCenterMsg::ToggleUnifiedMajorExpanded(major_key) => {
            if !state
                .env_center
                .unified_expanded_major_keys
                .remove(&major_key)
            {
                state
                    .env_center
                    .unified_expanded_major_keys
                    .insert(major_key.clone());
                let kind = state.env_center.kind;
                return Task::batch([
                    gui_ops::load_unified_children_cached(kind, major_key.clone()),
                    gui_ops::refresh_unified_children(kind, major_key),
                ]);
            }
            Task::none()
        }
        EnvCenterMsg::SubmitDirectInstall => {
            let spec = state.env_center.direct_install_input.trim().to_string();
            if spec.is_empty() || !direct_install_spec_ok(&spec) {
                return Task::none();
            }
            if duplicate_runtime_install_blocked(state, state.env_center.kind, &spec) {
                return Task::none();
            }
            if bun_direct_spec_blocked_on_windows(state.env_center.kind, &spec) {
                state.error = Some(envr_core::i18n::tr_key(
                    "gui.runtime.bun.win_0x_blocked",
                    "Windows 不支持 Bun 0.x，请输入 1.x 及以上版本。",
                    "Bun 0.x is unavailable on Windows. Please enter version 1.x or newer.",
                ));
                return Task::none();
            }
            if deno_direct_spec_blocked(state.env_center.kind, &spec) {
                state.error = Some(envr_core::i18n::tr_key(
                    "gui.runtime.deno.0x_blocked",
                    "Deno 0.x 不受托管安装支持，请输入 1.x/2.x 版本。",
                    "Deno 0.x is not supported for managed install. Please enter a 1.x/2.x version.",
                ));
                return Task::none();
            }
            state.error = None;
            let (id, _downloaded, _total, _cancel) = enqueue_runtime_install_job(
                state,
                runtime_install_task_label(state.env_center.kind, &spec, false),
                true,
                crate::view::downloads::DownloadJobPayload::RuntimeInstall {
                    kind: state.env_center.kind,
                    spec,
                    resolve_precheck: true,
                    install_and_use: false,
                },
            );
            state.env_center.active_install_job_ids.insert(id);
            maybe_start_queued_jobs(state)
        }
        EnvCenterMsg::SubmitDirectInstallAndUse => {
            if runtime_path_proxy_blocks_use(state) {
                return Task::none();
            }
            let spec = state.env_center.direct_install_input.trim().to_string();
            if spec.is_empty() || !direct_install_spec_ok(&spec) {
                return Task::none();
            }
            if duplicate_runtime_install_blocked(state, state.env_center.kind, &spec) {
                return Task::none();
            }
            if bun_direct_spec_blocked_on_windows(state.env_center.kind, &spec) {
                state.error = Some(envr_core::i18n::tr_key(
                    "gui.runtime.bun.win_0x_blocked",
                    "Windows 不支持 Bun 0.x，请输入 1.x 及以上版本。",
                    "Bun 0.x is unavailable on Windows. Please enter version 1.x or newer.",
                ));
                return Task::none();
            }
            if deno_direct_spec_blocked(state.env_center.kind, &spec) {
                state.error = Some(envr_core::i18n::tr_key(
                    "gui.runtime.deno.0x_blocked",
                    "Deno 0.x 不受托管安装支持，请输入 1.x/2.x 版本。",
                    "Deno 0.x is not supported for managed install. Please enter a 1.x/2.x version.",
                ));
                return Task::none();
            }
            state.error = None;
            let (id, _downloaded, _total, _cancel) = enqueue_runtime_install_job(
                state,
                runtime_install_task_label(state.env_center.kind, &spec, true),
                true,
                crate::view::downloads::DownloadJobPayload::RuntimeInstall {
                    kind: state.env_center.kind,
                    spec,
                    resolve_precheck: true,
                    install_and_use: true,
                },
            );
            state.env_center.active_install_job_ids.insert(id);
            maybe_start_queued_jobs(state)
        }
        EnvCenterMsg::SubmitInstall(spec) => {
            if spec.trim().is_empty() {
                return Task::none();
            }
            if duplicate_runtime_install_blocked(state, state.env_center.kind, &spec) {
                return Task::none();
            }
            state.error = None;
            let (id, _downloaded, _total, _cancel) = enqueue_runtime_install_job(
                state,
                runtime_install_task_label(state.env_center.kind, &spec, false),
                true,
                crate::view::downloads::DownloadJobPayload::RuntimeInstall {
                    kind: state.env_center.kind,
                    spec,
                    resolve_precheck: false,
                    install_and_use: false,
                },
            );
            state.env_center.active_install_job_ids.insert(id);
            maybe_start_queued_jobs(state)
        }
        EnvCenterMsg::SubmitInstallAndUse(spec) => {
            if runtime_path_proxy_blocks_use(state) {
                return Task::none();
            }
            if spec.trim().is_empty() {
                return Task::none();
            }
            if duplicate_runtime_install_blocked(state, state.env_center.kind, &spec) {
                return Task::none();
            }
            state.error = None;
            let (id, _downloaded, _total, _cancel) = enqueue_runtime_install_job(
                state,
                runtime_install_task_label(state.env_center.kind, &spec, true),
                true,
                crate::view::downloads::DownloadJobPayload::RuntimeInstall {
                    kind: state.env_center.kind,
                    spec,
                    resolve_precheck: false,
                    install_and_use: true,
                },
            );
            state.env_center.active_install_job_ids.insert(id);
            maybe_start_queued_jobs(state)
        }
        EnvCenterMsg::InstallFinished {
            job_id,
            kind,
            result,
        } => {
            state.env_center.active_install_job_ids.remove(&job_id);
            if let Some(j) = state.downloads.jobs.iter_mut().find(|j| j.id == job_id) {
                match &result {
                    Ok(_) => {
                        j.state = JobState::Done;
                        let d = j.downloaded.load(Ordering::Relaxed);
                        let t = j.total.load(Ordering::Relaxed);
                        if t == 0 || d < t {
                            j.total.store(d.max(t), Ordering::Relaxed);
                            j.downloaded.store(d.max(t), Ordering::Relaxed);
                        }
                    }
                    Err(e) => {
                        if looks_like_user_cancelled(e) {
                            j.state = JobState::Cancelled;
                            j.last_error = None;
                        } else {
                            j.state = JobState::Failed;
                            j.last_error = Some(e.clone());
                        }
                    }
                }
            }
            match &result {
                Ok(v) => {
                    tracing::info!(version = %v.0, "gui install ok");
                    // `install_input` is now the search keyword; keep it for better feedback.
                }
                Err(e) => {
                    if !looks_like_user_cancelled(e) {
                        state.error = Some(e.clone());
                    }
                }
            }
            match result {
                Ok(_) => {
                    let mut tasks = vec![
                        gui_ops::sync_shims_for_kind(kind),
                        gui_ops::refresh_runtimes(kind),
                    ];
                    tasks.push(env_center_jvm_check_task(kind));
                    tasks.push(maybe_start_queued_jobs(state));
                    Task::batch(tasks)
                }
                Err(_) => Task::batch([
                    gui_ops::refresh_runtimes(kind),
                    maybe_start_queued_jobs(state),
                ]),
            }
        }
        EnvCenterMsg::SubmitUse(v) => {
            if state.env_center.busy || runtime_path_proxy_blocks_use(state) {
                return Task::none();
            }
            state.env_center.busy = true;
            state.error = None;
            gui_ops::use_version(state.env_center.kind, v)
        }
        EnvCenterMsg::UseFinished(res) => {
            state.env_center.busy = false;
            if let Err(e) = res {
                state.error = Some(e);
            }
            let kind = state.env_center.kind;
            if envr_domain::jvm_hosted::is_jvm_hosted_runtime(
                envr_domain::runtime::runtime_descriptor(kind).key,
            ) {
                Task::batch([
                    gui_ops::refresh_runtimes(kind),
                    env_center_jvm_check_task(kind),
                ])
            } else {
                gui_ops::refresh_runtimes(kind)
            }
        }
        EnvCenterMsg::SubmitUninstall(v) => {
            if state.env_center.busy {
                return Task::none();
            }
            state.env_center.busy = true;
            state.error = None;
            gui_ops::uninstall_version(state.env_center.kind, v)
        }
        EnvCenterMsg::UninstallFinished(res) => {
            state.env_center.busy = false;
            if let Err(e) = res {
                state.error = Some(e);
            }
            gui_ops::refresh_runtimes(state.env_center.kind)
        }
        EnvCenterMsg::SetNodeDownloadSource(src) => {
            persist_runtime_settings_update(state, move |st| {
                st.runtime.node.download_source = src;
            })
        }
        EnvCenterMsg::SetNpmRegistryMode(mode) => {
            persist_runtime_settings_update(state, move |st| {
                st.runtime.node.npm_registry_mode = mode;
            })
        }
        EnvCenterMsg::SetNodePathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Node,
            on,
            |st, on| st.runtime.node.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetPythonDownloadSource(src) => {
            persist_runtime_settings_update(state, move |st| {
                st.runtime.python.download_source = src;
            })
        }
        EnvCenterMsg::SetPipRegistryMode(mode) => {
            persist_runtime_settings_update(state, move |st| {
                st.runtime.python.pip_registry_mode = mode;
            })
        }
        EnvCenterMsg::SetPythonPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Python,
            on,
            |st, on| st.runtime.python.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetJavaDistro(distro) => {
            state.env_center.remote_error = None;
            persist_runtime_settings_update(state, move |st| {
                st.runtime.java.current_distro = distro;
            })
        }
        EnvCenterMsg::SetJavaDownloadSource(src) => {
            persist_runtime_settings_update(state, move |st| {
                st.runtime.java.download_source = src;
            })
        }
        EnvCenterMsg::SetJavaPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Java,
            on,
            |st, on| st.runtime.java.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetJvmPathProxy(kind, on) => persist_jvm_path_proxy_toggle(state, kind, on),
        EnvCenterMsg::SetTerraformPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Terraform,
            on,
            |st, on| st.runtime.terraform.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetVPathProxy(on) => {
            persist_path_proxy_toggle(state, envr_domain::runtime::RuntimeKind::V, on, |st, on| {
                st.runtime.v.path_proxy_enabled = on
            })
        }
        EnvCenterMsg::SetOdinPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Odin,
            on,
            |st, on| st.runtime.odin.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetPurescriptPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Purescript,
            on,
            |st, on| st.runtime.purescript.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetElmPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Elm,
            on,
            |st, on| st.runtime.elm.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetGleamPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Gleam,
            on,
            |st, on| st.runtime.gleam.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetRacketPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Racket,
            on,
            |st, on| st.runtime.racket.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetDartPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Dart,
            on,
            |st, on| st.runtime.dart.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetFlutterPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Flutter,
            on,
            |st, on| st.runtime.flutter.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetGoDownloadSource(src) => {
            persist_runtime_settings_update(state, move |st| {
                st.runtime.go.download_source = src;
            })
        }
        EnvCenterMsg::SetGoProxyMode(mode) => {
            let draft_proxy = state.env_center.go_proxy_custom_draft.trim().to_string();
            persist_runtime_settings_update(state, move |st| {
                st.runtime.go.proxy_mode = mode;
                if mode == envr_config::settings::GoProxyMode::Custom {
                    let existing = st.runtime.go.proxy_custom.as_deref().unwrap_or("").trim();
                    let legacy = st.runtime.go.goproxy.as_deref().unwrap_or("").trim();
                    let chosen = if !draft_proxy.is_empty() {
                        draft_proxy
                    } else if !existing.is_empty() {
                        existing.to_string()
                    } else if !legacy.is_empty() {
                        legacy.to_string()
                    } else {
                        "https://proxy.golang.org,direct".to_string()
                    };
                    st.runtime.go.proxy_custom = Some(chosen);
                }
            })
        }
        EnvCenterMsg::SetGoPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Go,
            on,
            |st, on| st.runtime.go.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetGoProxyCustomDraft(s) => {
            state.env_center.go_proxy_custom_draft = s;
            Task::none()
        }
        EnvCenterMsg::SetGoPrivatePatternsDraft(s) => {
            state.env_center.go_private_patterns_draft = s;
            Task::none()
        }
        EnvCenterMsg::ApplyGoNetworkSettings => {
            let p = state.env_center.go_proxy_custom_draft.trim().to_string();
            let pr = state
                .env_center
                .go_private_patterns_draft
                .trim()
                .to_string();
            persist_runtime_settings_update(state, move |st| {
                st.runtime.go.proxy_custom = if p.is_empty() { None } else { Some(p) };
                st.runtime.go.private_patterns = if pr.is_empty() { None } else { Some(pr) };
            })
        }
        EnvCenterMsg::SetRustDownloadSource(src) => {
            persist_runtime_settings_update(state, move |st| {
                st.runtime.rust.download_source = src;
            })
        }
        EnvCenterMsg::SetPhpDownloadSource(src) => {
            mark_unified_major_rows_dirty_for_kind(state, envr_domain::runtime::RuntimeKind::Php);
            Task::batch([
                gui_ops::invalidate_unified_list_disk_cache(envr_domain::runtime::RuntimeKind::Php),
                persist_runtime_settings_update(state, move |st| {
                    st.runtime.php.download_source = src;
                }),
            ])
        }
        EnvCenterMsg::SetPhpWindowsBuild(flavor) => {
            mark_unified_major_rows_dirty_for_kind(state, envr_domain::runtime::RuntimeKind::Php);
            Task::batch([
                gui_ops::invalidate_unified_list_disk_cache(envr_domain::runtime::RuntimeKind::Php),
                persist_runtime_settings_update(state, move |st| {
                    st.runtime.php.windows_build = flavor;
                }),
            ])
        }
        EnvCenterMsg::SetPhpPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Php,
            on,
            |st, on| st.runtime.php.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetDenoDownloadSource(src) => {
            persist_runtime_settings_update(state, move |st| {
                st.runtime.deno.download_source = src;
            })
        }
        EnvCenterMsg::SetDenoPackageSource(mode) => {
            persist_runtime_settings_update(state, move |st| {
                st.runtime.deno.package_source = mode;
            })
        }
        EnvCenterMsg::SetDenoPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Deno,
            on,
            |st, on| st.runtime.deno.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetBunPackageSource(mode) => {
            persist_runtime_settings_update(state, move |st| {
                st.runtime.bun.package_source = mode;
            })
        }
        EnvCenterMsg::SetBunPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Bun,
            on,
            |st, on| st.runtime.bun.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetDotnetPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Dotnet,
            on,
            |st, on| st.runtime.dotnet.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetZigPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Zig,
            on,
            |st, on| st.runtime.zig.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetJuliaPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Julia,
            on,
            |st, on| st.runtime.julia.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetJanetPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Janet,
            on,
            |st, on| st.runtime.janet.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetC3PathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::C3,
            on,
            |st, on| st.runtime.c3.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetBabashkaPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Babashka,
            on,
            |st, on| st.runtime.babashka.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetSbclPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Sbcl,
            on,
            |st, on| st.runtime.sbcl.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetHaxePathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Haxe,
            on,
            |st, on| st.runtime.haxe.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetLuaPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Lua,
            on,
            |st, on| st.runtime.lua.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetNimPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Nim,
            on,
            |st, on| st.runtime.nim.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetCrystalPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Crystal,
            on,
            |st, on| st.runtime.crystal.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetPerlPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Perl,
            on,
            |st, on| st.runtime.perl.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetUnisonPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Unison,
            on,
            |st, on| st.runtime.unison.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetRLangPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::RLang,
            on,
            |st, on| st.runtime.r.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetRubyPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Ruby,
            on,
            |st, on| st.runtime.ruby.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetElixirPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Elixir,
            on,
            |st, on| st.runtime.elixir.path_proxy_enabled = on,
        ),
        EnvCenterMsg::SetErlangPathProxy(on) => persist_path_proxy_toggle(
            state,
            envr_domain::runtime::RuntimeKind::Erlang,
            on,
            |st, on| st.runtime.erlang.path_proxy_enabled = on,
        ),
        EnvCenterMsg::BunGlobalBinDirEdit(s) => {
            state.env_center.bun_global_bin_dir_draft = s;
            Task::none()
        }
        EnvCenterMsg::ApplyBunGlobalBinDir => {
            let t = state.env_center.bun_global_bin_dir_draft.trim().to_string();
            persist_runtime_settings_update(state, move |st| {
                st.runtime.bun.global_bin_dir = if t.is_empty() { None } else { Some(t) };
            })
        }
        EnvCenterMsg::RustRefresh => {
            state.env_center.busy = true;
            state.env_center.remote_error = None;
            state.env_center.rust_status = None;
            state.env_center.rust_components.clear();
            state.env_center.rust_targets.clear();
            Task::batch([
                gui_ops::rust_refresh(),
                gui_ops::rust_load_components(),
                gui_ops::rust_load_targets(),
            ])
        }
        EnvCenterMsg::RustStatusLoaded(res) => {
            state.env_center.busy = false;
            match res {
                Ok(s) => {
                    state.env_center.remote_error = None;
                    state.env_center.rust_status = Some(s);
                }
                Err(e) => {
                    state.env_center.remote_error = Some(e);
                }
            }
            Task::none()
        }
        EnvCenterMsg::RustSelectTab(tab) => {
            state.env_center.rust_tab = tab;
            Task::none()
        }
        EnvCenterMsg::RustComponentsLoaded(res) => {
            if let Ok(list) = res {
                state.env_center.rust_components = list;
            }
            Task::none()
        }
        EnvCenterMsg::RustTargetsLoaded(res) => {
            if let Ok(list) = res {
                state.env_center.rust_targets = list;
            }
            Task::none()
        }
        EnvCenterMsg::RustChannelInstallOrSwitch(channel) => {
            if state.env_center.busy {
                return Task::none();
            }
            state.env_center.busy = true;
            state.error = None;
            let label = rust_runtime_task_label("channel", &channel);
            let (id, _downloaded, _total, _cancel) = enqueue_op_job_running(state, label, false);
            state.env_center.op_job_id = Some(id);
            gui_ops::rust_channel_install_or_switch(channel)
        }
        EnvCenterMsg::RustUpdateCurrent => {
            if state.env_center.busy {
                return Task::none();
            }
            state.env_center.busy = true;
            state.error = None;
            let label = rust_runtime_task_label("update", "");
            let (id, _downloaded, _total, _cancel) = enqueue_op_job_running(state, label, false);
            state.env_center.op_job_id = Some(id);
            gui_ops::rust_update_current()
        }
        EnvCenterMsg::RustManagedInstallStable => {
            if state.env_center.busy {
                return Task::none();
            }
            state.env_center.busy = true;
            state.error = None;
            let label = envr_core::i18n::tr_key(
                "gui.runtime.rust.managed_install_task",
                "正在安装托管 rustup（stable）…",
                "Installing managed rustup (stable)…",
            );
            let (id, downloaded, total, cancel) = enqueue_op_job_running(state, label, true);
            state.env_center.op_job_id = Some(id);
            gui_ops::rust_managed_install_stable(downloaded, total, cancel)
        }
        EnvCenterMsg::RustManagedUninstall => {
            if state.env_center.busy {
                return Task::none();
            }
            state.env_center.busy = true;
            state.error = None;
            let label = rust_runtime_task_label("managed_uninstall", "");
            let (id, _downloaded, _total, _cancel) = enqueue_op_job_running(state, label, false);
            state.env_center.op_job_id = Some(id);
            gui_ops::rust_managed_uninstall()
        }
        EnvCenterMsg::RustComponentToggle(name, install) => {
            if state.env_center.busy {
                return Task::none();
            }
            state.env_center.busy = true;
            state.error = None;
            let label = if install {
                rust_runtime_task_label("component_install", &name)
            } else {
                rust_runtime_task_label("component_uninstall", &name)
            };
            let (id, _downloaded, _total, _cancel) = enqueue_op_job_running(state, label, false);
            state.env_center.op_job_id = Some(id);
            gui_ops::rust_component_toggle(name, install)
        }
        EnvCenterMsg::RustTargetToggle(name, install) => {
            if state.env_center.busy {
                return Task::none();
            }
            state.env_center.busy = true;
            state.error = None;
            let label = if install {
                rust_runtime_task_label("target_install", &name)
            } else {
                rust_runtime_task_label("target_uninstall", &name)
            };
            let (id, _downloaded, _total, _cancel) = enqueue_op_job_running(state, label, false);
            state.env_center.op_job_id = Some(id);
            gui_ops::rust_target_toggle(name, install)
        }
        EnvCenterMsg::RustOpFinished(res) => {
            state.env_center.busy = false;
            if let Some(id) = state.env_center.op_job_id.take()
                && let Some(j) = state.downloads.jobs.iter_mut().find(|j| j.id == id)
            {
                match &res {
                    Ok(()) => {
                        j.state = JobState::Done;
                        let d = j.downloaded.load(Ordering::Relaxed);
                        let t = j.total.load(Ordering::Relaxed);
                        if t == 0 || d < t {
                            j.total.store(d.max(t), Ordering::Relaxed);
                            j.downloaded.store(d.max(t), Ordering::Relaxed);
                        }
                    }
                    Err(e) => {
                        if looks_like_user_cancelled(e) {
                            j.state = JobState::Cancelled;
                            j.last_error = None;
                        } else {
                            j.state = JobState::Failed;
                            j.last_error = Some(e.clone());
                        }
                    }
                }
            }
            if let Err(e) = &res
                && !looks_like_user_cancelled(e)
            {
                state.error = Some(e.clone());
            }
            Task::batch([
                gui_ops::rust_refresh(),
                gui_ops::rust_load_components(),
                gui_ops::rust_load_targets(),
            ])
        }
        EnvCenterMsg::SyncShimsFinished(res) => {
            if let Err(e) = res {
                state.error = Some(e);
            }
            Task::none()
        }
    }
}
