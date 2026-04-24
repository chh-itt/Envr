use serde::{Deserialize, Serialize};

use super::defaults;

/// Node.js distribution index (`index.json`) selection for installs / remote lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NodeDownloadSource {
    /// Prefer npmmirror when UI locale suggests China, else nodejs.org.
    #[default]
    Auto,
    /// npmmirror.com mirror (China).
    Domestic,
    /// nodejs.org official.
    Official,
}

/// How GUI manages `npm config registry` (Restore leaves user `.npmrc` untouched).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NpmRegistryMode {
    #[default]
    Auto,
    Domestic,
    Official,
    /// Use a user-provided URL in `runtime.node.npm_registry_url_custom`.
    Custom,
    /// Do not run `npm config set`; user may use a custom registry.
    Restore,
}

/// Python bootstrap source choice for `get-pip.py` retrieval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PythonDownloadSource {
    #[default]
    Auto,
    Domestic,
    Official,
}

/// How `pip` bootstrap should resolve package index during `get-pip.py`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PipRegistryMode {
    #[default]
    Auto,
    Domestic,
    Official,
    /// Use a user-provided URL in `runtime.python.pip_index_url_custom`.
    Custom,
    /// Do not force `--index-url` during bootstrap.
    Restore,
}

/// Go toolchain download source preference (go.dev vs China mirror).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GoDownloadSource {
    #[default]
    Auto,
    Domestic,
    Official,
}

/// How `GOPROXY` should be injected in `envr env`/`run`/`exec` when Go is in scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GoProxyMode {
    #[default]
    Auto,
    Domestic,
    Official,
    /// Disable module proxy (`GOPROXY=direct`).
    Direct,
    /// Use user-provided `runtime.go.proxy_custom`.
    Custom,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeRuntimeSettings {
    #[serde(default)]
    pub download_source: NodeDownloadSource,
    #[serde(default)]
    pub npm_registry_mode: NpmRegistryMode,
    /// Used when `npm_registry_mode = "custom"`.
    #[serde(default)]
    pub npm_registry_url_custom: Option<String>,
    /// When false, Node/npm/npx shims resolve to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::node_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for NodeRuntimeSettings {
    fn default() -> Self {
        Self {
            download_source: NodeDownloadSource::default(),
            npm_registry_mode: NpmRegistryMode::default(),
            npm_registry_url_custom: None,
            path_proxy_enabled: defaults::node_path_proxy_enabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PythonRuntimeSettings {
    #[serde(default)]
    pub download_source: PythonDownloadSource,
    /// Windows distribution choice: `auto` (prefer full NuGet), `nuget`, or `embeddable`.
    #[serde(default)]
    pub windows_distribution: PythonWindowsDistribution,
    #[serde(default)]
    pub pip_registry_mode: PipRegistryMode,
    /// Used when `pip_registry_mode = "custom"`.
    #[serde(default)]
    pub pip_index_url_custom: Option<String>,
    /// When false, python/pip shims resolve to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::python_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for PythonRuntimeSettings {
    fn default() -> Self {
        Self {
            download_source: PythonDownloadSource::default(),
            windows_distribution: PythonWindowsDistribution::default(),
            pip_registry_mode: PipRegistryMode::default(),
            pip_index_url_custom: None,
            path_proxy_enabled: defaults::python_path_proxy_enabled(),
        }
    }
}

/// Windows distribution for CPython installs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PythonWindowsDistribution {
    /// Prefer full NuGet packages on Windows, fall back to embeddable zip when needed.
    #[default]
    Auto,
    /// Full Python from NuGet (`python`, `pythonx86`, `pythonarm64`).
    Nuget,
    /// python.org embeddable zip (may lack some stdlib modules such as `venv`).
    Embeddable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoRuntimeSettings {
    /// Go toolchain download source choice.
    #[serde(default)]
    pub download_source: GoDownloadSource,
    /// `GOPROXY` injection mode.
    #[serde(default)]
    pub proxy_mode: GoProxyMode,
    /// Custom `GOPROXY` value (only when `proxy_mode = custom`).
    #[serde(default)]
    pub proxy_custom: Option<String>,
    /// Optional private module patterns (comma-separated). When set, envr injects:
    /// - `GOPRIVATE`
    /// - `GONOSUMDB`
    /// - `GONOPROXY`
    #[serde(default)]
    pub private_patterns: Option<String>,
    /// When false, go/gofmt shims resolve to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::go_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
    /// Backward compatibility: older settings used a direct `goproxy` value.
    ///
    /// When `proxy_mode` is `auto` and this is set, it takes precedence.
    /// When `proxy_mode` is `custom` and `proxy_custom` is empty, this is used as fallback.
    #[serde(default)]
    pub goproxy: Option<String>,
}

impl Default for GoRuntimeSettings {
    fn default() -> Self {
        Self {
            download_source: GoDownloadSource::default(),
            proxy_mode: GoProxyMode::default(),
            proxy_custom: None,
            private_patterns: None,
            path_proxy_enabled: defaults::go_path_proxy_enabled(),
            goproxy: None,
        }
    }
}
