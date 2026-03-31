use envr_error::{EnvrError, EnvrResult};
use flate2::read::GzDecoder;
use std::{
    fs, io,
    path::{Component, Path, PathBuf},
};
use tar::Archive as TarArchive;
use zip::ZipArchive;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveKind {
    Zip,
    Tar,
    TarGz,
}

pub fn detect_archive_kind(path: impl AsRef<Path>) -> EnvrResult<ArchiveKind> {
    let p = path.as_ref();
    let name = p
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| EnvrError::Validation("archive path missing filename".to_string()))?;

    if name.ends_with(".zip") {
        Ok(ArchiveKind::Zip)
    } else if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        Ok(ArchiveKind::TarGz)
    } else if name.ends_with(".tar") {
        Ok(ArchiveKind::Tar)
    } else {
        Err(EnvrError::Validation(format!(
            "unsupported archive type: {name}"
        )))
    }
}

pub fn extract_archive(
    archive_path: impl AsRef<Path>,
    dest_dir: impl AsRef<Path>,
) -> EnvrResult<()> {
    let archive_path = archive_path.as_ref();
    let dest_dir = dest_dir.as_ref();
    fs::create_dir_all(dest_dir).map_err(EnvrError::from)?;

    match detect_archive_kind(archive_path)? {
        ArchiveKind::Zip => extract_zip(archive_path, dest_dir),
        ArchiveKind::Tar => extract_tar(archive_path, dest_dir),
        ArchiveKind::TarGz => extract_tar_gz(archive_path, dest_dir),
    }
}

pub fn extract_archive_atomic(
    archive_path: impl AsRef<Path>,
    dest_dir: impl AsRef<Path>,
) -> EnvrResult<()> {
    let archive_path = archive_path.as_ref();
    let dest_dir = dest_dir.as_ref();

    let parent = dest_dir
        .parent()
        .ok_or_else(|| EnvrError::Validation("dest_dir has no parent".to_string()))?;
    fs::create_dir_all(parent).map_err(EnvrError::from)?;

    let tmp_dir = parent.join(format!(".envr_extract_tmp_{}", std::process::id()));
    if tmp_dir.exists() {
        let _ = fs::remove_dir_all(&tmp_dir);
    }
    fs::create_dir_all(&tmp_dir).map_err(EnvrError::from)?;

    extract_archive(archive_path, &tmp_dir)?;

    if dest_dir.exists() {
        fs::remove_dir_all(dest_dir).map_err(EnvrError::from)?;
    }
    fs::rename(&tmp_dir, dest_dir).map_err(EnvrError::from)?;
    Ok(())
}

fn extract_zip(path: &Path, dest: &Path) -> EnvrResult<()> {
    let f = fs::File::open(path).map_err(EnvrError::from)?;
    let mut zip =
        ZipArchive::new(f).map_err(|e| EnvrError::Validation(format!("invalid zip: {e}")))?;

    for i in 0..zip.len() {
        let mut file = zip
            .by_index(i)
            .map_err(|e| EnvrError::Validation(format!("zip entry: {e}")))?;
        let name = file.name().to_string();
        let out_path = safe_join(dest, Path::new(&name))?;

        if file.is_dir() {
            fs::create_dir_all(&out_path).map_err(EnvrError::from)?;
            continue;
        }

        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent).map_err(EnvrError::from)?;
        }

        let mut out = fs::File::create(&out_path).map_err(EnvrError::from)?;
        io::copy(&mut file, &mut out).map_err(EnvrError::from)?;
    }

    Ok(())
}

fn extract_tar(path: &Path, dest: &Path) -> EnvrResult<()> {
    let f = fs::File::open(path).map_err(EnvrError::from)?;
    let mut ar = TarArchive::new(f);
    unpack_tar(&mut ar, dest)
}

fn extract_tar_gz(path: &Path, dest: &Path) -> EnvrResult<()> {
    let f = fs::File::open(path).map_err(EnvrError::from)?;
    let gz = GzDecoder::new(f);
    let mut ar = TarArchive::new(gz);
    unpack_tar(&mut ar, dest)
}

fn unpack_tar<R: io::Read>(ar: &mut TarArchive<R>, dest: &Path) -> EnvrResult<()> {
    for entry in ar
        .entries()
        .map_err(|e| EnvrError::Validation(format!("tar entries: {e}")))?
    {
        let mut entry = entry.map_err(|e| EnvrError::Validation(format!("tar entry: {e}")))?;
        let path = entry
            .path()
            .map_err(|e| EnvrError::Validation(format!("tar path: {e}")))?;
        let out_path = safe_join(dest, &path)?;

        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent).map_err(EnvrError::from)?;
        }

        entry
            .unpack(&out_path)
            .map_err(|e| EnvrError::Validation(format!("tar unpack: {e}")))?;
    }
    Ok(())
}

fn safe_join(base: &Path, rel: &Path) -> EnvrResult<PathBuf> {
    let mut out = base.to_path_buf();

    for comp in rel.components() {
        match comp {
            Component::Prefix(_) | Component::RootDir => {
                return Err(EnvrError::Validation(format!(
                    "unsafe path (absolute): {}",
                    rel.display()
                )));
            }
            Component::ParentDir => {
                return Err(EnvrError::Validation(format!(
                    "unsafe path (parent traversal): {}",
                    rel.display()
                )));
            }
            Component::CurDir => {}
            Component::Normal(seg) => out.push(seg),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn safe_join_rejects_parent_traversal() {
        let tmp = TempDir::new().expect("tmp");
        let base = tmp.path();
        let err = safe_join(base, Path::new("../evil")).expect_err("should reject");
        assert!(matches!(err, EnvrError::Validation(_)));
    }

    #[test]
    fn zip_path_traversal_is_rejected() {
        let tmp = TempDir::new().expect("tmp");
        let zip_path = tmp.path().join("a.zip");
        let out_dir = tmp.path().join("out");

        // create zip with ../evil entry
        let f = fs::File::create(&zip_path).expect("create");
        let mut zw = zip::ZipWriter::new(f);
        let options: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();
        zw.start_file("../evil.txt", options).expect("start");
        zw.write_all(b"bad").expect("write");
        zw.finish().expect("finish");

        let err = extract_archive(&zip_path, &out_dir).expect_err("reject");
        assert!(matches!(err, EnvrError::Validation(_)));
    }
}
