use serde::{Deserialize, Serialize};

use super::{NpmRegistryMode, defaults};

/// PHP download source preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PhpDownloadSource {
    #[default]
    Auto,
    Domestic,
    Official,
}

/// Deno binary zip source (`dl.deno.land` vs npmmirror binary mirror).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DenoDownloadSource {
    /// Prefer npmmirror when UI locale suggests China, else official.
    #[default]
    Auto,
    Domestic,
    Official,
}

/// Windows PHP build flavor preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PhpWindowsBuildFlavor {
    #[default]
    Nts,
    Ts,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhpRuntimeSettings {
    #[serde(default)]
    pub download_source: PhpDownloadSource,
    #[serde(default)]
    pub windows_build: PhpWindowsBuildFlavor,
    /// When false, php shim resolves to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::php_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for PhpRuntimeSettings {
    fn default() -> Self {
        Self {
            download_source: PhpDownloadSource::default(),
            windows_build: PhpWindowsBuildFlavor::default(),
            path_proxy_enabled: defaults::php_path_proxy_enabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DenoRuntimeSettings {
    #[serde(default)]
    pub download_source: DenoDownloadSource,
    /// Single preset for both `NPM_CONFIG_REGISTRY` and `JSR_URL` (see `deno_package_registry_env`).
    #[serde(default)]
    pub package_source: NpmRegistryMode,
    /// When false, `deno` shim resolves to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::deno_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for DenoRuntimeSettings {
    fn default() -> Self {
        Self {
            download_source: DenoDownloadSource::default(),
            package_source: NpmRegistryMode::default(),
            path_proxy_enabled: defaults::deno_path_proxy_enabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BunRuntimeSettings {
    /// Single preset for Bun package source env injection.
    #[serde(default)]
    pub package_source: NpmRegistryMode,
    /// When false, bun/bunx shims resolve to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::bun_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
    /// Optional override for Bun global bin directory (defaults to `bun pm bin -g`).
    ///
    /// This affects shim sync for global Bun executables.
    #[serde(default)]
    pub global_bin_dir: Option<String>,
}

impl Default for BunRuntimeSettings {
    fn default() -> Self {
        Self {
            package_source: NpmRegistryMode::default(),
            path_proxy_enabled: defaults::bun_path_proxy_enabled(),
            global_bin_dir: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DotnetRuntimeSettings {
    /// When false, dotnet shim resolves to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::dotnet_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for DotnetRuntimeSettings {
    fn default() -> Self {
        Self {
            path_proxy_enabled: defaults::dotnet_path_proxy_enabled(),
        }
    }
}
