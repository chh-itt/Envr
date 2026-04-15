//! `--env` / `--env-file` parsing for `exec`, `run`, and `template`.

use envr_error::{EnvrError, EnvrResult};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// `KEY=VALUE` (first `=` separates; value may be empty).
pub fn parse_env_pair(raw: &str) -> EnvrResult<(String, String)> {
    let s = raw.trim();
    if s.is_empty() {
        return Err(EnvrError::Validation(
            "empty `--env` entry (expected KEY=VALUE)".into(),
        ));
    }
    let Some((k, v)) = s.split_once('=') else {
        return Err(EnvrError::Validation(format!(
            "`--env` must be KEY=VALUE, got: {s:?}"
        )));
    };
    let key = k.trim();
    if key.is_empty() {
        return Err(EnvrError::Validation(
            "`--env` key is empty (expected KEY=VALUE)".into(),
        ));
    }
    Ok((key.to_string(), v.to_string()))
}

fn strip_quotes(val: &str) -> String {
    let t = val.trim();
    if t.len() >= 2 {
        let b = t.as_bytes();
        if (b[0] == b'"' && b[t.len() - 1] == b'"') || (b[0] == b'\'' && b[t.len() - 1] == b'\'') {
            return t[1..t.len() - 1].to_string();
        }
    }
    t.to_string()
}

/// Minimal dotenv loader (`KEY=VALUE`, `#` comments, optional `export ` prefix).
pub fn load_dotenv_file(path: &Path) -> EnvrResult<Vec<(String, String)>> {
    let raw = fs::read_to_string(path).map_err(|e| {
        EnvrError::Io(std::io::Error::new(
            e.kind(),
            format!("{}: {e}", path.display()),
        ))
    })?;
    let raw = raw.trim_start_matches('\u{feff}');
    let mut out = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line
            .strip_prefix("export ")
            .or_else(|| line.strip_prefix("export\t"))
            .unwrap_or(line)
            .trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let key = k.trim();
        if key.is_empty() {
            continue;
        }
        out.push((key.to_string(), strip_quotes(v)));
    }
    Ok(out)
}

/// Apply env files in order, then `--env` pairs (last wins).
pub fn apply_env_overrides(
    map: &mut HashMap<String, String>,
    env_files: &[std::path::PathBuf],
    env_pairs: &[String],
) -> EnvrResult<()> {
    for f in env_files {
        for (k, v) in load_dotenv_file(f)? {
            map.insert(k, v);
        }
    }
    for p in env_pairs {
        let (k, v) = parse_env_pair(p)?;
        map.insert(k, v);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_env_pair_basic() {
        assert_eq!(parse_env_pair("A=b").unwrap(), ("A".into(), "b".into()));
        assert_eq!(parse_env_pair("X=").unwrap(), ("X".into(), "".into()));
    }

    #[test]
    fn load_dotenv_skips_comments() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("t.env");
        fs::write(&p, "# c\nFOO=bar\nexport BAR=2\n\nBAZ='q u x'\n").unwrap();
        let v = load_dotenv_file(&p).unwrap();
        let m: HashMap<_, _> = v.into_iter().collect();
        assert_eq!(m.get("FOO").map(String::as_str), Some("bar"));
        assert_eq!(m.get("BAR").map(String::as_str), Some("2"));
        assert_eq!(m.get("BAZ").map(String::as_str), Some("q u x"));
    }
}
