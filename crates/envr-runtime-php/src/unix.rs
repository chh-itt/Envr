//! Discover existing PHP installs on Unix and register them as symlinks under `runtimes/php/versions/`.

use crate::manager::{PhpPaths, php_installation_valid, remove_stale_split_current_files};
use envr_domain::runtime::RuntimeVersion;
use envr_error::{EnvrError, EnvrResult};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Refresh symlinks in `versions/` for prefixes found via Homebrew / `PATH`.
pub fn sync_registered_versions(paths: &PhpPaths) -> EnvrResult<()> {
    let vd = paths.versions_dir();
    fs::create_dir_all(&vd).map_err(EnvrError::from)?;

    let mut seen_targets: HashSet<PathBuf> = HashSet::new();
    if vd.is_dir() {
        for e in fs::read_dir(&vd).map_err(EnvrError::from)? {
            let e = e.map_err(EnvrError::from)?;
            let p = e.path();
            let Ok(md) = fs::symlink_metadata(&p) else {
                continue;
            };
            if md.is_symlink() {
                if let Ok(t) = fs::read_link(&p) {
                    let abs = if t.is_relative() { vd.join(t) } else { t };
                    if let Ok(c) = fs::canonicalize(&abs) {
                        seen_targets.insert(c);
                    }
                }
            }
        }
    }

    for prefix in discover_prefixes() {
        let Ok(canon) = fs::canonicalize(&prefix) else {
            continue;
        };
        if seen_targets.contains(&canon) {
            continue;
        }
        if !php_installation_valid(&prefix) {
            continue;
        }
        let Some(ver) = php_version_for_prefix(&prefix) else {
            continue;
        };
        let name = allocate_version_dir_name(&vd, &ver);
        let link = vd.join(&name);
        std::os::unix::fs::symlink(&prefix, &link).map_err(EnvrError::from)?;
        seen_targets.insert(canon);
    }
    Ok(())
}

fn discover_prefixes() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut seen = HashSet::<PathBuf>::new();

    for formula in [
        "php", "php@8.5", "php@8.4", "php@8.3", "php@8.2", "php@8.1", "php@8.0", "php@7.4",
    ] {
        if let Some(p) = brew_prefix(formula) {
            push_unique(&mut out, &mut seen, p);
        }
    }

    if let Some(bin) = which_php() {
        if let Some(pref) = prefix_from_php_bin(&bin) {
            push_unique(&mut out, &mut seen, pref);
        }
    }

    out
}

fn push_unique(out: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>, p: PathBuf) {
    let c = fs::canonicalize(&p).unwrap_or(p);
    if seen.insert(c.clone()) {
        out.push(c);
    }
}

fn brew_prefix(formula: &str) -> Option<PathBuf> {
    let o = Command::new("brew")
        .args(["--prefix", formula])
        .output()
        .ok()?;
    if !o.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&o.stdout);
    let p = PathBuf::from(s.trim());
    if p.is_dir() { Some(p) } else { None }
}

fn which_php() -> Option<PathBuf> {
    let o = Command::new("which").arg("php").output().ok()?;
    if !o.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&o.stdout);
    let p = PathBuf::from(s.trim());
    if p.is_file() { Some(p) } else { None }
}

fn prefix_from_php_bin(bin: &Path) -> Option<PathBuf> {
    let canon = fs::canonicalize(bin).ok()?;
    let bin_dir = canon.parent()?;
    if bin_dir.file_name()?.to_str()? == "bin" {
        bin_dir.parent().map(|p| p.to_path_buf())
    } else {
        None
    }
}

fn php_version_for_prefix(prefix: &Path) -> Option<String> {
    let bin = if prefix.join("bin/php").is_file() {
        prefix.join("bin/php")
    } else if prefix.join("php").is_file() {
        prefix.join("php")
    } else {
        return None;
    };
    let o = Command::new(&bin)
        .arg("-r")
        .arg("echo PHP_VERSION;")
        .output()
        .ok()?;
    if !o.status.success() {
        return None;
    }
    let v = String::from_utf8_lossy(&o.stdout).trim().to_string();
    if v.is_empty() { None } else { Some(v) }
}

fn allocate_version_dir_name(versions_dir: &Path, semver: &str) -> String {
    let base: String = semver
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let base = if base.is_empty() {
        "php".to_string()
    } else {
        base
    };
    if !versions_dir.join(&base).exists() {
        return base;
    }
    let mut i = 2u32;
    loop {
        let n = format!("{base}-{i}");
        if !versions_dir.join(&n).exists() {
            return n;
        }
        i += 1;
    }
}

/// Set global `current` to a registered version (Unix).
pub fn set_global_current(paths: &PhpPaths, version: &RuntimeVersion) -> EnvrResult<()> {
    let dir = paths.versions_dir().join(&version.0);
    if !php_installation_valid(&dir) {
        return Err(EnvrError::Validation(format!(
            "php {} is not installed",
            version.0
        )));
    }
    envr_platform::links::ensure_link(
        envr_platform::links::LinkType::Soft,
        &dir,
        paths.current_link(),
    )?;
    remove_stale_split_current_files(paths);
    Ok(())
}

/// Remove a registration symlink (does not uninstall Homebrew/distro packages).
pub fn uninstall_registration(paths: &PhpPaths, version: &RuntimeVersion) -> EnvrResult<()> {
    use crate::manager::resolve_global_php_current_target;

    let dir = paths.versions_dir().join(&version.0);
    if !dir.exists() {
        return Err(EnvrError::Validation(format!(
            "php {} is not installed",
            version.0
        )));
    }
    let dir_canon = fs::canonicalize(&dir).unwrap_or_else(|_| dir.clone());
    let was_global = resolve_global_php_current_target(paths)?
        .is_some_and(|g| fs::canonicalize(&g).unwrap_or(g) == dir_canon);

    let md = fs::symlink_metadata(&dir).map_err(EnvrError::from)?;
    if md.is_symlink() {
        fs::remove_file(&dir).map_err(EnvrError::from)?;
    } else if md.is_dir() {
        fs::remove_dir_all(&dir).map_err(EnvrError::from)?;
    } else {
        return Err(EnvrError::Validation(
            "invalid php version path under versions/".into(),
        ));
    }

    if was_global {
        let _ = fs::remove_file(paths.current_link());
        remove_stale_split_current_files(paths);
    }
    Ok(())
}
