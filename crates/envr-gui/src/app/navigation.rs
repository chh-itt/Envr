use iced::Task;

use super::{AppState, Message, Route};
use crate::gui_ops;
use crate::view::env_center::env_center_clear_unified_list_render_state;
use crate::view::settings::SettingsMsg;

use super::env_center_ops::runtime_page_enter_tasks;
use super::persist_settings::settings_path;

pub(crate) fn handle_navigate(state: &mut AppState, route: Route) -> Task<Message> {
    tracing::debug!(?route, "navigate");
    let leaving_runtime = state.route() == Route::Runtime && route != Route::Runtime;
    state.route = route;
    if leaving_runtime {
        env_center_clear_unified_list_render_state(&mut state.env_center);
    }
    if route == Route::Runtime {
        return runtime_page_enter_tasks(state);
    }
    if route == Route::Dashboard {
        state.dashboard.busy = true;
        state.dashboard.last_error = None;
        return gui_ops::refresh_dashboard();
    }
    if matches!(route, Route::Settings | Route::RuntimeConfig) {
        state.settings.last_message = Some(envr_core::i18n::tr_key(
            "gui.app.loading",
            "正在加载…",
            "Loading…",
        ));
        let path = settings_path();
        return Task::perform(
            async move {
                envr_config::settings::Settings::load_or_default_from(&path)
                    .map_err(|e| e.to_string())
            },
            |res| Message::Settings(SettingsMsg::DiskLoaded(res)),
        );
    }
    Task::none()
}
