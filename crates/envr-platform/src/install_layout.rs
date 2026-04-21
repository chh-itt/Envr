//! Helpers for atomic installs under `.../versions/<version>`: build in a sibling
//! `.envr-staging-*` directory, validate, then rename into place.

use std::fs;
use std::path::{Path, PathBuf};

use envr_error::{EnvrError, EnvrResult};

fn is_cross_device_rename_error(e: &std::io::Error) -> bool {
    // Windows: ERROR_NOT_SAME_DEVICE (17)
    if e.raw_os_error() == Some(17) {
        return true;
    }
    // Unix: EXDEV (best-effort; std doesn't expose it consistently across toolchains).
    false
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> EnvrResult<()> {
    fs::create_dir_all(dst).map_err(EnvrError::from)?;
    for e in fs::read_dir(src).map_err(EnvrError::from)? {
        let e = e.map_err(EnvrError::from)?;
        let from = e.path();
        let to = dst.join(e.file_name());
        let meta = fs::symlink_metadata(&from).map_err(EnvrError::from)?;
        if meta.is_dir() {
            copy_dir_recursive(&from, &to)?;
            continue;
        }
        // Treat symlinks as files by following the link (most runtime archives don't ship symlinks).
        fs::copy(&from, &to).map_err(EnvrError::from)?;
    }
    Ok(())
}

/// Move a directory tree; cross-device safe (rename or copy+delete).
pub fn move_dir(src: &Path, dst: &Path) -> EnvrResult<()> {
    match fs::rename(src, dst) {
        Ok(()) => Ok(()),
        Err(e) if is_cross_device_rename_error(&e) => {
            copy_dir_recursive(src, dst)?;
            fs::remove_dir_all(src).map_err(EnvrError::from)?;
            Ok(())
        }
        Err(e) => Err(EnvrError::from(e)),
    }
}

/// Ensure `final_dir`'s parent exists (required before `rename` on some platforms).
pub fn ensure_final_parent(final_dir: &Path) -> EnvrResult<()> {
    let parent = final_dir
        .parent()
        .ok_or_else(|| EnvrError::Validation("final_dir has no parent".into()))?;
    fs::create_dir_all(parent).map_err(EnvrError::from)?;
    Ok(())
}

/// Sibling of `final_dir`: `.envr-staging-<leaf>-<unix_nanos>`.
pub fn sibling_staging_path(final_dir: &Path) -> EnvrResult<PathBuf> {
    let parent = final_dir
        .parent()
        .ok_or_else(|| EnvrError::Validation("final_dir has no parent".into()))?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let leaf = final_dir
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("version");
    Ok(parent.join(format!(".envr-staging-{leaf}-{stamp}")))
}

/// Remove a file or directory tree if it exists.
pub fn remove_if_exists(path: &Path) -> EnvrResult<()> {
    let meta = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(EnvrError::from(e)),
    };
    if meta.is_dir() {
        fs::remove_dir_all(path).map_err(EnvrError::from)
    } else {
        fs::remove_file(path).map_err(EnvrError::from)
    }
}

/// Move every direct child of `src` into `dst` (directory is created).
pub fn hoist_directory_children(src: &Path, dst: &Path) -> EnvrResult<()> {
    fs::create_dir_all(dst).map_err(EnvrError::from)?;
    for e in fs::read_dir(src).map_err(EnvrError::from)? {
        let e = e.map_err(EnvrError::from)?;
        let from = e.path();
        let to = dst.join(e.file_name());
        // Keep it robust in case src/dst are on different volumes.
        if from.is_dir() {
            move_dir(&from, &to)?;
        } else {
            fs::rename(&from, &to).map_err(EnvrError::from)?;
        }
    }
    Ok(())
}

/// Replace `final_dir` with the validated staging directory in one rename.
pub fn commit_staging_dir(validated_staging: &Path, final_dir: &Path) -> EnvrResult<()> {
    remove_if_exists(final_dir)?;
    move_dir(validated_staging, final_dir).map_err(|e| {
        let _ = fs::remove_dir_all(validated_staging);
        e
    })?;
    Ok(())
}
