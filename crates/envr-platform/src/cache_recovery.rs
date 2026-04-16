use std::path::Path;

/// True when `path` exists and its mtime is within `ttl_secs` seconds.
pub fn file_is_within_ttl(path: &Path, ttl_secs: u64) -> bool {
    if ttl_secs == 0 {
        return false;
    }
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    let Ok(mtime) = meta.modified() else {
        return false;
    };
    let Ok(age) = std::time::SystemTime::now().duration_since(mtime) else {
        return false;
    };
    age.as_secs() <= ttl_secs
}

/// Read a JSON string array from disk (cache file), optionally requiring a TTL hit.
///
/// - On read/parse/validation failure, returns `None` and best-effort removes the cache file.
/// - `validate` can enforce domain-specific invariants (non-empty, min length, etc).
pub fn read_json_string_list(
    path: &Path,
    require_ttl_hit: Option<u64>,
    validate: impl FnOnce(&[String]) -> bool,
) -> Option<Vec<String>> {
    if let Some(ttl) = require_ttl_hit {
        if !file_is_within_ttl(path, ttl) {
            return None;
        }
    }

    let body = std::fs::read_to_string(path).ok()?;
    let list = serde_json::from_str::<Vec<String>>(&body).ok();
    match list {
        Some(list) if validate(&list) => Some(list),
        _ => {
            let _ = std::fs::remove_file(path);
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::read_json_string_list;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn read_json_string_list_removes_bad_json() {
        let tmp = TempDir::new().expect("tmp");
        let p = tmp.path().join("x.json");
        fs::write(&p, b"{ not json").expect("write");
        let got = read_json_string_list(&p, None, |_| true);
        assert!(got.is_none());
        assert!(!p.exists(), "bad cache should be removed");
    }

    #[test]
    fn read_json_string_list_removes_empty_when_validate_rejects() {
        let tmp = TempDir::new().expect("tmp");
        let p = tmp.path().join("x.json");
        fs::write(&p, b"[]").expect("write");
        let got = read_json_string_list(&p, None, |list| !list.is_empty());
        assert!(got.is_none());
        assert!(!p.exists(), "empty cache should be removed when invalid");
    }
}

