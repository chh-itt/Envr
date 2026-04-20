//! PATH proxy toggles derived from [`crate::settings::RuntimeSettings`], shared by GUI and shims.

use crate::settings::RuntimeSettings;
use envr_domain::runtime::{RuntimeKind, runtime_descriptor};

/// Copy of per-runtime PATH-proxy flags for hot paths (e.g. shim resolution) without holding full settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathProxyRuntimeSnapshot {
    pub node: bool,
    pub python: bool,
    pub java: bool,
    pub kotlin: bool,
    pub scala: bool,
    pub clojure: bool,
    pub go: bool,
    pub php: bool,
    pub deno: bool,
    pub bun: bool,
    pub dotnet: bool,
    pub zig: bool,
    pub julia: bool,
    pub lua: bool,
    pub nim: bool,
    pub crystal: bool,
    pub r: bool,
    pub ruby: bool,
    pub elixir: bool,
    pub erlang: bool,
}

impl Default for PathProxyRuntimeSnapshot {
    fn default() -> Self {
        Self {
            node: true,
            python: true,
            java: true,
            kotlin: true,
            scala: true,
            clojure: true,
            go: true,
            php: true,
            deno: true,
            bun: true,
            dotnet: true,
            zig: true,
            julia: true,
            lua: true,
            nim: true,
            crystal: true,
            r: true,
            ruby: true,
            elixir: true,
            erlang: true,
        }
    }
}

impl From<&RuntimeSettings> for PathProxyRuntimeSnapshot {
    fn from(r: &RuntimeSettings) -> Self {
        Self {
            node: r.node.path_proxy_enabled,
            python: r.python.path_proxy_enabled,
            java: r.java.path_proxy_enabled,
            kotlin: r.kotlin.path_proxy_enabled,
            scala: r.scala.path_proxy_enabled,
            clojure: r.clojure.path_proxy_enabled,
            go: r.go.path_proxy_enabled,
            php: r.php.path_proxy_enabled,
            deno: r.deno.path_proxy_enabled,
            bun: r.bun.path_proxy_enabled,
            dotnet: r.dotnet.path_proxy_enabled,
            zig: r.zig.path_proxy_enabled,
            julia: r.julia.path_proxy_enabled,
            lua: r.lua.path_proxy_enabled,
            nim: r.nim.path_proxy_enabled,
            crystal: r.crystal.path_proxy_enabled,
            r: r.r.path_proxy_enabled,
            ruby: r.ruby.path_proxy_enabled,
            elixir: r.elixir.path_proxy_enabled,
            erlang: r.erlang.path_proxy_enabled,
        }
    }
}

impl PathProxyRuntimeSnapshot {
    /// Effective PATH-proxy toggle for `kind`. [`None`] when the runtime has no toggle (e.g. Rust).
    pub fn enabled_for_kind(self, kind: RuntimeKind) -> Option<bool> {
        match kind {
            RuntimeKind::Rust => None,
            RuntimeKind::Node => Some(self.node),
            RuntimeKind::Python => Some(self.python),
            RuntimeKind::Java => Some(self.java),
            RuntimeKind::Kotlin => Some(self.kotlin),
            RuntimeKind::Scala => Some(self.scala),
            RuntimeKind::Clojure => Some(self.clojure),
            RuntimeKind::Go => Some(self.go),
            RuntimeKind::Ruby => Some(self.ruby),
            RuntimeKind::Elixir => Some(self.elixir),
            RuntimeKind::Erlang => Some(self.erlang),
            RuntimeKind::Php => Some(self.php),
            RuntimeKind::Deno => Some(self.deno),
            RuntimeKind::Bun => Some(self.bun),
            RuntimeKind::Dotnet => Some(self.dotnet),
            RuntimeKind::Zig => Some(self.zig),
            RuntimeKind::Julia => Some(self.julia),
            RuntimeKind::Lua => Some(self.lua),
            RuntimeKind::Nim => Some(self.nim),
            RuntimeKind::Crystal => Some(self.crystal),
            RuntimeKind::RLang => Some(self.r),
        }
    }
}

impl RuntimeSettings {
    /// Effective PATH-proxy toggle for `kind`. [`None`] when the runtime has no toggle (e.g. Rust).
    pub fn path_proxy_enabled_for_kind(&self, kind: RuntimeKind) -> Option<bool> {
        PathProxyRuntimeSnapshot::from(self).enabled_for_kind(kind)
    }

    /// Compact snapshot for shim hot paths (single copy from [`RuntimeSettings`]).
    #[inline]
    pub fn path_proxy_snapshot(&self) -> PathProxyRuntimeSnapshot {
        self.into()
    }
}

/// When PATH proxy is off for a runtime that supports it, env-center disables Use / Install & Use.
#[inline]
pub fn path_proxy_blocks_managed_use(kind: RuntimeKind, settings: &RuntimeSettings) -> bool {
    if !runtime_descriptor(kind).supports_path_proxy {
        return false;
    }
    matches!(settings.path_proxy_enabled_for_kind(kind), Some(false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_proxy_blocks_only_when_supported_and_disabled() {
        let mut s = RuntimeSettings::default();
        s.nim.path_proxy_enabled = false;
        assert!(path_proxy_blocks_managed_use(RuntimeKind::Nim, &s));
        s.nim.path_proxy_enabled = true;
        assert!(!path_proxy_blocks_managed_use(RuntimeKind::Nim, &s));
        s.crystal.path_proxy_enabled = false;
        assert!(path_proxy_blocks_managed_use(RuntimeKind::Crystal, &s));
        s.crystal.path_proxy_enabled = true;
        assert!(!path_proxy_blocks_managed_use(RuntimeKind::Crystal, &s));
        s.r.path_proxy_enabled = false;
        assert!(path_proxy_blocks_managed_use(RuntimeKind::RLang, &s));
        s.r.path_proxy_enabled = true;
        assert!(!path_proxy_blocks_managed_use(RuntimeKind::RLang, &s));
        assert!(!path_proxy_blocks_managed_use(RuntimeKind::Rust, &s));
    }
}
