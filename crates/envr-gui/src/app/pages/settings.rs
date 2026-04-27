use super::super::{AppState, Message, Route};
use crate::view::settings::SettingsMsg;
use envr_ui::theme::Srgb;
use iced::Task;

async fn browse_runtime_root_folder(
    start: Option<std::path::PathBuf>,
) -> Option<std::path::PathBuf> {
    tokio::task::spawn_blocking(move || {
        let mut dlg = rfd::FileDialog::new();
        if let Some(p) = start
            && p.is_dir()
        {
            dlg = dlg.set_directory(p);
        }
        dlg.pick_folder()
    })
    .await
    .ok()
    .flatten()
}

pub(crate) fn handle_settings(state: &mut AppState, msg: SettingsMsg) -> Task<Message> {
    match msg {
        SettingsMsg::BrowseRuntimeRoot => {
            let start = {
                let t = state.settings.runtime_root_draft.trim();
                if t.is_empty() {
                    None
                } else {
                    let p = std::path::PathBuf::from(t);
                    p.is_dir().then_some(p)
                }
            };
            Task::perform(browse_runtime_root_folder(start), |r| {
                Message::Settings(SettingsMsg::RuntimeRootBrowseResult(r))
            })
        }
        SettingsMsg::RuntimeRootBrowseResult(pb) => {
            if let Some(pb) = pb {
                state.settings.runtime_root_draft = pb.to_string_lossy().to_string();
                let rr = state.settings.runtime_root_draft.trim();
                state.settings.draft.paths.runtime_root = if rr.is_empty() {
                    None
                } else {
                    Some(rr.to_string())
                };
                state.settings.last_message = Some(envr_core::i18n::tr_key(
                    "gui.app.saving",
                    "正在保存…",
                    "Saving…",
                ));
                return super::super::persist_settings_draft_task(state);
            }
            Task::none()
        }
        SettingsMsg::ClearRuntimeRoot => {
            state.settings.runtime_root_draft.clear();
            state.settings.draft.paths.runtime_root = None;
            state.settings.last_message = Some(envr_core::i18n::tr_key(
                "gui.app.saving",
                "正在保存…",
                "Saving…",
            ));
            super::super::persist_settings_draft_task(state)
        }
        SettingsMsg::MaxConcEdit(s) => {
            state.settings.max_conc_text = s;
            Task::none()
        }
        SettingsMsg::MaxBpsEdit(s) => {
            state.settings.max_bps_text = s;
            Task::none()
        }
        SettingsMsg::RetryEdit(s) => {
            state.settings.retry_text = s;
            Task::none()
        }
        SettingsMsg::SetPreferChinaMirrors(v) => {
            state.settings.draft.mirror.prefer_china_mirrors = v;
            Task::none()
        }
        SettingsMsg::SetCleanup(v) => {
            state
                .settings
                .draft
                .behavior
                .cleanup_downloads_after_install = v;
            Task::none()
        }
        SettingsMsg::SetFontMode(m) => {
            state.settings.draft.appearance.font.mode = m;
            Task::none()
        }
        SettingsMsg::FontFamilyEdit(s) => {
            state.settings.font_family_draft = s;
            Task::none()
        }
        SettingsMsg::PickFontFamily(s) => {
            state.settings.font_family_draft = s;
            Task::none()
        }
        SettingsMsg::SetThemeMode(m) => {
            state.settings.draft.appearance.theme_mode = m;
            Task::none()
        }
        SettingsMsg::AccentColorEdit(s) => {
            state.settings.accent_color_draft = s;
            let t = state.settings.accent_color_draft.trim();
            state.settings.draft.appearance.accent_color = if t.is_empty() {
                None
            } else {
                Srgb::from_hex(t).ok().map(|_| t.to_string())
            };
            Task::none()
        }
        SettingsMsg::SetLocaleMode(m) => {
            state.settings.locale_mode_draft = m;
            // Apply immediately so all views re-render with new language.
            let mut st = state.settings.draft.clone();
            st.i18n.locale = m;
            state.locale = envr_core::i18n::locale_from_settings(&st);
            Task::none()
        }
        SettingsMsg::SetRuntimeCacheAutoUpdateOnLaunch(v) => {
            state.settings.draft.gui.runtime_cache_auto_update_on_launch = v;
            state.settings.last_message = Some(envr_core::i18n::tr_key(
                "gui.app.saving",
                "正在保存…",
                "Saving…",
            ));
            super::super::persist_settings_draft_task(state)
        }
        SettingsMsg::Save => {
            state.settings.last_message = Some(envr_core::i18n::tr_key(
                "gui.app.saving",
                "正在保存…",
                "Saving…",
            ));
            super::super::persist_settings_draft_task(state)
        }
        SettingsMsg::ReloadDisk => {
            state.settings.last_message = Some(envr_core::i18n::tr_key(
                "gui.app.loading",
                "正在加载…",
                "Loading…",
            ));
            let path = super::super::settings_path();
            Task::perform(
                async move {
                    envr_config::settings::Settings::load_or_default_from(&path)
                        .map_err(|e| e.to_string())
                },
                |res| Message::Settings(SettingsMsg::DiskLoaded(res)),
            )
        }
        SettingsMsg::DiskLoaded(res) => {
            match res {
                Ok(st) => {
                    // If the user picked a folder but never got a successful save, disk can still be
                    // empty while `runtime_root_draft` holds the path — reloading would wipe it.
                    let unsaved_rr = state.settings.runtime_root_draft.trim().to_string();
                    let had_unsaved = !unsaved_rr.is_empty();
                    let disk_rr_empty = st
                        .paths
                        .runtime_root
                        .as_deref()
                        .is_none_or(|r| r.trim().is_empty());

                    state.settings.cache.set_cached(st);
                    if let Err(e) = state.settings.sync_from_cache() {
                        state.settings.last_message = Some(format!(
                            "{}: {e}",
                            envr_core::i18n::tr_key(
                                "gui.app.sync_failed",
                                "同步失败",
                                "Sync failed"
                            )
                        ));
                    } else if had_unsaved && disk_rr_empty {
                        state.settings.runtime_root_draft = unsaved_rr.clone();
                        state.settings.draft.paths.runtime_root = Some(unsaved_rr.clone());
                        let mut merged = state.settings.cache.snapshot().clone();
                        merged.paths.runtime_root = Some(unsaved_rr);
                        state.settings.cache.set_cached(merged.clone());
                        let _ = state.settings.sync_from_cache();
                        state.settings.last_message = None;
                    } else {
                        state.settings.last_message = Some(envr_core::i18n::tr_key(
                            "gui.app.reloaded_from_disk",
                            "已从磁盘重新加载。",
                            "Reloaded from disk.",
                        ));
                    }
                }
                Err(e) => {
                    state.settings.last_message = Some(format!(
                        "{}: {e}",
                        envr_core::i18n::tr_key(
                            "gui.app.reload_failed",
                            "重新加载失败",
                            "Reload failed"
                        )
                    ));
                }
            }
            Task::none()
        }
        SettingsMsg::DiskSaved(res) => {
            match res {
                Ok(st) => {
                    state.settings.cache.set_cached(st.clone());
                    if let Err(e) = state.settings.sync_from_cache() {
                        state.settings.last_message = Some(format!(
                            "{}: {e}",
                            envr_core::i18n::tr_key(
                                "gui.app.sync_failed",
                                "同步失败",
                                "Sync failed"
                            )
                        ));
                    } else {
                        state.settings.last_message = Some(envr_core::i18n::tr_key(
                            "gui.app.saved_settings_toml",
                            "已保存到 settings.toml。",
                            "Saved.",
                        ));
                    }
                    // Apply global download bandwidth cap immediately for the current process.
                    let _ = envr_download::set_global_download_limit(Some(
                        state.settings.cache.snapshot().download.max_bytes_per_sec,
                    ));
                    let _ = envr_download::set_global_download_concurrency_limit(Some(
                        state.settings.cache.snapshot().download.max_concurrent_downloads as usize,
                    ));
                    super::super::sync_go_env_center_drafts_from_settings(state);
                    if matches!(state.route(), Route::Runtime) {
                        let k = state.env_center.kind;
                        if envr_domain::runtime::runtime_descriptor(k).supports_remote_latest {
                            return Task::batch([
                                super::super::gui_ops::refresh_runtimes(k),
                                envr_domain::runtime::unified_major_list_rollout_enabled(k)
                                    .then_some(
                                        super::super::gui_ops::load_unified_major_rows_cached(k),
                                    )
                                    .unwrap_or_else(Task::none),
                                envr_domain::runtime::unified_major_list_rollout_enabled(k)
                                    .then_some(super::super::gui_ops::refresh_unified_major_rows(k))
                                    .unwrap_or_else(Task::none),
                            ]);
                        }
                    }
                }
                Err(e) => {
                    state.settings.last_message = Some(format!(
                        "{}: {e}",
                        envr_core::i18n::tr_key("gui.app.save_failed", "保存失败", "Save failed")
                    ));
                }
            }
            Task::none()
        }
    }
}
