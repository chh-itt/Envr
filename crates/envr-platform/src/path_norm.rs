use std::path::{Path, PathBuf};

/// Normalize filesystem paths for downstream tooling.
///
/// On Windows, `std::fs::canonicalize` and some APIs may yield verbatim paths like `\\?\C:\...`
/// or `\\?\UNC\server\share\...`. Those forms are valid for Win32 file APIs, but they often break
/// when embedded into:
/// - `cmd.exe` batch files
/// - subprocess command lines / environment variables
/// - tools like `setx` and some installers
///
/// This helper removes verbatim prefixes and normalizes common alternate spellings.
pub fn normalize_fs_path(path: &Path) -> PathBuf {
    #[cfg(not(windows))]
    {
        path.to_path_buf()
    }

    #[cfg(windows)]
    {
        let s = path.as_os_str().to_string_lossy();

        // 1) Canonical verbatim forms.
        if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
            // `\\?\UNC\server\share\dir` => `\\server\share\dir`
            return PathBuf::from(format!(r"\\{rest}"));
        }
        if let Some(rest) = s.strip_prefix(r"\\?\") {
            // `\\?\C:\dir` => `C:\dir`
            return PathBuf::from(rest);
        }

        // 2) Alternate slash form we sometimes store/receive.
        if let Some(rest) = s.strip_prefix("//?/UNC/") {
            // `//?/UNC/server/share` => `\\server\share`
            return PathBuf::from(format!(r"\\{}", rest.replace('/', "\\")));
        }
        if let Some(rest) = s.strip_prefix("//?/") {
            // `//?/C:/dir` => `C:\dir`
            return PathBuf::from(rest.replace('/', "\\"));
        }

        path.to_path_buf()
    }
}

/// Convenience for callers that ultimately need a string (env vars / cmd scripts).
///
/// This is intentionally *lossy* to match existing behavior in the codebase.
pub fn normalize_fs_path_string_lossy(path: &Path) -> String {
    normalize_fs_path(path).to_string_lossy().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn strips_verbatim_drive_prefix() {
        let p = Path::new(r"\\?\D:\runtime\node\npm.cmd");
        assert_eq!(
            normalize_fs_path(p),
            PathBuf::from(r"D:\runtime\node\npm.cmd")
        );
    }

    #[cfg(windows)]
    #[test]
    fn strips_alt_slash_form() {
        let p = Path::new("//?/D:/runtime/node/npm.cmd");
        assert_eq!(
            normalize_fs_path(p),
            PathBuf::from(r"D:\runtime\node\npm.cmd")
        );
    }

    #[cfg(windows)]
    #[test]
    fn strips_verbatim_unc_prefix() {
        let p = Path::new(r"\\?\UNC\server\share\dir\file.exe");
        assert_eq!(
            normalize_fs_path(p),
            PathBuf::from(r"\\server\share\dir\file.exe")
        );
    }

    #[cfg(windows)]
    #[test]
    fn strips_alt_unc_slash_form() {
        let p = Path::new("//?/UNC/server/share/dir/file.exe");
        assert_eq!(
            normalize_fs_path(p),
            PathBuf::from(r"\\server\share\dir\file.exe")
        );
    }
}
