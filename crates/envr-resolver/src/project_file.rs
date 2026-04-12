use crate::pin_spec::{RuntimePinSpec, runtime_kind_toml_key};
use envr_config::project_config::{
    ProjectConfig, PROJECT_CONFIG_FILE, parse_project_config, save_project_config,
};
use envr_error::{EnvrError, EnvrResult};
use std::path::{Path, PathBuf};

/// Create or update `./.envr.toml` under `dir` with `[runtimes.<kind>].version`.
pub fn upsert_runtime_pin(dir: &Path, pin: &RuntimePinSpec) -> EnvrResult<PathBuf> {
    let path = dir.join(PROJECT_CONFIG_FILE);
    let mut cfg = if path.exists() {
        parse_project_config(&path).map_err(|e| {
            EnvrError::Config(format!(
                "read {}: {e}",
                path.display()
            ))
        })?
    } else {
        ProjectConfig::default()
    };

    let key = runtime_kind_toml_key(pin.kind).to_string();
    let rt = cfg.runtimes.entry(key).or_default();
    rt.version = Some(pin.version.clone());

    save_project_config(&path, &cfg)?;
    Ok(path)
}
