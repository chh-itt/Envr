use std::time::Duration;

use envr_config::settings::ThemeMode;
use iced::Subscription;
use iced::window;

use super::{AppState, Message, Route};
use crate::view::downloads::DownloadMsg;

/// Motion ticks, download progress, theme follow, a11y poll, and window resize.
pub(crate) fn shell_subscription(state: &AppState) -> Subscription<Message> {
    let runtime_skeleton = matches!(state.route(), Route::Runtime)
        && state.env_center.installed.is_empty()
        && (state.env_center.busy
            || (envr_domain::runtime::runtime_descriptor(state.env_center.kind)
                .supports_remote_latest
                && state
                    .env_center
                    .unified_major_rows_by_kind
                    .get(&state.env_center.kind)
                    .is_none_or(|rows| rows.is_empty())));
    let need_motion = state.downloads.needs_motion_tick()
        || state.downloads.title_drag_armed_since.is_some()
        || runtime_skeleton;
    let maybe_motion = need_motion
        .then(|| iced::time::every(Duration::from_millis(32)))
        .map(|s| s.map(|_| Message::MotionTick));

    let progress_only = state.downloads.needs_tick() && !need_motion;
    let maybe_tick = progress_only
        .then(|| iced::time::every(Duration::from_millis(400)))
        .map(|s| s.map(|_| Message::Download(DownloadMsg::Tick)));

    let need_pointer_events =
        state.downloads.dragging || state.downloads.title_drag_armed_since.is_some();
    let maybe_events = need_pointer_events
        .then(|| iced::event::listen().map(|e| Message::Download(DownloadMsg::Event(e))));

    let theme_poll = (state.settings.draft.appearance.theme_mode == ThemeMode::FollowSystem)
        .then(|| iced::time::every(Duration::from_secs(1)))
        .map(|s| s.map(|_| Message::ThemePollTick));

    let mut subs = Vec::new();
    if let Some(t) = maybe_motion {
        subs.push(t);
    }
    if let Some(t) = maybe_tick {
        subs.push(t);
    }
    if let Some(e) = maybe_events {
        subs.push(e);
    }
    if let Some(t) = theme_poll {
        subs.push(t);
    }
    subs.push(iced::time::every(Duration::from_secs(3)).map(|_| Message::A11yPollTick));
    subs.push(window::resize_events().map(|(_id, s)| Message::WindowResized(s)));
    Subscription::batch(subs)
}
