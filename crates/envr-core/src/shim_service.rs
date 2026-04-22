//! Writes launcher stubs under `{runtime_root}/shims` for `PATH`, and syncs runtime global-bin forwards.
//!
//! Core tools use [`envr_shim_core::CoreCommand`] dispatch names (`envr-shim node`, ??. Global npm
//! packages get small stubs that `call` / symlink the real file under `npm bin -g`.

use envr_config::settings::{Settings, settings_path_from_platform};
use envr_domain::runtime::{RuntimeKind, runtime_kinds_all};
use envr_error::{EnvrError, EnvrResult};
use envr_shim_core::{CoreCommand, core_tool_executable};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(windows)]
const JS_BIN_EXTS: [&str; 3] = ["js", "cjs", "mjs"];

#[cfg(windows)]
fn normalize_windows_path_for_cmd(p: &Path) -> String {
    let s = p.display().to_string();
    // Rust/Windows canonicalize often produces `\\?\` long paths.
    // `cmd.exe` and some Win32 CreateProcess paths don't like them in batch files.
    let b = s.as_bytes();
    if b.len() >= 4 && b[0] == b'\\' && b[1] == b'\\' && b[2] == b'?' && b[3] == b'\\' {
        s[4..].to_string()
    } else {
        s
    }
}

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
        RuntimeKind::Kotlin => &[
            (CoreCommand::Kotlin, "kotlin"),
            (CoreCommand::Kotlinc, "kotlinc"),
        ],
        RuntimeKind::Scala => &[
            (CoreCommand::Scala, "scala"),
            (CoreCommand::Scalac, "scalac"),
        ],
        RuntimeKind::Clojure => &[(CoreCommand::Clojure, "clojure"), (CoreCommand::Clj, "clj")],
        RuntimeKind::Groovy => &[
            (CoreCommand::Groovy, "groovy"),
            (CoreCommand::Groovyc, "groovyc"),
        ],
        RuntimeKind::Terraform => &[(CoreCommand::Terraform, "terraform")],
        RuntimeKind::V => &[(CoreCommand::V, "v")],
        RuntimeKind::Odin => &[(CoreCommand::Odin, "odin")],
        RuntimeKind::Purescript => &[(CoreCommand::Purs, "purs")],
        RuntimeKind::Elm => &[(CoreCommand::Elm, "elm")],
        RuntimeKind::Gleam => &[(CoreCommand::Gleam, "gleam")],
        RuntimeKind::Racket => &[(CoreCommand::Racket, "racket"), (CoreCommand::Raco, "raco")],
        RuntimeKind::Dart => &[(CoreCommand::Dart, "dart")],
        RuntimeKind::Flutter => &[(CoreCommand::Flutter, "flutter")],
        RuntimeKind::Go => &[(CoreCommand::Go, "go"), (CoreCommand::Gofmt, "gofmt")],
        RuntimeKind::Rust => &[],
        RuntimeKind::Ruby => &[
            (CoreCommand::Ruby, "ruby"),
            (CoreCommand::Gem, "gem"),
            (CoreCommand::Bundle, "bundle"),
            (CoreCommand::Irb, "irb"),
        ],
        RuntimeKind::Elixir => &[
            (CoreCommand::Elixir, "elixir"),
            (CoreCommand::Mix, "mix"),
            (CoreCommand::Iex, "iex"),
        ],
        RuntimeKind::Erlang => &[
            (CoreCommand::Erl, "erl"),
            (CoreCommand::Erlc, "erlc"),
            (CoreCommand::Escript, "escript"),
        ],
        RuntimeKind::Php => &[(CoreCommand::Php, "php")],
        RuntimeKind::Deno => &[(CoreCommand::Deno, "deno")],
        RuntimeKind::Bun => &[(CoreCommand::Bun, "bun"), (CoreCommand::Bunx, "bunx")],
        RuntimeKind::Dotnet => &[(CoreCommand::Dotnet, "dotnet")],
        RuntimeKind::Zig => &[(CoreCommand::Zig, "zig")],
        RuntimeKind::Julia => &[(CoreCommand::Julia, "julia")],
        RuntimeKind::Janet => &[(CoreCommand::Janet, "janet"), (CoreCommand::Jpm, "jpm")],
        RuntimeKind::Lua => &[(CoreCommand::Lua, "lua"), (CoreCommand::Luac, "luac")],
        RuntimeKind::Nim => &[(CoreCommand::Nim, "nim")],
        RuntimeKind::Crystal => &[(CoreCommand::Crystal, "crystal")],
        RuntimeKind::Perl => &[(CoreCommand::Perl, "perl")],
        RuntimeKind::RLang => &[(CoreCommand::R, "R"), (CoreCommand::Rscript, "Rscript")],
    }
}

