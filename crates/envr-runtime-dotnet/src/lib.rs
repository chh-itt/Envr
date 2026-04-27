mod index;
mod manager;

pub use index::{DEFAULT_RELEASES_INDEX_URL, DotnetFile, DotnetSdkRelease, resolve_dotnet_version};
pub use manager::{
    DotnetManager, DotnetPaths, dotnet_installation_valid, list_installed_versions, read_current,
};

use envr_config::env_context::runtime_root;
use envr_domain::installer::install_via_manager;
use envr_domain::runtime::{
    InstallRequest, RemoteFilter, ResolvedVersion, RuntimeKind, RuntimeProvider, RuntimeVersion,
    VersionSpec,
};
use envr_domain::runtime::{
    MajorVersionRecord, VersionListAdapter, VersionRecord, major_line_remote_install_blocked,
    version_line_key_for_kind,
};
use envr_error::EnvrResult;
use std::collections::BTreeMap;
use std::path::PathBuf;

pub struct DotnetRuntimeProvider {
    releases_index_url: String,
    runtime_root_override: Option<PathBuf>,
}

impl DotnetRuntimeProvider {
    pub fn new() -> Self {
        Self {
            releases_index_url: DEFAULT_RELEASES_INDEX_URL.to_string(),
            runtime_root_override: None,
        }
    }

    pub fn with_releases_index_url(mut self, url: impl Into<String>) -> Self {
        self.releases_index_url = url.into();
        self
    }

    pub fn with_runtime_root(mut self, root: PathBuf) -> Self {
        self.runtime_root_override = Some(root);
        self
    }

    fn runtime_root(&self) -> EnvrResult<PathBuf> {
        Ok(match &self.runtime_root_override {
            Some(p) => p.clone(),
            None => runtime_root()?,
        })
    }

    fn manager(&self) -> EnvrResult<DotnetManager> {
        DotnetManager::try_new(self.runtime_root()?, self.releases_index_url.clone())
    }

    fn unified_list_dir(&self) -> EnvrResult<PathBuf> {
        let root = self.runtime_root()?;
        Ok(root
            .join("cache")
            .join("dotnet")
            .join("unified_version_list"))
    }

    fn unified_major_rows_path(&self) -> EnvrResult<PathBuf> {
        Ok(self.unified_list_dir()?.join("major_rows.json"))
    }

    fn unified_children_path(&self, major_key: &str) -> EnvrResult<PathBuf> {
        Ok(self
            .unified_list_dir()?
            .join("children")
            .join(format!("{major_key}.json")))
    }
}

impl Default for DotnetRuntimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeProvider for DotnetRuntimeProvider {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Dotnet
    }

    fn list_installed(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let paths = DotnetPaths::new(self.runtime_root()?);
        list_installed_versions(&paths)
    }

    fn current(&self) -> EnvrResult<Option<RuntimeVersion>> {
        let paths = DotnetPaths::new(self.runtime_root()?);
        read_current(&paths)
    }

    fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        self.manager()?.set_current(version)
    }

    fn list_remote(&self, filter: &RemoteFilter) -> EnvrResult<Vec<RuntimeVersion>> {
        let releases = self.manager()?.load_releases()?;
        let mut out: Vec<RuntimeVersion> = releases
            .into_iter()
            .map(|r| RuntimeVersion(r.version))
            .filter(|v| {
                filter
                    .prefix
                    .as_deref()
                    .is_none_or(|p| v.0.starts_with(p.trim()))
            })
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out.dedup_by(|a, b| a.0 == b.0);
        Ok(out)
    }

    fn list_remote_majors(&self) -> EnvrResult<Vec<String>> {
        let mut majors = Vec::<String>::new();
        for v in self.list_remote(&RemoteFilter::default())? {
            if let Some(m) = v.0.split('.').next()
                && !m.is_empty()
            {
                majors.push(m.to_string());
            }
        }
        majors.sort();
        majors.dedup();
        Ok(majors)
    }

    fn list_remote_latest_per_major(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let all = self.list_remote(&RemoteFilter::default())?;
        let mut by_major = BTreeMap::<String, RuntimeVersion>::new();
        for v in all {
            let Some(major) = v.0.split('.').next() else {
                continue;
            };
            match by_major.get(major) {
                None => {
                    by_major.insert(major.to_string(), v);
                }
                Some(old) if v.0 > old.0 => {
                    by_major.insert(major.to_string(), v);
                }
                _ => {}
            }
        }
        Ok(by_major.into_values().collect())
    }

    fn resolve(&self, spec: &VersionSpec) -> EnvrResult<ResolvedVersion> {
        let v = self.manager()?.resolve_spec(&spec.0)?;
        Ok(ResolvedVersion { version: v })
    }

    fn install(&self, request: &InstallRequest) -> EnvrResult<RuntimeVersion> {
        install_via_manager(self.manager(), request)
    }

    fn uninstall(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        self.manager()?.uninstall(version)
    }

    fn uninstall_dry_run_targets(
        &self,
        version: &RuntimeVersion,
    ) -> EnvrResult<(Vec<PathBuf>, Option<String>)> {
        let paths = DotnetPaths::new(self.runtime_root()?);
        Ok((vec![paths.version_dir(&version.0)], None))
    }

    fn version_list_adapter(&self) -> Option<&dyn VersionListAdapter> {
        Some(self)
    }
}

