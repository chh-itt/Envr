//! Crystal distribution layout on disk (official tarballs vs Windows portable zip).
//!
//! Keep this module free of HTTP/install logic so `envr-shim-core` and `envr-runtime-crystal`
//! can share one source of truth for PATH and “does this directory look like Crystal?”.

use std::path::{Path, PathBuf};

/// Paths to try for the `crystal` compiler under a **home** directory (ordered: preferred first).
pub fn crystal_compiler_candidate_paths(home: &Path) -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        vec![
            home.join("bin").join("crystal.exe"),
            home.join("crystal.exe"),
        ]
    }
    #[cfg(not(windows))]
    {
        vec![home.join("bin").join("crystal"), home.join("crystal")]
    }
}

/// Directories to prepend on `PATH` for this Crystal home.
///
/// Official archives use `bin/`; Windows portable zips place `crystal.exe` at the package root
/// next to `lib/`, so the home directory itself must appear on `PATH` after `bin/`.
pub fn crystal_path_entries(home: &Path) -> Vec<PathBuf> {
    vec![home.join("bin"), home.to_path_buf()]
}

/// True if `home` contains a recognizable Crystal compiler binary.
pub fn crystal_home_has_compiler(home: &Path) -> bool {
    crystal_compiler_candidate_paths(home)
        .into_iter()
        .any(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn detects_bin_layout() {
        let tmp =
            std::env::temp_dir().join(format!("envr_crystal_paths_bin_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join("bin")).expect("mkdir");
        #[cfg(windows)]
        fs::write(tmp.join("bin").join("crystal.exe"), []).expect("touch");
        #[cfg(not(windows))]
        fs::write(tmp.join("bin").join("crystal"), []).expect("touch");
        assert!(crystal_home_has_compiler(&tmp));
        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn detects_portable_root_layout() {
        let tmp =
            std::env::temp_dir().join(format!("envr_crystal_paths_root_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).expect("mkdir");
        #[cfg(windows)]
        fs::write(tmp.join("crystal.exe"), []).expect("touch");
        #[cfg(not(windows))]
        fs::write(tmp.join("crystal"), []).expect("touch");
        assert!(crystal_home_has_compiler(&tmp));
        let _ = fs::remove_dir_all(&tmp);
    }
}
