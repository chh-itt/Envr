use envr_config::PathProxyRuntimeSnapshot;
use envr_config::project_config::{ProjectConfig, load_project_config_profile};
use envr_config::settings::{
    Settings, bun_package_registry_env, deno_package_registry_env, resolve_runtime_root,
    settings_path_from_platform,
};
use envr_domain::runtime::parse_runtime_kind;
use envr_error::{EnvrError, EnvrResult};
use envr_platform::bin_tool_layout;
use envr_platform::lua_binaries;
use envr_platform::paths::EnvSnapshot;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ShimSettingsSnapshot {
    /// PATH-proxy flags copied once from settings (see [`PathProxyRuntimeSnapshot`]).
    pub path_proxy: PathProxyRuntimeSnapshot,
    php_windows_build_want_ts: bool,
    deno_registry_env: Vec<(String, String)>,
    bun_registry_env: Vec<(String, String)>,
}

impl Default for ShimSettingsSnapshot {
    fn default() -> Self {
        Self {
            path_proxy: PathProxyRuntimeSnapshot::default(),
            php_windows_build_want_ts: false,
            deno_registry_env: Vec::new(),
            bun_registry_env: Vec::new(),
        }
    }
}

impl ShimSettingsSnapshot {
    pub fn from_settings(settings: &Settings) -> Self {
        Self {
            path_proxy: settings.runtime.path_proxy_snapshot(),
            php_windows_build_want_ts: matches!(
                settings.runtime.php.windows_build,
                envr_config::settings::PhpWindowsBuildFlavor::Ts
            ),
            deno_registry_env: deno_package_registry_env(settings),
            bun_registry_env: bun_package_registry_env(settings),
        }
    }

    pub fn from_disk() -> Self {
        let Ok(platform) = envr_platform::paths::current_platform_paths() else {
            return Self::default();
        };
        let path = settings_path_from_platform(&platform);
        let Ok(settings) = Settings::load_or_default_from(&path) else {
            return Self::default();
        };
        Self::from_settings(&settings)
    }
}

pub fn load_shim_settings_snapshot() -> ShimSettingsSnapshot {
    ShimSettingsSnapshot::from_disk()
}

fn uses_path_proxy_bypass(cmd: CoreCommand, settings: &ShimSettingsSnapshot) -> bool {
    let Ok(kind) = parse_runtime_kind(cmd.project_runtime_key()) else {
        return false;
    };
    matches!(settings.path_proxy.enabled_for_kind(kind), Some(false))
}

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

    /// Build context with an explicit runtime data root (CLI session cache, tests, shims).
    pub fn with_runtime_root(
        runtime_root: PathBuf,
        working_dir: PathBuf,
        profile: Option<String>,
    ) -> Self {
        Self {
            runtime_root,
            working_dir,
            profile,
        }
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
    Kotlin,
    Kotlinc,
    Scala,
    Scalac,
    Clojure,
    Clj,
    Groovy,
    Groovyc,
    Terraform,
    V,
    Odin,
    Purs,
    Elm,
    Dart,
    Flutter,
    Go,
    Gofmt,
    Php,
    Deno,
    Bun,
    Bunx,
    Dotnet,
    Ruby,
    Gem,
    Bundle,
    Irb,
    Elixir,
    Mix,
    Iex,
    Erl,
    Erlc,
    Escript,
    Zig,
    Julia,
    Lua,
    Luac,
    Nim,
    Crystal,
    Perl,
    R,
    Rscript,
}

impl CoreCommand {
    pub fn project_runtime_key(self) -> &'static str {
        match self {
            CoreCommand::Node | CoreCommand::Npm | CoreCommand::Npx => "node",
            CoreCommand::Python | CoreCommand::Pip => "python",
            CoreCommand::Java | CoreCommand::Javac => "java",
            CoreCommand::Kotlin | CoreCommand::Kotlinc => "kotlin",
            CoreCommand::Scala | CoreCommand::Scalac => "scala",
            CoreCommand::Clojure | CoreCommand::Clj => "clojure",
            CoreCommand::Groovy | CoreCommand::Groovyc => "groovy",
            CoreCommand::Terraform => "terraform",
            CoreCommand::V => "v",
            CoreCommand::Odin => "odin",
            CoreCommand::Purs => "purescript",
            CoreCommand::Elm => "elm",
            CoreCommand::Dart => "dart",
            CoreCommand::Flutter => "flutter",
            CoreCommand::Go | CoreCommand::Gofmt => "go",
            CoreCommand::Php => "php",
            CoreCommand::Deno => "deno",
            CoreCommand::Bun | CoreCommand::Bunx => "bun",
            CoreCommand::Dotnet => "dotnet",
            CoreCommand::Ruby | CoreCommand::Gem | CoreCommand::Bundle | CoreCommand::Irb => "ruby",
            CoreCommand::Elixir | CoreCommand::Mix | CoreCommand::Iex => "elixir",
            CoreCommand::Erl | CoreCommand::Erlc | CoreCommand::Escript => "erlang",
            CoreCommand::Zig => "zig",
            CoreCommand::Julia => "julia",
            CoreCommand::Lua | CoreCommand::Luac => "lua",
            CoreCommand::Nim => "nim",
            CoreCommand::Crystal => "crystal",
            CoreCommand::Perl => "perl",
            CoreCommand::R | CoreCommand::Rscript => "r",
        }
    }
}

/// Bin / root dirs to put on `PATH` for a resolved runtime home (order matters).
pub fn runtime_bin_dirs_for_key(home: &Path, key: &str) -> Vec<PathBuf> {
    match key {
        "node" => vec![home.join("bin"), home.to_path_buf()],
        "python" => vec![home.join("Scripts"), home.join("bin")],
        "java" => vec![home.join("bin")],
        "kotlin" => vec![home.join("bin")],
        "scala" => vec![home.join("bin")],
        "clojure" => vec![home.to_path_buf(), home.join("bin")],
        "groovy" => vec![home.join("bin")],
        "terraform" => vec![home.to_path_buf(), home.join("bin")],
        "v" => vec![home.to_path_buf(), home.join("bin")],
        "odin" => vec![home.to_path_buf(), home.join("bin")],
        "purescript" => vec![home.to_path_buf(), home.join("bin")],
        "elm" => vec![home.to_path_buf(), home.join("bin")],
        "dart" => vec![home.join("bin"), home.to_path_buf()],
        "flutter" => vec![home.join("bin"), home.to_path_buf()],
        "go" => vec![home.join("bin")],
        "rust" => vec![home.to_path_buf()],
        "ruby" => vec![home.join("bin"), home.to_path_buf()],
        "elixir" => vec![home.join("bin"), home.to_path_buf()],
        "erlang" => vec![home.join("bin"), home.to_path_buf()],
        "php" => vec![home.to_path_buf(), home.join("bin")],
        "deno" => vec![home.to_path_buf(), home.join("bin")],
        "bun" => vec![home.to_path_buf(), home.join("bin")],
        "dotnet" => vec![home.to_path_buf(), home.join("bin")],
        "zig" => vec![home.to_path_buf(), home.join("bin")],
        "julia" => vec![home.join("bin")],
        "lua" => vec![home.to_path_buf()],
        "nim" => vec![home.join("bin")],
        "crystal" => envr_domain::crystal_paths::crystal_path_entries(home),
        "perl" => vec![home.join("bin"), home.to_path_buf()],
        "r" => vec![home.join("bin")],
        _ => vec![],
    }
}

