mod floating_panel;
mod model;
mod panel;

pub use floating_panel::floating_download_panel;
pub use model::{DownloadJob, DownloadPanelState, JobState};
pub use panel::DownloadMsg;
