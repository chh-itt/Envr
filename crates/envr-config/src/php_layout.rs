//! On-disk layout for PHP runtimes on Windows (NTS vs TS).
//!
//! Version trees live under `runtimes/php/versions/` as `{semver}-nts` or `{semver}-ts`.
//! Legacy installs may use a plain `{semver}` directory; those are treated as NTS-only.

/// Folder name for a given semver and Windows thread-safe (TS) vs non-thread-safe (NTS) build.
pub fn version_dir_name(semver: &str, want_ts: bool) -> String {
    let s = semver.trim();
    if want_ts {
        format!("{s}-ts")
    } else {
        format!("{s}-nts")
    }
}

/// Strip `-nts` / `-ts` suffix for UI and [`RuntimeVersion`] labels.
pub fn display_version_label_from_dir_name(dir_name: &str) -> String {
    dir_name
        .strip_suffix("-nts")
        .or_else(|| dir_name.strip_suffix("-ts"))
        .unwrap_or(dir_name)
        .to_string()
}

/// `true` if the version directory name denotes a **TS** tree (`…-ts`).
/// Legacy plain `{semver}` and `…-nts` are non-TS.
pub fn dir_flavor_is_ts(dir_name: &str) -> bool {
    dir_name.ends_with("-ts")
}

/// Whether `versions/<dirname>` belongs to the selected build flavor.
/// Plain `{semver}` (no suffix) is NTS-only for backward compatibility.
pub fn dir_matches_build_flavor(dir_name: &str, want_ts: bool) -> bool {
    if dir_name.ends_with("-ts") {
        return want_ts;
    }
    if dir_name.ends_with("-nts") {
        return !want_ts;
    }
    !want_ts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_dir_names() {
        assert_eq!(version_dir_name("8.4.20", false), "8.4.20-nts");
        assert_eq!(version_dir_name("8.4.20", true), "8.4.20-ts");
    }

    #[test]
    fn display_strips_suffix() {
        assert_eq!(display_version_label_from_dir_name("8.4.20-nts"), "8.4.20");
        assert_eq!(display_version_label_from_dir_name("8.5.0-ts"), "8.5.0");
        assert_eq!(display_version_label_from_dir_name("8.4.20"), "8.4.20");
    }

    #[test]
    fn dir_flavor_ts() {
        assert!(dir_flavor_is_ts("8.4.20-ts"));
        assert!(!dir_flavor_is_ts("8.4.20-nts"));
        assert!(!dir_flavor_is_ts("8.4.20"));
    }

    #[test]
    fn flavor_filter() {
        assert!(dir_matches_build_flavor("8.4.20-nts", false));
        assert!(!dir_matches_build_flavor("8.4.20-nts", true));
        assert!(dir_matches_build_flavor("8.4.20-ts", true));
        assert!(!dir_matches_build_flavor("8.4.20-ts", false));
        assert!(dir_matches_build_flavor("8.4.20", false));
        assert!(!dir_matches_build_flavor("8.4.20", true));
    }
}
