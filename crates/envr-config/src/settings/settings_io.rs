use std::{fs, path::Path};

use envr_error::{EnvrError, EnvrResult};

use super::Settings;

pub(super) fn format_toml_settings_deser_error(content: &str, e: &toml::de::Error) -> String {
    match e.span() {
        Some(span) => {
            let start = span.start.min(content.len());
            let line = content[..start].bytes().filter(|&b| b == b'\n').count() + 1;
            format!("line {line}: {e}")
        }
        None => e.to_string(),
    }
}

/// Read `settings.toml` from disk, deserialize, and run [`Settings::validate`].
///
/// Fails on missing file, TOML/serde errors (with best-effort **line number**), or semantic validation.
pub fn validate_settings_file(path: impl AsRef<Path>) -> EnvrResult<()> {
    let path = path.as_ref();
    if !path.is_file() {
        return Err(EnvrError::Validation(format!(
            "not a file: {}",
            path.display()
        )));
    }
    let content = fs::read_to_string(path).map_err(EnvrError::from)?;
    let settings: Settings = toml::from_str(&content).map_err(|e| {
        EnvrError::Config(format!(
            "{}: {}",
            path.display(),
            format_toml_settings_deser_error(&content, &e)
        ))
    })?;
    settings.validate()?;
    Ok(())
}
