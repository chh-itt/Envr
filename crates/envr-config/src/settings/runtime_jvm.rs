use serde::{Deserialize, Serialize};

use super::{JavaDistro, JavaDownloadSource, defaults};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JavaRuntimeSettings {
    #[serde(default)]
    pub current_distro: JavaDistro,
    #[serde(default)]
    pub download_source: JavaDownloadSource,
    /// When false, java/javac shims resolve to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::java_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for JavaRuntimeSettings {
    fn default() -> Self {
        Self {
            current_distro: JavaDistro::default(),
            download_source: JavaDownloadSource::default(),
            path_proxy_enabled: defaults::java_path_proxy_enabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KotlinRuntimeSettings {
    /// When false, kotlin/kotlinc shims resolve to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::kotlin_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for KotlinRuntimeSettings {
    fn default() -> Self {
        Self {
            path_proxy_enabled: defaults::kotlin_path_proxy_enabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScalaRuntimeSettings {
    /// When false, scala/scalac shims resolve to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::scala_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for ScalaRuntimeSettings {
    fn default() -> Self {
        Self {
            path_proxy_enabled: defaults::scala_path_proxy_enabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClojureRuntimeSettings {
    /// When false, clojure/clj shims resolve to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::clojure_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for ClojureRuntimeSettings {
    fn default() -> Self {
        Self {
            path_proxy_enabled: defaults::clojure_path_proxy_enabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroovyRuntimeSettings {
    /// When false, groovy/groovyc shims resolve to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::groovy_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for GroovyRuntimeSettings {
    fn default() -> Self {
        Self {
            path_proxy_enabled: defaults::groovy_path_proxy_enabled(),
        }
    }
}
