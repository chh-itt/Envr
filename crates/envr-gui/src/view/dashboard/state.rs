use envr_domain::runtime::RuntimeKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeRow {
    pub kind: RuntimeKind,
    pub installed: usize,
    pub current: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DashboardData {
    pub runtime_root: String,
    pub shims_dir: String,
    pub shims_empty: bool,
    pub rows: Vec<RuntimeRow>,
    pub issues: Vec<String>,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum DashboardMsg {
    Refresh,
    DataLoaded(Result<DashboardData, String>),
}

#[derive(Debug)]
pub struct DashboardState {
    pub busy: bool,
    pub last_error: Option<String>,
    pub data: Option<DashboardData>,
    /// When true, runtime overview cards show reorder controls instead of navigating on press.
    pub runtime_overview_layout_editing: bool,
    /// When true, the dashboard “hidden runtimes” block starts collapsed (only a summary row).
    pub runtime_overview_hidden_collapsed: bool,
}

impl Default for DashboardState {
    fn default() -> Self {
        Self {
            busy: false,
            last_error: None,
            data: None,
            runtime_overview_layout_editing: false,
            runtime_overview_hidden_collapsed: true,
        }
    }
}
