//! Shared vocabulary for blocking runtime installs (`install_from_spec`, progress handles).
//!
//! `JavaManager` and `PythonManager` also keep an inherent
//! `install_for_spec` that takes `&VersionSpec` plus progress/cancel refs; their
//! [`SpecDrivenInstaller`] implementation forwards from [`InstallRequest`] via
//! [`install_progress_handles`]. Providers should call
//! `SpecDrivenInstaller::install_from_spec(&manager, request)` for a consistent
//! entry point.

use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64},
};

use crate::runtime::{InstallRequest, RuntimeVersion, VersionSpec};
use envr_error::EnvrResult;

/// Optional download progress and cooperative cancellation from [`InstallRequest`].
pub type InstallProgressHandles<'a> = (
    Option<&'a Arc<AtomicU64>>,
    Option<&'a Arc<AtomicU64>>,
    Option<&'a Arc<AtomicBool>>,
);

#[inline]
pub fn install_progress_handles(request: &InstallRequest) -> InstallProgressHandles<'_> {
    (
        request.progress_downloaded.as_ref(),
        request.progress_total.as_ref(),
        request.cancel.as_ref(),
    )
}

/// Primary install entry used by most `*Manager` types (`install_from_spec`).
pub trait SpecDrivenInstaller: Send + Sync {
    fn install_from_spec(&self, request: &InstallRequest) -> EnvrResult<RuntimeVersion>;
}

#[inline]
pub fn install_via_manager<M>(
    manager: EnvrResult<M>,
    request: &InstallRequest,
) -> EnvrResult<RuntimeVersion>
where
    M: SpecDrivenInstaller,
{
    let mgr = manager?;
    SpecDrivenInstaller::install_from_spec(&mgr, request)
}

#[inline]
pub fn install_via_version_spec<F>(request: &InstallRequest, f: F) -> EnvrResult<RuntimeVersion>
where
    F: FnOnce(
        &VersionSpec,
        Option<&Arc<AtomicU64>>,
        Option<&Arc<AtomicU64>>,
        Option<&Arc<AtomicBool>>,
    ) -> EnvrResult<RuntimeVersion>,
{
    let (downloaded, total, cancel) = install_progress_handles(request);
    f(&request.spec, downloaded, total, cancel)
}
