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
    Zig,
    Julia,
    Nim,
    Crystal,
    RLang,
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

pub const RUNTIME_DESCRIPTORS: [RuntimeDescriptor; 17] = [
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
    RuntimeDescriptor {
        kind: RuntimeKind::Zig,
        key: "zig",
        label_en: "Zig",
        label_zh: "Zig",
        supports_remote_latest: true,
        supports_path_proxy: true,
    },
    RuntimeDescriptor {
        kind: RuntimeKind::Julia,
        key: "julia",
        label_en: "Julia",
        label_zh: "Julia",
        supports_remote_latest: true,
        supports_path_proxy: true,
    },
    RuntimeDescriptor {
        kind: RuntimeKind::Nim,
        key: "nim",
        label_en: "Nim",
        label_zh: "Nim",
        supports_remote_latest: true,
        supports_path_proxy: true,
    },
    RuntimeDescriptor {
        kind: RuntimeKind::Crystal,
        key: "crystal",
        label_en: "Crystal",
        label_zh: "Crystal",
        supports_remote_latest: true,
        supports_path_proxy: true,
    },
    RuntimeDescriptor {
        kind: RuntimeKind::RLang,
        key: "r",
        label_en: "R",
        label_zh: "R",
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

/// Env-center hub uses the unified major-line remote list UX for this runtime.
///
/// Rust alone uses a dedicated page; every other [`RuntimeKind`] shares the unified shell.
#[inline]
pub fn unified_major_list_rollout_enabled(kind: RuntimeKind) -> bool {
    kind != RuntimeKind::Rust
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionSpec(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeVersion(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MajorVersionRecord {
    pub major_key: String,
    pub latest_installable: Option<RuntimeVersion>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionRecord {
    pub version: RuntimeVersion,
}

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

pub fn numeric_version_segments(version: &str) -> Option<Vec<u64>> {
    let t = version.trim().trim_start_matches('v');
    if t.is_empty() {
        return None;
    }
    let mut parts = Vec::new();
    for seg in t.split('.') {
        if seg.is_empty() || !seg.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        parts.push(seg.parse::<u64>().ok()?);
    }
    if parts.is_empty() {
        return None;
    }
    Some(parts)
}

pub fn major_key_from_version(version: &str) -> Option<String> {
    numeric_version_segments(version)
        .and_then(|parts| parts.first().copied())
        .map(|m| m.to_string())
}

pub fn version_line_key_for_kind(kind: RuntimeKind, version: &str) -> Option<String> {
    let parts = numeric_version_segments(version)?;
    match kind {
        RuntimeKind::Python
        | RuntimeKind::Php
        | RuntimeKind::Go
        | RuntimeKind::Zig
        | RuntimeKind::Julia
        | RuntimeKind::Nim
        | RuntimeKind::Crystal
        | RuntimeKind::RLang => {
            let major = parts.first().copied()?;
            let minor = parts.get(1).copied()?;
            Some(format!("{major}.{minor}"))
        }
        _ => parts.first().copied().map(|m| m.to_string()),
    }
}

/// Bun/Deno **0.x** lines are not installable via envr’s managed installers (no supported release path).
/// Use this to drop `"0"` major rows from remote summaries and caches; keep the line in UI only when
/// [`version_line_key_for_kind`] shows the user still has a local install on that line (uninstall/switch).
pub fn major_line_remote_install_blocked(kind: RuntimeKind, major_key: &str) -> bool {
    matches!(kind, RuntimeKind::Bun | RuntimeKind::Deno) && major_key == "0"
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
        assert_eq!(
            parse_runtime_kind("CRYSTAL").expect("crystal"),
            RuntimeKind::Crystal
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
        assert_eq!(kinds.len(), 17);
        assert!(kinds.contains(&RuntimeKind::Ruby));
        assert!(kinds.contains(&RuntimeKind::Elixir));
        assert!(kinds.contains(&RuntimeKind::Erlang));
        assert!(kinds.contains(&RuntimeKind::Dotnet));
        assert!(kinds.contains(&RuntimeKind::Zig));
        assert!(kinds.contains(&RuntimeKind::Julia));
        assert!(kinds.contains(&RuntimeKind::Nim));
        assert!(kinds.contains(&RuntimeKind::Crystal));
        assert!(kinds.contains(&RuntimeKind::RLang));
    }

    #[test]
    fn unified_major_list_rollout_is_everything_except_rust_hub_page() {
        assert!(unified_major_list_rollout_enabled(RuntimeKind::Nim));
        assert!(unified_major_list_rollout_enabled(RuntimeKind::Crystal));
        assert!(unified_major_list_rollout_enabled(RuntimeKind::Node));
        assert!(!unified_major_list_rollout_enabled(RuntimeKind::Rust));
        for k in runtime_kinds_all() {
            assert_eq!(
                unified_major_list_rollout_enabled(k),
                k != RuntimeKind::Rust
            );
        }
    }

    #[test]
    fn numeric_version_segments_accepts_three_and_four_parts() {
        assert_eq!(
            numeric_version_segments("27.3.4").expect("three"),
            vec![27, 3, 4]
        );
        assert_eq!(
            numeric_version_segments("27.3.4.10").expect("four"),
            vec![27, 3, 4, 10]
        );
        assert_eq!(
            numeric_version_segments("v25.9.0").expect("v-prefixed"),
            vec![25, 9, 0]
        );
    }

    #[test]
    fn major_key_from_version_extracts_numeric_major() {
        assert_eq!(major_key_from_version("27.3.4.10").as_deref(), Some("27"));
        assert_eq!(major_key_from_version("v25.9.0").as_deref(), Some("25"));
        assert_eq!(major_key_from_version("27.3.4-rc1"), None);
    }

    #[test]
    fn version_line_key_for_kind_matches_runtime_grouping_contract() {
        assert_eq!(
            version_line_key_for_kind(RuntimeKind::Node, "25.9.0").as_deref(),
            Some("25")
        );
        assert_eq!(
            version_line_key_for_kind(RuntimeKind::Python, "3.13.2").as_deref(),
            Some("3.13")
        );
        assert_eq!(
            version_line_key_for_kind(RuntimeKind::Go, "1.24.7").as_deref(),
            Some("1.24")
        );
        assert_eq!(
            version_line_key_for_kind(RuntimeKind::Php, "8.4.11").as_deref(),
            Some("8.4")
        );
        assert_eq!(
            version_line_key_for_kind(RuntimeKind::Zig, "0.14.1").as_deref(),
            Some("0.14")
        );
        assert_eq!(
            version_line_key_for_kind(RuntimeKind::Julia, "1.10.5").as_deref(),
            Some("1.10")
        );
        assert_eq!(
            version_line_key_for_kind(RuntimeKind::Nim, "2.0.14").as_deref(),
            Some("2.0")
        );
        assert_eq!(
            version_line_key_for_kind(RuntimeKind::Crystal, "1.20.0").as_deref(),
            Some("1.20")
        );
        assert_eq!(
            version_line_key_for_kind(RuntimeKind::RLang, "4.4.2").as_deref(),
            Some("4.4")
        );
        assert_eq!(
            version_line_key_for_kind(RuntimeKind::Erlang, "27.3.4.10").as_deref(),
            Some("27")
        );
    }

    #[test]
    fn major_line_remote_install_blocked_bun_deno_zero_only() {
        assert!(major_line_remote_install_blocked(
            RuntimeKind::Bun,
            "0"
        ));
        assert!(major_line_remote_install_blocked(
            RuntimeKind::Deno,
            "0"
        ));
        assert!(!major_line_remote_install_blocked(RuntimeKind::Bun, "1"));
        assert!(!major_line_remote_install_blocked(RuntimeKind::Node, "0"));
    }
}
