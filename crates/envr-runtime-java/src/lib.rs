mod index;
mod manager;
mod vendor;

pub use index::{
    DEFAULT_ADOPTIUM_API_BASE, JavaIndex, JavaVersionEntry, adoptium_arch,
    adoptium_assets_version_range_segment, adoptium_os, blocking_http_client,
    fetch_available_lts_majors, fetch_release_versions, find_version_entry, list_remote_versions,
    load_java_index, normalize_openjdk_version_label, resolve_java_version,
};
pub use manager::{
    JavaManager, JavaPaths, java_installation_valid, list_installed_versions,
    promote_single_root_dir, read_current, read_java_home_export, sync_java_home_export,
};
pub use vendor::JavaVendor;

use envr_config::settings::{
    JavaDistro, JavaDownloadSource, Settings, prefer_china_mirror_locale,
    settings_path_from_platform,
};
use envr_domain::runtime::{
    InstallRequest, RemoteFilter, ResolvedVersion, RuntimeKind, RuntimeProvider, RuntimeVersion,
    VersionSpec,
};
use envr_error::EnvrResult;
use envr_platform::paths::current_platform_paths;
use std::path::PathBuf;

/// JDK runtime provider (Adoptium Temurin: index, download, `current`, `JAVA_HOME` marker file).
pub struct JavaRuntimeProvider {
    api_base: String,
    vendor: JavaVendor,
    runtime_root_override: Option<std::path::PathBuf>,
}

impl JavaRuntimeProvider {
    pub fn new() -> Self {
        Self {
            api_base: DEFAULT_ADOPTIUM_API_BASE.to_string(),
            vendor: JavaVendor::default(),
            runtime_root_override: None,
        }
    }

    pub fn with_api_base(mut self, base: impl Into<String>) -> Self {
        self.api_base = base.into();
        self
    }

    pub fn with_vendor(mut self, vendor: JavaVendor) -> Self {
        self.vendor = vendor;
        self
    }

    pub fn with_runtime_root(mut self, root: std::path::PathBuf) -> Self {
        self.runtime_root_override = Some(root);
        self
    }

    fn runtime_root(&self) -> EnvrResult<std::path::PathBuf> {
        Ok(match &self.runtime_root_override {
            Some(p) => p.clone(),
            None => current_platform_paths()?.runtime_root,
        })
    }

    fn manager(&self) -> EnvrResult<JavaManager> {
        let source = self.resolved_download_source()?;
        JavaManager::try_new(
            self.runtime_root()?,
            self.api_base.clone(),
            self.resolved_vendor()?,
            source,
        )
    }

    fn load_index(&self) -> EnvrResult<JavaIndex> {
        let client = blocking_http_client()?;
        load_java_index(
            &client,
            &self.api_base,
            self.resolved_vendor()?,
            std::env::consts::OS,
            std::env::consts::ARCH,
        )
    }

    fn resolved_vendor(&self) -> EnvrResult<JavaVendor> {
        let platform = current_platform_paths()?;
        let path = settings_path_from_platform(&platform);
        let s = Settings::load_or_default_from(&path).unwrap_or_default();
        Ok(match s.runtime.java.current_distro {
            JavaDistro::Temurin => JavaVendor::EclipseTemurin,
            JavaDistro::OracleOpenJdk => JavaVendor::OracleOpenJdk,
            JavaDistro::AmazonCorretto => JavaVendor::AmazonCorretto,
            JavaDistro::Microsoft => JavaVendor::Microsoft,
            JavaDistro::OracleJdk => JavaVendor::OracleJdk,
            JavaDistro::AzulZulu => JavaVendor::AzulZulu,
            JavaDistro::AlibabaDragonwell => JavaVendor::AlibabaDragonwell,
            JavaDistro::OpenJdk => JavaVendor::EclipseTemurin,
        })
    }

    fn allowed_lts_majors() -> &'static [u32] {
        &[8, 11, 17, 21, 25]
    }

    fn resolved_download_source(&self) -> EnvrResult<JavaDownloadSource> {
        let platform = current_platform_paths()?;
        let path = settings_path_from_platform(&platform);
        let s = Settings::load_or_default_from(&path).unwrap_or_default();
        let src = match s.runtime.java.download_source {
            JavaDownloadSource::Auto => {
                if prefer_china_mirror_locale(&s) {
                    JavaDownloadSource::Domestic
                } else {
                    JavaDownloadSource::Official
                }
            }
            other => other,
        };
        Ok(src)
    }

    fn remote_cache_ttl_secs() -> u64 {
        const DEFAULT: u64 = 24 * 60 * 60;
        std::env::var("ENVR_JAVA_REMOTE_CACHE_TTL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT)
    }

    fn remote_latest_per_major_cache_file(
        &self,
        os: &str,
        arch: &str,
    ) -> EnvrResult<std::path::PathBuf> {
        let paths = JavaPaths::new(self.runtime_root()?, self.resolved_vendor()?.dir_name());
        let distro = self.resolved_vendor()?.dir_name();
        Ok(paths
            .cache_dir()
            .join(format!("remote_latest_per_major_{distro}_{os}_{arch}.json")))
    }

    fn major_from_label(label: &str) -> Option<u32> {
        label
            .trim()
            .split(['.', '+', '-'])
            .next()
            .and_then(|s| s.parse::<u32>().ok())
    }
}

