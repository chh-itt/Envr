//! Single-binary and simple multi-binary layouts under a version **home** directory.
//! Keeps install validation (`*_installation_valid`) aligned with shim resolution (`resolve_*_exe`).

use crate::layout_common::first_existing_path;
use std::path::{Path, PathBuf};

// --- Generic `bin/<tool>` (Windows: `bin/<tool>.exe`) ------------------------------------------

/// `bin/<stem>` on Unix, `bin/<stem>.exe` on Windows when that file exists.
pub fn resolve_bin_tool_exe(home: &Path, stem: &str) -> Option<PathBuf> {
    #[cfg(windows)]
    let p = home.join("bin").join(format!("{stem}.exe"));
    #[cfg(not(windows))]
    let p = home.join("bin").join(stem);
    p.is_file().then_some(p)
}

// --- Nim --------------------------------------------------------------------------------------

pub fn nim_installation_valid(home: &Path) -> bool {
    resolve_nim_exe(home).is_some()
}

pub fn resolve_nim_exe(home: &Path) -> Option<PathBuf> {
    resolve_bin_tool_exe(home, "nim")
}

// --- Julia ------------------------------------------------------------------------------------

pub fn julia_installation_valid(home: &Path) -> bool {
    resolve_julia_exe(home).is_some()
}

pub fn resolve_julia_exe(home: &Path) -> Option<PathBuf> {
    resolve_bin_tool_exe(home, "julia")
}

// --- Zig (root or `bin/`, same order as shim) -------------------------------------------------

pub fn zig_installation_valid(home: &Path) -> bool {
    resolve_zig_exe(home).is_some()
}

pub fn resolve_zig_exe(home: &Path) -> Option<PathBuf> {
    first_existing_path(&zig_exe_candidate_paths(home))
}

fn zig_exe_candidate_paths(home: &Path) -> Vec<PathBuf> {
    vec![
        home.join("zig.exe"),
        home.join("bin").join("zig.exe"),
        home.join("bin").join("zig"),
        home.join("zig"),
    ]
}

// --- R (R + Rscript under `bin/`) -------------------------------------------------------------

pub fn rlang_installation_valid(home: &Path) -> bool {
    resolve_r_exe(home).is_some() && resolve_rscript_exe(home).is_some()
}

pub fn resolve_r_exe(home: &Path) -> Option<PathBuf> {
    resolve_bin_tool_exe(home, "R")
}

pub fn resolve_rscript_exe(home: &Path) -> Option<PathBuf> {
    resolve_bin_tool_exe(home, "Rscript")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn zig_exe_candidate_paths_length_and_prefix() {
        let h = Path::new("/opt/zig/0.14.0");
        let v = zig_exe_candidate_paths(h);
        assert_eq!(v.len(), 4);
        assert_eq!(v[0], h.join("zig.exe"));
    }

    #[test]
    fn rlang_valid_when_r_and_rscript_exist_under_bin() {
        let tmp = tempdir().unwrap();
        let home = tmp.path();
        std::fs::create_dir_all(home.join("bin")).unwrap();
        #[cfg(windows)]
        {
            std::fs::write(home.join("bin").join("R.exe"), "").unwrap();
            std::fs::write(home.join("bin").join("Rscript.exe"), "").unwrap();
        }
        #[cfg(not(windows))]
        {
            std::fs::write(home.join("bin").join("R"), "").unwrap();
            std::fs::write(home.join("bin").join("Rscript"), "").unwrap();
        }
        assert!(rlang_installation_valid(home));
        assert!(resolve_r_exe(home).is_some());
        assert!(resolve_rscript_exe(home).is_some());
    }
}
