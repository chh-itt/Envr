use serde::{Deserialize, Serialize};

use super::defaults;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZigRuntimeSettings {
    /// When false, the zig shim resolves to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::zig_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for ZigRuntimeSettings {
    fn default() -> Self {
        Self {
            path_proxy_enabled: defaults::zig_path_proxy_enabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JuliaRuntimeSettings {
    /// When false, the julia shim resolves to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::julia_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for JuliaRuntimeSettings {
    fn default() -> Self {
        Self {
            path_proxy_enabled: defaults::julia_path_proxy_enabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JanetRuntimeSettings {
    /// When false, janet/jpm shims resolve to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::janet_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for JanetRuntimeSettings {
    fn default() -> Self {
        Self {
            path_proxy_enabled: defaults::janet_path_proxy_enabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct C3RuntimeSettings {
    /// When false, c3c shim resolves to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::c3_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for C3RuntimeSettings {
    fn default() -> Self {
        Self {
            path_proxy_enabled: defaults::c3_path_proxy_enabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BabashkaRuntimeSettings {
    /// When false, bb shim resolves to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::babashka_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for BabashkaRuntimeSettings {
    fn default() -> Self {
        Self {
            path_proxy_enabled: defaults::babashka_path_proxy_enabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SbclRuntimeSettings {
    /// When false, sbcl shim resolves to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::sbcl_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for SbclRuntimeSettings {
    fn default() -> Self {
        Self {
            path_proxy_enabled: defaults::sbcl_path_proxy_enabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HaxeRuntimeSettings {
    /// When false, haxe/haxelib shims resolve to the next matching binaries on PATH outside envr shims.
    #[serde(default = "defaults::haxe_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for HaxeRuntimeSettings {
    fn default() -> Self {
        Self {
            path_proxy_enabled: defaults::haxe_path_proxy_enabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LuaRuntimeSettings {
    /// When false, lua/luac shims resolve to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::lua_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for LuaRuntimeSettings {
    fn default() -> Self {
        Self {
            path_proxy_enabled: defaults::lua_path_proxy_enabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NimRuntimeSettings {
    /// When false, the nim shim resolves to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::nim_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for NimRuntimeSettings {
    fn default() -> Self {
        Self {
            path_proxy_enabled: defaults::nim_path_proxy_enabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrystalRuntimeSettings {
    /// When false, the crystal shim resolves to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::crystal_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for CrystalRuntimeSettings {
    fn default() -> Self {
        Self {
            path_proxy_enabled: defaults::crystal_path_proxy_enabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PerlRuntimeSettings {
    /// When false, the perl shim resolves to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::perl_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for PerlRuntimeSettings {
    fn default() -> Self {
        Self {
            path_proxy_enabled: defaults::perl_path_proxy_enabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnisonRuntimeSettings {
    /// When false, the ucm shim resolves to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::unison_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for UnisonRuntimeSettings {
    fn default() -> Self {
        Self {
            path_proxy_enabled: defaults::unison_path_proxy_enabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RlangRuntimeSettings {
    /// When false, the `R` / `Rscript` shims resolve to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::rlang_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for RlangRuntimeSettings {
    fn default() -> Self {
        Self {
            path_proxy_enabled: defaults::rlang_path_proxy_enabled(),
        }
    }
}
