//! One-shot connection to the runtime data root and [`RuntimeService`] (install / list / doctor paths).
//!
//! ## Which API to use
//!
//! - **[`CliRuntimeSession::connect`]** / [`crate::commands::common::runtime_service`] — default data
//!   root for this process: `ENVR_RUNTIME_ROOT`, then `settings.toml`, then platform default
//!   ([`envr_config::settings::resolve_runtime_root`]). Use for almost all CLI commands (`doctor`,
//!   `install`, `bundle create`, [`crate::run_context::RunExecContext`], …).
//! - **`RuntimeService::with_runtime_root(path)`** — when the service must target a **specific** root
//!   that is **not** the process default, e.g. `envr bundle apply --runtime-root <dir>` unpacking into
//!   an alternate tree. Prefer **not** duplicating `connect` + `with_runtime_root` for the same path.
//!
//! Internally, [`CliRuntimeSession::connect`] is implemented with `with_runtime_root(resolve_runtime_root()?)`.

use envr_config::settings::resolve_runtime_root;
use envr_core::runtime::service::RuntimeService;
use envr_error::EnvrResult;
use std::ops::Deref;
use std::path::PathBuf;

/// Resolved `ENVR_RUNTIME_ROOT` / `settings.toml` data directory plus a [`RuntimeService`] for this process.
///
/// Built via [`Self::connect`]; use [`Deref`] or [`Self::service`] where a `&RuntimeService` is required.
pub struct CliRuntimeSession {
    service: RuntimeService,
    /// Root directory this session’s service was constructed with (same as [`crate::commands::common::session_runtime_root`]).
    pub runtime_root: PathBuf,
}

impl CliRuntimeSession {
    pub fn connect() -> EnvrResult<Self> {
        let runtime_root = resolve_runtime_root()?;
        let service = RuntimeService::with_runtime_root(runtime_root.clone())?;
        Ok(Self {
            service,
            runtime_root,
        })
    }

    #[inline]
    pub fn service(&self) -> &RuntimeService {
        &self.service
    }

    pub fn into_service(self) -> RuntimeService {
        self.service
    }
}

impl Deref for CliRuntimeSession {
    type Target = RuntimeService;

    fn deref(&self) -> &Self::Target {
        &self.service
    }
}

#[cfg(test)]
mod tests {
    use super::CliRuntimeSession;
    use envr_config::settings::resolve_runtime_root;

    #[test]
    fn connect_matches_resolve_runtime_root() {
        let root = resolve_runtime_root().expect("root");
        let session = CliRuntimeSession::connect().expect("connect");
        assert_eq!(session.runtime_root, root);
    }
}