/// Environment variables that should point at the selected runtime home.
pub fn runtime_home_env_for_key(home: &Path, key: &str) -> Vec<(String, String)> {
    fn env_path_string(home: &Path) -> String {
        let raw = home.display().to_string();
        #[cfg(windows)]
        {
            if let Some(stripped) = raw.strip_prefix(r"\\?\") {
                return stripped.to_string();
            }
        }
        raw
    }

    let home_env = env_path_string(home);
    match key {
        "java" => vec![("JAVA_HOME".into(), home_env.clone())],
        // Override stale parent env values so the selected runtime stays authoritative.
        "go" => vec![("GOROOT".into(), home_env.clone())],
        "elixir" => {
            #[cfg(windows)]
            fn erts_bin_from_host_path() -> Option<String> {
                use std::path::PathBuf;
                let path_os = std::env::var_os("PATH")?;
                for dir in std::env::split_paths(&path_os) {
                    // Skip envr shims directory (avoid accidental self-resolution).
                    if should_skip_envr_shims_path_entry(&dir) {
                        continue;
                    }
                    let candidate: PathBuf = dir.join("erl.exe");
                    if candidate.is_file() {
                        let s = dir.display().to_string();
                        let s = s.strip_prefix(r"\\?\").unwrap_or(&s).to_string();
                        // `elixir.bat` concatenates `%ERTS_BIN%erl.exe`, so keep a trailing backslash.
                        return Some(if s.ends_with('\\') {
                            s
                        } else {
                            format!("{s}\\")
                        });
                    }
                }
                None
            }

            #[cfg(not(windows))]
            fn erts_bin_from_host_path() -> Option<String> {
                None
            }

            let mut out = Vec::new();
            if let Some(erts) = erts_bin_from_host_path() {
                out.push(("ERTS_BIN".into(), erts));
            }
            out
        }
        "dotnet" => vec![
            ("DOTNET_ROOT".into(), home_env),
            ("DOTNET_MULTILEVEL_LOOKUP".into(), "0".into()),
        ],
        "erlang" => vec![("ERLANG_HOME".into(), home_env.clone())],
        "julia" => vec![("JULIA_HOME".into(), home_env)],
        "perl" => vec![("PERL_HOME".into(), home_env)],
        "purescript" => vec![("PURESCRIPT_HOME".into(), home_env)],
        "elm" => vec![("ELM_HOME".into(), home_env)],
        "odin" => vec![("ODIN_ROOT".into(), home_env)],
        "r" => vec![("R_HOME".into(), home_env)],
        "scala" => vec![("SCALA_HOME".into(), home_env)],
        "clojure" => vec![("CLOJURE_HOME".into(), home_env)],
        "groovy" => vec![("GROOVY_HOME".into(), home_env)],
        "terraform" => vec![("TERRAFORM_HOME".into(), home_env)],
        "v" => vec![("V_HOME".into(), home_env)],
        "dart" => vec![("DART_HOME".into(), home_env)],
        "flutter" => vec![("FLUTTER_HOME".into(), home_env)],
        _ => Vec::new(),
    }
}

/// Result of resolution: real executable and extra environment for the child process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedShim {
    pub executable: PathBuf,
    pub extra_env: Vec<(String, String)>,
}

/// How `envr which` / JSON labels the selected runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhichRuntimeSource {
    /// `[runtimes.<key>].version` from merged project config.
    ProjectPin,
    /// Global `runtimes/<key>/current` (no project pin for this key).
    GlobalCurrent,
    /// PATH proxy disabled in settings; binary was taken from system `PATH`.
    PathProxyBypass,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhichRuntimeDetail {
    /// Version directory label (or `system` when bypassing and not under `.../versions/<label>/...`).
    pub version: String,
    pub source: WhichRuntimeSource,
}

/// `true` when [`resolve_core_shim_command`] would use the PATH-proxy-bypass branch.
pub fn core_command_uses_path_proxy_bypass(cmd: CoreCommand) -> bool {
    let settings = load_shim_settings_snapshot();
    uses_path_proxy_bypass(cmd, &settings)
}

fn envr_version_dir_from_executable(executable: &Path) -> Option<String> {
    let mut cur = executable.parent()?;
    loop {
        let parent = cur.parent()?;
        if parent.file_name().and_then(|n| n.to_str()) == Some("versions") {
            return cur.file_name()?.to_str().map(|s| s.to_string());
        }
        cur = parent;
    }
}

pub fn runtime_version_label_from_executable(executable: &Path) -> Option<String> {
    envr_version_dir_from_executable(executable)
}

