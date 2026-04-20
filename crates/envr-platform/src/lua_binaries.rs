//! [LuaBinaries](https://luabinaries.sourceforge.net/) “Tools Executables” layouts differ by OS:
//! on Windows, builds often ship **`lua54.exe` / `lua55.exe`** (and **`luacNN.exe`**) instead of plain `lua.exe`.
//! This module centralizes discovery so install validation and shim resolution stay aligned.

use crate::layout_common::first_existing_path;
use std::fs;
use std::path::{Path, PathBuf};

/// `true` when `home` contains a usable Lua interpreter (same rule as successful post-extract validation).
pub fn lua_installation_valid(home: &Path) -> bool {
    resolve_lua_interpreter_exe(home).is_some()
}

/// Interpreter used to validate an extracted tree and to resolve the `lua` shim.
pub fn resolve_lua_interpreter_exe(home: &Path) -> Option<PathBuf> {
    #[cfg(windows)]
    {
        first_existing_path(&lua_interpreter_candidate_paths_windows(home))
            .or_else(|| scan_windows_digit_tag_exe(home, false))
    }
    #[cfg(not(windows))]
    {
        first_existing_path(&lua_interpreter_candidate_paths_unix(home))
    }
}

/// `luac` shim target (compiler), when present in the same tree.
pub fn resolve_luac_exe(home: &Path) -> Option<PathBuf> {
    #[cfg(windows)]
    {
        first_existing_path(&luac_candidate_paths_windows(home))
            .or_else(|| scan_windows_digit_tag_exe(home, true))
    }
    #[cfg(not(windows))]
    {
        first_existing_path(&luac_candidate_paths_unix(home))
    }
}

/// Whether `name` matches LuaBinaries-style **`luaNN.exe`** (interpreter) or **`luacNN.exe`** (compiler), `NN` ≥ 2 digits.
/// Plain `lua.exe` / `luac.exe` do not match (handled by fixed candidate lists).
#[cfg(any(windows, test))]
pub(crate) fn tools_executable_digit_tag_matches(name: &str, want_luac: bool) -> bool {
    let name = name.to_ascii_lowercase();
    if !name.ends_with(".exe") {
        return false;
    }
    if want_luac {
        if !name.starts_with("luac") {
            return false;
        }
        let Some(stem) = name.strip_suffix(".exe") else {
            return false;
        };
        let after = &stem[4..];
        after.len() >= 2 && after.chars().all(|c| c.is_ascii_digit())
    } else {
        if !name.starts_with("lua") || name.starts_with("luac") {
            return false;
        }
        let Some(stem) = name.strip_suffix(".exe") else {
            return false;
        };
        let after = &stem[3..];
        after.len() >= 2 && after.chars().all(|c| c.is_ascii_digit())
    }
}

#[cfg(windows)]
fn lua_interpreter_candidate_paths_windows(home: &Path) -> Vec<PathBuf> {
    const ROOT: &[&str] = &[
        "lua.exe",
        "lua55.exe",
        "lua54.exe",
        "lua53.exe",
        "lua52.exe",
        "lua51.exe",
        "wlua.exe",
        "wlua55.exe",
        "wlua54.exe",
    ];
    const UNDER_BIN: &[&str] = &["lua.exe", "lua55.exe", "lua54.exe"];
    let mut v = Vec::with_capacity(ROOT.len() + UNDER_BIN.len());
    for name in ROOT {
        v.push(home.join(name));
    }
    for name in UNDER_BIN {
        v.push(home.join("bin").join(name));
    }
    v
}

#[cfg(windows)]
fn luac_candidate_paths_windows(home: &Path) -> Vec<PathBuf> {
    const ROOT: &[&str] = &[
        "luac.exe",
        "luac55.exe",
        "luac54.exe",
        "luac53.exe",
        "luac52.exe",
    ];
    const UNDER_BIN: &[&str] = &["luac.exe", "luac55.exe", "luac54.exe"];
    let mut v = Vec::with_capacity(ROOT.len() + UNDER_BIN.len());
    for name in ROOT {
        v.push(home.join(name));
    }
    for name in UNDER_BIN {
        v.push(home.join("bin").join(name));
    }
    v
}

#[cfg(not(windows))]
fn lua_interpreter_candidate_paths_unix(home: &Path) -> Vec<PathBuf> {
    const ROOT: &[&str] = &[
        "lua",
        "lua5.5",
        "lua5.4",
        "lua5.3",
        "lua5.2",
        "lua5.1",
    ];
    const UNDER_BIN: &[&str] = &["lua", "lua5.5", "lua5.4", "lua5.3"];
    let mut v = Vec::with_capacity(ROOT.len() + UNDER_BIN.len());
    for name in ROOT {
        v.push(home.join(name));
    }
    for name in UNDER_BIN {
        v.push(home.join("bin").join(name));
    }
    v
}

#[cfg(not(windows))]
fn luac_candidate_paths_unix(home: &Path) -> Vec<PathBuf> {
    vec![home.join("luac"), home.join("bin").join("luac")]
}

/// Scan for `luaNN.exe` / `luacNN.exe` under `home` and `home/bin`.
#[cfg(windows)]
fn scan_windows_digit_tag_exe(home: &Path, want_luac: bool) -> Option<PathBuf> {
    for base in [home.to_path_buf(), home.join("bin")] {
        let entries = match fs::read_dir(&base) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for e in entries.flatten() {
            let p = e.path();
            if !p.is_file() {
                continue;
            }
            let name = p.file_name()?.to_str()?;
            if tools_executable_digit_tag_matches(name, want_luac) {
                return Some(p);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digit_tag_matches_lua_binaries_names() {
        assert!(tools_executable_digit_tag_matches("lua55.exe", false));
        assert!(tools_executable_digit_tag_matches("Lua54.EXE", false));
        assert!(tools_executable_digit_tag_matches("luac55.exe", true));
        assert!(!tools_executable_digit_tag_matches("lua.exe", false));
        assert!(!tools_executable_digit_tag_matches("luac.exe", true));
        assert!(!tools_executable_digit_tag_matches("luac55.exe", false));
        assert!(!tools_executable_digit_tag_matches("lua55.exe", true));
        assert!(!tools_executable_digit_tag_matches("wlua54.exe", false));
    }

    #[cfg(not(windows))]
    #[test]
    fn unix_candidate_lists_cover_home_and_bin() {
        let home = Path::new("/tmp");
        let pi = lua_interpreter_candidate_paths_unix(home);
        assert!(pi.iter().any(|p| p.ends_with("lua5.4")));
        assert!(pi.contains(&home.join("bin").join("lua")));
        let lc = luac_candidate_paths_unix(home);
        assert!(lc.contains(&home.join("luac")));
    }
}
