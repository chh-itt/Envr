use envr_domain::runtime::{RuntimeVersion, VersionSpec};
use envr_error::{EnvrError, EnvrResult};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct RustPaths {
    runtime_root: PathBuf,
}

impl RustPaths {
    pub fn new(runtime_root: PathBuf) -> Self {
        Self { runtime_root }
    }

    pub fn runtime_root(&self) -> PathBuf {
        self.runtime_root.clone()
    }

    /// Managed Rust root: `{runtime_root}/runtimes/rust`
    pub fn rust_root(&self) -> PathBuf {
        self.runtime_root.join("runtimes").join("rust")
    }

    /// Isolated rustup home: `{runtime_root}/runtimes/rust/rustup`
    pub fn rustup_home(&self) -> PathBuf {
        self.runtime_root
            .join("runtimes")
            .join("rust")
            .join("rustup")
    }

    /// Isolated cargo home: `{runtime_root}/runtimes/rust/cargo`
    pub fn cargo_home(&self) -> PathBuf {
        self.runtime_root
            .join("runtimes")
            .join("rust")
            .join("cargo")
    }

    pub fn managed_rustup_exe(&self) -> PathBuf {
        #[cfg(windows)]
        {
            self.cargo_home().join("bin").join("rustup.exe")
        }
        #[cfg(not(windows))]
        {
            self.cargo_home().join("bin").join("rustup")
        }
    }

