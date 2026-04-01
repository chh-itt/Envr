//! Distributors / vendors for JDK binaries (Adoptium API naming).

use envr_error::{EnvrError, EnvrResult};

/// JDK vendor supported by the Adoptium v3 API (`vendor` query parameter).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum JavaVendor {
    /// Eclipse Temurin (default; `vendor=eclipse` on api.adoptium.net).
    #[default]
    EclipseTemurin,
    /// User-facing alias; same Adoptium `eclipse` builds as Temurin for discovery/install.
    OpenJdk,
}

impl JavaVendor {
    /// `vendor` query value for <https://api.adoptium.net/>.
    pub fn adoptium_vendor_param(self) -> &'static str {
        match self {
            JavaVendor::EclipseTemurin | JavaVendor::OpenJdk => "eclipse",
        }
    }

    pub fn parse(s: &str) -> EnvrResult<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "temurin" | "eclipse" | "eclipse-temurin" | "adoptium" => {
                Ok(JavaVendor::EclipseTemurin)
            }
            "openjdk" | "jdk" => Ok(JavaVendor::OpenJdk),
            other => Err(EnvrError::Validation(format!(
                "unknown java vendor: {other} (supported: temurin, openjdk)"
            ))),
        }
    }
}
