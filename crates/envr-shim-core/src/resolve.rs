use envr_config::project_config::{ProjectConfig, load_project_config};
use envr_error::{EnvrError, EnvrResult};
use envr_platform::paths::{EnvSnapshot, current_platform_paths};
use std::path::{Path, PathBuf};

/// Process context for resolving a shim (runtime data root + config search directory).
#[derive(Debug, Clone)]
pub struct ShimContext {
    pub runtime_root: PathBuf,
    pub working_dir: PathBuf,
}

impl ShimContext {
    /// Uses `ENVR_RUNTIME_ROOT` when set, otherwise [`envr_platform::paths::current_platform_paths`].
    /// `working_dir` is [`std::env::current_dir`].
    pub fn from_process_env() -> EnvrResult<Self> {
        let envs = EnvSnapshot::capture_current()?;
        let runtime_root = if let Some(r) = envs.get("ENVR_RUNTIME_ROOT").filter(|s| !s.is_empty())
        {
            PathBuf::from(r)
        } else {
            current_platform_paths()?.runtime_root
        };
        let working_dir = std::env::current_dir().map_err(EnvrError::from)?;
        Ok(Self {
            runtime_root,
            working_dir,
        })
    }
}

/// Well-known language entrypoints handled by envr shims (not arbitrary global npm bins yet).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CoreCommand {
    Node,
    Npm,
    Npx,
    Python,
    Pip,
    Java,
    Javac,
}

impl CoreCommand {
    fn project_runtime_key(self) -> &'static str {
        match self {
            CoreCommand::Node | CoreCommand::Npm | CoreCommand::Npx => "node",
            CoreCommand::Python | CoreCommand::Pip => "python",
            CoreCommand::Java | CoreCommand::Javac => "java",
        }
    }
}

/// Result of resolution: real executable and extra environment for the child process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedShim {
    pub executable: PathBuf,
    pub extra_env: Vec<(String, String)>,
}

pub fn normalize_invoked_basename(invoked_as: &str) -> String {
    Path::new(invoked_as)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(invoked_as)
        .to_ascii_lowercase()
}

pub fn parse_core_command(basename: &str) -> Option<CoreCommand> {
    match basename {
        "node" => Some(CoreCommand::Node),
        "npm" => Some(CoreCommand::Npm),
        "npx" => Some(CoreCommand::Npx),
        "python" | "python3" => Some(CoreCommand::Python),
        "pip" | "pip3" => Some(CoreCommand::Pip),
        "java" => Some(CoreCommand::Java),
        "javac" => Some(CoreCommand::Javac),
        _ => None,
    }
}

/// Picks `versions_dir/<name>` for an installed tree matching `spec` (exact dir name or semver selection).
pub fn pick_version_home(versions_dir: &Path, spec: &str) -> EnvrResult<PathBuf> {
    let spec = spec.trim();
    if spec.is_empty() {
        return Err(EnvrError::Validation(
            "empty runtime version spec in project config".into(),
        ));
    }

    if !versions_dir.is_dir() {
        return Err(EnvrError::Runtime(format!(
            "no versions directory at {}",
            versions_dir.display()
        )));
    }

    let exact = versions_dir.join(spec);
    if exact.is_dir() {
        return Ok(exact);
    }

    let dirs: Vec<PathBuf> = std::fs::read_dir(versions_dir)
        .map_err(EnvrError::from)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.path())
        .collect();

    let constraint = SpecConstraint::parse(spec)?;

    let mut best: Option<((u32, u32, u32), PathBuf)> = None;
    for d in dirs {
        let Some(name) = d.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let Some(triple) = parse_dir_version_triplet(name) else {
            continue;
        };
        if constraint.matches(triple) && best.as_ref().is_none_or(|(v, _)| triple > *v) {
            best = Some((triple, d));
        }
    }

    let Some((_, path)) = best else {
        return Err(EnvrError::Runtime(format!(
            "no installed version matches project pin {spec:?} under {}",
            versions_dir.display()
        )));
    };

    Ok(path)
}

