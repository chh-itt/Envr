mod floating_panel;
mod model;
mod panel;

pub use floating_panel::{DOWNLOAD_PANEL_SHELL_W, floating_download_panel};
pub use model::{DownloadJob, DownloadPanelState, JobState, TITLE_DRAG_HOLD};
pub use panel::DownloadMsg;
