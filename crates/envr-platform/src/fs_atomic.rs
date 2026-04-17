use std::fs;
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};

fn tmp_path_for(path: &Path) -> PathBuf {
    let mut tmp = path.to_path_buf();
    let ext = match path.extension().and_then(|s| s.to_str()) {
        Some(e) if !e.is_empty() => format!("{e}.tmp"),
        _ => "tmp".to_string(),
    };
    tmp.set_extension(ext);
    tmp
}

fn sync_parent_dir(path: &Path) -> io::Result<()> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    // Best-effort: directory sync is not supported uniformly across platforms / filesystems.
    if let Ok(dir) = fs::File::open(parent) {
        let _ = dir.sync_all();
    }
    Ok(())
}

/// Atomic-ish write for small config files: write to sibling temp file, fsync, rename, fsync dir.
///
/// Guarantees (best effort):
/// - Data is flushed to disk before rename (`sync_all` on the temp file).
/// - The rename is atomic when it stays on the same filesystem.
/// - The parent directory is synced after rename when supported.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = tmp_path_for(path);
    {
        let mut f = fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path)?;
    let _ = sync_parent_dir(path);
    Ok(())
}

/// Like [`write_atomic`], but preserves an existing file as `<path>.<backup_ext>` before replacing.
pub fn write_atomic_with_backup(path: &Path, bytes: &[u8], backup_ext: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = tmp_path_for(path);
    {
        let mut f = fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }

    if path.exists() {
        let bak = path.with_extension(backup_ext);
        let _ = fs::remove_file(&bak);
        // If rename fails (e.g. cross-device), we still prefer returning the error rather than
        // risking partial writes.
        fs::rename(path, &bak)?;
        let _ = sync_parent_dir(path);
    }

    fs::rename(&tmp, path)?;
    let _ = sync_parent_dir(path);
    Ok(())
}