#[derive(Debug, Clone, Copy)]
enum SpecConstraint {
    Major(u32),
    MajorMinor(u32, u32),
    Triple(u32, u32, u32),
}

impl SpecConstraint {
    fn parse(spec: &str) -> EnvrResult<Self> {
        let s = spec.trim().trim_start_matches('v');
        let s = s.split('-').next().unwrap_or(s);
        let parts: Vec<&str> = s.split('.').collect();
        match (&parts[..], parts.len()) {
            ([maj], 1) => Ok(Self::Major(maj.parse().map_err(|_| {
                EnvrError::Validation(format!("invalid runtime version spec: {spec}"))
            })?)),
            ([maj, min], 2) => Ok(Self::MajorMinor(
                maj.parse().map_err(|_| {
                    EnvrError::Validation(format!("invalid runtime version spec: {spec}"))
                })?,
                min.parse().map_err(|_| {
                    EnvrError::Validation(format!("invalid runtime version spec: {spec}"))
                })?,
            )),
            ([maj, min, sec], 3) => Ok(Self::Triple(
                maj.parse().map_err(|_| {
                    EnvrError::Validation(format!("invalid runtime version spec: {spec}"))
                })?,
                min.parse().map_err(|_| {
                    EnvrError::Validation(format!("invalid runtime version spec: {spec}"))
                })?,
                sec.parse().map_err(|_| {
                    EnvrError::Validation(format!("invalid runtime version spec: {spec}"))
                })?,
            )),
            _ => Err(EnvrError::Validation(format!(
                "unsupported runtime version spec: {spec}"
            ))),
        }
    }

    fn matches(self, triple: (u32, u32, u32)) -> bool {
        match self {
            SpecConstraint::Major(m) => triple.0 == m,
            SpecConstraint::MajorMinor(m, n) => triple.0 == m && triple.1 == n,
            SpecConstraint::Triple(m, n, s) => triple == (m, n, s),
        }
    }
}

type VersionTriple = (u32, u32, u32);

fn parse_dir_version_triplet(dirname: &str) -> Option<VersionTriple> {
    let s = dirname.strip_prefix('v').unwrap_or(dirname);
    let s = s.split('-').next().unwrap_or(s);
    let s = s.split('+').next().unwrap_or(s);
    let mut parts = s.split('.');
    let a = parts.next()?.parse().ok()?;
    let b = if let Some(p) = parts.next() {
        p.parse().ok()?
    } else {
        0
    };
    let c = if let Some(p) = parts.next() {
        p.parse().ok()?
    } else {
        0
    };
    Some((a, b, c))
}

fn runtime_home_for_key(
    ctx: &ShimContext,
    key: &str,
    config: Option<&ProjectConfig>,
) -> EnvrResult<PathBuf> {
    let versions_dir = ctx.runtime_root.join("runtimes").join(key).join("versions");
    let current_link = ctx.runtime_root.join("runtimes").join(key).join("current");

    let pinned = config
        .and_then(|c| c.runtimes.get(key))
        .and_then(|r| r.version.as_deref());

    if let Some(spec) = pinned {
        pick_version_home(&versions_dir, spec)
    } else {
        if !current_link.exists() {
            return Err(EnvrError::Runtime(format!(
                "no global current for {key} at {}; install and select a version",
                current_link.display()
            )));
        }
        std::fs::canonicalize(&current_link).map_err(EnvrError::from)
    }
}

