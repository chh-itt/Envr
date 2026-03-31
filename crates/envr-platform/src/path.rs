use std::path::{Path, PathBuf};

pub fn split_path_list(path_value: &str) -> Vec<String> {
    let sep = if cfg!(windows) { ';' } else { ':' };
    path_value
        .split(sep)
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.to_string())
        .collect()
}

pub fn join_path_list(parts: &[String]) -> String {
    let sep = if cfg!(windows) { ';' } else { ':' };
    parts.join(&sep.to_string())
}

pub fn prepend_unique_path(existing: &str, dir: impl AsRef<Path>) -> String {
    let dir = normalize(dir.as_ref());
    let mut parts = split_path_list(existing);
    parts.retain(|p| normalize(Path::new(p)) != dir);
    parts.insert(0, dir.to_string_lossy().to_string());
    join_path_list(&parts)
}

fn normalize(p: &Path) -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(p.to_string_lossy().to_lowercase())
    } else {
        p.to_path_buf()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepend_unique_removes_duplicates() {
        let existing = if cfg!(windows) {
            r"C:\A;C:\B;C:\A"
        } else {
            "/a:/b:/a"
        };
        let out = prepend_unique_path(existing, if cfg!(windows) { r"C:\A" } else { "/a" });
        let parts = split_path_list(&out);
        assert_eq!(parts[0], if cfg!(windows) { r"c:\a" } else { "/a" });
        assert_eq!(parts.len(), 2);
    }
}
