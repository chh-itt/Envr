use serde::{Deserialize, Serialize};

use super::{RustDownloadSource, defaults};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RustRuntimeSettings {
    /// Rust toolchain download source choice (used for `rustup` env injection).
    #[serde(default)]
    pub download_source: RustDownloadSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RubyRuntimeSettings {
    /// When false, ruby/gem/bundle/irb shims resolve to the next matching binary on PATH
    /// outside envr shims.
    #[serde(default = "defaults::ruby_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for RubyRuntimeSettings {
    fn default() -> Self {
        Self {
            path_proxy_enabled: defaults::ruby_path_proxy_enabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElixirRuntimeSettings {
    /// When false, elixir/mix/iex shims resolve to the next matching binary on PATH
    /// outside envr shims.
    #[serde(default = "defaults::elixir_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for ElixirRuntimeSettings {
    fn default() -> Self {
        Self {
            path_proxy_enabled: defaults::elixir_path_proxy_enabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErlangRuntimeSettings {
    /// When false, erl/erlc/escript shims resolve to the next matching binary on PATH
    /// outside envr shims.
    #[serde(default = "defaults::erlang_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for ErlangRuntimeSettings {
    fn default() -> Self {
        Self {
            path_proxy_enabled: defaults::erlang_path_proxy_enabled(),
        }
    }
}
