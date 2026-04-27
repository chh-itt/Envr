mod floating_panel;
mod model;
mod page;
mod panel;

pub use floating_panel::{DOWNLOAD_PANEL_SHELL_W, floating_download_panel};
pub use model::{
    DownloadJob, DownloadJobPayload, DownloadPanelState, JobPhaseProgress, JobState,
    TITLE_DRAG_HOLD,
};
pub use page::downloads_page_view;
pub use panel::DownloadMsg;
