//! Distributors / vendors for JDK binaries (Adoptium API naming).

use envr_error::{EnvrError, EnvrResult};

/// JDK vendor supported by the Adoptium v3 API (`vendor` query parameter).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum JavaVendor {
    /// Eclipse Temurin (default; `vendor=eclipse` on api.adoptium.net).
    #[default]
    EclipseTemurin,
    /// Oracle OpenJDK builds.
    OracleOpenJdk,
    /// Amazon Corretto builds.
    AmazonCorretto,
    /// Microsoft Build of OpenJDK (`vendor=microsoft`).
    Microsoft,
    /// Oracle JDK builds.
    OracleJdk,
    /// Azul Zulu (Azul metadata API + CDN; not Adoptium).
    AzulZulu,
    /// Alibaba Dragonwell (GitHub `dragonwell-project/dragonwell*` releases; not Adoptium).
    AlibabaDragonwell,
    /// Backward-compatibility alias for Temurin.
    OpenJdk,
}

impl JavaVendor {
    pub fn dir_name(self) -> &'static str {
        match self {
            JavaVendor::EclipseTemurin => "temurin",
            JavaVendor::OracleOpenJdk => "oracle-openjdk",
            JavaVendor::AmazonCorretto => "corretto",
            JavaVendor::Microsoft => "microsoft",
            JavaVendor::OracleJdk => "oracle-jdk",
            JavaVendor::AzulZulu => "zulu",
            JavaVendor::AlibabaDragonwell => "dragonwell",
            JavaVendor::OpenJdk => "openjdk",
        }
    }

    /// `vendor` query value for <https://api.adoptium.net/>.
    pub fn adoptium_vendor_param(self) -> &'static str {
        match self {
            JavaVendor::EclipseTemurin | JavaVendor::OpenJdk => "eclipse",
            JavaVendor::OracleOpenJdk | JavaVendor::AmazonCorretto | JavaVendor::OracleJdk => {
                "eclipse"
            }
            JavaVendor::Microsoft => "microsoft",
            JavaVendor::AzulZulu | JavaVendor::AlibabaDragonwell => {
                panic!("internal error: adoptium_vendor_param called for non-Adoptium Java vendor")
            }
        }
    }

    pub fn parse(s: &str) -> EnvrResult<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "temurin" | "eclipse" | "eclipse-temurin" | "adoptium" => {
                Ok(JavaVendor::EclipseTemurin)
            }
            "oracle-openjdk" | "oracle_openjdk" | "openjdk-oracle" => {
                Ok(JavaVendor::OracleOpenJdk)
            }
            "corretto" | "amazon-corretto" | "amazon_corretto" => Ok(JavaVendor::AmazonCorretto),
            "microsoft" | "ms-openjdk" | "microsoft-openjdk" => Ok(JavaVendor::Microsoft),
            "oracle-jdk" | "oracle_jdk" | "oraclejdk" => Ok(JavaVendor::OracleJdk),
            "zulu" | "azul" | "azul-zulu" | "azul_zulu" => Ok(JavaVendor::AzulZulu),
            "dragonwell" | "alibaba-dragonwell" | "alibaba_dragonwell" => {
                Ok(JavaVendor::AlibabaDragonwell)
            }
            "openjdk" | "jdk" => Ok(JavaVendor::OpenJdk),
            other => Err(EnvrError::Validation(format!(
                "unknown java vendor: {other} (supported: temurin, zulu, dragonwell, oracle-openjdk, corretto, microsoft, oracle-jdk)"
            ))),
        }
    }
}