fn node_tool_path(home: &Path, cmd: CoreCommand) -> EnvrResult<PathBuf> {
    match cmd {
        CoreCommand::Node => Ok(first_existing(&[
            home.join("node.exe"),
            home.join("bin").join("node.exe"),
            home.join("bin").join("node"),
        ])
        .ok_or_else(|| {
            EnvrError::Runtime(format!("node binary missing under {}", home.display()))
        })?),
        CoreCommand::Npm => Ok(first_existing(&[
            home.join("npm.cmd"),
            home.join("bin").join("npm.cmd"),
            home.join("bin").join("npm"),
        ])
        .ok_or_else(|| EnvrError::Runtime(format!("npm cli missing under {}", home.display())))?),
        CoreCommand::Npx => Ok(first_existing(&[
            home.join("npx.cmd"),
            home.join("bin").join("npx.cmd"),
            home.join("bin").join("npx"),
        ])
        .ok_or_else(|| EnvrError::Runtime(format!("npx cli missing under {}", home.display())))?),
        _ => Err(EnvrError::Runtime("internal: not a node tool".into())),
    }
}

fn first_existing(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates.iter().find(|p| p.is_file()).cloned()
}

fn python_tool_path(home: &Path, cmd: CoreCommand) -> EnvrResult<PathBuf> {
    match cmd {
        CoreCommand::Python => {
            let p = first_existing(&[
                home.join("python.exe"),
                home.join("bin").join("python3"),
                home.join("bin").join("python"),
            ])
            .ok_or_else(|| {
                EnvrError::Runtime(format!(
                    "python executable missing under {}",
                    home.display()
                ))
            })?;
            Ok(p)
        }
        CoreCommand::Pip => {
            let p = first_existing(&[
                home.join("Scripts").join("pip.exe"),
                home.join("Scripts").join("pip3.exe"),
                home.join("bin").join("pip3"),
                home.join("bin").join("pip"),
            ])
            .ok_or_else(|| EnvrError::Runtime(format!("pip missing under {}", home.display())))?;
            Ok(p)
        }
        _ => Err(EnvrError::Runtime("internal: not a python tool".into())),
    }
}

fn java_tool_path(home: &Path, cmd: CoreCommand) -> EnvrResult<PathBuf> {
    let bin = home.join("bin");
    match cmd {
        CoreCommand::Java => Ok(first_existing(&[bin.join("java.exe"), bin.join("java")])
            .ok_or_else(|| EnvrError::Runtime(format!("java missing under {}", home.display())))?),
        CoreCommand::Javac => Ok(first_existing(&[bin.join("javac.exe"), bin.join("javac")])
            .ok_or_else(|| {
                EnvrError::Runtime(format!("javac missing under {}", home.display()))
            })?),
        _ => Err(EnvrError::Runtime("internal: not a java tool".into())),
    }
}