impl Default for JavaRuntimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeProvider for JavaRuntimeProvider {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Java
    }

    fn list_installed(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let paths = JavaPaths::new(self.runtime_root()?, self.resolved_vendor()?.dir_name());
        list_installed_versions(&paths)
    }

    fn current(&self) -> EnvrResult<Option<RuntimeVersion>> {
        let paths = JavaPaths::new(self.runtime_root()?, self.resolved_vendor()?.dir_name());
        read_current(&paths)
    }

    fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        self.manager()?.set_current(version)
    }

    fn list_remote(&self, filter: &RemoteFilter) -> EnvrResult<Vec<RuntimeVersion>> {
        let index = self.load_index()?;
        let mut out = list_remote_versions(&index, filter)?;
        out.retain(|v| {
            let major = v.0.split('.').next().and_then(|s| s.parse::<u32>().ok());
            major.is_some_and(|m| Self::allowed_lts_majors().contains(&m))
        });
        Ok(out)
    }

    fn list_remote_latest_per_major(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;
        let ttl_secs = Self::remote_cache_ttl_secs();
        let cache_file = self.remote_latest_per_major_cache_file(os, arch)?;
        if let Some(list) = envr_platform::cache_recovery::read_json_string_list(
            &cache_file,
            Some(ttl_secs),
            |xs| !xs.is_empty(),
        ) {
            return Ok(list.into_iter().map(RuntimeVersion).collect());
        }

        let index = self.load_index()?;
        let out: Vec<RuntimeVersion> = index
            .versions
            .iter()
            .map(|v| RuntimeVersion(v.openjdk_version.clone()))
            .collect();

        let _ = (|| -> EnvrResult<()> {
            let paths = JavaPaths::new(self.runtime_root()?, self.resolved_vendor()?.dir_name());
            std::fs::create_dir_all(paths.cache_dir())?;
            let strings: Vec<String> = out.iter().map(|v| v.0.clone()).collect();
            let s = serde_json::to_string(&strings)
                .map_err(|e| envr_error::EnvrError::Validation(e.to_string()))?;
            envr_platform::fs_atomic::write_atomic(&cache_file, s.as_bytes())?;
            Ok(())
        })();
        Ok(out)
    }

    fn try_load_remote_latest_per_major_from_disk(&self) -> Vec<RuntimeVersion> {
        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;
        let Ok(cache_file) = self.remote_latest_per_major_cache_file(os, arch) else {
            return Vec::new();
        };
        let Some(list) =
            envr_platform::cache_recovery::read_json_string_list(&cache_file, None, |xs| {
                !xs.is_empty()
            })
        else {
            return Vec::new();
        };
        list.into_iter().map(RuntimeVersion).collect()
    }

    fn resolve(&self, spec: &VersionSpec) -> EnvrResult<ResolvedVersion> {
        let index = self.load_index()?;
        let v = resolve_java_version(&index, &spec.0)?;
        let Some(major) = Self::major_from_label(&v) else {
            return Err(envr_error::EnvrError::Validation(format!(
                "invalid java version label: {v}"
            )));
        };
        if !Self::allowed_lts_majors().contains(&major) {
            return Err(envr_error::EnvrError::Validation(format!(
                "only LTS majors are supported in GUI path: 8/11/17/21/25 (got {v})"
            )));
        }
        Ok(ResolvedVersion {
            version: RuntimeVersion(v),
        })
    }

    fn install(&self, request: &InstallRequest) -> EnvrResult<RuntimeVersion> {
        self.manager()?.install_from_spec(
            &request.spec,
            request.progress_downloaded.as_ref(),
            request.progress_total.as_ref(),
            request.cancel.as_ref(),
        )
    }

    fn uninstall(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        self.manager()?.uninstall(version)
    }

    fn uninstall_dry_run_targets(
        &self,
        version: &RuntimeVersion,
    ) -> EnvrResult<(Vec<PathBuf>, Option<String>)> {
        let paths = JavaPaths::new(self.runtime_root()?, self.resolved_vendor()?.dir_name());
        Ok((vec![paths.version_dir(&version.0)], None))
    }
}