    pub fn managed_rustc_exe(&self) -> PathBuf {
        #[cfg(windows)]
        {
            self.cargo_home().join("bin").join("rustc.exe")
        }
        #[cfg(not(windows))]
        {
            self.cargo_home().join("bin").join("rustc")
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RustupMode {
    /// Use system `rustup` found on PATH; do not override `RUSTUP_HOME`/`CARGO_HOME`.
    System,
    /// Use envr-managed `rustup` under runtime root; inject `RUSTUP_HOME`/`CARGO_HOME`.
    Managed,
}

fn read_settings_rustup_env() -> HashMap<String, String> {
    let Ok(platform) = envr_platform::paths::current_platform_paths() else {
        return HashMap::new();
    };
    let path = envr_config::settings::settings_path_from_platform(&platform);
    let Ok(s) = envr_config::settings::Settings::load_or_default_from(&path) else {
        return HashMap::new();
    };
    let mut out = HashMap::new();
    if let Some(v) = envr_config::settings::rustup_dist_server_from_settings(&s) {
        out.insert("RUSTUP_DIST_SERVER".to_string(), v);
    }
    if let Some(v) = envr_config::settings::rustup_update_root_from_settings(&s) {
        out.insert("RUSTUP_UPDATE_ROOT".to_string(), v);
    }
    out
}

fn run_rustup(
    mode: RustupMode,
    paths: &RustPaths,
    args: &[&str],
) -> EnvrResult<(i32, String, String)> {
    let rustup_program = match mode {
        RustupMode::System => "rustup".into(),
        RustupMode::Managed => paths.managed_rustup_exe(),
    };
    let mut cmd = Command::new(rustup_program);
    cmd.args(args);
    if mode == RustupMode::Managed {
        cmd.env("RUSTUP_HOME", paths.rustup_home());
        cmd.env("CARGO_HOME", paths.cargo_home());
    }
    for (k, v) in read_settings_rustup_env() {
        cmd.env(k, v);
    }
    let out = cmd
        .output()
        .map_err(|e| EnvrError::Runtime(format!("failed to spawn rustup: {e}")))?;
    let code = out.status.code().unwrap_or(1);
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    Ok((code, stdout, stderr))
}

fn ensure_dirs(paths: &RustPaths) -> EnvrResult<()> {
    std::fs::create_dir_all(paths.rustup_home()).map_err(EnvrError::from)?;
    std::fs::create_dir_all(paths.cargo_home()).map_err(EnvrError::from)?;
    Ok(())
}

fn normalize_toolchain(s: &str) -> String {
    s.trim()
        .trim_end_matches("(default)")
        .trim_end_matches("(active)")
        .trim_end_matches("(override)")
        .trim()
        .to_string()
}

pub struct RustManager {
    paths: RustPaths,
    mode: RustupMode,
}

impl RustManager {
    pub fn try_new(runtime_root: PathBuf) -> EnvrResult<Self> {
        let paths = RustPaths::new(runtime_root);
        ensure_dirs(&paths)?;
        let mode = Self::detect_mode(&paths);
        Ok(Self { paths, mode })
    }

    pub fn mode(&self) -> RustupMode {
        self.mode
    }

    pub fn rustup_available(&self) -> bool {
        match self.mode {
            RustupMode::System => Self::system_rustup_available(),
            RustupMode::Managed => self.managed_rustup_installed(),
        }
    }

    fn detect_mode(paths: &RustPaths) -> RustupMode {
        // Rule B: if system rustup exists, prefer it. Only use managed when system rustup is absent.
        if Command::new("rustup").arg("--version").output().is_ok() {
            return RustupMode::System;
        }
        if paths.managed_rustup_exe().is_file() {
            return RustupMode::Managed;
        }
        // No rustup available.
        RustupMode::System
    }

    pub fn system_rustup_available() -> bool {
        Command::new("rustup").arg("--version").output().is_ok()
    }

    pub fn managed_rustup_installed(&self) -> bool {
        self.paths.managed_rustup_exe().is_file()
    }

    pub fn managed_uninstall(&self) -> EnvrResult<()> {
        let root = self.paths.rust_root();
        if root.is_dir() {
            std::fs::remove_dir_all(&root).map_err(EnvrError::from)?;
        }
        Ok(())
    }

    pub fn list_installed_toolchains(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let (code, stdout, stderr) =
            match run_rustup(self.mode, &self.paths, &["toolchain", "list"]) {
                Ok(v) => v,
                // In CI / fresh machines, `rustup` might not be installed; keep CLI `doctor/list/current` usable.
                Err(_) => return Ok(vec![]),
            };
        if code != 0 {
            return Err(EnvrError::Runtime(format!(
                "rustup toolchain list failed: {stderr}"
            )));
        }
        let mut out = Vec::new();
        for line in stdout.lines() {
            let t = normalize_toolchain(line);
            if t.is_empty() {
                continue;
            }
            // `toolchain list` can output extra notes; keep only first token
            let name = t.split_whitespace().next().unwrap_or("").trim();
            if !name.is_empty() {
                out.push(RuntimeVersion(name.to_string()));
            }
        }
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out.dedup_by(|a, b| a.0 == b.0);
        Ok(out)
    }

    pub fn active_toolchain(&self) -> EnvrResult<Option<RuntimeVersion>> {
        let (code, stdout, stderr) =
            match run_rustup(self.mode, &self.paths, &["show", "active-toolchain"]) {
                Ok(v) => v,
                Err(_) => return Ok(None),
            };
        if code != 0 {
            // When nothing is installed, rustup returns non-zero; treat as none.
            if stderr.to_ascii_lowercase().contains("no active toolchain") {
                return Ok(None);
            }
            return Ok(None);
        }
        let first = stdout.split_whitespace().next().unwrap_or("").trim();
        if first.is_empty() {
            return Ok(None);
        }
        Ok(Some(RuntimeVersion(first.to_string())))
    }

    pub fn set_default(&self, toolchain: &RuntimeVersion) -> EnvrResult<()> {
        let (code, _stdout, stderr) =
            run_rustup(self.mode, &self.paths, &["default", &toolchain.0])?;
        if code != 0 {
            return Err(EnvrError::Runtime(format!(
                "rustup default failed: {stderr}"
            )));
        }
        Ok(())
    }

    pub fn install_toolchain(&self, spec: &VersionSpec) -> EnvrResult<RuntimeVersion> {
        let raw = spec.0.trim();
        if raw.is_empty() {
            return Err(EnvrError::Validation("empty rust toolchain spec".into()));
        }
        let resolved = match raw.to_ascii_lowercase().as_str() {
            "latest" => "stable".to_string(),
            other => other.to_string(),
        };
        let (code, _stdout, stderr) = run_rustup(
            self.mode,
            &self.paths,
            &["toolchain", "install", &resolved, "--profile", "minimal"],
        )?;
        if code != 0 {
            return Err(EnvrError::Runtime(format!(
                "rustup toolchain install failed: {stderr}"
            )));
        }
        Ok(RuntimeVersion(resolved))
    }

    pub fn uninstall_toolchain(&self, toolchain: &RuntimeVersion) -> EnvrResult<()> {
        let (code, _stdout, stderr) = run_rustup(
            self.mode,
            &self.paths,
            &["toolchain", "uninstall", &toolchain.0],
        )?;
        if code != 0 {
            return Err(EnvrError::Runtime(format!(
                "rustup toolchain uninstall failed: {stderr}"
            )));
        }
        Ok(())
    }

    pub fn update_all(&self) -> EnvrResult<()> {
        let (code, _stdout, stderr) = run_rustup(self.mode, &self.paths, &["update"])?;
        if code != 0 {
            return Err(EnvrError::Runtime(format!(
                "rustup update failed: {stderr}"
            )));
        }
        Ok(())
    }

    pub fn update_toolchain(&self, toolchain: &RuntimeVersion) -> EnvrResult<()> {
        let (code, _stdout, stderr) =
            run_rustup(self.mode, &self.paths, &["update", &toolchain.0])?;
        if code != 0 {
            return Err(EnvrError::Runtime(format!(
                "rustup update failed: {stderr}"
            )));
        }
        Ok(())
    }

    pub fn list_components(
        &self,
        toolchain: Option<&RuntimeVersion>,
    ) -> EnvrResult<Vec<(String, bool)>> {
        let mut args: Vec<String> = vec!["component".into(), "list".into()];
        if let Some(tc) = toolchain {
            args.push("--toolchain".into());
            args.push(tc.0.clone());
        }
        let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let (code, stdout, stderr) = run_rustup(self.mode, &self.paths, &refs)?;
        if code != 0 {
            return Err(EnvrError::Runtime(format!(
                "rustup component list failed: {stderr}"
            )));
        }
        let mut out = Vec::new();
        for line in stdout.lines() {
            let t = line.trim();
            if t.is_empty() {
                continue;
            }
            let installed = t.ends_with("(installed)");
            let name = t
                .trim_end_matches("(installed)")
                .trim()
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim();
            if !name.is_empty() {
                out.push((name.to_string(), installed));
            }
        }
        Ok(out)
    }

    pub fn component_add(&self, name: &str, toolchain: Option<&RuntimeVersion>) -> EnvrResult<()> {
        let mut args: Vec<String> = vec!["component".into(), "add".into(), name.into()];
        if let Some(tc) = toolchain {
            args.push("--toolchain".into());
            args.push(tc.0.clone());
        }
        let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let (code, _stdout, stderr) = run_rustup(self.mode, &self.paths, &refs)?;
        if code != 0 {
            return Err(EnvrError::Runtime(format!(
                "rustup component add failed: {stderr}"
            )));
        }
        Ok(())
    }

    pub fn component_remove(
        &self,
        name: &str,
        toolchain: Option<&RuntimeVersion>,
    ) -> EnvrResult<()> {
        let mut args: Vec<String> = vec!["component".into(), "remove".into(), name.into()];
        if let Some(tc) = toolchain {
            args.push("--toolchain".into());
            args.push(tc.0.clone());
        }
        let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let (code, _stdout, stderr) = run_rustup(self.mode, &self.paths, &refs)?;
        if code != 0 {
            return Err(EnvrError::Runtime(format!(
                "rustup component remove failed: {stderr}"
            )));
        }
        Ok(())
    }

    pub fn list_targets(
        &self,
        toolchain: Option<&RuntimeVersion>,
    ) -> EnvrResult<Vec<(String, bool)>> {
        let mut args: Vec<String> = vec!["target".into(), "list".into()];
        if let Some(tc) = toolchain {
            args.push("--toolchain".into());
            args.push(tc.0.clone());
        }
        let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let (code, stdout, stderr) = run_rustup(self.mode, &self.paths, &refs)?;
        if code != 0 {
            return Err(EnvrError::Runtime(format!(
                "rustup target list failed: {stderr}"
            )));
        }
        let mut out = Vec::new();
        for line in stdout.lines() {
            let t = line.trim();
            if t.is_empty() {
                continue;
            }
            let installed = t.ends_with("(installed)");
            let name = t
                .trim_end_matches("(installed)")
                .trim()
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim();
            if !name.is_empty() {
                out.push((name.to_string(), installed));
            }
        }
        Ok(out)
    }

    pub fn target_add(&self, name: &str, toolchain: Option<&RuntimeVersion>) -> EnvrResult<()> {
        let mut args: Vec<String> = vec!["target".into(), "add".into(), name.into()];
        if let Some(tc) = toolchain {
            args.push("--toolchain".into());
            args.push(tc.0.clone());
        }
        let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let (code, _stdout, stderr) = run_rustup(self.mode, &self.paths, &refs)?;
        if code != 0 {
            return Err(EnvrError::Runtime(format!(
                "rustup target add failed: {stderr}"
            )));
        }
        Ok(())
    }

    pub fn target_remove(&self, name: &str, toolchain: Option<&RuntimeVersion>) -> EnvrResult<()> {
        let mut args: Vec<String> = vec!["target".into(), "remove".into(), name.into()];
        if let Some(tc) = toolchain {
            args.push("--toolchain".into());
            args.push(tc.0.clone());
        }
        let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let (code, _stdout, stderr) = run_rustup(self.mode, &self.paths, &refs)?;
        if code != 0 {
            return Err(EnvrError::Runtime(format!(
                "rustup target remove failed: {stderr}"
            )));
        }
        Ok(())
    }

    pub fn cargo_bin_dir(&self) -> PathBuf {
        self.paths.cargo_home().join("bin")
    }

    pub fn toolchain_bin_dir(&self, toolchain: &RuntimeVersion) -> PathBuf {
        self.paths
            .rustup_home()
            .join("toolchains")
            .join(&toolchain.0)
            .join("bin")
    }
}
