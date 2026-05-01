use envr_error::{EnvrError, EnvrResult, ErrorCode};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    env, fs,
    path::{Path, PathBuf},
};

pub const PROJECT_CONFIG_FILE: &str = ".envr.toml";
pub const PROJECT_CONFIG_LOCAL_FILE: &str = ".envr.local.toml";
pub const PROJECT_LOCK_FILE: &str = ".envr.lock.toml";
pub const PROJECT_LOCK_FILE_ALT: &str = ".envr.lock";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectConfigLocation {
    pub dir: PathBuf,
    pub base_file: Option<PathBuf>,
    pub local_file: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectConfig {
    /// Remote layers merged before this file (URLs or `github.com/owner/repo/ref` shorthand).
    #[serde(default)]
    pub extends: Vec<String>,

    #[serde(default)]
    pub env: HashMap<String, String>,

    #[serde(default)]
    pub runtimes: HashMap<String, RuntimeConfig>,

    /// Compatibility helpers for migrating from `.tool-versions` and similar file formats.
    #[serde(default)]
    pub compat: ProjectCompatConfig,

    /// Named command aliases for `envr run <name>` (shell one-liners).
    #[serde(default)]
    pub scripts: HashMap<String, String>,

    /// Named overlays (e.g. CI vs local). Activated via `ENVR_PROFILE` or `envr exec --profile`.
    #[serde(default)]
    pub profiles: HashMap<String, ProjectProfile>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectCompatConfig {
    #[serde(default)]
    pub asdf: AsdfCompatConfig,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AsdfCompatConfig {
    #[serde(default)]
    pub names: HashMap<String, String>,
}

/// Pins + env for a single named profile (`[profiles.name]` in TOML).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectProfile {
    #[serde(default)]
    pub runtimes: HashMap<String, RuntimeConfig>,

    #[serde(default)]
    pub env: HashMap<String, String>,

    #[serde(default)]
    pub scripts: HashMap<String, String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeConfig {
    pub version: Option<String>,
    /// Rust-only: required release channel (`stable`/`beta`/`nightly`) for this project.
    #[serde(default)]
    pub channel: Option<String>,
    /// Rust-only: required `rustc` version prefix (e.g. `1.78`).
    #[serde(default)]
    pub version_prefix: Option<String>,
    /// Rust-only: enforcement mode. Defaults to `warn` when any rust constraint is set.
    #[serde(default)]
    pub enforce: Option<RustEnforceMode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RustEnforceMode {
    #[default]
    Warn,
    Error,
}

impl ProjectConfig {
    pub fn merge_over(mut self, base: ProjectConfig) -> ProjectConfig {
        // local/self overrides base
        let mut merged = base;
        merged.extends.append(&mut self.extends);
        merged.env.extend(self.env.drain());
        merged.runtimes.extend(self.runtimes.drain());
        merged.compat.asdf.names.extend(self.compat.asdf.names.drain());
        merged.scripts.extend(self.scripts.drain());
        merged.profiles.extend(self.profiles.drain());
        merged
    }

    pub fn expand_vars(mut self) -> EnvrResult<ProjectConfig> {
        let env_snapshot = env::vars().collect::<HashMap<String, String>>();
        let expanded_env = expand_env_map(&self.env, &env_snapshot)?;
        self.env = expanded_env;

        for runtime in self.runtimes.values_mut() {
            if let Some(version) = runtime.version.take() {
                runtime.version = Some(expand_string(&version, &self.env, &env_snapshot)?);
            }
        }

        for script in self.scripts.values_mut() {
            *script = expand_string(script, &self.env, &env_snapshot)?;
        }

        Ok(self)
    }
}

pub fn load_project_config(
    start_dir: impl AsRef<Path>,
) -> EnvrResult<Option<(ProjectConfig, ProjectConfigLocation)>> {
    load_project_config_profile(start_dir, None)
}

/// Merge `.envr.toml` + `.envr.local.toml` only (no `ENVR_PROFILE` / `[profiles]` activation).
pub fn load_project_config_disk_only(
    start_dir: impl AsRef<Path>,
) -> EnvrResult<Option<(ProjectConfig, ProjectConfigLocation)>> {
    let start_dir = start_dir.as_ref();
    let norm = normalize_project_config_start_dir(start_dir)?;
    let key = (norm, ProjectConfigCacheProfile::DiskOnly);
    if let Some(hit) = project_config_cache_get(&key) {
        return Ok(hit);
    }
    let res = load_project_config_inner_uncached(start_dir, None);
    project_config_cache_store_ok(&key, &res);
    res
}

/// Load project config; `profile` overrides [`ENVR_PROFILE`] when set.
pub fn load_project_config_profile(
    start_dir: impl AsRef<Path>,
    profile: Option<&str>,
) -> EnvrResult<Option<(ProjectConfig, ProjectConfigLocation)>> {
    let start_dir = start_dir.as_ref();
    let norm = normalize_project_config_start_dir(start_dir)?;
    let effective_profile = profile
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| {
            env::var("ENVR_PROFILE")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        });
    let key = (
        norm,
        ProjectConfigCacheProfile::Effective(effective_profile.clone()),
    );
    if let Some(hit) = project_config_cache_get(&key) {
        return Ok(hit);
    }
    let res = load_project_config_inner_uncached(start_dir, effective_profile);
    project_config_cache_store_ok(&key, &res);
    res
}

/// Clears the in-process project config cache for the **current OS thread** (CLI / shims are single-threaded).
/// Benchmarks and tests that mutate `.envr.toml` between loads should call this so results stay fresh.
pub fn reset_project_config_load_cache() {
    PROJECT_CONFIG_CACHE.with(|c| c.borrow_mut().clear());
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum ProjectConfigCacheProfile {
    /// `load_project_config_disk_only`: never apply `ENVR_PROFILE` / `[profiles.*]` overlay.
    DiskOnly,
    /// `load_project_config_profile` after merging CLI `profile` and `ENVR_PROFILE`.
    Effective(Option<String>),
}

type ProjectConfigCacheKey = (PathBuf, ProjectConfigCacheProfile);

thread_local! {
    static PROJECT_CONFIG_CACHE: RefCell<HashMap<ProjectConfigCacheKey, Option<(ProjectConfig, ProjectConfigLocation)>>> =
        RefCell::new(HashMap::new());
}

fn normalize_project_config_start_dir(start_dir: &Path) -> EnvrResult<PathBuf> {
    if start_dir.is_dir() {
        Ok(start_dir.to_path_buf())
    } else {
        match start_dir.parent() {
            Some(p) => Ok(p.to_path_buf()),
            None => Err(EnvrError::Config("start_dir has no parent".to_string())),
        }
    }
}

fn project_config_cache_get(
    key: &ProjectConfigCacheKey,
) -> Option<Option<(ProjectConfig, ProjectConfigLocation)>> {
    PROJECT_CONFIG_CACHE.with(|c| c.borrow().get(key).cloned())
}

fn project_config_cache_store_ok(
    key: &ProjectConfigCacheKey,
    res: &EnvrResult<Option<(ProjectConfig, ProjectConfigLocation)>>,
) {
    if let Ok(v) = res {
        PROJECT_CONFIG_CACHE.with(|c| {
            c.borrow_mut().insert(key.clone(), v.clone());
        });
    }
}

fn load_project_config_inner_uncached(
    start_dir: impl AsRef<Path>,
    effective_profile: Option<String>,
) -> EnvrResult<Option<(ProjectConfig, ProjectConfigLocation)>> {
    let start_dir = start_dir.as_ref();
    let mut current = normalize_project_config_start_dir(start_dir)?;

    loop {
        let base_path = current.join(PROJECT_CONFIG_FILE);
        let local_path = current.join(PROJECT_CONFIG_LOCAL_FILE);

        let base_exists = base_path.is_file();
        let local_exists = local_path.is_file();

        if base_exists || local_exists {
            let base_cfg = if base_exists {
                Some(crate::project_extends::resolve_extends(
                    parse_project_config(&base_path)?,
                )?)
            } else {
                None
            };
            let local_cfg = if local_exists {
                Some(crate::project_extends::resolve_extends(
                    parse_project_config(&local_path)?,
                )?)
            } else {
                None
            };

            let mut merged = match (base_cfg, local_cfg) {
                (Some(base), Some(local)) => local.merge_over(base),
                (Some(base), None) => base,
                (None, Some(local)) => local,
                (None, None) => ProjectConfig::default(),
            };

            if let Some(ref pname) = effective_profile
                && let Some(p) = merged.profiles.get(pname)
            {
                for (k, v) in &p.runtimes {
                    merged.runtimes.insert(k.clone(), v.clone());
                }
                for (k, v) in &p.env {
                    merged.env.insert(k.clone(), v.clone());
                }
                for (k, v) in &p.scripts {
                    merged.scripts.insert(k.clone(), v.clone());
                }
            }

            for (asdf_name, envr_name) in merged.compat.asdf.names.clone() {
                if let Some(runtime) = merged.runtimes.remove(&asdf_name)
                    && !merged.runtimes.contains_key(&envr_name)
                {
                    merged.runtimes.insert(envr_name, runtime);
                }
            }

            let merged = merged.expand_vars()?;

            let loc = ProjectConfigLocation {
                dir: current.clone(),
                base_file: if base_exists { Some(base_path) } else { None },
                local_file: if local_exists { Some(local_path) } else { None },
            };

            return Ok(Some((merged, loc)));
        }

        let parent = match current.parent() {
            Some(p) => p.to_path_buf(),
            None => return Ok(None),
        };

        if parent == current {
            return Ok(None);
        }
        current = parent;
    }
}

pub fn parse_project_config(path: impl AsRef<Path>) -> EnvrResult<ProjectConfig> {
    let path = path.as_ref();
    let content = fs::read_to_string(path).map_err(EnvrError::from)?;
    parse_project_config_str(&content)
        .map_err(|err| EnvrError::Config(format!("failed to parse {}: {err}", path.display())))
}

pub fn parse_project_config_str(content: &str) -> EnvrResult<ProjectConfig> {
    toml::from_str(content)
        .map_err(|err| EnvrError::Config(format!("failed to parse project config: {err}")))
}

pub fn save_project_config(path: impl AsRef<Path>, cfg: &ProjectConfig) -> EnvrResult<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(EnvrError::from)?;
    }
    let content = toml::to_string_pretty(cfg)
        .map_err(|e| EnvrError::with_source(ErrorCode::Runtime, "toml encode project config", e))?;
    fs::write(path, content).map_err(EnvrError::from)?;
    Ok(())
}

pub fn load_project_lock(path: impl AsRef<Path>) -> EnvrResult<Option<ProjectConfig>> {
    let path = path.as_ref();
    if !path.is_file() {
        return Ok(None);
    }
    parse_project_config(path).map(Some)
}

pub fn load_project_lock_any(dir: impl AsRef<Path>) -> EnvrResult<Option<(ProjectConfig, PathBuf)>> {
    let dir = dir.as_ref();
    let candidates = [dir.join(PROJECT_LOCK_FILE), dir.join(PROJECT_LOCK_FILE_ALT)];
    for candidate in candidates {
        if let Some(cfg) = load_project_lock(&candidate)? {
            return Ok(Some((cfg, candidate)));
        }
    }
    Ok(None)
}

pub fn save_project_lock(path: impl AsRef<Path>, cfg: &ProjectConfig) -> EnvrResult<()> {
    save_project_config(path, cfg)
}

fn expand_env_map(
    input: &HashMap<String, String>,
    env_snapshot: &HashMap<String, String>,
) -> EnvrResult<HashMap<String, String>> {
    let mut resolved = HashMap::<String, String>::new();

    for key in input.keys() {
        let value = resolve_env_key(key, input, env_snapshot, &mut resolved, &mut HashSet::new())?;
        resolved.insert(key.clone(), value);
    }

    Ok(resolved)
}

fn resolve_env_key(
    key: &str,
    input: &HashMap<String, String>,
    env_snapshot: &HashMap<String, String>,
    resolved: &mut HashMap<String, String>,
    visiting: &mut HashSet<String>,
) -> EnvrResult<String> {
    if let Some(v) = resolved.get(key) {
        return Ok(v.clone());
    }

    if !visiting.insert(key.to_string()) {
        return Err(EnvrError::Validation(format!(
            "env var expansion cycle detected at {key}"
        )));
    }

    let raw = input.get(key).cloned().unwrap_or_default();
    let expanded = expand_string_with_resolver(&raw, |var| {
        if input.contains_key(var) {
            Some(resolve_env_key(
                var,
                input,
                env_snapshot,
                resolved,
                visiting,
            ))
        } else {
            env_snapshot.get(var).cloned().map(Ok)
        }
    })?;

    visiting.remove(key);
    Ok(expanded)
}

fn expand_string(
    input: &str,
    config_env: &HashMap<String, String>,
    env_snapshot: &HashMap<String, String>,
) -> EnvrResult<String> {
    expand_string_with_resolver(input, |var| {
        if let Some(v) = config_env.get(var) {
            Some(Ok(v.clone()))
        } else {
            env_snapshot.get(var).cloned().map(Ok)
        }
    })
}

fn expand_string_with_resolver<F>(input: &str, mut resolver: F) -> EnvrResult<String>
where
    F: FnMut(&str) -> Option<EnvrResult<String>>,
{
    // ${VAR}
    // compiled each call is fine for current scale; can be cached later.
    let re = Regex::new(r"\$\{([A-Za-z_][A-Za-z0-9_]*)\}")
        .map_err(|err| EnvrError::Runtime(format!("regex init failed: {err}")))?;

    let mut out = String::with_capacity(input.len());
    let mut last = 0;
    for caps in re.captures_iter(input) {
        let m = caps.get(0).expect("full match");
        let var = caps.get(1).expect("var").as_str();
        out.push_str(&input[last..m.start()]);

        match resolver(var) {
            Some(Ok(v)) => out.push_str(&v),
            Some(Err(e)) => return Err(e),
            None => {
                return Err(EnvrError::Validation(format!(
                    "unresolved variable {var} in expansion"
                )));
            }
        }
        last = m.end();
    }
    out.push_str(&input[last..]);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::fs;
    use tempfile::TempDir;

    fn write(path: &Path, content: &str) {
        fs::write(path, content).expect("write");
    }

    #[test]
    fn finds_nearest_config_upwards_and_local_overrides() {
        let tmp = TempDir::new().expect("tmp");
        let root = tmp.path();
        let a = root.join("a");
        let b = a.join("b");
        fs::create_dir_all(&b).expect("mkdirs");

        write(
            &root.join(PROJECT_CONFIG_FILE),
            r#"
[env]
FOO = "root"

[runtimes.node]
version = "18"
"#,
        );

        write(
            &a.join(PROJECT_CONFIG_FILE),
            r#"
[env]
FOO = "a"

[runtimes.node]
version = "20"
"#,
        );

        write(
            &a.join(PROJECT_CONFIG_LOCAL_FILE),
            r#"
[env]
FOO = "a-local"
"#,
        );

        let (cfg, loc) = load_project_config(&b)
            .expect("load")
            .expect("found config");

        assert_eq!(loc.dir, a);
        assert_eq!(cfg.env.get("FOO").map(String::as_str), Some("a-local"));
        assert_eq!(
            cfg.runtimes.get("node").and_then(|r| r.version.as_deref()),
            Some("20")
        );
    }

    #[test]
    fn merge_scripts_prefers_local_over_base() {
        let mut base = ProjectConfig::default();
        base.scripts.insert("dev".into(), "vite".into());
        let mut local = ProjectConfig::default();
        local
            .scripts
            .insert("dev".into(), "vite --port 3001".into());
        let merged = local.merge_over(base);
        assert_eq!(
            merged.scripts.get("dev").map(String::as_str),
            Some("vite --port 3001")
        );
    }

    #[test]
    fn expands_env_vars_and_detects_cycles() {
        reset_project_config_load_cache();
        let tmp = TempDir::new().expect("tmp");
        let dir = tmp.path();

        write(
            &dir.join(PROJECT_CONFIG_FILE),
            r#"
[env]
A = "hello"
B = "${A}-world"
"#,
        );

        let (cfg, _) = load_project_config(dir)
            .expect("load")
            .expect("found config");
        assert_eq!(cfg.env.get("B").map(String::as_str), Some("hello-world"));

        write(
            &dir.join(PROJECT_CONFIG_FILE),
            r#"
[env]
A = "${B}"
B = "${A}"
"#,
        );
        reset_project_config_load_cache();
        let err = load_project_config(dir).expect_err("cycle should error");
        assert!(matches!(err, EnvrError::Validation(_)));
    }

    #[test]
    fn project_config_process_cache_reused_until_reset() {
        reset_project_config_load_cache();
        let tmp = TempDir::new().expect("tmp");
        let root = tmp.path();
        let leaf = root.join("a").join("b");
        fs::create_dir_all(&leaf).expect("mkdir");
        fs::write(
            root.join(PROJECT_CONFIG_FILE),
            "[runtimes.node]\nversion = \"18\"\n",
        )
        .expect("write");
        let first = load_project_config_profile(&leaf, None)
            .expect("load")
            .expect("found");
        fs::write(
            root.join(PROJECT_CONFIG_FILE),
            "[runtimes.node]\nversion = \"22\"\n",
        )
        .expect("rewrite");
        let cached = load_project_config_profile(&leaf, None)
            .expect("load")
            .expect("found");
        assert_eq!(
            first
                .0
                .runtimes
                .get("node")
                .and_then(|r| r.version.as_deref()),
            cached
                .0
                .runtimes
                .get("node")
                .and_then(|r| r.version.as_deref()),
            "second load should hit in-process cache"
        );
        reset_project_config_load_cache();
        let fresh = load_project_config_profile(&leaf, None)
            .expect("load")
            .expect("found");
        assert_eq!(
            fresh
                .0
                .runtimes
                .get("node")
                .and_then(|r| r.version.as_deref()),
            Some("22")
        );
    }

    proptest! {
        #[test]
        fn merge_over_prefers_local_values(
            base_env in proptest::collection::hash_map("[A-Z_]{1,8}", ".*", 0..8),
            base_rt in proptest::collection::hash_map("[a-z]{2,8}", "([0-9]{1,2}(\\.[0-9]{1,2}){0,2})?", 0..8),
            local_env in proptest::collection::hash_map("[A-Z_]{1,8}", ".*", 0..8),
            local_rt in proptest::collection::hash_map("[a-z]{2,8}", "([0-9]{1,2}(\\.[0-9]{1,2}){0,2})?", 0..8),
        ) {
            let base = ProjectConfig {
                env: base_env.clone(),
                runtimes: base_rt
                    .iter()
                    .map(|(k, v)| (k.clone(), RuntimeConfig { version: Some(v.clone()), ..Default::default() }))
                    .collect(),
                ..Default::default()
            };

            let local = ProjectConfig {
                env: local_env.clone(),
                runtimes: local_rt
                    .iter()
                    .map(|(k, v)| (k.clone(), RuntimeConfig { version: Some(v.clone()), ..Default::default() }))
                    .collect(),
                ..Default::default()
            };

            let merged = local.merge_over(base);

            for (k, v) in &local_env {
                prop_assert_eq!(merged.env.get(k), Some(v));
            }
            for (k, v) in &base_env {
                if !local_env.contains_key(k) {
                    prop_assert_eq!(merged.env.get(k), Some(v));
                }
            }

            for (k, v) in &local_rt {
                let got = merged.runtimes.get(k).and_then(|r| r.version.as_ref());
                prop_assert_eq!(got, Some(v));
            }
            for (k, v) in &base_rt {
                if !local_rt.contains_key(k) {
                    let got = merged.runtimes.get(k).and_then(|r| r.version.as_ref());
                    prop_assert_eq!(got, Some(v));
                }
            }
        }
    }
}
