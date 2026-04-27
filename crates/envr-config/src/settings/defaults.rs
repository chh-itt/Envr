use super::{FontMode, LocaleMode, MirrorMode, ThemeMode};

pub fn max_concurrent_downloads() -> u32 {
    4
}

pub fn max_bytes_per_sec() -> u64 {
    0
}

pub fn retry_max() -> u32 {
    3
}

pub fn mirror_mode() -> MirrorMode {
    MirrorMode::Auto
}

pub fn prefer_china_mirrors() -> bool {
    false
}

pub fn font_mode() -> FontMode {
    FontMode::Auto
}

pub fn theme_mode() -> ThemeMode {
    ThemeMode::FollowSystem
}

pub fn locale_mode() -> LocaleMode {
    LocaleMode::EnUs
}

pub fn auto_sync_shims_on_use() -> bool {
    true
}

pub fn auto_sync_globals_on_use() -> bool {
    false
}

pub fn auto_sync_windows_path_mirror_on_use() -> bool {
    cfg!(windows)
}

pub fn cache_artifact_ttl_days() -> u32 {
    30
}

pub fn cache_max_size_mb() -> u64 {
    10 * 1024
}

pub fn cache_auto_prune_on_start() -> bool {
    true
}

pub fn downloads_panel_visible() -> bool {
    true
}

pub fn downloads_panel_expanded() -> bool {
    true
}

pub fn downloads_panel_x() -> i32 {
    12
}

pub fn downloads_panel_y() -> i32 {
    12
}

pub fn gui_runtime_cache_auto_update_on_launch() -> bool {
    false
}

pub fn node_path_proxy_enabled() -> bool {
    true
}

pub fn python_path_proxy_enabled() -> bool {
    true
}

pub fn java_path_proxy_enabled() -> bool {
    true
}

pub fn kotlin_path_proxy_enabled() -> bool {
    true
}

pub fn scala_path_proxy_enabled() -> bool {
    true
}

pub fn clojure_path_proxy_enabled() -> bool {
    true
}

pub fn groovy_path_proxy_enabled() -> bool {
    true
}

pub fn terraform_path_proxy_enabled() -> bool {
    true
}

pub fn v_path_proxy_enabled() -> bool {
    true
}

pub fn odin_path_proxy_enabled() -> bool {
    true
}

pub fn purescript_path_proxy_enabled() -> bool {
    true
}
pub fn elm_path_proxy_enabled() -> bool {
    true
}
pub fn gleam_path_proxy_enabled() -> bool {
    true
}
pub fn racket_path_proxy_enabled() -> bool {
    true
}

pub fn dart_path_proxy_enabled() -> bool {
    true
}

pub fn flutter_path_proxy_enabled() -> bool {
    true
}

pub fn go_path_proxy_enabled() -> bool {
    true
}

pub fn php_path_proxy_enabled() -> bool {
    true
}

pub fn deno_path_proxy_enabled() -> bool {
    true
}

pub fn bun_path_proxy_enabled() -> bool {
    true
}

pub fn dotnet_path_proxy_enabled() -> bool {
    true
}

pub fn zig_path_proxy_enabled() -> bool {
    true
}

pub fn julia_path_proxy_enabled() -> bool {
    true
}

pub fn janet_path_proxy_enabled() -> bool {
    true
}

pub fn c3_path_proxy_enabled() -> bool {
    true
}

pub fn babashka_path_proxy_enabled() -> bool {
    true
}

pub fn sbcl_path_proxy_enabled() -> bool {
    true
}

pub fn haxe_path_proxy_enabled() -> bool {
    true
}

pub fn lua_path_proxy_enabled() -> bool {
    true
}

pub fn luau_path_proxy_enabled() -> bool {
    true
}

pub fn nim_path_proxy_enabled() -> bool {
    true
}

pub fn crystal_path_proxy_enabled() -> bool {
    true
}

pub fn perl_path_proxy_enabled() -> bool {
    true
}

pub fn unison_path_proxy_enabled() -> bool {
    true
}

pub fn rlang_path_proxy_enabled() -> bool {
    true
}

pub fn ruby_path_proxy_enabled() -> bool {
    true
}

pub fn elixir_path_proxy_enabled() -> bool {
    true
}

pub fn erlang_path_proxy_enabled() -> bool {
    true
}
