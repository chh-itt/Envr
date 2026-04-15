//! Optional stderr hint when `package.json` `engines.node` does not match the active Node.

use envr_shim_core::ShimContext;
use std::fs;
use std::io::IsTerminal;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::shim_i18n;

const THROTTLE_SECS: u64 = 2 * 3600;

fn parse_version_triple_dir(name: &str) -> Option<semver::Version> {
    let s = name.strip_prefix('v').unwrap_or(name);
    let s = s.split('-').next().unwrap_or(s);
    let s = s.split('+').next().unwrap_or(s);
    let mut parts = s.split('.');
    let major: u64 = parts.next()?.parse().ok()?;
    let minor: u64 = parts.next().unwrap_or("0").parse().unwrap_or(0);
    let patch: u64 = parts.next().unwrap_or("0").parse().unwrap_or(0);
    Some(semver::Version::new(major, minor, patch))
}

fn find_package_json(mut dir: PathBuf) -> Option<PathBuf> {
    loop {
        let p = dir.join("package.json");
        if p.is_file() {
            return Some(p);
        }
        dir = dir.parent()?.to_path_buf();
    }
}

fn read_engines_node(pkg: &Path) -> Option<String> {
    let mut s = String::new();
    fs::File::open(pkg).ok()?.read_to_string(&mut s).ok()?;
    if !(s.contains("\"engines\"") && s.contains("\"node\"")) {
        return None;
    }
    let v: serde_json::Value = serde_json::from_str(&s).ok()?;
    let eng = v.get("engines")?;
    let n = eng.get("node")?;
    match n {
        serde_json::Value::String(x) => {
            let t = x.trim();
            if t.is_empty() {
                None
            } else {
                Some(t.to_string())
            }
        }
        _ => None,
    }
}

fn throttle_allows_emit(cache_dir: &Path, key: &str) -> bool {
    let _ = fs::create_dir_all(cache_dir);
    let path = cache_dir.join(".envr-node-engines-hint");
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if let Ok(prev) = fs::read_to_string(&path) {
        let mut lines = prev.lines();
        let ts_s = lines.next().unwrap_or("0");
        let prev_key = lines.next().unwrap_or("");
        if prev_key == key
            && let Ok(ts) = ts_s.parse::<u64>()
            && now.saturating_sub(ts) < THROTTLE_SECS
        {
            return false;
        }
    }
    let mut f = match fs::File::create(&path) {
        Ok(f) => f,
        Err(_) => return true,
    };
    let _ = writeln!(f, "{now}\n{key}");
    true
}

/// Best-effort: print one stderr line when `engines.node` rejects the active version.
pub fn maybe_emit(ctx: &ShimContext, active_label: &str) {
    if std::env::var_os("ENVR_NO_NODE_ENGINES_HINT").is_some_and(|v| !v.is_empty()) {
        return;
    }
    // Default to interactive terminals only; CI/non-interactive tools can opt in explicitly.
    if std::env::var_os("ENVR_SHIM_NODE_ENGINES_HINT").is_some_and(|v| !v.is_empty()) {
        // Explicitly enabled.
    } else if !std::io::stderr().is_terminal() {
        return;
    }
    let pkg = match find_package_json(ctx.working_dir.clone()) {
        Some(p) => p,
        None => return,
    };
    let spec = match read_engines_node(&pkg) {
        Some(s) => s,
        None => return,
    };
    let Ok(req) = semver::VersionReq::parse(&spec) else {
        return;
    };
    let Some(active_ver) = parse_version_triple_dir(active_label) else {
        return;
    };
    if req.matches(&active_ver) {
        return;
    }
    let cache_dir = ctx.runtime_root.join("cache");
    let key = format!(
        "{}|{}|{}",
        pkg.display(),
        spec,
        active_label
    );
    if !throttle_allows_emit(&cache_dir, &key) {
        return;
    }
    let msg = shim_i18n::node_engines_hint(&spec, &active_label);
    eprintln!("{msg}");
}
