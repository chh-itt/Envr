//! Writes launcher stubs under `{runtime_root}/shims` for `PATH`, and syncs Node global-bin forwards.
//!
//! Core tools use [`envr_shim_core::CoreCommand`] dispatch names (`envr-shim node`, …). Global npm
//! packages get small stubs that `call` / symlink the real file under `npm bin -g`.

use envr_domain::runtime::RuntimeKind;
use envr_error::{EnvrError, EnvrResult};
use envr_shim_core::{CoreCommand, core_tool_executable};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn core_shim_entries(kind: RuntimeKind) -> &'static [(CoreCommand, &'static str)] {
    match kind {
        RuntimeKind::Node => &[
            (CoreCommand::Node, "node"),
            (CoreCommand::Npm, "npm"),
            (CoreCommand::Npx, "npx"),
        ],
        RuntimeKind::Python => &[
            (CoreCommand::Python, "python"),
            (CoreCommand::Python, "python3"),
            (CoreCommand::Pip, "pip"),
            (CoreCommand::Pip, "pip3"),
        ],
        RuntimeKind::Java => &[(CoreCommand::Java, "java"), (CoreCommand::Javac, "javac")],
    }
}

fn core_stems_set() -> HashSet<String> {
    let mut s = HashSet::new();
    for k in [RuntimeKind::Node, RuntimeKind::Python, RuntimeKind::Java] {
        for (_, d) in core_shim_entries(k) {
            s.insert((*d).to_ascii_lowercase());
        }
    }
    s
}

/// Manages `{runtime_root}/shims` (core envr-shim launchers + optional npm global forwards).
pub struct ShimService {
    runtime_root: PathBuf,
    shim_exe: PathBuf,
}

impl ShimService {
    pub fn new(runtime_root: PathBuf, shim_exe: PathBuf) -> Self {
        Self {
            runtime_root,
            shim_exe,
        }
    }

    pub fn shim_dir(&self) -> PathBuf {
        self.runtime_root.join("shims")
    }

    /// Writes core shims for one runtime (e.g. `node` / `npm` / `npx` for Node).
    pub fn ensure_shims(&self, kind: RuntimeKind) -> EnvrResult<()> {
        fs::create_dir_all(self.shim_dir())?;
        for (_, dispatch) in core_shim_entries(kind) {
            self.write_core_shim(dispatch)?;
        }
        Ok(())
    }

    /// Removes core shims for one runtime (by dispatch name).
    pub fn remove_shims(&self, kind: RuntimeKind) -> EnvrResult<()> {
        for (_, dispatch) in core_shim_entries(kind) {
            let p = self.shim_dir().join(shim_filename(dispatch));
            if p.exists() {
                fs::remove_file(&p).map_err(EnvrError::from)?;
            }
        }
        Ok(())
    }

    /// Refreshes stubs for executables in `npm bin -g` (excluding core tools). Removes stale forwards.
    pub fn sync_global_package_shims(
        &self,
        kind: RuntimeKind,
        _version_label: &str,
    ) -> EnvrResult<()> {
        if kind != RuntimeKind::Node {
            return Ok(());
        }
        let Some(node_home) = self.try_current_node_home() else {
            return Ok(());
        };
        let npm = match core_tool_executable(&node_home, CoreCommand::Npm) {
            Ok(p) => p,
            Err(_) => return Ok(()),
        };
        let global_bin = match self.npm_global_bin_dir(&npm, &node_home) {
            Ok(p) => p,
            Err(_) => return Ok(()),
        };
        if !global_bin.is_dir() {
            return Ok(());
        }

        fs::create_dir_all(self.shim_dir())?;
        let mut seen = HashSet::<String>::new();
        for e in fs::read_dir(&global_bin).map_err(EnvrError::from)? {
            let e = e.map_err(EnvrError::from)?;
            let path = e.path();
            if !path.is_file() {
                continue;
            }
            let stem = normalized_stem(&path);
            if is_global_skip_stem(&stem) {
                continue;
            }
            seen.insert(stem.clone());
            self.write_global_forward(&path, &stem)?;
        }
        self.remove_stale_non_core_shims(&seen)?;
        Ok(())
    }

    fn try_current_node_home(&self) -> Option<PathBuf> {
        let link = self.runtime_root.join("runtimes/node/current");
        if !link.exists() {
            return None;
        }
        fs::canonicalize(&link).ok()
    }

    fn npm_global_bin_dir(&self, npm: &Path, node_home: &Path) -> EnvrResult<PathBuf> {
        let mut cmd = Command::new(npm);
        cmd.args(["bin", "-g"]);
        cmd.env("PATH", npm_path_env(node_home)?);
        let out = cmd.output().map_err(EnvrError::from)?;
        if !out.status.success() {
            return Err(EnvrError::Runtime(format!(
                "npm bin -g failed: {}",
                String::from_utf8_lossy(&out.stderr)
            )));
        }
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if s.is_empty() {
            return Err(EnvrError::Runtime(
                "npm bin -g returned empty output".into(),
            ));
        }
        Ok(PathBuf::from(s))
    }

