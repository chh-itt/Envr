use envr_error::{EnvrError, EnvrResult};
use flate2::read::GzDecoder;
use std::{
    fs, io,
    path::{Component, Path, PathBuf},
};
use tar::Archive as TarArchive;
use xz2::read::XzDecoder;
use zip::ZipArchive;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveKind {
    Zip,
    Tar,
    TarGz,
    TarXz,
}

pub fn detect_archive_kind(path: impl AsRef<Path>) -> EnvrResult<ArchiveKind> {
    let p = path.as_ref();
    let name = p
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| EnvrError::Validation("archive path missing filename".to_string()))?;

    // NuGet `.nupkg` is a zip container.
    if name.ends_with(".zip") || name.ends_with(".nupkg") {
        Ok(ArchiveKind::Zip)
    } else if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        Ok(ArchiveKind::TarGz)
    } else if name.ends_with(".tar.xz") || name.ends_with(".txz") {
        Ok(ArchiveKind::TarXz)
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
        ArchiveKind::TarXz => extract_tar_xz(archive_path, dest_dir),
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

fn extract_tar_xz(path: &Path, dest: &Path) -> EnvrResult<()> {
    let f = fs::File::open(path).map_err(EnvrError::from)?;
    let xz = XzDecoder::new(f);
    let mut ar = TarArchive::new(xz);
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
    use flate2::Compression;
    use flate2::write::GzEncoder;
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

    #[test]
    fn detect_archive_kind_covers_common_suffixes() {
        assert_eq!(detect_archive_kind("x.zip").expect("zip"), ArchiveKind::Zip);
        assert_eq!(
            detect_archive_kind("x.nupkg").expect("nupkg"),
            ArchiveKind::Zip
        );
        assert_eq!(detect_archive_kind("x.tar").expect("tar"), ArchiveKind::Tar);
        assert_eq!(
            detect_archive_kind("x.tgz").expect("tgz"),
            ArchiveKind::TarGz
        );
        assert_eq!(
            detect_archive_kind("x.txz").expect("txz"),
            ArchiveKind::TarXz
        );
    }

    #[test]
    fn detect_archive_kind_rejects_unknown_suffix() {
        let err = detect_archive_kind("x.7z").expect_err("must reject");
        assert!(matches!(err, EnvrError::Validation(_)));
    }

    #[test]
    fn detect_archive_kind_rejects_missing_filename() {
        let err = detect_archive_kind(Path::new("")).expect_err("must reject");
        assert!(matches!(err, EnvrError::Validation(_)));
    }

    #[test]
    fn extract_archive_unpacks_zip_with_subdir() {
        let tmp = TempDir::new().expect("tmp");
        let zip_path = tmp.path().join("good.zip");
        let f = fs::File::create(&zip_path).expect("create");
        let mut zw = zip::ZipWriter::new(f);
        let options: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();
        zw.start_file("d/nested.txt", options).expect("start");
        zw.write_all(b"zip-data").expect("write");
        zw.finish().expect("finish");
        let out = tmp.path().join("zip_out");
        extract_archive(&zip_path, &out).expect("extract");
        assert_eq!(
            fs::read_to_string(out.join("d/nested.txt")).expect("read"),
            "zip-data"
        );
    }

    #[test]
    fn extract_archive_unpacks_plain_tar() {
        let tmp = TempDir::new().expect("tmp");
        let tar_path = tmp.path().join("a.tar");
        let src = tmp.path().join("src.txt");
        fs::write(&src, b"tar-content").expect("write");
        let f = fs::File::create(&tar_path).expect("create");
        let mut ar = tar::Builder::new(f);
        ar.append_path_with_name(&src, "inner/a.txt")
            .expect("append");
        ar.finish().expect("finish");
        let out = tmp.path().join("tar_out");
        extract_archive(&tar_path, &out).expect("extract");
        assert_eq!(
            fs::read_to_string(out.join("inner/a.txt")).expect("read"),
            "tar-content"
        );
    }

    #[test]
    fn extract_archive_unpacks_tar_gz() {
        let tmp = TempDir::new().expect("tmp");
        let tgz_path = tmp.path().join("a.tar.gz");
        let src = tmp.path().join("blob");
        fs::write(&src, b"gzip-me").expect("write");
        let file = fs::File::create(&tgz_path).expect("create");
        let gz = GzEncoder::new(file, Compression::default());
        let mut ar = tar::Builder::new(gz);
        ar.append_path_with_name(&src, "z/inner.txt")
            .expect("append");
        ar.finish().expect("finish");
        let gz = ar.into_inner().expect("into_inner");
        gz.finish().expect("gz finish");
        let out = tmp.path().join("tgz_out");
        extract_archive(&tgz_path, &out).expect("extract");
        assert_eq!(
            fs::read_to_string(out.join("z/inner.txt")).expect("read"),
            "gzip-me"
        );
    }

    #[test]
    fn extract_archive_atomic_replaces_existing_dest() {
        let tmp = TempDir::new().expect("tmp");
        let zip_path = tmp.path().join("atomic.zip");
        let f = fs::File::create(&zip_path).expect("create");
        let mut zw = zip::ZipWriter::new(f);
        let options: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();
        zw.start_file("only.txt", options).expect("start");
        zw.write_all(b"new").expect("write");
        zw.finish().expect("finish");

        let dest = tmp.path().join("dest_dir");
        fs::create_dir_all(&dest).expect("mkdir");
        fs::write(dest.join("stale.txt"), b"old").expect("stale");

        extract_archive_atomic(&zip_path, &dest).expect("atomic");
        assert!(dest.join("only.txt").is_file());
        assert!(!dest.join("stale.txt").exists());
        assert_eq!(
            fs::read_to_string(dest.join("only.txt")).expect("read"),
            "new"
        );
    }

    #[test]
    fn safe_join_accepts_dot_components() {
        let tmp = TempDir::new().expect("tmp");
        let base = tmp.path();
        let p = safe_join(base, Path::new("./a/./b")).expect("ok");
        assert_eq!(p, base.join("a").join("b"));
    }

    #[cfg(unix)]
    #[test]
    fn safe_join_rejects_absolute_unix_path() {
        let tmp = TempDir::new().expect("tmp");
        let err = safe_join(tmp.path(), Path::new("/etc/passwd")).expect_err("abs");
        assert!(matches!(err, EnvrError::Validation(_)));
    }
}
