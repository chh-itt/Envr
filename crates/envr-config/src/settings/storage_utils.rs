use std::{fs, path::Path, time::SystemTime};

use envr_error::{EnvrError, EnvrResult, ErrorCode};

pub(super) fn file_mtime(path: &Path) -> EnvrResult<SystemTime> {
    let meta = fs::metadata(path).map_err(EnvrError::from)?;
    meta.modified()
        .map_err(|e| EnvrError::Io(std::io::Error::other(e)))
}

pub(super) fn backup_corrupted_file(path: &Path) -> EnvrResult<()> {
    let ts = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|e| EnvrError::with_source(ErrorCode::Runtime, "time error", e))?
        .as_secs();
    let bad = path.with_extension(format!("toml.bad.{ts}"));
    let _ = fs::rename(path, bad);
    Ok(())
}