    fn write_core_shim(&self, dispatch_name: &str) -> EnvrResult<()> {
        let dst = self.shim_dir().join(shim_filename(dispatch_name));
        let shim = &self.shim_exe;
        #[cfg(windows)]
        {
            let body = format!(
                "@echo off\r\n\"{}\" {} %*\r\n",
                shim.display(),
                dispatch_name
            );
            fs::write(&dst, body).map_err(EnvrError::from)?;
        }
        #[cfg(not(windows))]
        {
            let body = format!(
                "#!/bin/sh\nexec \"{}\" {} \"$@\"\n",
                shim.display(),
                dispatch_name
            );
            fs::write(&dst, body).map_err(EnvrError::from)?;
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&dst).map_err(EnvrError::from)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&dst, perms).map_err(EnvrError::from)?;
        }
        Ok(())
    }

    fn write_global_forward(&self, target: &Path, stem: &str) -> EnvrResult<()> {
        let dst = self.shim_dir().join(shim_filename(stem));
        #[cfg(windows)]
        {
            let body = format!("@echo off\r\ncall \"{}\" %*\r\n", target.display());
            fs::write(&dst, body).map_err(EnvrError::from)?;
        }
        #[cfg(not(windows))]
        {
            if dst.exists() {
                fs::remove_file(&dst).map_err(EnvrError::from)?;
            }
            std::os::unix::fs::symlink(target, &dst).map_err(EnvrError::from)?;
        }
        Ok(())
    }

    pub(crate) fn remove_stale_non_core_shims(
        &self,
        active_globals: &HashSet<String>,
    ) -> EnvrResult<()> {
        let core = core_stems_set();
        let dir = self.shim_dir();
        if !dir.is_dir() {
            return Ok(());
        }
        for e in fs::read_dir(&dir).map_err(EnvrError::from)? {
            let e = e.map_err(EnvrError::from)?;
            let path = e.path();
            if !path.is_file() {
                continue;
            }
            let stem = normalized_stem(&path);
            if core.contains(&stem) {
                continue;
            }
            if active_globals.contains(&stem) {
                continue;
            }
            fs::remove_file(&path).map_err(EnvrError::from)?;
        }
        Ok(())
    }
}

fn shim_filename(dispatch_name: &str) -> String {
    #[cfg(windows)]
    {
        format!("{dispatch_name}.cmd")
    }
    #[cfg(not(windows))]
    {
        dispatch_name.to_string()
    }
}

fn normalized_stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

fn is_global_skip_stem(stem: &str) -> bool {
    matches!(
        stem,
        "node"
            | "npm"
            | "npx"
            | "corepack"
            | "yarn"
            | "pnpm"
            | "python"
            | "python3"
            | "pip"
            | "pip3"
            | "java"
            | "javac"
    )
}

fn npm_path_env(node_home: &Path) -> EnvrResult<String> {
    let node_bin = node_home.join("bin");
    let rest = std::env::var("PATH").unwrap_or_default();
    #[cfg(windows)]
    {
        Ok(format!(
            "{};{};{}",
            node_home.display(),
            node_bin.display(),
            rest
        ))
    }
    #[cfg(not(windows))]
    {
        Ok(format!(
            "{}:{}:{}",
            node_bin.display(),
            node_home.display(),
            rest
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalized_stem_trims_cmd() {
        let p = PathBuf::from(r"C:\npm\cowsay.cmd");
        assert_eq!(normalized_stem(&p), "cowsay");
    }

    #[test]
    fn ensure_node_shims_writes_launchers() {
        let tmp = tempfile::TempDir::new().expect("tmp");
        let root = tmp.path().to_path_buf();
        let shim = root.join("envr-shim.exe");
        fs::write(&shim, []).expect("touch");
        let svc = ShimService::new(root.clone(), shim);
        svc.ensure_shims(RuntimeKind::Node).expect("ensure");
        assert!(svc.shim_dir().join(shim_filename("node")).is_file());
        assert!(svc.shim_dir().join(shim_filename("npm")).is_file());
    }

    #[test]
    fn remove_stale_drops_orphan_global_stub() {
        let tmp = tempfile::TempDir::new().expect("tmp");
        let root = tmp.path().to_path_buf();
        let shim = root.join("envr-shim.exe");
        fs::write(&shim, []).expect("touch");
        let svc = ShimService::new(root.clone(), shim);
        fs::create_dir_all(svc.shim_dir()).expect("d");
        let orphan = svc.shim_dir().join(shim_filename("gonepkg"));
        fs::write(&orphan, b"x").expect("w");
        let mut set = HashSet::new();
        set.insert("keep".into());
        fs::write(svc.shim_dir().join(shim_filename("keep")), b"y").expect("w");
        svc.remove_stale_non_core_shims(&set).expect("clean");
        assert!(!orphan.exists());
        assert!(svc.shim_dir().join(shim_filename("keep")).exists());
    }
}
