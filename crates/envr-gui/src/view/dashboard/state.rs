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

#[derive(Debug, Default)]
pub struct DashboardState {
    pub busy: bool,
    pub last_error: Option<String>,
    pub data: Option<DashboardData>,
}