/// Resolve a core shim to a filesystem executable path.
pub fn resolve_core_shim(invoked_as: &str, ctx: &ShimContext) -> EnvrResult<ResolvedShim> {
    let base = normalize_invoked_basename(invoked_as);
    let Some(cmd) = parse_core_command(&base) else {
        return Err(EnvrError::Runtime(format!(
            "unknown shim command {base:?} (only core tools are supported here)"
        )));
    };

    let cfg = load_project_config(&ctx.working_dir)?.map(|(c, _)| c);

    let key = cmd.project_runtime_key();
    let home = runtime_home_for_key(ctx, key, cfg.as_ref())?;
    let home = std::fs::canonicalize(&home).map_err(EnvrError::from)?;

    let mut extra_env = Vec::new();

    let executable = match cmd {
        CoreCommand::Node | CoreCommand::Npm | CoreCommand::Npx => node_tool_path(&home, cmd)?,
        CoreCommand::Python | CoreCommand::Pip => python_tool_path(&home, cmd)?,
        CoreCommand::Java | CoreCommand::Javac => {
            extra_env.push(("JAVA_HOME".into(), home.display().to_string()));
            java_tool_path(&home, cmd)?
        }
    };

    Ok(ResolvedShim {
        executable,
        extra_env,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn pick_version_major_prefers_latest_minor() {
        let tmp = tempfile::TempDir::new().expect("tmp");
        let v = tmp.path().join("versions");
        fs::create_dir_all(v.join("20.0.0")).expect("d");
        fs::create_dir_all(v.join("20.9.0")).expect("d");
        fs::create_dir_all(v.join("20.10.0")).expect("d");
        let p = pick_version_home(&v, "20").expect("pick");
        assert!(p.ends_with("20.10.0"));
    }

    #[test]
    fn pick_version_exact_dir_name() {
        let tmp = tempfile::TempDir::new().expect("tmp");
        let v = tmp.path().join("versions");
        fs::create_dir_all(v.join("21.0.6+9-LTS")).expect("d");
        let p = pick_version_home(&v, "21.0.6+9-LTS").expect("pick");
        assert!(p.ends_with("21.0.6+9-LTS"));
    }

    #[cfg(unix)]
    #[test]
    fn resolve_uses_project_pin_over_global_current() {
        let tmp = tempfile::TempDir::new().expect("tmp");
        let root = tmp.path();
        fs::create_dir_all(root.join("prj")).expect("d");
        fs::write(
            root.join("prj/.envr.toml"),
            r#"
[runtimes.node]
version = "20"
"#,
        )
        .expect("write");

        let versions = root.join("runtimes/node/versions");
        fs::create_dir_all(versions.join("18.0.0").join("bin")).expect("d");
        fs::create_dir_all(versions.join("20.5.0").join("bin")).expect("d");

        let current = root.join("runtimes/node/current");
        fs::write(versions.join("18.0.0/bin/node"), []).expect("node");
        fs::write(versions.join("20.5.0/bin/node"), []).expect("node");
        std::os::unix::fs::symlink(versions.join("18.0.0"), &current).expect("symlink");

        let ctx = ShimContext {
            runtime_root: root.to_path_buf(),
            working_dir: root.join("prj"),
        };

        let shim = resolve_core_shim("node", &ctx).expect("resolve");
        assert!(
            shim.executable.starts_with(versions.join("20.5.0")),
            "{:?}",
            shim.executable
        );
        assert!(shim.extra_env.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn resolve_global_uses_current_when_no_project_pin() {
        let tmp = tempfile::TempDir::new().expect("tmp");
        let root = tmp.path();
        fs::create_dir_all(root.join("prj")).expect("d");

        let versions = root.join("runtimes/python/versions");
        fs::create_dir_all(versions.join("3.12.0")).expect("d");
        fs::create_dir_all(versions.join("3.12.0").join("bin")).expect("bin");
        fs::write(versions.join("3.12.0/bin/python3"), []).expect("py");

        let current = root.join("runtimes/python/current");
        std::os::unix::fs::symlink(versions.join("3.12.0"), &current).expect("symlink");

        let ctx = ShimContext {
            runtime_root: root.to_path_buf(),
            working_dir: root.join("prj"),
        };

        let shim = resolve_core_shim("python3", &ctx).expect("resolve");
        assert!(shim.executable.ends_with("python3"));
    }

    #[cfg(unix)]
    #[test]
    fn resolve_java_sets_java_home() {
        let tmp = tempfile::TempDir::new().expect("tmp");
        let root = tmp.path();
        fs::create_dir_all(root.join("prj")).expect("d");

        let versions = root.join("runtimes/java/versions");
        let jdk = versions.join("17.0.2+8");
        fs::create_dir_all(jdk.join("bin")).expect("bin");
        fs::write(jdk.join("bin/java"), []).expect("java");

        let current = root.join("runtimes/java/current");
        std::os::unix::fs::symlink(&jdk, &current).expect("symlink");

        let ctx = ShimContext {
            runtime_root: root.to_path_buf(),
            working_dir: root.join("prj"),
        };

        let shim = resolve_core_shim("java", &ctx).expect("resolve");
        assert!(shim.executable.ends_with("java"));
        let jh = shim
            .extra_env
            .iter()
            .find(|(k, _)| k == "JAVA_HOME")
            .map(|(_, v)| v)
            .expect("JAVA_HOME");
        assert!(jh.contains("17.0.2"));
    }
}