fn core_stems_set() -> HashSet<String> {
    let mut s = HashSet::new();
    for k in runtime_kinds_all() {
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

    /// Refreshes stubs for global package executables (excluding core tools).
    ///
    /// - For Node: scans `npm bin -g` (or package.json fallback)
    /// - For Python: scans `Scripts` / `bin`
    /// - Java is intentionally excluded: scanning JDK `bin` can create unsafe
    ///   forwards for generic names (e.g. `net`) that conflict with system commands.
    /// - For Bun: scans `bun pm bin -g`
    ///
    /// Removes stale forwards across all supported global-bin sources to avoid deleting
    /// another runtime's global shims.
    pub fn sync_global_package_shims(
        &self,
        kind: RuntimeKind,
        _version_label: &str,
    ) -> EnvrResult<()> {
        match kind {
            RuntimeKind::Node | RuntimeKind::Python | RuntimeKind::Java | RuntimeKind::Bun => {
                self.sync_all_global_package_shims()
            }
            _ => Ok(()),
        }
    }

    /// Sync global executable forwards for Node + Python + Bun, then drop stale non-core stubs.
    ///
    /// Java `bin` is excluded from global-forward scanning to avoid collisions with
    /// built-in shell commands (e.g. `net` on Windows).
    pub fn sync_all_global_package_shims(&self) -> EnvrResult<()> {
        let mut seen = HashSet::<String>::new();
        seen.extend(self.scan_node_global_bins()?);
        seen.extend(self.scan_python_global_bins()?);
        seen.extend(self.scan_bun_global_bins()?);
        self.remove_stale_non_core_shims(&seen)?;
        Ok(())
    }

    fn try_current_node_home(&self) -> Option<PathBuf> {
        let link = self.runtime_root.join("runtimes/node/current");
        if !link.exists() {
            return None;
        }
        if link.is_file() {
            let s = fs::read_to_string(&link).ok()?;
            let t = s.trim();
            if t.is_empty() {
                return None;
            }
            fs::canonicalize(t).ok()
        } else {
            fs::canonicalize(&link).ok()
        }
    }

    fn try_current_bun_home(&self) -> Option<PathBuf> {
        let link = self.runtime_root.join("runtimes/bun/current");
        if !link.exists() {
            return None;
        }
        fs::canonicalize(&link).ok()
    }

    fn try_current_python_home(&self) -> Option<PathBuf> {
        let link = self.runtime_root.join("runtimes/python/current");
        if !link.exists() {
            return None;
        }
        fs::canonicalize(&link).ok()
    }

    fn npm_global_bin_dir(&self, npm: &Path, node_home: &Path) -> EnvrResult<PathBuf> {
        // `npm bin -g` was removed/changed in some npm versions.
        // The stable way is to use `npm root -g` and then look under `.bin`.
        let mut cmd = Command::new(npm);
        cmd.args(["root", "-g"]);
        cmd.env("PATH", npm_path_env(node_home)?);
        let out = cmd.output().map_err(EnvrError::from)?;
        if !out.status.success() {
            return Err(EnvrError::Runtime(format!(
                "npm root -g failed: {}",
                String::from_utf8_lossy(&out.stderr)
            )));
        }
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if s.is_empty() {
            return Err(EnvrError::Runtime(
                "npm root -g returned empty output".into(),
            ));
        }
        Ok(PathBuf::from(s).join(".bin"))
    }

    fn bun_global_bin_dir(&self, bun: &Path, bun_home: &Path) -> EnvrResult<PathBuf> {
        let mut cmd = Command::new(bun);
        cmd.args(["pm", "bin", "-g"]);
        cmd.env("PATH", bun_path_env(bun_home)?);
        let out = cmd.output().map_err(EnvrError::from)?;
        if !out.status.success() {
            return Err(EnvrError::Runtime(format!(
                "bun pm bin -g failed: {}",
                String::from_utf8_lossy(&out.stderr)
            )));
        }
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if s.is_empty() {
            return Err(EnvrError::Runtime(
                "bun pm bin -g returned empty output".into(),
            ));
        }
        Ok(PathBuf::from(s))
    }

    fn scan_bin_dir(&self, global_bin: &Path) -> EnvrResult<HashSet<String>> {
        if !global_bin.is_dir() {
            return Ok(HashSet::new());
        }
        fs::create_dir_all(self.shim_dir())?;
        let mut seen = HashSet::<String>::new();
        for e in fs::read_dir(global_bin).map_err(EnvrError::from)? {
            let e = e.map_err(EnvrError::from)?;
            let path = e.path();
            if !path.is_file() {
                continue;
            }
            let stem = normalized_stem(&path);
            if should_skip_global_forward(&stem, &path) {
                continue;
            }
            seen.insert(stem.clone());
            self.write_global_forward(&path, &stem, None)?;
        }
        Ok(seen)
    }

    fn scan_node_global_bins(&self) -> EnvrResult<HashSet<String>> {
        let Some(node_home) = self.try_current_node_home() else {
            return Ok(HashSet::new());
        };
        let npm = match core_tool_executable(&node_home, CoreCommand::Npm) {
            Ok(p) => p,
            Err(_) => return Ok(HashSet::new()),
        };

        // Preferred: scan `<npm root -g>/.bin` for wrapper scripts.
        // Fallback: some npm versions or policies may not create `.bin` wrappers
        // (e.g. missing symlink privileges). Then we parse `package.json#bin` from
        // global packages and generate forwarders ourselves.
        let global_bin = match self.npm_global_bin_dir(&npm, &node_home) {
            Ok(p) => p,
            Err(_) => return Ok(HashSet::new()),
        };
        if global_bin.is_dir() {
            return self.scan_bin_dir(&global_bin);
        }

        // Fallback: scan global node_modules packages and their `bin` entries.
        let node_exe = match core_tool_executable(&node_home, CoreCommand::Node) {
            Ok(p) => p,
            Err(_) => return Ok(HashSet::new()),
        };
        self.scan_node_global_bins_from_package_json(&npm, &node_home, &node_exe)
    }

    fn scan_node_global_bins_from_package_json(
        &self,
        npm: &Path,
        node_home: &Path,
        node_exe: &Path,
    ) -> EnvrResult<HashSet<String>> {
        let global_root = self.npm_global_root_dir(npm, node_home)?;
        let mut seen = HashSet::<String>::new();

        // Enumerate both plain packages and scoped packages.
        let entries = match fs::read_dir(&global_root) {
            Ok(e) => e,
            Err(_) => return Ok(HashSet::new()),
        };

        for e in entries.flatten() {
            let p = e.path();
            if !p.is_dir() {
                continue;
            }
            let name = match p.file_name().and_then(|s| s.to_str()) {
                Some(s) => s,
                None => continue,
            };

            if name.starts_with('@') {
                // Scoped package directory: `@scope/pkg/...`
                let scoped_entries = match fs::read_dir(&p) {
                    Ok(se) => se,
                    Err(_) => continue,
                };
                for se in scoped_entries.flatten() {
                    if let Some(pkg_dir) = se.path().is_dir().then_some(se.path()) {
                        self.scan_single_node_pkg_bin(&pkg_dir, node_exe, &mut seen)?;
                    }
                }
            } else {
                self.scan_single_node_pkg_bin(&p, node_exe, &mut seen)?;
            }
        }

        Ok(seen)
    }

    fn scan_single_node_pkg_bin(
        &self,
        pkg_dir: &Path,
        node_exe: &Path,
        seen: &mut HashSet<String>,
    ) -> EnvrResult<()> {
        let pkg_json_path = pkg_dir.join("package.json");
        if !pkg_json_path.is_file() {
            return Ok(());
        }
        let s = match fs::read_to_string(&pkg_json_path) {
            Ok(s) => s,
            Err(_) => return Ok(()),
        };
        let v: serde_json::Value = match serde_json::from_str(&s) {
            Ok(v) => v,
            Err(_) => return Ok(()),
        };

        let pkg_name = v
            .get("name")
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string();

        let Some(bin) = v.get("bin") else {
            return Ok(());
        };

        match bin {
            serde_json::Value::String(rel) => {
                let stem = pkg_name
                    .split('/')
                    .next_back()
                    .unwrap_or(&pkg_name)
                    .to_ascii_lowercase();
                self.try_write_pkg_bin(&pkg_dir.join(rel), &stem, node_exe, seen)?;
            }
            serde_json::Value::Object(map) => {
                for (k, rel_v) in map {
                    let stem = k.to_ascii_lowercase();
                    let rel = rel_v.as_str().unwrap_or("");
                    if rel.is_empty() {
                        continue;
                    }
                    self.try_write_pkg_bin(&pkg_dir.join(rel), &stem, node_exe, seen)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn try_write_pkg_bin(
        &self,
        target: &Path,
        stem: &str,
        node_exe: &Path,
        seen: &mut HashSet<String>,
    ) -> EnvrResult<()> {
        if should_skip_global_forward(stem, target) {
            return Ok(());
        }
        if !target.is_file() {
            return Ok(());
        }
        seen.insert(stem.to_string());
        self.write_global_forward(target, stem, Some(node_exe))?;
        Ok(())
    }

    fn npm_global_root_dir(&self, npm: &Path, node_home: &Path) -> EnvrResult<PathBuf> {
        // `npm root -g` returns the global node_modules directory.
        let mut cmd = Command::new(npm);
        cmd.args(["root", "-g"]);
        cmd.env("PATH", npm_path_env(node_home)?);
        let out = cmd.output().map_err(EnvrError::from)?;
        if !out.status.success() {
            return Err(EnvrError::Runtime(format!(
                "npm root -g failed: {}",
                String::from_utf8_lossy(&out.stderr)
            )));
        }
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if s.is_empty() {
            return Err(EnvrError::Runtime(
                "npm root -g returned empty output".into(),
            ));
        }
        Ok(PathBuf::from(s))
    }

    fn scan_bun_global_bins(&self) -> EnvrResult<HashSet<String>> {
        let Some(bun_home) = self.try_current_bun_home() else {
            return Ok(HashSet::new());
        };
        let bun = match core_tool_executable(&bun_home, CoreCommand::Bun) {
            Ok(p) => p,
            Err(_) => return Ok(HashSet::new()),
        };
        let global_bin = match self
            .bun_global_bin_dir_from_settings()
            .or_else(|| self.bun_global_bin_dir(&bun, &bun_home).ok())
        {
            Some(p) => p,
            None => return Ok(HashSet::new()),
        };
        self.scan_bin_dir(&global_bin)
    }

    fn scan_python_global_bins(&self) -> EnvrResult<HashSet<String>> {
        let Some(py_home) = self.try_current_python_home() else {
            return Ok(HashSet::new());
        };
        #[cfg(windows)]
        let global_bin = py_home.join("Scripts");
        #[cfg(not(windows))]
        let global_bin = py_home.join("bin");
        self.scan_bin_dir(&global_bin)
    }

    /// Fast path for Python switching: refresh current Python script forwards only.
    ///
    /// This intentionally skips stale cleanup across other runtimes to avoid slow
    /// Node/Bun global-bin probing on every Python "切换".
    pub fn sync_python_global_package_shims_fast(&self) -> EnvrResult<()> {
        let _ = self.scan_python_global_bins()?;
        Ok(())
    }

    pub fn sync_java_global_package_shims_fast(&self) -> EnvrResult<()> {
        // Keep API shape for callers, but no longer scan Java `bin`.
        // We still trigger stale cleanup to remove historical unsafe forwards
        // such as `net.cmd`.
        self.sync_all_global_package_shims()
    }

    fn bun_global_bin_dir_from_settings(&self) -> Option<PathBuf> {
        let paths = envr_platform::paths::current_platform_paths().ok()?;
        let settings_path = settings_path_from_platform(&paths);
        let st = Settings::load_or_default_from(&settings_path).ok()?;
        st.runtime
            .bun
            .global_bin_dir
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
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

    fn write_global_forward(
        &self,
        target: &Path,
        stem: &str,
        node_exe: Option<&Path>,
    ) -> EnvrResult<()> {
        let dst = self.shim_dir().join(shim_filename(stem));
        #[cfg(windows)]
        {
            let body = match (node_exe, target.extension().and_then(|e| e.to_str())) {
                (Some(node_exe), Some(ext)) if JS_BIN_EXTS.contains(&ext) => {
                    let node_s = normalize_windows_path_for_cmd(node_exe);
                    let target_s = normalize_windows_path_for_cmd(target);
                    format!("@echo off\r\n\"{}\" \"{}\" %*\r\n", node_s, target_s)
                }
                _ => {
                    let target_s = normalize_windows_path_for_cmd(target);
                    format!("@echo off\r\ncall \"{}\" %*\r\n", target_s)
                }
            };
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
            | "python"
            | "python3"
            | "pip"
            | "pip3"
            | "java"
            | "javac"
            | "clojure"
            | "clj"
            | "groovy"
            | "groovyc"
            | "terraform"
            | "v"
            | "gleam"
            | "janet"
            | "jpm"
            | "dart"
            | "flutter"
            | "php"
            | "bun"
            | "bunx"
            | "dotnet"
            | "erl"
            | "erlc"
            | "escript"
    )
}

fn should_skip_global_forward(stem: &str, target: &Path) -> bool {
    if is_global_skip_stem(stem) {
        return true;
    }
    #[cfg(windows)]
    {
        if target
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("dll"))
            .unwrap_or(false)
        {
            return true;
        }
        if is_windows_system_command_stem(stem) {
            return true;
        }
    }
    false
}

#[cfg(windows)]
fn is_windows_system_command_stem(stem: &str) -> bool {
    let windir = std::env::var_os("WINDIR").unwrap_or_else(|| "C:\\Windows".into());
    let system32 = PathBuf::from(windir).join("System32");
    for ext in ["exe", "cmd", "bat", "com"] {
        if system32.join(format!("{stem}.{ext}")).is_file() {
            return true;
        }
    }
    false
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

fn bun_path_env(bun_home: &Path) -> EnvrResult<String> {
    let bun_bin = bun_home.join("bin");
    let rest = std::env::var("PATH").unwrap_or_default();
    #[cfg(windows)]
    {
        Ok(format!(
            "{};{};{}",
            bun_home.display(),
            bun_bin.display(),
            rest
        ))
    }
    #[cfg(not(windows))]
    {
        Ok(format!(
            "{}:{}:{}",
            bun_bin.display(),
            bun_home.display(),
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
    fn remove_shims_deletes_existing_core_launchers_only() {
        let tmp = tempfile::TempDir::new().expect("tmp");
        let root = tmp.path().to_path_buf();
        let shim = root.join("envr-shim.exe");
        fs::write(&shim, []).expect("touch");
        let svc = ShimService::new(root.clone(), shim);
        svc.ensure_shims(RuntimeKind::Node).expect("ensure");
        let keep = svc.shim_dir().join(shim_filename("custom-tool"));
        fs::write(&keep, b"x").expect("write keep");

        svc.remove_shims(RuntimeKind::Node).expect("remove");
        assert!(!svc.shim_dir().join(shim_filename("node")).exists());
        assert!(!svc.shim_dir().join(shim_filename("npm")).exists());
        assert!(keep.exists());
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

    #[test]
    fn global_skip_stem_includes_bun_and_excludes_user_bins() {
        assert!(is_global_skip_stem("bun"));
        assert!(is_global_skip_stem("bunx"));
        assert!(!is_global_skip_stem("tsx"));
        assert!(!is_global_skip_stem("mytool"));
    }

    #[cfg(windows)]
    #[test]
    fn should_skip_global_forward_blocks_dll_targets() {
        let target = PathBuf::from(r"C:\runtime\java\bin\net.dll");
        assert!(should_skip_global_forward("net", &target));
    }

    #[test]
    fn core_stems_set_contains_expected_core_commands() {
        let s = core_stems_set();
        for k in [
            "node", "npm", "npx", "python", "pip", "java", "javac", "go", "gofmt", "php", "bun",
            "bunx", "dotnet", "erl", "erlc", "escript",
        ] {
            assert!(s.contains(k), "missing {k}");
        }
        assert!(!s.contains("tsx"));
    }

    #[test]
    fn scan_bin_dir_skips_core_stems_and_keeps_user_bins() {
        let tmp = tempfile::TempDir::new().expect("tmp");
        let root = tmp.path().to_path_buf();
        let shim = root.join("envr-shim.exe");
        fs::write(&shim, []).expect("touch");
        let svc = ShimService::new(root.clone(), shim);

        let global_bin = root.join("global-bin");
        fs::create_dir_all(&global_bin).expect("mkdir");
        #[cfg(windows)]
        {
            fs::write(global_bin.join("node.cmd"), b"x").expect("node");
            fs::write(global_bin.join("mytool.cmd"), b"x").expect("mytool");
        }
        #[cfg(not(windows))]
        {
            fs::write(global_bin.join("node"), b"x").expect("node");
            fs::write(global_bin.join("mytool"), b"x").expect("mytool");
        }

        let seen = svc.scan_bin_dir(&global_bin).expect("scan");
        assert!(seen.contains("mytool"));
        assert!(!seen.contains("node"));
        assert!(svc.shim_dir().join(shim_filename("mytool")).exists());
        assert!(!svc.shim_dir().join(shim_filename("node")).exists());
    }

    #[cfg(windows)]
    #[test]
    fn shim_filename_is_cmd_on_windows() {
        assert_eq!(shim_filename("node"), "node.cmd");
    }

    #[cfg(not(windows))]
    #[test]
    fn shim_filename_has_no_suffix_on_unix() {
        assert_eq!(shim_filename("node"), "node");
    }
}
