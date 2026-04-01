use envr_domain::runtime::{RuntimeVersion, VersionSpec};
use envr_error::{EnvrError, EnvrResult};
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
}

fn run_rustup(paths: &RustPaths, args: &[&str]) -> EnvrResult<(i32, String, String)> {
    let mut cmd = Command::new("rustup");
    cmd.args(args);
    cmd.env("RUSTUP_HOME", paths.rustup_home());
    cmd.env("CARGO_HOME", paths.cargo_home());
    let out = cmd.output().map_err(|e| {
        EnvrError::Runtime(format!(
            "failed to spawn rustup (is it installed and on PATH?): {e}"
        ))
    })?;
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
}

impl RustManager {
    pub fn try_new(runtime_root: PathBuf) -> EnvrResult<Self> {
        let paths = RustPaths::new(runtime_root);
        ensure_dirs(&paths)?;
        Ok(Self { paths })
    }

    pub fn list_installed_toolchains(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let (code, stdout, stderr) = match run_rustup(&self.paths, &["toolchain", "list"]) {
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
        let (code, stdout, stderr) = match run_rustup(&self.paths, &["show", "active-toolchain"]) {
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
        let (code, _stdout, stderr) = run_rustup(&self.paths, &["default", &toolchain.0])?;
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
        let (code, _stdout, stderr) =
            run_rustup(&self.paths, &["toolchain", "uninstall", &toolchain.0])?;
        if code != 0 {
            return Err(EnvrError::Runtime(format!(
                "rustup toolchain uninstall failed: {stderr}"
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
