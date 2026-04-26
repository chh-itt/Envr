use super::{PhpWindowsBuildFlavor, Settings, settings_path_from_platform};

fn load_runtime_bool_from_disk<F>(read: F) -> bool
where
    F: FnOnce(&Settings) -> bool,
{
    let Ok(platform) = envr_platform::paths::current_platform_paths() else {
        return true;
    };
    let path = settings_path_from_platform(&platform);
    Settings::load_or_default_from(&path)
        .map(|s| read(&s))
        .unwrap_or(true)
}

/// Read [`super::NodeRuntimeSettings::path_proxy_enabled`] from disk; on error defaults to `true`.
pub fn node_path_proxy_enabled_from_disk() -> bool {
    load_runtime_bool_from_disk(|s| s.runtime.node.path_proxy_enabled)
}

/// Read [`super::PythonRuntimeSettings::path_proxy_enabled`] from disk; on error defaults to `true`.
pub fn python_path_proxy_enabled_from_disk() -> bool {
    load_runtime_bool_from_disk(|s| s.runtime.python.path_proxy_enabled)
}

/// Read [`super::JavaRuntimeSettings::path_proxy_enabled`] from disk; on error defaults to `true`.
pub fn java_path_proxy_enabled_from_disk() -> bool {
    load_runtime_bool_from_disk(|s| s.runtime.java.path_proxy_enabled)
}

/// Read [`super::GoRuntimeSettings::path_proxy_enabled`] from disk; on error defaults to `true`.
pub fn go_path_proxy_enabled_from_disk() -> bool {
    load_runtime_bool_from_disk(|s| s.runtime.go.path_proxy_enabled)
}

/// Read [`super::PhpRuntimeSettings::path_proxy_enabled`] from disk; on error defaults to `true`.
pub fn php_path_proxy_enabled_from_disk() -> bool {
    load_runtime_bool_from_disk(|s| s.runtime.php.path_proxy_enabled)
}

/// Read [`super::DenoRuntimeSettings::path_proxy_enabled`] from disk; on error defaults to `true`.
pub fn deno_path_proxy_enabled_from_disk() -> bool {
    load_runtime_bool_from_disk(|s| s.runtime.deno.path_proxy_enabled)
}

/// Read [`super::BunRuntimeSettings::path_proxy_enabled`] from disk; on error defaults to `true`.
pub fn bun_path_proxy_enabled_from_disk() -> bool {
    load_runtime_bool_from_disk(|s| s.runtime.bun.path_proxy_enabled)
}

/// Read [`super::DotnetRuntimeSettings::path_proxy_enabled`] from disk; on error defaults to `true`.
pub fn dotnet_path_proxy_enabled_from_disk() -> bool {
    load_runtime_bool_from_disk(|s| s.runtime.dotnet.path_proxy_enabled)
}

/// Read [`super::JuliaRuntimeSettings::path_proxy_enabled`] from disk; on error defaults to `true`.
pub fn julia_path_proxy_enabled_from_disk() -> bool {
    load_runtime_bool_from_disk(|s| s.runtime.julia.path_proxy_enabled)
}

/// Read [`super::LuaRuntimeSettings::path_proxy_enabled`] from disk; on error defaults to `true`.
pub fn lua_path_proxy_enabled_from_disk() -> bool {
    load_runtime_bool_from_disk(|s| s.runtime.lua.path_proxy_enabled)
}

/// Read [`super::LuauRuntimeSettings::path_proxy_enabled`] from disk; on error defaults to `true`.
pub fn luau_path_proxy_enabled_from_disk() -> bool {
    load_runtime_bool_from_disk(|s| s.runtime.luau.path_proxy_enabled)
}

/// Read [`super::PerlRuntimeSettings::path_proxy_enabled`] from disk; on error defaults to `true`.
pub fn perl_path_proxy_enabled_from_disk() -> bool {
    load_runtime_bool_from_disk(|s| s.runtime.perl.path_proxy_enabled)
}

/// Read [`super::CrystalRuntimeSettings::path_proxy_enabled`] from disk; on error defaults to `true`.
pub fn crystal_path_proxy_enabled_from_disk() -> bool {
    load_runtime_bool_from_disk(|s| s.runtime.crystal.path_proxy_enabled)
}

/// Read [`super::NimRuntimeSettings::path_proxy_enabled`] from disk; on error defaults to `true`.
pub fn nim_path_proxy_enabled_from_disk() -> bool {
    load_runtime_bool_from_disk(|s| s.runtime.nim.path_proxy_enabled)
}

/// Read [`super::RlangRuntimeSettings::path_proxy_enabled`] from disk; on error defaults to `true`.
pub fn rlang_path_proxy_enabled_from_disk() -> bool {
    load_runtime_bool_from_disk(|s| s.runtime.r.path_proxy_enabled)
}

/// Read [`super::ZigRuntimeSettings::path_proxy_enabled`] from disk; on error defaults to `true`.
pub fn zig_path_proxy_enabled_from_disk() -> bool {
    load_runtime_bool_from_disk(|s| s.runtime.zig.path_proxy_enabled)
}

/// Read [`super::VRuntimeSettings::path_proxy_enabled`] from disk; on error defaults to `true`.
pub fn v_path_proxy_enabled_from_disk() -> bool {
    load_runtime_bool_from_disk(|s| s.runtime.v.path_proxy_enabled)
}

/// Read [`super::DartRuntimeSettings::path_proxy_enabled`] from disk; on error defaults to `true`.
pub fn dart_path_proxy_enabled_from_disk() -> bool {
    load_runtime_bool_from_disk(|s| s.runtime.dart.path_proxy_enabled)
}

/// Read [`super::FlutterRuntimeSettings::path_proxy_enabled`] from disk; on error defaults to `true`.
pub fn flutter_path_proxy_enabled_from_disk() -> bool {
    load_runtime_bool_from_disk(|s| s.runtime.flutter.path_proxy_enabled)
}

/// Read [`super::RubyRuntimeSettings::path_proxy_enabled`] from disk; on error defaults to `true`.
pub fn ruby_path_proxy_enabled_from_disk() -> bool {
    load_runtime_bool_from_disk(|s| s.runtime.ruby.path_proxy_enabled)
}

/// Read [`super::ElixirRuntimeSettings::path_proxy_enabled`] from disk; on error defaults to `true`.
pub fn elixir_path_proxy_enabled_from_disk() -> bool {
    load_runtime_bool_from_disk(|s| s.runtime.elixir.path_proxy_enabled)
}

/// Read [`super::ErlangRuntimeSettings::path_proxy_enabled`] from disk; on error defaults to `true`.
pub fn erlang_path_proxy_enabled_from_disk() -> bool {
    load_runtime_bool_from_disk(|s| s.runtime.erlang.path_proxy_enabled)
}

/// Read [`super::PhpRuntimeSettings::windows_build`] from disk: `true` = TS, `false` = NTS.
pub fn php_windows_build_want_ts_from_disk() -> bool {
    let Ok(platform) = envr_platform::paths::current_platform_paths() else {
        return false;
    };
    let path = settings_path_from_platform(&platform);
    Settings::load_or_default_from(&path)
        .map(|s| matches!(s.runtime.php.windows_build, PhpWindowsBuildFlavor::Ts))
        .unwrap_or(false)
}
