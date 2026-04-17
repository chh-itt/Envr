use envr_error::{EnvrError, EnvrResult};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkType {
    Hard,
    Soft,
}

pub fn ensure_link(
    link_type: LinkType,
    src: impl AsRef<Path>,
    dst: impl AsRef<Path>,
) -> EnvrResult<()> {
    let src = src.as_ref();
    let dst = dst.as_ref();

    if std::fs::symlink_metadata(dst).is_ok() {
        // Best-effort idempotency: if dst already points to src, keep it; otherwise replace.
        if let Ok(target) = read_link_target(dst)
            && target == src
        {
            return Ok(());
        }
        remove_path_best_effort(dst)?;
    }

    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent).map_err(EnvrError::from)?;
    }

    match link_type {
        LinkType::Hard => fs::hard_link(src, dst).map_err(EnvrError::from),
        LinkType::Soft => create_symlink(src, dst),
    }
}

fn remove_path_best_effort(path: &Path) -> EnvrResult<()> {
    // Windows reparse points (junction/symlink) are tricky: remove_file/remove_dir
    // usually removes the link node itself, while remove_dir_all may recurse target.
    // Try lightweight removals first, then fallback to recursive delete.
    if fs::remove_file(path).is_ok() {
        return Ok(());
    }
    if fs::remove_dir(path).is_ok() {
        return Ok(());
    }
    if fs::remove_dir_all(path).is_ok() {
        return Ok(());
    }
    Err(EnvrError::Io(std::io::Error::other(format!(
        "failed to remove existing path: {}",
        path.display()
    ))))
}

fn read_link_target(path: &Path) -> EnvrResult<PathBuf> {
    if let Ok(t) = fs::read_link(path) {
        return Ok(t);
    }
    Ok(path.to_path_buf())
}

#[cfg(windows)]
fn create_symlink(src: &Path, dst: &Path) -> EnvrResult<()> {
    use std::os::windows::fs::{symlink_dir, symlink_file};
    let is_dir = src.is_dir();
    let res = if is_dir {
        symlink_dir(src, dst)
    } else {
        symlink_file(src, dst)
    };

    match res {
        Ok(()) => Ok(()),
        Err(e) => {
            // On Windows, creating symlinks may require extra privileges.
            // When that happens, we should still be able to set up `current` links.
            // - For directories: fall back to junction (`mklink /J`), which doesn't require
            //   "Create symbolic links" privilege in most setups.
            // - For files: fall back to hard link.
            if e.raw_os_error() == Some(1314) {
                if is_dir {
                    if try_create_junction(src, dst)? {
                        return Ok(());
                    }
                } else if fs::hard_link(src, dst).is_ok() {
                    return Ok(());
                }
            }
            Err(EnvrError::from(e))
        }
    }
}

#[cfg(windows)]
fn try_create_junction(src: &Path, dst: &Path) -> EnvrResult<bool> {
    use std::process::{Command, Stdio};

    fn normalize_for_cmd(p: &Path) -> String {
        let s = p.to_string_lossy().to_string();
        s.strip_prefix(r"\\?\").unwrap_or(&s).to_string()
    }

    let src_s = normalize_for_cmd(src);
    let dst_s = normalize_for_cmd(dst);

    // `mklink /J <dst> <src>` creates a junction.
    // We intentionally run through `cmd /C` to let Windows handle quoting.
    let cmdline = format!("mklink /J \"{}\" \"{}\"", dst_s, src_s);
    let status = Command::new("cmd")
        .args(["/C", &cmdline])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    match status {
        Ok(st) if st.success() => Ok(true),
        _ => {
            // Second fallback for environments where `mklink` is restricted by shell policy.
            let ps = format!(
                "New-Item -ItemType Junction -Path '{}' -Target '{}' | Out-Null",
                dst_s.replace('\'', "''"),
                src_s.replace('\'', "''"),
            );
            let status2 = Command::new("powershell")
                .args(["-NoProfile", "-Command", &ps])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            Ok(matches!(status2, Ok(st) if st.success()))
        }
    }
}

#[cfg(unix)]
fn create_symlink(src: &Path, dst: &Path) -> EnvrResult<()> {
    use std::os::unix::fs::symlink;
    symlink(src, dst).map_err(EnvrError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn hard_link_can_be_created_for_file() {
        let tmp = TempDir::new().expect("tmp");
        let src = tmp.path().join("a.txt");
        let dst = tmp.path().join("b.txt");
        {
            let mut f = std::fs::File::create(&src).expect("create");
            writeln!(f, "hello").expect("write");
        }

        ensure_link(LinkType::Hard, &src, &dst).expect("link");
        let a = std::fs::read_to_string(&src).expect("read");
        let b = std::fs::read_to_string(&dst).expect("read");
        assert_eq!(a, b);
    }
}
