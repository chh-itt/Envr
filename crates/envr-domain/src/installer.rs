//! Shared vocabulary for blocking runtime installs (`install_from_spec`, progress handles).

use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64},
};

use crate::runtime::{InstallRequest, RuntimeVersion};
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
