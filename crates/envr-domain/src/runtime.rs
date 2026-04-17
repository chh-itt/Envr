use envr_error::{EnvrError, EnvrResult};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuntimeKind {
    Node,
    Python,
    Java,
    Go,
    Rust,
    Ruby,
    Elixir,
    Erlang,
    Php,
    Deno,
    Bun,
    Dotnet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeDescriptor {
    pub kind: RuntimeKind,
    pub key: &'static str,
    pub label_en: &'static str,
    pub label_zh: &'static str,
    pub supports_remote_latest: bool,
    pub supports_path_proxy: bool,
}

pub const RUNTIME_DESCRIPTORS: [RuntimeDescriptor; 12] = [
    RuntimeDescriptor {
        kind: RuntimeKind::Node,
        key: "node",
        label_en: "Node",
        label_zh: "Node",
        supports_remote_latest: true,
        supports_path_proxy: true,
    },
    RuntimeDescriptor {
        kind: RuntimeKind::Python,
        key: "python",
        label_en: "Python",
        label_zh: "Python",
        supports_remote_latest: true,
        supports_path_proxy: true,
    },
    RuntimeDescriptor {
        kind: RuntimeKind::Java,
        key: "java",
        label_en: "Java",
        label_zh: "Java",
        supports_remote_latest: true,
        supports_path_proxy: true,
    },
    RuntimeDescriptor {
        kind: RuntimeKind::Go,
        key: "go",
        label_en: "Go",
        label_zh: "Go",
        supports_remote_latest: true,
        supports_path_proxy: true,
    },
    RuntimeDescriptor {
        kind: RuntimeKind::Rust,
        key: "rust",
        label_en: "Rust",
        label_zh: "Rust",
        supports_remote_latest: false,
        supports_path_proxy: false,
    },
    RuntimeDescriptor {
        kind: RuntimeKind::Ruby,
        key: "ruby",
        label_en: "Ruby",
        label_zh: "Ruby",
        supports_remote_latest: true,
        supports_path_proxy: true,
    },
    RuntimeDescriptor {
        kind: RuntimeKind::Elixir,
        key: "elixir",
        label_en: "Elixir",
        label_zh: "Elixir",
        supports_remote_latest: true,
        supports_path_proxy: true,
    },
    RuntimeDescriptor {
        kind: RuntimeKind::Erlang,
        key: "erlang",
        label_en: "Erlang/OTP",
        label_zh: "Erlang/OTP",
        supports_remote_latest: true,
        supports_path_proxy: true,
    },
    RuntimeDescriptor {
        kind: RuntimeKind::Php,
        key: "php",
        label_en: "PHP",
        label_zh: "PHP",
        supports_remote_latest: true,
        supports_path_proxy: true,
    },
    RuntimeDescriptor {
        kind: RuntimeKind::Deno,
        key: "deno",
        label_en: "Deno",
        label_zh: "Deno",
        supports_remote_latest: true,
        supports_path_proxy: true,
    },
    RuntimeDescriptor {
        kind: RuntimeKind::Bun,
        key: "bun",
        label_en: "Bun",
        label_zh: "Bun",
        supports_remote_latest: true,
        supports_path_proxy: true,
    },
    RuntimeDescriptor {
        kind: RuntimeKind::Dotnet,
        key: "dotnet",
        label_en: ".NET",
        label_zh: ".NET",
        supports_remote_latest: true,
        supports_path_proxy: true,
    },
];

pub fn runtime_descriptor(kind: RuntimeKind) -> &'static RuntimeDescriptor {
    RUNTIME_DESCRIPTORS
        .iter()
        .find(|d| d.kind == kind)
        .expect("runtime descriptor must exist for kind")
}

