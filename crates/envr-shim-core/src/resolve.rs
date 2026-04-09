use envr_config::project_config::{ProjectConfig, load_project_config_profile};
use envr_config::settings::{
    go_path_proxy_enabled_from_disk, java_path_proxy_enabled_from_disk,
    node_path_proxy_enabled_from_disk, python_path_proxy_enabled_from_disk, resolve_runtime_root,
};
use envr_error::{EnvrError, EnvrResult};
use envr_platform::paths::EnvSnapshot;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Process context for resolving a shim (runtime data root + config search directory).
#[derive(Debug, Clone)]
pub struct ShimContext {
    pub runtime_root: PathBuf,
    pub working_dir: PathBuf,
    /// When set, selects `[profiles.<name>]` overlay from `.envr.toml` (overrides `ENVR_PROFILE`).
    pub profile: Option<String>,
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
            // Keep shim resolution consistent with CLI/GUI settings.
            // GUI edits `settings.toml` under the platform home, so read it here too.
            resolve_runtime_root()?
        };
        let working_dir = std::env::current_dir().map_err(EnvrError::from)?;
        let profile = envs
            .get("ENVR_PROFILE")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        Ok(Self {
            runtime_root,
            working_dir,
            profile,
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
    Go,
    Gofmt,
    Bun,
    Bunx,
}

impl CoreCommand {
    fn project_runtime_key(self) -> &'static str {
        match self {
            CoreCommand::Node | CoreCommand::Npm | CoreCommand::Npx => "node",
            CoreCommand::Python | CoreCommand::Pip => "python",
            CoreCommand::Java | CoreCommand::Javac => "java",
            CoreCommand::Go | CoreCommand::Gofmt => "go",
            CoreCommand::Bun | CoreCommand::Bunx => "bun",
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
        "go" => Some(CoreCommand::Go),
        "gofmt" => Some(CoreCommand::Gofmt),
        "bun" => Some(CoreCommand::Bun),
        "bunx" => Some(CoreCommand::Bunx),
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
    spec_override: Option<&str>,
) -> EnvrResult<PathBuf> {
    let versions_dir = ctx.runtime_root.join("runtimes").join(key).join("versions");
    let current_link = ctx.runtime_root.join("runtimes").join(key).join("current");

    let pinned = spec_override
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or_else(|| {
            config
                .and_then(|c| c.runtimes.get(key))
                .and_then(|r| r.version.as_deref())
        });

    if let Some(spec) = pinned {
        pick_version_home(&versions_dir, spec)
    } else if !current_link.exists() {
        Err(EnvrError::Runtime(format!(
            "no global current for {key} at {}; install and select a version",
            current_link.display()
        )))
    } else {
        // `current` is usually a symlink/junction, but on Windows some environments
        // forbid creating links. In that case we may fall back to a pointer file:
        // `current` contains the absolute target dir.
        if current_link.is_file() {
            let s = std::fs::read_to_string(&current_link).map_err(EnvrError::from)?;
            let t = s.trim();
            let target = std::path::PathBuf::from(t);
            std::fs::canonicalize(&target).map_err(EnvrError::from)
        } else {
            std::fs::canonicalize(&current_link).map_err(EnvrError::from)
        }
    }
}

/// Runtime installation directory for `lang_key` (`node` / `python` / `java`), matching shim routing:
/// `spec_override` wins, else `[runtimes.lang_key]` in `.envr.toml`, else global `current` symlink.
pub fn resolve_runtime_home_for_lang(
    ctx: &ShimContext,
    lang_key: &str,
    spec_override: Option<&str>,
) -> EnvrResult<PathBuf> {
    let cfg =
        load_project_config_profile(&ctx.working_dir, ctx.profile.as_deref())?.map(|(c, _)| c);
    runtime_home_for_key(ctx, lang_key, cfg.as_ref(), spec_override)
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

fn bun_tool_path(home: &Path, cmd: CoreCommand) -> EnvrResult<PathBuf> {
    match cmd {
        CoreCommand::Bun => Ok(first_existing(&[home.join("bun.exe"), home.join("bun")])
            .ok_or_else(|| EnvrError::Runtime(format!("bun missing under {}", home.display())))?),
        CoreCommand::Bunx => Ok(first_existing(&[home.join("bunx.exe"), home.join("bunx")])
            .ok_or_else(|| EnvrError::Runtime(format!("bunx missing under {}", home.display())))?),
        _ => Err(EnvrError::Runtime("internal: not a bun tool".into())),
    }
}

fn go_tool_path(home: &Path, cmd: CoreCommand) -> EnvrResult<PathBuf> {
    let bin = home.join("bin");
    match cmd {
        CoreCommand::Go => Ok(first_existing(&[bin.join("go.exe"), bin.join("go")])
            .ok_or_else(|| EnvrError::Runtime(format!("go missing under {}", home.display())))?),
        CoreCommand::Gofmt => Ok(first_existing(&[bin.join("gofmt.exe"), bin.join("gofmt")])
            .ok_or_else(|| {
                EnvrError::Runtime(format!("gofmt missing under {}", home.display()))
            })?),
        _ => Err(EnvrError::Runtime("internal: not a go tool".into())),
    }
}

/// Resolved path to a core tool under a runtime **home** directory (e.g. `current` target).
pub fn core_tool_executable(home: &Path, cmd: CoreCommand) -> EnvrResult<PathBuf> {
    match cmd {
        CoreCommand::Node | CoreCommand::Npm | CoreCommand::Npx => node_tool_path(home, cmd),
        CoreCommand::Python | CoreCommand::Pip => python_tool_path(home, cmd),
        CoreCommand::Java | CoreCommand::Javac => java_tool_path(home, cmd),
        CoreCommand::Go | CoreCommand::Gofmt => go_tool_path(home, cmd),
        CoreCommand::Bun | CoreCommand::Bunx => bun_tool_path(home, cmd),
    }
}

/// Interprets OS arguments: either `node ...` when argv0 is `node(.exe)`, or `envr-shim node ...`
/// when argv0 is the shim binary name.
pub fn parse_shim_invocation(args: &[OsString]) -> EnvrResult<(CoreCommand, Vec<OsString>)> {
    let Some(arg0) = args.first() else {
        return Err(EnvrError::Runtime("missing program name (argv0)".into()));
    };
    let base0 = normalize_invoked_basename(&arg0.to_string_lossy());
    if let Some(cmd) = parse_core_command(&base0) {
        return Ok((cmd, args.iter().skip(1).cloned().collect()));
    }
    if args.len() >= 2 {
        let base1 = normalize_invoked_basename(&args[1].to_string_lossy());
        if let Some(cmd) = parse_core_command(&base1) {
            return Ok((cmd, args.iter().skip(2).cloned().collect()));
        }
    }
    Err(EnvrError::Runtime(format!(
        "could not determine core tool from argv0={arg0:?} argv1={argv1:?}",
        argv1 = args.get(1)
    )))
}

fn is_likely_envr_shims_dir(dir: &Path) -> bool {
    let name = dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if name != "shims" {
        return false;
    }
    dir.parent()
        .and_then(|p| p.to_str())
        .is_some_and(|s| s.to_ascii_lowercase().contains("envr"))
}

fn find_on_path_outside_envr_shims(tool_stem: &str) -> EnvrResult<PathBuf> {
    let path_os = std::env::var_os("PATH").ok_or_else(|| {
        EnvrError::Runtime("PATH is not set; cannot bypass envr Node shims".into())
    })?;
    #[cfg(windows)]
    let suffixes: &[&str] = &[".cmd", ".exe", ".bat", ""];
    #[cfg(not(windows))]
    let suffixes: &[&str] = &[""];

    for dir in std::env::split_paths(&path_os) {
        if !dir.is_dir() {
            continue;
        }
        if is_likely_envr_shims_dir(&dir) {
            continue;
        }
        for suf in suffixes {
            let fname = if suf.is_empty() {
                tool_stem.to_string()
            } else {
                format!("{tool_stem}{suf}")
            };
            let candidate = dir.join(fname);
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }
    Err(EnvrError::Runtime(format!(
        "could not find `{tool_stem}` on PATH outside envr shims (enable PATH proxy in settings if you use envr-managed Node)"
    )))
}

fn resolve_node_tool_bypass_envr(cmd: CoreCommand) -> EnvrResult<ResolvedShim> {
    let stem = match cmd {
        CoreCommand::Node => "node",
        CoreCommand::Npm => "npm",
        CoreCommand::Npx => "npx",
        _ => {
            return Err(EnvrError::Runtime(
                "internal: bypass only supports node tools".into(),
            ));
        }
    };
    let executable = find_on_path_outside_envr_shims(stem)?;
    Ok(ResolvedShim {
        executable,
        extra_env: vec![],
    })
}

fn resolve_python_tool_bypass_envr(cmd: CoreCommand) -> EnvrResult<ResolvedShim> {
    let stem = match cmd {
        CoreCommand::Python => "python",
        CoreCommand::Pip => "pip",
        _ => {
            return Err(EnvrError::Runtime(
                "internal: bypass only supports python tools".into(),
            ));
        }
    };
    let executable = find_on_path_outside_envr_shims(stem)?;
    Ok(ResolvedShim {
        executable,
        extra_env: vec![],
    })
}

fn resolve_java_tool_bypass_envr(cmd: CoreCommand) -> EnvrResult<ResolvedShim> {
    let stem = match cmd {
        CoreCommand::Java => "java",
        CoreCommand::Javac => "javac",
        _ => {
            return Err(EnvrError::Runtime(
                "internal: bypass only supports java tools".into(),
            ));
        }
    };
    let executable = find_on_path_outside_envr_shims(stem)?;
    Ok(ResolvedShim {
        executable,
        extra_env: vec![],
    })
}

fn resolve_go_tool_bypass_envr(cmd: CoreCommand) -> EnvrResult<ResolvedShim> {
    let stem = match cmd {
        CoreCommand::Go => "go",
        CoreCommand::Gofmt => "gofmt",
        _ => {
            return Err(EnvrError::Runtime(
                "internal: bypass only supports go tools".into(),
            ));
        }
    };
    let executable = find_on_path_outside_envr_shims(stem)?;
    Ok(ResolvedShim {
        executable,
        extra_env: vec![],
    })
}

/// Resolve a core tool to a filesystem executable path.
pub fn resolve_core_shim_command(cmd: CoreCommand, ctx: &ShimContext) -> EnvrResult<ResolvedShim> {
    if matches!(cmd, CoreCommand::Node | CoreCommand::Npm | CoreCommand::Npx)
        && !node_path_proxy_enabled_from_disk()
    {
        return resolve_node_tool_bypass_envr(cmd);
    }
    if matches!(cmd, CoreCommand::Python | CoreCommand::Pip)
        && !python_path_proxy_enabled_from_disk()
    {
        return resolve_python_tool_bypass_envr(cmd);
    }
    if matches!(cmd, CoreCommand::Java | CoreCommand::Javac) && !java_path_proxy_enabled_from_disk()
    {
        return resolve_java_tool_bypass_envr(cmd);
    }
    if matches!(cmd, CoreCommand::Go | CoreCommand::Gofmt) && !go_path_proxy_enabled_from_disk() {
        return resolve_go_tool_bypass_envr(cmd);
    }

    let cfg =
        load_project_config_profile(&ctx.working_dir, ctx.profile.as_deref())?.map(|(c, _)| c);

    let key = cmd.project_runtime_key();
    let home = runtime_home_for_key(ctx, key, cfg.as_ref(), None)?;
    let home = std::fs::canonicalize(&home).map_err(EnvrError::from)?;

    let mut extra_env = Vec::new();

    let executable = match cmd {
        CoreCommand::Node | CoreCommand::Npm | CoreCommand::Npx => node_tool_path(&home, cmd)?,
        CoreCommand::Python | CoreCommand::Pip => python_tool_path(&home, cmd)?,
        CoreCommand::Java | CoreCommand::Javac => {
            extra_env.push(("JAVA_HOME".into(), home.display().to_string()));
            java_tool_path(&home, cmd)?
        }
        CoreCommand::Go | CoreCommand::Gofmt => {
            // Override stale GOROOT from the parent environment (e.g. old tests or manual exports).
            extra_env.push(("GOROOT".into(), home.display().to_string()));
            go_tool_path(&home, cmd)?
        }
        CoreCommand::Bun | CoreCommand::Bunx => bun_tool_path(&home, cmd)?,
    };

    Ok(ResolvedShim {
        executable,
        extra_env,
    })
}

/// Resolve from argv0 basename only (when the shim binary is a copy named `node`, etc.).
pub fn resolve_core_shim(invoked_as: &str, ctx: &ShimContext) -> EnvrResult<ResolvedShim> {
    let base = normalize_invoked_basename(invoked_as);
    let Some(cmd) = parse_core_command(&base) else {
        return Err(EnvrError::Runtime(format!(
            "unknown shim command {base:?} (only core tools are supported here)"
        )));
    };
    resolve_core_shim_command(cmd, ctx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::fs;

    #[test]
    fn parse_invocation_argv0_basename() {
        let args = vec![
            OsString::from("node"),
            OsString::from("-e"),
            OsString::from("0"),
        ];
        let (cmd, rest) = parse_shim_invocation(&args).expect("parse");
        assert_eq!(cmd, CoreCommand::Node);
        assert_eq!(rest.len(), 2);
    }

    #[test]
    fn parse_invocation_rejects_empty_argv() {
        let args: Vec<OsString> = vec![];
        let err = parse_shim_invocation(&args).expect_err("must fail");
        assert!(err.to_string().contains("missing program name"));
    }

    #[test]
    fn parse_invocation_dispatch_subcommand() {
        let args = vec![
            OsString::from("envr-shim"),
            OsString::from("python3"),
            OsString::from("-c"),
            OsString::from("pass"),
        ];
        let (cmd, rest) = parse_shim_invocation(&args).expect("parse");
        assert_eq!(cmd, CoreCommand::Python);
        assert_eq!(rest, vec![OsString::from("-c"), OsString::from("pass")]);
    }

    #[test]
    fn normalize_and_parse_core_command_cover_aliases() {
        assert_eq!(normalize_invoked_basename(r"C:\bin\PYTHON3.EXE"), "python3");
        assert_eq!(parse_core_command("python3"), Some(CoreCommand::Python));
        assert_eq!(parse_core_command("pip3"), Some(CoreCommand::Pip));
        assert_eq!(parse_core_command("bunx"), Some(CoreCommand::Bunx));
        assert_eq!(parse_core_command("go"), Some(CoreCommand::Go));
        assert_eq!(parse_core_command("gofmt"), Some(CoreCommand::Gofmt));
        assert_eq!(parse_core_command("unknown"), None);
    }

    #[test]
    fn parse_invocation_rejects_unknown_dispatch() {
        let args = vec![
            OsString::from("envr-shim"),
            OsString::from("not-a-core-tool"),
        ];
        let err = parse_shim_invocation(&args).expect_err("must fail");
        assert!(err.to_string().contains("could not determine core tool"));
    }

    #[test]
    fn pick_version_home_rejects_empty_spec() {
        let tmp = tempfile::TempDir::new().expect("tmp");
        let versions = tmp.path().join("versions");
        fs::create_dir_all(&versions).expect("d");
        let err = pick_version_home(&versions, "  ").expect_err("must fail");
        assert!(err.to_string().contains("empty runtime version spec"));
    }

    #[test]
    fn pick_version_home_rejects_missing_versions_dir() {
        let tmp = tempfile::TempDir::new().expect("tmp");
        let versions = tmp.path().join("missing");
        let err = pick_version_home(&versions, "20").expect_err("must fail");
        assert!(err.to_string().contains("no versions directory"));
    }

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

    #[test]
    fn resolve_runtime_home_for_lang_uses_spec_override() {
        let tmp = tempfile::TempDir::new().expect("tmp");
        let root = tmp.path();
        let versions = root.join("runtimes/node/versions");
        fs::create_dir_all(versions.join("18.0.0")).expect("d");
        fs::create_dir_all(versions.join("20.1.0")).expect("d");
        let prj = root.join("prj");
        fs::create_dir_all(&prj).expect("d");

        let ctx = ShimContext {
            runtime_root: root.to_path_buf(),
            working_dir: prj,
            profile: None,
        };
        let p = resolve_runtime_home_for_lang(&ctx, "node", Some("20")).expect("resolve");
        assert!(p.ends_with("20.1.0"));
    }

    #[test]
    fn resolve_runtime_home_for_lang_errors_when_no_current_and_no_pin() {
        let tmp = tempfile::TempDir::new().expect("tmp");
        let root = tmp.path();
        fs::create_dir_all(root.join("prj")).expect("d");
        let ctx = ShimContext {
            runtime_root: root.to_path_buf(),
            working_dir: root.join("prj"),
            profile: None,
        };
        let err = resolve_runtime_home_for_lang(&ctx, "node", None).expect_err("must fail");
        assert!(err.to_string().contains("no global current for node"));
    }

    #[test]
    fn core_tool_executable_node_and_pip_pick_expected_files() {
        let tmp = tempfile::TempDir::new().expect("tmp");
        let home = tmp.path();
        fs::create_dir_all(home.join("bin")).expect("bin");
        fs::create_dir_all(home.join("Scripts")).expect("scripts");

        #[cfg(windows)]
        {
            fs::write(home.join("bin/node.exe"), []).expect("node");
            fs::write(home.join("Scripts/pip.exe"), []).expect("pip");
        }
        #[cfg(not(windows))]
        {
            fs::write(home.join("bin/node"), []).expect("node");
            fs::write(home.join("bin/pip"), []).expect("pip");
        }

        let node = core_tool_executable(home, CoreCommand::Node).expect("node");
        let pip = core_tool_executable(home, CoreCommand::Pip).expect("pip");
        assert!(node.exists());
        assert!(pip.exists());
    }

    #[test]
    fn core_tool_executable_reports_missing_binary() {
        let tmp = tempfile::TempDir::new().expect("tmp");
        let home = tmp.path();
        let err = core_tool_executable(home, CoreCommand::Bun).expect_err("missing bun");
        assert!(err.to_string().contains("bun missing under"));
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
            profile: None,
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
            profile: None,
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
            profile: None,
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
