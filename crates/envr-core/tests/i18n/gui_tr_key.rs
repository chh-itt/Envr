//! T915: GUI-oriented `tr_key` strings resolve from embedded `locales/*` (no full GUI harness).

use envr_core::i18n::{Locale, set, tr_key};
use std::sync::Mutex;

static I18N_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn gui_route_labels_match_locale_files() {
    let _g = I18N_LOCK.lock().expect("i18n lock");

    set(Locale::ZhCn);
    assert_eq!(
        tr_key("gui.route.dashboard", "wrong-zh", "wrong-en"),
        "仪表盘"
    );
    assert_eq!(tr_key("gui.route.settings", "wrong-zh", "wrong-en"), "设置");

    set(Locale::EnUs);
    assert_eq!(
        tr_key("gui.route.dashboard", "wrong-zh", "wrong-en"),
        "Dashboard"
    );
    assert_eq!(
        tr_key("gui.route.settings", "wrong-zh", "wrong-en"),
        "Settings"
    );
}
