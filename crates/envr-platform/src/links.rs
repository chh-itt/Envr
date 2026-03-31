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

    if dst.exists() {
        // Best-effort idempotency: if dst already points to src, keep it; otherwise replace.
        if let Ok(target) = read_link_target(dst)
            && target == src
        {
            return Ok(());
        }
        if dst.is_dir() {
            fs::remove_dir_all(dst).map_err(EnvrError::from)?;
        } else {
            fs::remove_file(dst).map_err(EnvrError::from)?;
        }
    }

    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent).map_err(EnvrError::from)?;
    }

    match link_type {
        LinkType::Hard => fs::hard_link(src, dst).map_err(EnvrError::from),
        LinkType::Soft => create_symlink(src, dst),
    }
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
    if src.is_dir() {
        symlink_dir(src, dst).map_err(EnvrError::from)
    } else {
        symlink_file(src, dst).map_err(EnvrError::from)
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