pub fn runtime_kinds_all() -> impl Iterator<Item = RuntimeKind> {
    RUNTIME_DESCRIPTORS.iter().map(|d| d.kind)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionSpec(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeVersion(pub String);

#[derive(Debug, Clone)]
pub struct InstallRequest {
    pub spec: VersionSpec,
    /// Optional progress counters for GUI observability.
    pub progress_downloaded: Option<Arc<AtomicU64>>,
    pub progress_total: Option<Arc<AtomicU64>>,
    /// When set, installers should poll and abort long work (e.g. artifact download) cooperatively.
    pub cancel: Option<Arc<AtomicBool>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteFilter {
    pub prefix: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedVersion {
    pub version: RuntimeVersion,
}

pub trait RuntimeProvider: Send + Sync {
    fn kind(&self) -> RuntimeKind;

    fn list_installed(&self) -> EnvrResult<Vec<RuntimeVersion>>;
    fn current(&self) -> EnvrResult<Option<RuntimeVersion>>;
    fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()>;

    /// Remote versions for display and CLI listing.
    ///
    /// **Contract:** For runtimes where [`Self::install`] consumes the same index as this list,
    /// implementations should only return versions that can actually be installed. When the
    /// upstream “marketing” index is wider than installable artifacts, override
    /// [`Self::list_remote_installable`] (and related installable helpers) instead of widening
    /// what [`Self::install`] accepts without documentation.
    fn list_remote(&self, filter: &RemoteFilter) -> EnvrResult<Vec<RuntimeVersion>>;

    /// Subset (or equal) of [`Self::list_remote`] that [`Self::install`] is guaranteed to satisfy.
    ///
    /// Defaults to forwarding to [`Self::list_remote`]. Override when remote discovery and install
    /// artifacts diverge (e.g. language release page vs platform installer feed).
    fn list_remote_installable(&self, filter: &RemoteFilter) -> EnvrResult<Vec<RuntimeVersion>> {
        self.list_remote(filter)
    }

    /// Returns remote version `major` keys (e.g. `25` for `25.x.x`) without
    /// materializing the full remote leaf version list.
    /// Non-implemented providers may return an empty vec via the default impl.
    fn list_remote_majors(&self) -> EnvrResult<Vec<String>> {
        Ok(Vec::new())
    }

    /// Latest patch per major line for GUI list rows (e.g. Node). Default: empty.
    fn list_remote_latest_per_major(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        Ok(Vec::new())
    }

    /// Like [`Self::list_remote_latest_per_major`] but restricted to versions [`Self::install`] can use.
    ///
    /// Defaults to [`Self::list_remote_latest_per_major`].
    fn list_remote_latest_installable_per_major(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        self.list_remote_latest_per_major()
    }

    /// Read cached [`Self::list_remote_latest_installable_per_major`] from disk without TTL (for instant UI paint).
    ///
    /// Defaults to [`Self::try_load_remote_latest_per_major_from_disk`].
    fn try_load_remote_latest_installable_per_major_from_disk(&self) -> Vec<RuntimeVersion> {
        self.try_load_remote_latest_per_major_from_disk()
    }

    /// Read cached [`Self::list_remote_latest_per_major`] from disk without TTL (for instant UI paint).
    fn try_load_remote_latest_per_major_from_disk(&self) -> Vec<RuntimeVersion> {
        Vec::new()
    }

    fn resolve(&self, spec: &VersionSpec) -> EnvrResult<ResolvedVersion>;

    fn install(&self, request: &InstallRequest) -> EnvrResult<RuntimeVersion>;
    fn uninstall(&self, version: &RuntimeVersion) -> EnvrResult<()>;

    /// Directories envr would remove, plus an optional external command line (e.g. `rustup`).
    fn uninstall_dry_run_targets(
        &self,
        version: &RuntimeVersion,
    ) -> EnvrResult<(Vec<PathBuf>, Option<String>)>;
}

pub fn parse_runtime_kind(s: &str) -> EnvrResult<RuntimeKind> {
    let normalized = s.to_ascii_lowercase();
    if let Some(kind) = RUNTIME_DESCRIPTORS
        .iter()
        .find(|d| d.key == normalized.as_str())
        .map(|d| d.kind)
    {
        Ok(kind)
    } else {
        Err(EnvrError::Validation(format!("unknown runtime kind: {s}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_runtime_kind_accepts_ascii_case() {
        assert_eq!(parse_runtime_kind("NODE").expect("node"), RuntimeKind::Node);
        assert_eq!(
            parse_runtime_kind("Python").expect("py"),
            RuntimeKind::Python
        );
    }

    #[test]
    fn parse_runtime_kind_rejects_unknown() {
        let err = parse_runtime_kind("unknown-runtime").expect_err("unknown");
        assert!(matches!(err, EnvrError::Validation(_)));
    }

    #[test]
    fn descriptors_cover_all_runtime_kinds() {
        let kinds: Vec<RuntimeKind> = runtime_kinds_all().collect();
        assert_eq!(kinds.len(), 12);
        assert!(kinds.contains(&RuntimeKind::Ruby));
        assert!(kinds.contains(&RuntimeKind::Elixir));
        assert!(kinds.contains(&RuntimeKind::Erlang));
        assert!(kinds.contains(&RuntimeKind::Dotnet));
    }
}
