//! Shared helpers for “installed runtime home” probing (candidate paths, first existing file).

use std::path::PathBuf;

pub fn first_existing_path(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates.iter().find(|p| p.is_file()).cloned()
}