fn ruby_version_from_version_file(working_dir: &Path) -> Option<String> {
    let p = working_dir.join(".ruby-version");
    let s = std::fs::read_to_string(&p).ok()?;
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

/// Metadata for `envr which` (no subprocess; aligns with [`resolve_core_shim_command`] routing).
pub fn which_runtime_detail(
    cmd: CoreCommand,
    ctx: &ShimContext,
    executable: &Path,
) -> EnvrResult<WhichRuntimeDetail> {
    if core_command_uses_path_proxy_bypass(cmd) {
        let version =
            envr_version_dir_from_executable(executable).unwrap_or_else(|| "system".into());
        return Ok(WhichRuntimeDetail {
            version,
            source: WhichRuntimeSource::PathProxyBypass,
        });
    }

    let key = cmd.project_runtime_key();
    let cfg =
        load_project_config_profile(&ctx.working_dir, ctx.profile.as_deref())?.map(|(c, _)| c);

    let from_project_cfg = cfg
        .as_ref()
        .and_then(|c| c.runtimes.get(key))
        .and_then(|r| r.version.as_deref())
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);

    let from_project = from_project_cfg || {
        if key == "ruby" {
            ruby_version_from_version_file(&ctx.working_dir).is_some()
        } else {
            false
        }
    };

    let home = resolve_runtime_home_for_lang(ctx, key, None)?;
    let version = home
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("?")
        .to_string();

    let source = if from_project {
        WhichRuntimeSource::ProjectPin
    } else {
        WhichRuntimeSource::GlobalCurrent
    };

    Ok(WhichRuntimeDetail { version, source })
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
        "kotlin" => Some(CoreCommand::Kotlin),
        "kotlinc" => Some(CoreCommand::Kotlinc),
        "scala" => Some(CoreCommand::Scala),
        "scalac" => Some(CoreCommand::Scalac),
        "clojure" => Some(CoreCommand::Clojure),
        "clj" => Some(CoreCommand::Clj),
        "groovy" => Some(CoreCommand::Groovy),
        "groovyc" => Some(CoreCommand::Groovyc),
        "terraform" => Some(CoreCommand::Terraform),
        "v" => Some(CoreCommand::V),
        "odin" => Some(CoreCommand::Odin),
        "purs" => Some(CoreCommand::Purs),
        "elm" => Some(CoreCommand::Elm),
        "dart" => Some(CoreCommand::Dart),
        "flutter" => Some(CoreCommand::Flutter),
        "go" => Some(CoreCommand::Go),
        "gofmt" => Some(CoreCommand::Gofmt),
        "php" => Some(CoreCommand::Php),
        "deno" => Some(CoreCommand::Deno),
        "bun" => Some(CoreCommand::Bun),
        "bunx" => Some(CoreCommand::Bunx),
        "dotnet" => Some(CoreCommand::Dotnet),
        "ruby" => Some(CoreCommand::Ruby),
        "gem" => Some(CoreCommand::Gem),
        "bundle" => Some(CoreCommand::Bundle),
        "irb" => Some(CoreCommand::Irb),
        "elixir" => Some(CoreCommand::Elixir),
        "mix" => Some(CoreCommand::Mix),
        "iex" => Some(CoreCommand::Iex),
        "erl" => Some(CoreCommand::Erl),
        "erlc" => Some(CoreCommand::Erlc),
        "escript" => Some(CoreCommand::Escript),
        "zig" => Some(CoreCommand::Zig),
        "julia" => Some(CoreCommand::Julia),
        "lua" => Some(CoreCommand::Lua),
        "luac" => Some(CoreCommand::Luac),
        "nim" => Some(CoreCommand::Nim),
        "crystal" => Some(CoreCommand::Crystal),
        "perl" => Some(CoreCommand::Perl),
        "r" => Some(CoreCommand::R),
        "rscript" => Some(CoreCommand::Rscript),
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

    let constraint = SpecConstraint::parse(spec)?;

    let mut best: Option<((u32, u32, u32), PathBuf)> = None;
    for e in std::fs::read_dir(versions_dir).map_err(EnvrError::from)? {
        let e = e.map_err(EnvrError::from)?;
        if !e.file_type().map_err(EnvrError::from)?.is_dir() {
            continue;
        }
        let d = e.path();
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

/// Like [`pick_version_home`] but respects PHP Windows NTS vs TS install directories (`*-nts` / `*-ts`).
pub fn pick_php_version_home(
    versions_dir: &Path,
    spec: &str,
    want_ts: bool,
) -> EnvrResult<PathBuf> {
    #[cfg(not(windows))]
    {
        let _ = want_ts;
        return pick_version_home(versions_dir, spec);
    }
    #[cfg(windows)]
    {
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

        let flavored = versions_dir.join(envr_config::php_layout::version_dir_name(spec, want_ts));
        if flavored.is_dir() {
            return Ok(flavored);
        }

        let direct = versions_dir.join(spec);
        if direct.is_dir()
            && let Some(name) = direct.file_name().and_then(|n| n.to_str())
            && envr_config::php_layout::dir_matches_build_flavor(name, want_ts)
        {
            return Ok(direct);
        }

        let constraint = SpecConstraint::parse(spec)?;

        let mut best: Option<((u32, u32, u32), PathBuf)> = None;
        for e in std::fs::read_dir(versions_dir).map_err(EnvrError::from)? {
            let e = e.map_err(EnvrError::from)?;
            if !e.file_type().map_err(EnvrError::from)?.is_dir() {
                continue;
            }
            let d = e.path();
            let Some(name) = d.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if !envr_config::php_layout::dir_matches_build_flavor(name, want_ts) {
                continue;
            }
            let Some(triple) = parse_dir_version_triplet(name) else {
                continue;
            };
            if constraint.matches(triple) && best.as_ref().is_none_or(|(v, _)| triple > *v) {
                best = Some((triple, d));
            }
        }

        let Some((_, path)) = best else {
            return Err(EnvrError::Runtime(format!(
                "no installed php matches project pin {spec:?} ({}) under {}",
                if want_ts { "TS" } else { "NTS" },
                versions_dir.display()
            )));
        };

        Ok(path)
    }
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

fn resolve_current_link_to_home(current_link: &Path) -> EnvrResult<PathBuf> {
    if !current_link.exists() {
        return Err(EnvrError::Runtime(format!(
            "no global current at {}",
            current_link.display()
        )));
    }
    // `current` is usually a symlink/junction, but on Windows some environments
    // forbid creating links. In that case we may fall back to a pointer file:
    // `current` contains the absolute target dir.
    if current_link.is_file() {
        let s = std::fs::read_to_string(current_link).map_err(EnvrError::from)?;
        let t = s.trim();
        let target = std::path::PathBuf::from(t);
        return std::fs::canonicalize(&target).map_err(EnvrError::from);
    }
    if let Ok(t) = std::fs::read_link(current_link) {
        let resolved = if t.is_relative() {
            current_link.parent().map(|p| p.join(&t)).unwrap_or(t)
        } else {
            t
        };
        return std::fs::canonicalize(&resolved).map_err(EnvrError::from);
    }
    std::fs::canonicalize(current_link).map_err(EnvrError::from)
}

fn runtime_home_for_php(
    ctx: &ShimContext,
    config: Option<&ProjectConfig>,
    spec_override: Option<&str>,
    settings: &ShimSettingsSnapshot,
) -> EnvrResult<PathBuf> {
    let want_ts = settings.php_windows_build_want_ts;
    let versions_dir = ctx
        .runtime_root
        .join("runtimes")
        .join("php")
        .join("versions");
    let php_home = ctx.runtime_root.join("runtimes").join("php");

    let pinned = spec_override
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or_else(|| {
            config
                .and_then(|c| c.runtimes.get("php"))
                .and_then(|r| r.version.as_deref())
        });

    if let Some(spec) = pinned {
        pick_php_version_home(&versions_dir, spec, want_ts)
    } else {
        for name in ["current", "current-ts", "current-nts"] {
            let link = php_home.join(name);
            if link.exists() {
                return resolve_current_link_to_home(&link);
            }
        }
        Err(EnvrError::Runtime(format!(
            "no global current for php under {}; install and select a version",
            php_home.display()
        )))
    }
}

fn runtime_home_for_key(
    ctx: &ShimContext,
    key: &str,
    config: Option<&ProjectConfig>,
    spec_override: Option<&str>,
    settings: &ShimSettingsSnapshot,
) -> EnvrResult<PathBuf> {
    if key == "php" {
        return runtime_home_for_php(ctx, config, spec_override, settings);
    }

    let versions_dir = ctx.runtime_root.join("runtimes").join(key).join("versions");
    let current_link = ctx.runtime_root.join("runtimes").join(key).join("current");

    let pinned: Option<String> = spec_override
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| {
            config
                .and_then(|c| c.runtimes.get(key))
                .and_then(|r| r.version.as_deref())
                .map(|s| s.to_string())
        });

    let pinned = pinned.or_else(|| {
        // Project-local Ruby convention: use `.ruby-version` when `.envr.toml`
        // doesn't pin ruby. `.envr.toml` still wins on conflict.
        if key == "ruby" {
            ruby_version_from_version_file(&ctx.working_dir)
        } else {
            None
        }
    });

    if let Some(spec) = pinned.as_deref() {
        pick_version_home(&versions_dir, spec)
    } else if !current_link.exists() {
        Err(EnvrError::Runtime(format!(
            "no global current for {key} at {}; install and select a version",
            current_link.display()
        )))
    } else {
        resolve_current_link_to_home(&current_link)
    }
}

/// Like [`resolve_runtime_home_for_lang`], but uses `project_config` when `Some` instead of
/// loading `.envr.toml` from disk. Pass config from `load_project_config_profile` when you
/// already have it to avoid a second read.
pub fn resolve_runtime_home_for_lang_with_project(
    ctx: &ShimContext,
    lang_key: &str,
    spec_override: Option<&str>,
    project_config: Option<&ProjectConfig>,
) -> EnvrResult<PathBuf> {
    let settings = load_shim_settings_snapshot();
    runtime_home_for_key(ctx, lang_key, project_config, spec_override, &settings)
}

/// Like [`resolve_runtime_home_for_lang_with_project`], but reuses a preloaded
/// [`ShimSettingsSnapshot`] to avoid duplicate settings reads in a shim invocation.
pub fn resolve_runtime_home_for_lang_with_project_and_settings(
    ctx: &ShimContext,
    lang_key: &str,
    spec_override: Option<&str>,
    project_config: Option<&ProjectConfig>,
    settings: &ShimSettingsSnapshot,
) -> EnvrResult<PathBuf> {
    runtime_home_for_key(ctx, lang_key, project_config, spec_override, settings)
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
    resolve_runtime_home_for_lang_with_project(ctx, lang_key, spec_override, cfg.as_ref())
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

fn scala_tool_path(home: &Path, cmd: CoreCommand) -> EnvrResult<PathBuf> {
    let bin = home.join("bin");
    match cmd {
        CoreCommand::Scala => Ok(first_existing(&[
            bin.join("scala.cmd"),
            bin.join("scala.bat"),
            bin.join("scala.exe"),
            bin.join("scala"),
        ])
        .ok_or_else(|| EnvrError::Runtime(format!("scala missing under {}", home.display())))?),
        CoreCommand::Scalac => Ok(first_existing(&[
            bin.join("scalac.cmd"),
            bin.join("scalac.bat"),
            bin.join("scalac.exe"),
            bin.join("scalac"),
        ])
        .ok_or_else(|| EnvrError::Runtime(format!("scalac missing under {}", home.display())))?),
        _ => Err(EnvrError::Runtime("internal: not a scala tool".into())),
    }
}

fn clojure_tool_path(home: &Path, cmd: CoreCommand) -> EnvrResult<PathBuf> {
    let bases = [home.to_path_buf(), home.join("bin")];
    let mut cands = Vec::new();
    match cmd {
        CoreCommand::Clojure => {
            for base in &bases {
                cands.push(base.join("clojure.cmd"));
                cands.push(base.join("clojure.bat"));
                cands.push(base.join("clojure.exe"));
                cands.push(base.join("clojure"));
            }
            Ok(first_existing(&cands).ok_or_else(|| {
                EnvrError::Runtime(format!("clojure missing under {}", home.display()))
            })?)
        }
        CoreCommand::Clj => {
            for base in &bases {
                cands.push(base.join("clj.cmd"));
                cands.push(base.join("clj.bat"));
                cands.push(base.join("clj.exe"));
                cands.push(base.join("clj"));
            }
            Ok(first_existing(&cands).ok_or_else(|| {
                EnvrError::Runtime(format!("clj missing under {}", home.display()))
            })?)
        }
        _ => Err(EnvrError::Runtime("internal: not a clojure tool".into())),
    }
}

fn groovy_tool_path(home: &Path, cmd: CoreCommand) -> EnvrResult<PathBuf> {
    let bin = home.join("bin");
    match cmd {
        CoreCommand::Groovy => Ok(first_existing(&[
            bin.join("groovy.cmd"),
            bin.join("groovy.bat"),
            bin.join("groovy.exe"),
            bin.join("groovy"),
        ])
        .ok_or_else(|| EnvrError::Runtime(format!("groovy missing under {}", home.display())))?),
        CoreCommand::Groovyc => Ok(first_existing(&[
            bin.join("groovyc.cmd"),
            bin.join("groovyc.bat"),
            bin.join("groovyc.exe"),
            bin.join("groovyc"),
        ])
        .ok_or_else(|| EnvrError::Runtime(format!("groovyc missing under {}", home.display())))?),
        _ => Err(EnvrError::Runtime("internal: not a groovy tool".into())),
    }
}

fn terraform_tool_path(home: &Path, cmd: CoreCommand) -> EnvrResult<PathBuf> {
    match cmd {
        CoreCommand::Terraform => Ok(first_existing(&[
            home.join("terraform.exe"),
            home.join("terraform"),
            home.join("bin").join("terraform.exe"),
            home.join("bin").join("terraform"),
        ])
        .ok_or_else(|| {
            EnvrError::Runtime(format!("terraform missing under {}", home.display()))
        })?),
        _ => Err(EnvrError::Runtime("internal: not a terraform tool".into())),
    }
}

fn v_tool_path(home: &Path, cmd: CoreCommand) -> EnvrResult<PathBuf> {
    match cmd {
        CoreCommand::V => Ok(first_existing(&[
            home.join("v.exe"),
            home.join("v"),
            home.join("bin").join("v.exe"),
            home.join("bin").join("v"),
        ])
        .ok_or_else(|| EnvrError::Runtime(format!("v missing under {}", home.display())))?),
        _ => Err(EnvrError::Runtime("internal: not a v tool".into())),
    }
}

fn odin_tool_path(home: &Path, cmd: CoreCommand) -> EnvrResult<PathBuf> {
    match cmd {
        CoreCommand::Odin => Ok(first_existing(&[
            home.join("odin.exe"),
            home.join("odin"),
            home.join("bin").join("odin.exe"),
            home.join("bin").join("odin"),
        ])
        .ok_or_else(|| EnvrError::Runtime(format!("odin missing under {}", home.display())))?),
        _ => Err(EnvrError::Runtime("internal: not an odin tool".into())),
    }
}

fn purs_tool_path(home: &Path, cmd: CoreCommand) -> EnvrResult<PathBuf> {
    match cmd {
        CoreCommand::Purs => Ok(first_existing(&[
            home.join("purs.exe"),
            home.join("purs"),
            home.join("bin").join("purs.exe"),
            home.join("bin").join("purs"),
        ])
        .ok_or_else(|| EnvrError::Runtime(format!("purs missing under {}", home.display())))?),
        _ => Err(EnvrError::Runtime("internal: not a purescript tool".into())),
    }
}

fn elm_tool_path(home: &Path, cmd: CoreCommand) -> EnvrResult<PathBuf> {
    match cmd {
        CoreCommand::Elm => Ok(first_existing(&[
            home.join("elm.exe"),
            home.join("elm"),
            home.join("bin").join("elm.exe"),
            home.join("bin").join("elm"),
        ])
        .ok_or_else(|| EnvrError::Runtime(format!("elm missing under {}", home.display())))?),
        _ => Err(EnvrError::Runtime("internal: not an elm tool".into())),
    }
}

fn dart_tool_path(home: &Path, cmd: CoreCommand) -> EnvrResult<PathBuf> {
    match cmd {
        CoreCommand::Dart => bin_tool_layout::resolve_dart_exe(home)
            .ok_or_else(|| EnvrError::Runtime(format!("dart missing under {}", home.display()))),
        _ => Err(EnvrError::Runtime("internal: not a dart tool".into())),
    }
}

fn flutter_tool_path(home: &Path, cmd: CoreCommand) -> EnvrResult<PathBuf> {
    match cmd {
        CoreCommand::Flutter => bin_tool_layout::resolve_flutter_exe(home)
            .ok_or_else(|| EnvrError::Runtime(format!("flutter missing under {}", home.display()))),
        _ => Err(EnvrError::Runtime("internal: not a flutter tool".into())),
    }
}

fn kotlin_tool_path(home: &Path, cmd: CoreCommand) -> EnvrResult<PathBuf> {
    let bin = home.join("bin");
    match cmd {
        CoreCommand::Kotlin => Ok(first_existing(&[
            bin.join("kotlin.cmd"),
            bin.join("kotlin.bat"),
            bin.join("kotlin.exe"),
            bin.join("kotlin"),
        ])
        .ok_or_else(|| EnvrError::Runtime(format!("kotlin missing under {}", home.display())))?),
        CoreCommand::Kotlinc => Ok(first_existing(&[
            bin.join("kotlinc.cmd"),
            bin.join("kotlinc.bat"),
            bin.join("kotlinc.exe"),
            bin.join("kotlinc"),
        ])
        .ok_or_else(|| EnvrError::Runtime(format!("kotlinc missing under {}", home.display())))?),
        _ => Err(EnvrError::Runtime("internal: not a kotlin tool".into())),
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

fn deno_tool_path(home: &Path, cmd: CoreCommand) -> EnvrResult<PathBuf> {
    match cmd {
        CoreCommand::Deno => Ok(first_existing(&[
            home.join("deno.exe"),
            home.join("bin").join("deno.exe"),
            home.join("bin").join("deno"),
            home.join("deno"),
        ])
        .ok_or_else(|| EnvrError::Runtime(format!("deno missing under {}", home.display())))?),
        _ => Err(EnvrError::Runtime("internal: not a deno tool".into())),
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

fn php_tool_path(home: &Path, cmd: CoreCommand) -> EnvrResult<PathBuf> {
    match cmd {
        CoreCommand::Php => Ok(first_existing(&[
            home.join("php.exe"),
            home.join("bin").join("php"),
            home.join("php"),
        ])
        .ok_or_else(|| EnvrError::Runtime(format!("php missing under {}", home.display())))?),
        _ => Err(EnvrError::Runtime("internal: not a php tool".into())),
    }
}

fn ruby_tool_path(home: &Path, cmd: CoreCommand) -> EnvrResult<PathBuf> {
    let bin = home.join("bin");
    match cmd {
        CoreCommand::Ruby => Ok(first_existing(&[
            bin.join("ruby.exe"),
            bin.join("ruby.cmd"),
            bin.join("ruby.bat"),
            bin.join("ruby"),
        ])
        .ok_or_else(|| EnvrError::Runtime(format!("ruby missing under {}", home.display())))?),
        CoreCommand::Gem => Ok(first_existing(&[
            bin.join("gem.cmd"),
            bin.join("gem.bat"),
            bin.join("gem.exe"),
            bin.join("gem"),
        ])
        .ok_or_else(|| EnvrError::Runtime(format!("gem missing under {}", home.display())))?),
        CoreCommand::Bundle => Ok(first_existing(&[
            bin.join("bundle.cmd"),
            bin.join("bundle.bat"),
            bin.join("bundle.exe"),
            bin.join("bundle"),
        ])
        .ok_or_else(|| EnvrError::Runtime(format!("bundle missing under {}", home.display())))?),
        CoreCommand::Irb => Ok(first_existing(&[
            bin.join("irb.cmd"),
            bin.join("irb.bat"),
            bin.join("irb.exe"),
            bin.join("irb"),
        ])
        .ok_or_else(|| EnvrError::Runtime(format!("irb missing under {}", home.display())))?),
        _ => Err(EnvrError::Runtime("internal: not a ruby tool".into())),
    }
}

fn elixir_tool_path(home: &Path, cmd: CoreCommand) -> EnvrResult<PathBuf> {
    let bin = home.join("bin");
    match cmd {
        CoreCommand::Elixir => Ok(first_existing(&[
            bin.join("elixir.bat"),
            bin.join("elixir.cmd"),
            bin.join("elixir.exe"),
            bin.join("elixir"),
        ])
        .ok_or_else(|| EnvrError::Runtime(format!("elixir missing under {}", home.display())))?),
        CoreCommand::Mix => Ok(first_existing(&[
            bin.join("mix.bat"),
            bin.join("mix.cmd"),
            bin.join("mix.exe"),
            bin.join("mix"),
        ])
        .ok_or_else(|| EnvrError::Runtime(format!("mix missing under {}", home.display())))?),
        CoreCommand::Iex => Ok(first_existing(&[
            bin.join("iex.bat"),
            bin.join("iex.cmd"),
            bin.join("iex.exe"),
            bin.join("iex"),
        ])
        .ok_or_else(|| EnvrError::Runtime(format!("iex missing under {}", home.display())))?),
        _ => Err(EnvrError::Runtime("internal: not an elixir tool".into())),
    }
}

fn erlang_tool_path(home: &Path, cmd: CoreCommand) -> EnvrResult<PathBuf> {
    let bin = home.join("bin");
    match cmd {
        CoreCommand::Erl => Ok(first_existing(&[
            bin.join("erl.exe"),
            bin.join("erl.cmd"),
            bin.join("erl.bat"),
            bin.join("erl"),
        ])
        .ok_or_else(|| EnvrError::Runtime(format!("erl missing under {}", home.display())))?),
        CoreCommand::Erlc => Ok(first_existing(&[
            bin.join("erlc.exe"),
            bin.join("erlc.cmd"),
            bin.join("erlc.bat"),
            bin.join("erlc"),
        ])
        .ok_or_else(|| EnvrError::Runtime(format!("erlc missing under {}", home.display())))?),
        CoreCommand::Escript => Ok(first_existing(&[
            bin.join("escript.exe"),
            bin.join("escript.cmd"),
            bin.join("escript.bat"),
            bin.join("escript"),
        ])
        .ok_or_else(|| EnvrError::Runtime(format!("escript missing under {}", home.display())))?),
        _ => Err(EnvrError::Runtime("internal: not an erlang tool".into())),
    }
}

fn dotnet_tool_path(home: &Path, cmd: CoreCommand) -> EnvrResult<PathBuf> {
    match cmd {
        CoreCommand::Dotnet => Ok(first_existing(&[
            home.join("dotnet.exe"),
            home.join("dotnet"),
            home.join("bin").join("dotnet"),
        ])
        .ok_or_else(|| EnvrError::Runtime(format!("dotnet missing under {}", home.display())))?),
        _ => Err(EnvrError::Runtime("internal: not a dotnet tool".into())),
    }
}

fn zig_tool_path(home: &Path, cmd: CoreCommand) -> EnvrResult<PathBuf> {
    match cmd {
        CoreCommand::Zig => bin_tool_layout::resolve_zig_exe(home)
            .ok_or_else(|| EnvrError::Runtime(format!("zig missing under {}", home.display()))),
        _ => Err(EnvrError::Runtime("internal: not a zig tool".into())),
    }
}

fn julia_tool_path(home: &Path, cmd: CoreCommand) -> EnvrResult<PathBuf> {
    match cmd {
        CoreCommand::Julia => bin_tool_layout::resolve_julia_exe(home)
            .ok_or_else(|| EnvrError::Runtime(format!("julia missing under {}", home.display()))),
        _ => Err(EnvrError::Runtime("internal: not a julia tool".into())),
    }
}

fn lua_tool_path(home: &Path, cmd: CoreCommand) -> EnvrResult<PathBuf> {
    match cmd {
        CoreCommand::Lua => lua_binaries::resolve_lua_interpreter_exe(home)
            .ok_or_else(|| EnvrError::Runtime(format!("lua missing under {}", home.display()))),
        CoreCommand::Luac => lua_binaries::resolve_luac_exe(home)
            .ok_or_else(|| EnvrError::Runtime(format!("luac missing under {}", home.display()))),
        _ => Err(EnvrError::Runtime("internal: not a lua tool".into())),
    }
}

fn nim_tool_path(home: &Path, cmd: CoreCommand) -> EnvrResult<PathBuf> {
    match cmd {
        CoreCommand::Nim => bin_tool_layout::resolve_nim_exe(home)
            .ok_or_else(|| EnvrError::Runtime(format!("nim missing under {}", home.display()))),
        _ => Err(EnvrError::Runtime("internal: not a nim tool".into())),
    }
}

fn crystal_tool_path(home: &Path, cmd: CoreCommand) -> EnvrResult<PathBuf> {
    match cmd {
        CoreCommand::Crystal => {
            let c = envr_domain::crystal_paths::crystal_compiler_candidate_paths(home);
            Ok(first_existing(&c).ok_or_else(|| {
                EnvrError::Runtime(format!("crystal missing under {}", home.display()))
            })?)
        }
        _ => Err(EnvrError::Runtime("internal: not a crystal tool".into())),
    }
}

fn perl_tool_path(home: &Path, cmd: CoreCommand) -> EnvrResult<PathBuf> {
    match cmd {
        CoreCommand::Perl => bin_tool_layout::resolve_perl_exe(home).ok_or_else(|| {
            EnvrError::Runtime(format!("perl missing under {}", home.display()))
        }),
        _ => Err(EnvrError::Runtime("internal: not a perl tool".into())),
    }
}

fn rlang_tool_path(home: &Path, cmd: CoreCommand) -> EnvrResult<PathBuf> {
    match cmd {
        CoreCommand::R => bin_tool_layout::resolve_r_exe(home)
            .ok_or_else(|| EnvrError::Runtime(format!("R missing under {}", home.display()))),
        CoreCommand::Rscript => bin_tool_layout::resolve_rscript_exe(home)
            .ok_or_else(|| EnvrError::Runtime(format!("Rscript missing under {}", home.display()))),
        _ => Err(EnvrError::Runtime("internal: not an R tool".into())),
    }
}

/// Resolved path to a core tool under a runtime **home** directory (e.g. `current` target).
pub fn core_tool_executable(home: &Path, cmd: CoreCommand) -> EnvrResult<PathBuf> {
    match cmd {
        CoreCommand::Node | CoreCommand::Npm | CoreCommand::Npx => node_tool_path(home, cmd),
        CoreCommand::Python | CoreCommand::Pip => python_tool_path(home, cmd),
        CoreCommand::Java | CoreCommand::Javac => java_tool_path(home, cmd),
        CoreCommand::Kotlin | CoreCommand::Kotlinc => kotlin_tool_path(home, cmd),
        CoreCommand::Scala | CoreCommand::Scalac => scala_tool_path(home, cmd),
        CoreCommand::Clojure | CoreCommand::Clj => clojure_tool_path(home, cmd),
        CoreCommand::Groovy | CoreCommand::Groovyc => groovy_tool_path(home, cmd),
        CoreCommand::Terraform => terraform_tool_path(home, cmd),
        CoreCommand::V => v_tool_path(home, cmd),
        CoreCommand::Odin => odin_tool_path(home, cmd),
        CoreCommand::Purs => purs_tool_path(home, cmd),
        CoreCommand::Elm => elm_tool_path(home, cmd),
        CoreCommand::Dart => dart_tool_path(home, cmd),
        CoreCommand::Flutter => flutter_tool_path(home, cmd),
        CoreCommand::Go | CoreCommand::Gofmt => go_tool_path(home, cmd),
        CoreCommand::Php => php_tool_path(home, cmd),
        CoreCommand::Deno => deno_tool_path(home, cmd),
        CoreCommand::Bun | CoreCommand::Bunx => bun_tool_path(home, cmd),
        CoreCommand::Dotnet => dotnet_tool_path(home, cmd),
        CoreCommand::Ruby | CoreCommand::Gem | CoreCommand::Bundle | CoreCommand::Irb => {
            ruby_tool_path(home, cmd)
        }
        CoreCommand::Elixir | CoreCommand::Mix | CoreCommand::Iex => elixir_tool_path(home, cmd),
        CoreCommand::Erl | CoreCommand::Erlc | CoreCommand::Escript => erlang_tool_path(home, cmd),
        CoreCommand::Zig => zig_tool_path(home, cmd),
        CoreCommand::Julia => julia_tool_path(home, cmd),
        CoreCommand::Lua | CoreCommand::Luac => lua_tool_path(home, cmd),
        CoreCommand::Nim => nim_tool_path(home, cmd),
        CoreCommand::Crystal => crystal_tool_path(home, cmd),
        CoreCommand::Perl => perl_tool_path(home, cmd),
        CoreCommand::R | CoreCommand::Rscript => rlang_tool_path(home, cmd),
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

fn paths_equal_trimmed_case_insensitive(a: &Path, b: &Path) -> bool {
    fn norm(p: &Path) -> String {
        p.as_os_str()
            .to_string_lossy()
            .to_ascii_lowercase()
            .trim_end_matches(|c| c == '/' || c == '\\')
            .to_string()
    }
    norm(a) == norm(b)
}

/// `dir` is the envr shims folder for the effective [`resolve_runtime_root`] layout
/// (logical match or same inode after `canonicalize`).
fn path_matches_managed_shims(dir: &Path, managed: &Path) -> bool {
    if paths_equal_trimmed_case_insensitive(dir, managed) {
        return true;
    }
    if dir.is_dir() && managed.is_dir() {
        if let (Ok(a), Ok(b)) = (std::fs::canonicalize(dir), std::fs::canonicalize(managed)) {
            return a == b;
        }
    }
    false
}

/// Skip a PATH segment that points at envr shims when we have no [`ShimContext`]
/// (e.g. Elixir `ERTS_BIN` discovery). Uses the same root as shim resolution plus a
/// narrow legacy heuristic.
fn should_skip_envr_shims_path_entry(dir: &Path) -> bool {
    if let Ok(root) = resolve_runtime_root() {
        if path_matches_managed_shims(dir, &root.join("shims")) {
            return true;
        }
    }
    is_likely_envr_shims_dir(dir)
}

/// True when `dir` is the envr-managed shims directory for this installation.
///
/// Prefer this over [`is_likely_envr_shims_dir`] alone: the legacy heuristic misses
/// `.../runtimes/shims` when the parent path has no `"envr"` substring (custom roots),
/// and it fails for Windows short (8.3) PATH segments that do not spell `envr`.
fn is_envr_managed_shims_dir(dir: &Path, ctx: &ShimContext) -> bool {
    let managed = ctx.runtime_root.join("shims");
    path_matches_managed_shims(dir, &managed) || is_likely_envr_shims_dir(dir)
}

fn find_on_path_outside_envr_shims(ctx: &ShimContext, tool_stem: &str) -> EnvrResult<PathBuf> {
    let path_os = std::env::var_os("PATH").ok_or_else(|| {
        EnvrError::Runtime(
            "PATH is not set; cannot search for host executables outside envr shims".into(),
        )
    })?;
    #[cfg(windows)]
    let suffixes: &[&str] = &[".cmd", ".exe", ".bat", ""];
    #[cfg(not(windows))]
    let suffixes: &[&str] = &[""];

    for dir in std::env::split_paths(&path_os) {
        if is_envr_managed_shims_dir(&dir, ctx) {
            continue;
        }
        for suf in suffixes {
            let candidate = if suf.is_empty() {
                dir.join(tool_stem)
            } else {
                dir.join(format!("{tool_stem}{suf}"))
            };
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }
    Err(EnvrError::Runtime(format!(
        "could not find `{tool_stem}` on PATH outside envr shims (enable PATH proxy in settings if this command should come from an envr-managed runtime)"
    )))
}

fn path_proxy_bypass_host_stem(cmd: CoreCommand) -> &'static str {
    match cmd {
        CoreCommand::Node => "node",
        CoreCommand::Npm => "npm",
        CoreCommand::Npx => "npx",
        CoreCommand::Python => "python",
        CoreCommand::Pip => "pip",
        CoreCommand::Java => "java",
        CoreCommand::Javac => "javac",
        CoreCommand::Kotlin => "kotlin",
        CoreCommand::Kotlinc => "kotlinc",
        CoreCommand::Scala => "scala",
        CoreCommand::Scalac => "scalac",
        CoreCommand::Clojure => "clojure",
        CoreCommand::Clj => "clj",
        CoreCommand::Groovy => "groovy",
        CoreCommand::Groovyc => "groovyc",
        CoreCommand::Terraform => "terraform",
        CoreCommand::V => "v",
        CoreCommand::Odin => "odin",
        CoreCommand::Purs => "purs",
        CoreCommand::Elm => "elm",
        CoreCommand::Dart => "dart",
        CoreCommand::Flutter => "flutter",
        CoreCommand::Go => "go",
        CoreCommand::Gofmt => "gofmt",
        CoreCommand::Php => "php",
        CoreCommand::Deno => "deno",
        CoreCommand::Bun => "bun",
        CoreCommand::Bunx => "bunx",
        CoreCommand::Dotnet => "dotnet",
        CoreCommand::Ruby => "ruby",
        CoreCommand::Gem => "gem",
        CoreCommand::Bundle => "bundle",
        CoreCommand::Irb => "irb",
        CoreCommand::Elixir => "elixir",
        CoreCommand::Mix => "mix",
        CoreCommand::Iex => "iex",
        CoreCommand::Erl => "erl",
        CoreCommand::Erlc => "erlc",
        CoreCommand::Escript => "escript",
        CoreCommand::Zig => "zig",
        CoreCommand::Julia => "julia",
        CoreCommand::Lua => "lua",
        CoreCommand::Luac => "luac",
        CoreCommand::Nim => "nim",
        CoreCommand::Crystal => "crystal",
        CoreCommand::Perl => "perl",
        CoreCommand::R => "r",
        CoreCommand::Rscript => "rscript",
    }
}

fn resolve_core_tool_bypass_envr(cmd: CoreCommand, ctx: &ShimContext) -> EnvrResult<ResolvedShim> {
    let stem = path_proxy_bypass_host_stem(cmd);
    let executable = find_on_path_outside_envr_shims(ctx, stem)?;
    Ok(ResolvedShim {
        executable,
        extra_env: vec![],
    })
}

/// Resolve a core tool to a filesystem executable path.
pub fn resolve_core_shim_command(cmd: CoreCommand, ctx: &ShimContext) -> EnvrResult<ResolvedShim> {
    let settings = load_shim_settings_snapshot();
    resolve_core_shim_command_with_settings(cmd, ctx, &settings)
}

/// Resolve a core tool to an executable path using a preloaded settings snapshot.
pub fn resolve_core_shim_command_with_settings(
    cmd: CoreCommand,
    ctx: &ShimContext,
    settings: &ShimSettingsSnapshot,
) -> EnvrResult<ResolvedShim> {
    if uses_path_proxy_bypass(cmd, settings) {
        return resolve_core_tool_bypass_envr(cmd, ctx);
    }
    let cfg =
        load_project_config_profile(&ctx.working_dir, ctx.profile.as_deref())?.map(|(c, _)| c);

    let key = cmd.project_runtime_key();
    let home = runtime_home_for_key(ctx, key, cfg.as_ref(), None, settings)?;

    let mut extra_env = runtime_home_env_for_key(&home, key);
    if envr_domain::jvm_hosted::is_jvm_hosted_runtime(key) {
        let java_home = runtime_home_for_key(ctx, "java", cfg.as_ref(), None, settings)?;
        let runtime_label = home.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let java_label = java_home.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if !runtime_label.is_empty()
            && let Some(msg) = envr_domain::jvm_hosted::hosted_runtime_jdk_mismatch_message(
                key,
                runtime_label,
                java_label,
            )
        {
            return Err(EnvrError::Runtime(msg));
        }
        extra_env.extend(runtime_home_env_for_key(&java_home, "java"));
    }

    let executable = match cmd {
        CoreCommand::Node | CoreCommand::Npm | CoreCommand::Npx => node_tool_path(&home, cmd)?,
        CoreCommand::Python | CoreCommand::Pip => python_tool_path(&home, cmd)?,
        CoreCommand::Java | CoreCommand::Javac => java_tool_path(&home, cmd)?,
        CoreCommand::Kotlin | CoreCommand::Kotlinc => kotlin_tool_path(&home, cmd)?,
        CoreCommand::Scala | CoreCommand::Scalac => scala_tool_path(&home, cmd)?,
        CoreCommand::Clojure | CoreCommand::Clj => clojure_tool_path(&home, cmd)?,
        CoreCommand::Groovy | CoreCommand::Groovyc => groovy_tool_path(&home, cmd)?,
        CoreCommand::Terraform => terraform_tool_path(&home, cmd)?,
        CoreCommand::V => v_tool_path(&home, cmd)?,
        CoreCommand::Odin => odin_tool_path(&home, cmd)?,
        CoreCommand::Purs => purs_tool_path(&home, cmd)?,
        CoreCommand::Elm => elm_tool_path(&home, cmd)?,
        CoreCommand::Dart => dart_tool_path(&home, cmd)?,
        CoreCommand::Flutter => flutter_tool_path(&home, cmd)?,
        CoreCommand::Go | CoreCommand::Gofmt => go_tool_path(&home, cmd)?,
        CoreCommand::Php => php_tool_path(&home, cmd)?,
        CoreCommand::Deno => {
            extra_env.extend(settings.deno_registry_env.clone());
            deno_tool_path(&home, cmd)?
        }
        CoreCommand::Bun | CoreCommand::Bunx => {
            extra_env.extend(settings.bun_registry_env.clone());
            bun_tool_path(&home, cmd)?
        }
        CoreCommand::Dotnet => dotnet_tool_path(&home, cmd)?,
        CoreCommand::Ruby | CoreCommand::Gem | CoreCommand::Bundle | CoreCommand::Irb => {
            ruby_tool_path(&home, cmd)?
        }
        CoreCommand::Elixir | CoreCommand::Mix | CoreCommand::Iex => elixir_tool_path(&home, cmd)?,
        CoreCommand::Erl | CoreCommand::Erlc | CoreCommand::Escript => {
            erlang_tool_path(&home, cmd)?
        }
        CoreCommand::Zig => zig_tool_path(&home, cmd)?,
        CoreCommand::Julia => julia_tool_path(&home, cmd)?,
        CoreCommand::Lua | CoreCommand::Luac => lua_tool_path(&home, cmd)?,
        CoreCommand::Nim => nim_tool_path(&home, cmd)?,
        CoreCommand::Crystal => crystal_tool_path(&home, cmd)?,
        CoreCommand::Perl => perl_tool_path(&home, cmd)?,
        CoreCommand::R | CoreCommand::Rscript => rlang_tool_path(&home, cmd)?,
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
    use std::path::Path;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn envr_version_dir_from_executable_finds_segment() {
        let p = Path::new("/data/runtimes/node/versions/20.10.0/bin/node");
        assert_eq!(
            envr_version_dir_from_executable(p).as_deref(),
            Some("20.10.0")
        );
    }

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
        assert_eq!(parse_core_command("php"), Some(CoreCommand::Php));
        assert_eq!(parse_core_command("deno"), Some(CoreCommand::Deno));
        assert_eq!(parse_core_command("ruby"), Some(CoreCommand::Ruby));
        assert_eq!(parse_core_command("gem"), Some(CoreCommand::Gem));
        assert_eq!(parse_core_command("bundle"), Some(CoreCommand::Bundle));
        assert_eq!(parse_core_command("irb"), Some(CoreCommand::Irb));
        assert_eq!(parse_core_command("elixir"), Some(CoreCommand::Elixir));
        assert_eq!(parse_core_command("mix"), Some(CoreCommand::Mix));
        assert_eq!(parse_core_command("iex"), Some(CoreCommand::Iex));
        assert_eq!(parse_core_command("erl"), Some(CoreCommand::Erl));
        assert_eq!(parse_core_command("erlc"), Some(CoreCommand::Erlc));
        assert_eq!(parse_core_command("escript"), Some(CoreCommand::Escript));
        assert_eq!(parse_core_command("zig"), Some(CoreCommand::Zig));
        assert_eq!(parse_core_command("julia"), Some(CoreCommand::Julia));
        assert_eq!(parse_core_command("lua"), Some(CoreCommand::Lua));
        assert_eq!(parse_core_command("luac"), Some(CoreCommand::Luac));
        assert_eq!(parse_core_command("nim"), Some(CoreCommand::Nim));
        assert_eq!(parse_core_command("crystal"), Some(CoreCommand::Crystal));
        assert_eq!(parse_core_command("perl"), Some(CoreCommand::Perl));
        assert_eq!(parse_core_command("r"), Some(CoreCommand::R));
        assert_eq!(parse_core_command("rscript"), Some(CoreCommand::Rscript));
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
    fn resolve_runtime_home_for_lang_uses_ruby_version_file_when_no_project_pin() {
        let tmp = tempfile::TempDir::new().expect("tmp");
        let root = tmp.path();
        let prj = root.join("prj");
        fs::create_dir_all(&prj).expect("prj");

        let versions = root.join("runtimes/ruby/versions");
        fs::create_dir_all(versions.join("3.3.11")).expect("ruby 3.3.11 dir");

        fs::write(prj.join(".ruby-version"), "3.3.11").expect("write .ruby-version");

        let ctx = ShimContext {
            runtime_root: root.to_path_buf(),
            working_dir: prj,
            profile: None,
        };
        let got = resolve_runtime_home_for_lang(&ctx, "ruby", None).expect("resolve");
        assert!(got.ends_with("3.3.11"), "{got:?}");
    }

    #[test]
    fn resolve_runtime_home_for_lang_ruby_version_file_ignored_when_project_pin_present() {
        let tmp = tempfile::TempDir::new().expect("tmp");
        let root = tmp.path();
        let prj = root.join("prj");
        fs::create_dir_all(&prj).expect("prj");

        let versions = root.join("runtimes/ruby/versions");
        fs::create_dir_all(versions.join("3.3.11")).expect("ruby 3.3.11 dir");
        fs::create_dir_all(versions.join("3.3.12")).expect("ruby 3.3.12 dir");

        fs::write(
            prj.join(".envr.toml"),
            r#"
[runtimes.ruby]
version = "3.3.12"
"#,
        )
        .expect("write .envr.toml");
        fs::write(prj.join(".ruby-version"), "3.3.11").expect("write .ruby-version");

        let ctx = ShimContext {
            runtime_root: root.to_path_buf(),
            working_dir: prj,
            profile: None,
        };
        let got = resolve_runtime_home_for_lang(&ctx, "ruby", None).expect("resolve");
        assert!(got.ends_with("3.3.12"), "{got:?}");
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

    #[test]
    fn runtime_helpers_cover_dotnet_and_go_env_policy() {
        let home = Path::new("/tmp/envr-runtime");
        let dotnet_bins = runtime_bin_dirs_for_key(home, "dotnet");
        assert_eq!(dotnet_bins, vec![home.to_path_buf(), home.join("bin")]);

        let go_env = runtime_home_env_for_key(home, "go");
        assert_eq!(go_env, vec![("GOROOT".into(), home.display().to_string())]);

        let dotnet_env = runtime_home_env_for_key(home, "dotnet");
        assert_eq!(
            dotnet_env,
            vec![
                ("DOTNET_ROOT".into(), home.display().to_string()),
                ("DOTNET_MULTILEVEL_LOOKUP".into(), "0".into()),
            ]
        );
    }

    #[cfg(windows)]
    #[test]
    fn resolve_runtime_home_uses_current_pointer_file() {
        let tmp = tempfile::TempDir::new().expect("tmp");
        let root = tmp.path();
        let home = root.join("runtimes/node/versions/20.10.0");
        fs::create_dir_all(&home).expect("home");
        fs::create_dir_all(root.join("runtimes/node")).expect("node root");
        fs::create_dir_all(root.join("prj")).expect("prj");
        fs::write(
            root.join("runtimes/node/current"),
            home.display().to_string(),
        )
        .expect("current");

        let ctx = ShimContext {
            runtime_root: root.to_path_buf(),
            working_dir: root.join("prj"),
            profile: None,
        };
        let got = resolve_runtime_home_for_lang(&ctx, "node", None).expect("resolve");
        assert!(got.ends_with("20.10.0"), "{got:?}");
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

    #[test]
    fn ruby_and_elixir_path_proxy_bypass_follow_settings_disk() {
        let _guard = ENV_LOCK.lock().expect("lock");

        let tmp = tempfile::TempDir::new().expect("tmp");
        let cfg_dir = tmp.path().join("config");
        fs::create_dir_all(&cfg_dir).expect("config dir");
        let cfg = cfg_dir.join("settings.toml");

        let old = std::env::var_os("ENVR_ROOT");
        unsafe { std::env::set_var("ENVR_ROOT", tmp.path()) };

        // Restore even if assertions fail.
        struct RestoreEnv {
            key: &'static str,
            prev: Option<std::ffi::OsString>,
        }
        impl Drop for RestoreEnv {
            fn drop(&mut self) {
                match self.prev.take() {
                    Some(v) => unsafe { std::env::set_var(self.key, v) },
                    None => unsafe { std::env::remove_var(self.key) },
                }
            }
        }
        let _restore = RestoreEnv {
            key: "ENVR_ROOT",
            prev: old,
        };

        fs::write(
            &cfg,
            "[runtime.ruby]\npath_proxy_enabled = false\n[runtime.elixir]\npath_proxy_enabled = false\n[runtime.erlang]\npath_proxy_enabled = false\n",
        )
        .expect("write");
        assert!(core_command_uses_path_proxy_bypass(CoreCommand::Ruby));
        assert!(core_command_uses_path_proxy_bypass(CoreCommand::Gem));
        assert!(core_command_uses_path_proxy_bypass(CoreCommand::Bundle));
        assert!(core_command_uses_path_proxy_bypass(CoreCommand::Irb));
        assert!(core_command_uses_path_proxy_bypass(CoreCommand::Elixir));
        assert!(core_command_uses_path_proxy_bypass(CoreCommand::Mix));
        assert!(core_command_uses_path_proxy_bypass(CoreCommand::Iex));
        assert!(core_command_uses_path_proxy_bypass(CoreCommand::Erl));
        assert!(core_command_uses_path_proxy_bypass(CoreCommand::Erlc));
        assert!(core_command_uses_path_proxy_bypass(CoreCommand::Escript));

        fs::write(
            &cfg,
            "[runtime.ruby]\npath_proxy_enabled = true\n[runtime.elixir]\npath_proxy_enabled = true\n[runtime.erlang]\npath_proxy_enabled = true\n",
        )
        .expect("write");
        assert!(!core_command_uses_path_proxy_bypass(CoreCommand::Ruby));
        assert!(!core_command_uses_path_proxy_bypass(CoreCommand::Gem));
        assert!(!core_command_uses_path_proxy_bypass(CoreCommand::Bundle));
        assert!(!core_command_uses_path_proxy_bypass(CoreCommand::Irb));
        assert!(!core_command_uses_path_proxy_bypass(CoreCommand::Elixir));
        assert!(!core_command_uses_path_proxy_bypass(CoreCommand::Mix));
        assert!(!core_command_uses_path_proxy_bypass(CoreCommand::Iex));
        assert!(!core_command_uses_path_proxy_bypass(CoreCommand::Erl));
        assert!(!core_command_uses_path_proxy_bypass(CoreCommand::Erlc));
        assert!(!core_command_uses_path_proxy_bypass(CoreCommand::Escript));
    }

    /// When PATH lists envr `shims` before a real `julia.exe`, bypass must not pick `julia.cmd`
    /// in that shims dir: parent paths like `...\plaindata\runtimes` do not contain `"envr"`, so the
    /// legacy heuristic alone would recurse through `cmd /c` batch shims on Windows.
    #[cfg(windows)]
    #[test]
    fn path_proxy_bypass_skips_managed_shims_dir_without_envr_parent_heuristic() {
        let _guard = ENV_LOCK.lock().expect("lock");
        let tmp = tempfile::TempDir::new().expect("tmp");
        let root = tmp.path();
        let cfg_dir = root.join("config");
        fs::create_dir_all(&cfg_dir).expect("config dir");
        fs::write(
            cfg_dir.join("settings.toml"),
            "[runtime.julia]\npath_proxy_enabled = false\n",
        )
        .expect("settings");

        let old_root = std::env::var_os("ENVR_ROOT");
        unsafe { std::env::set_var("ENVR_ROOT", root) };
        struct RestoreRoot {
            key: &'static str,
            prev: Option<std::ffi::OsString>,
        }
        impl Drop for RestoreRoot {
            fn drop(&mut self) {
                match self.prev.take() {
                    Some(v) => unsafe { std::env::set_var(self.key, v) },
                    None => unsafe { std::env::remove_var(self.key) },
                }
            }
        }
        let _restore_root = RestoreRoot {
            key: "ENVR_ROOT",
            prev: old_root,
        };

        let runtime_root = root.join("plaindata").join("runtimes");
        let shims = runtime_root.join("shims");
        let system_bin = root.join("system_bin");
        fs::create_dir_all(&shims).expect("shims");
        fs::write(shims.join("julia.cmd"), b"@echo off\r\n").expect("julia.cmd");
        fs::create_dir_all(&system_bin).expect("system_bin");
        fs::write(system_bin.join("julia.exe"), []).expect("julia.exe");

        let old_path = std::env::var_os("PATH");
        let path_joined = format!("{};{}", shims.display(), system_bin.display());
        unsafe { std::env::set_var("PATH", &path_joined) };
        struct RestorePath(Option<std::ffi::OsString>);
        impl Drop for RestorePath {
            fn drop(&mut self) {
                match &self.0 {
                    Some(v) => unsafe { std::env::set_var("PATH", v) },
                    None => unsafe { std::env::remove_var("PATH") },
                }
            }
        }
        let _restore_path = RestorePath(old_path);

        let paths = envr_platform::paths::current_platform_paths().expect("paths");
        let settings = Settings::load_or_default_from(&paths.settings_file).expect("settings");
        let snap = ShimSettingsSnapshot::from_settings(&settings);

        let ctx = ShimContext {
            runtime_root,
            working_dir: root.join("prj"),
            profile: None,
        };

        let resolved = resolve_core_shim_command_with_settings(CoreCommand::Julia, &ctx, &snap)
            .expect("resolve");
        assert!(
            resolved
                .executable
                .to_string_lossy()
                .to_ascii_lowercase()
                .ends_with("julia.exe"),
            "expected system julia.exe, got {:?}",
            resolved.executable
        );
    }

    /// Same as [`path_proxy_bypass_skips_managed_shims_dir_without_envr_parent_heuristic`] but for `nim`.
    #[cfg(windows)]
    #[test]
    fn path_proxy_bypass_skips_managed_shims_dir_for_nim_without_envr_parent_heuristic() {
        let _guard = ENV_LOCK.lock().expect("lock");
        let tmp = tempfile::TempDir::new().expect("tmp");
        let root = tmp.path();
        let cfg_dir = root.join("config");
        fs::create_dir_all(&cfg_dir).expect("config dir");
        fs::write(
            cfg_dir.join("settings.toml"),
            "[runtime.nim]\npath_proxy_enabled = false\n",
        )
        .expect("settings");

        let old_root = std::env::var_os("ENVR_ROOT");
        unsafe { std::env::set_var("ENVR_ROOT", root) };
        struct RestoreRoot {
            key: &'static str,
            prev: Option<std::ffi::OsString>,
        }
        impl Drop for RestoreRoot {
            fn drop(&mut self) {
                match self.prev.take() {
                    Some(v) => unsafe { std::env::set_var(self.key, v) },
                    None => unsafe { std::env::remove_var(self.key) },
                }
            }
        }
        let _restore_root = RestoreRoot {
            key: "ENVR_ROOT",
            prev: old_root,
        };

        let runtime_root = root.join("plaindata").join("runtimes");
        let shims = runtime_root.join("shims");
        let system_bin = root.join("system_bin");
        fs::create_dir_all(&shims).expect("shims");
        fs::write(shims.join("nim.cmd"), b"@echo off\r\n").expect("nim.cmd");
        fs::create_dir_all(&system_bin).expect("system_bin");
        fs::write(system_bin.join("nim.exe"), []).expect("nim.exe");

        let old_path = std::env::var_os("PATH");
        let path_joined = format!("{};{}", shims.display(), system_bin.display());
        unsafe { std::env::set_var("PATH", &path_joined) };
        struct RestorePath(Option<std::ffi::OsString>);
        impl Drop for RestorePath {
            fn drop(&mut self) {
                match &self.0 {
                    Some(v) => unsafe { std::env::set_var("PATH", v) },
                    None => unsafe { std::env::remove_var("PATH") },
                }
            }
        }
        let _restore_path = RestorePath(old_path);

        let paths = envr_platform::paths::current_platform_paths().expect("paths");
        let settings = Settings::load_or_default_from(&paths.settings_file).expect("settings");
        let snap = ShimSettingsSnapshot::from_settings(&settings);

        let ctx = ShimContext {
            runtime_root,
            working_dir: root.join("prj"),
            profile: None,
        };

        let resolved = resolve_core_shim_command_with_settings(CoreCommand::Nim, &ctx, &snap)
            .expect("resolve");
        assert!(
            resolved
                .executable
                .to_string_lossy()
                .to_ascii_lowercase()
                .ends_with("nim.exe"),
            "expected system nim.exe, got {:?}",
            resolved.executable
        );
    }
}
