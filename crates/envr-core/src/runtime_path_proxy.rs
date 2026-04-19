//! PATH proxy: single place to map [`RuntimeKind`] → settings, shared by GUI and other surfaces.

use envr_config::settings::RuntimeSettings;
use envr_domain::runtime::{RuntimeKind, runtime_descriptor};

/// Settings-backed PATH-proxy toggle for `kind`.
///
/// Returns [`None`] for runtimes that have no such toggle (e.g. Rust).
pub fn path_proxy_enabled_for_kind(kind: RuntimeKind, settings: &RuntimeSettings) -> Option<bool> {
    match kind {
        RuntimeKind::Node => Some(settings.node.path_proxy_enabled),
        RuntimeKind::Python => Some(settings.python.path_proxy_enabled),
        RuntimeKind::Java => Some(settings.java.path_proxy_enabled),
        RuntimeKind::Go => Some(settings.go.path_proxy_enabled),
        RuntimeKind::Ruby => Some(settings.ruby.path_proxy_enabled),
        RuntimeKind::Elixir => Some(settings.elixir.path_proxy_enabled),
        RuntimeKind::Erlang => Some(settings.erlang.path_proxy_enabled),
        RuntimeKind::Php => Some(settings.php.path_proxy_enabled),
        RuntimeKind::Deno => Some(settings.deno.path_proxy_enabled),
        RuntimeKind::Bun => Some(settings.bun.path_proxy_enabled),
        RuntimeKind::Dotnet => Some(settings.dotnet.path_proxy_enabled),
        RuntimeKind::Zig => Some(settings.zig.path_proxy_enabled),
        RuntimeKind::Julia => Some(settings.julia.path_proxy_enabled),
        RuntimeKind::Nim => Some(settings.nim.path_proxy_enabled),
        RuntimeKind::Rust => None,
    }
}

/// When PATH proxy is off for a runtime that supports it, env-center disables Use / Install & Use.
#[inline]
pub fn path_proxy_blocks_managed_use(kind: RuntimeKind, settings: &RuntimeSettings) -> bool {
    if !runtime_descriptor(kind).supports_path_proxy {
        return false;
    }
    matches!(path_proxy_enabled_for_kind(kind, settings), Some(false))
}

#[cfg(test)]
mod tests {
    use super::*;
    use envr_domain::runtime::RuntimeKind;

    #[test]
    fn path_proxy_blocks_only_when_supported_and_disabled() {
        let mut s = RuntimeSettings::default();
        s.nim.path_proxy_enabled = false;
        assert!(path_proxy_blocks_managed_use(RuntimeKind::Nim, &s));
        s.nim.path_proxy_enabled = true;
        assert!(!path_proxy_blocks_managed_use(RuntimeKind::Nim, &s));
        assert!(!path_proxy_blocks_managed_use(RuntimeKind::Rust, &s));
    }
}
