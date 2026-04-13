//! T915: GUI-oriented `tr_key` strings resolve from embedded `locales/*` (no full GUI harness).

use envr_core::i18n::{Locale, RestoreLocale, lock_locale_for_test, set, tr_key};

#[test]
fn gui_route_labels_match_locale_files() {
    let _lock = lock_locale_for_test();
    let _restore = RestoreLocale::new();

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