impl VersionListAdapter for DotnetRuntimeProvider {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Dotnet
    }

    fn load_major_rows_cached(&self) -> EnvrResult<Vec<MajorVersionRecord>> {
        let path = self.unified_major_rows_path()?;
        let raw =
            envr_platform::cache_recovery::read_json_string_list(&path, None, |xs| !xs.is_empty())
                .unwrap_or_default();
        Ok(raw
            .into_iter()
            .filter_map(|v| {
                let major = version_line_key_for_kind(RuntimeKind::Dotnet, &v)?;
                Some(MajorVersionRecord {
                    major_key: major,
                    latest_installable: Some(RuntimeVersion(v)),
                })
            })
            .filter(|r| !major_line_remote_install_blocked(RuntimeKind::Dotnet, &r.major_key))
            .collect())
    }

    fn refresh_major_rows_remote(&self) -> EnvrResult<Vec<MajorVersionRecord>> {
        let latest = RuntimeProvider::list_remote_latest_installable_per_major(self)?;
        let rows: Vec<MajorVersionRecord> = latest
            .into_iter()
            .filter_map(|v| {
                let major = version_line_key_for_kind(RuntimeKind::Dotnet, &v.0)?;
                Some(MajorVersionRecord {
                    major_key: major,
                    latest_installable: Some(v),
                })
            })
            .filter(|r| !major_line_remote_install_blocked(RuntimeKind::Dotnet, &r.major_key))
            .collect();
        let labels: Vec<String> = rows
            .iter()
            .filter_map(|r| r.latest_installable.as_ref().map(|v| v.0.clone()))
            .collect();
        let _ = (|| -> EnvrResult<()> {
            let p = self.unified_major_rows_path()?;
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let s = serde_json::to_string(&labels).unwrap_or_default();
            envr_platform::fs_atomic::write_atomic(&p, s.as_bytes())?;
            Ok(())
        })();
        Ok(rows)
    }

    fn load_children_cached(&self, major_key: &str) -> EnvrResult<Vec<VersionRecord>> {
        if major_line_remote_install_blocked(RuntimeKind::Dotnet, major_key) {
            return Ok(Vec::new());
        }
        let path = self.unified_children_path(major_key)?;
        let raw =
            envr_platform::cache_recovery::read_json_string_list(&path, None, |xs| !xs.is_empty())
                .unwrap_or_default();
        Ok(raw
            .into_iter()
            .map(|v| VersionRecord {
                version: RuntimeVersion(v),
            })
            .collect())
    }

    fn refresh_children_remote(&self, major_key: &str) -> EnvrResult<Vec<VersionRecord>> {
        if major_line_remote_install_blocked(RuntimeKind::Dotnet, major_key) {
            return Ok(Vec::new());
        }
        let all = RuntimeProvider::list_remote_installable(self, &RemoteFilter::default())?;
        let filtered: Vec<RuntimeVersion> = all
            .into_iter()
            .filter(|v| {
                version_line_key_for_kind(RuntimeKind::Dotnet, &v.0).as_deref() == Some(major_key)
            })
            .collect();
        let labels: Vec<String> = filtered.iter().map(|v| v.0.clone()).collect();
        let _ = (|| -> EnvrResult<()> {
            let p = self.unified_children_path(major_key)?;
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let s = serde_json::to_string(&labels).unwrap_or_default();
            envr_platform::fs_atomic::write_atomic(&p, s.as_bytes())?;
            Ok(())
        })();
        Ok(filtered
            .into_iter()
            .map(|v| VersionRecord { version: v })
            .collect())
    }

    fn is_installable_on_host(&self, _version: &VersionRecord) -> bool {
        true
    }
}
