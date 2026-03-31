use envr_error::{EnvrError, EnvrResult};
use std::{fs, path::Path};

const BEGIN: &str = "# >>> envr >>>";
const END: &str = "# <<< envr <<<";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellKind {
    Bash,
    Zsh,
    Fish,
    PowerShell,
}

pub fn inject_path_block(existing: &str, lines: &[&str]) -> String {
    let without = remove_block(existing);
    let mut out = String::new();
    out.push_str(without.trim_end_matches(['\r', '\n']));
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str(BEGIN);
    out.push('\n');
    for l in lines {
        out.push_str(l);
        out.push('\n');
    }
    out.push_str(END);
    out.push('\n');
    out
}

pub fn remove_block(existing: &str) -> String {
    let mut out = String::with_capacity(existing.len());
    let mut in_block = false;

    for line in existing.lines() {
        if line.trim_end() == BEGIN {
            in_block = true;
            continue;
        }
        if line.trim_end() == END {
            in_block = false;
            continue;
        }
        if !in_block {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

pub fn render_path_export(shell: ShellKind, dir: &str) -> Vec<String> {
    match shell {
        ShellKind::Bash | ShellKind::Zsh => vec![format!(r#"export PATH="{}:$PATH""#, dir)],
        ShellKind::Fish => vec![format!(r#"set -gx PATH {} $PATH"#, dir)],
        ShellKind::PowerShell => vec![format!(r#"$env:Path = "{};{0}" -f $env:Path"#, dir)],
    }
}

pub fn update_shell_config_file(path: impl AsRef<Path>, lines: &[String]) -> EnvrResult<()> {
    let path = path.as_ref();
    let existing = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(EnvrError::from(e)),
    };

    let line_refs = lines.iter().map(|s| s.as_str()).collect::<Vec<_>>();
    let updated = inject_path_block(&existing, &line_refs);

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(EnvrError::from)?;
    }
    fs::write(path, updated).map_err(EnvrError::from)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inject_is_idempotent() {
        let lines = vec!["export PATH=\"/x:$PATH\""];
        let first = inject_path_block("", &lines);
        let second = inject_path_block(&first, &lines);
        assert_eq!(first, second);
    }

    #[test]
    fn remove_block_removes_only_envr_section() {
        let lines = vec!["export PATH=\"/x:$PATH\""];
        let content = format!("keep\n{}\n", inject_path_block("", &lines));
        let removed = remove_block(&content);
        assert!(removed.contains("keep"));
        assert!(!removed.contains(BEGIN));
        assert!(!removed.contains(END));
    }
}
