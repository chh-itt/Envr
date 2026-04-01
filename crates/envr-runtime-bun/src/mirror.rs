use envr_config::settings::{Settings, settings_path_from_platform};
use envr_error::{EnvrError, EnvrResult};
use envr_mirror::registry::MirrorRegistry;
use envr_mirror::strategy::{ResolvedMirror, mirror_base_url, resolve_mirror};
use reqwest::Url;

pub fn load_settings() -> EnvrResult<Settings> {
    let platform = envr_platform::paths::current_platform_paths()?;
    let path = settings_path_from_platform(&platform);
    Settings::load_or_default_from(&path)
}

pub fn maybe_mirror_url(settings: &Settings, original: &str) -> EnvrResult<String> {
    let reg = MirrorRegistry::with_presets()?;
    match resolve_mirror(settings, &reg)? {
        ResolvedMirror::Offline => Err(EnvrError::Download(format!(
            "mirror.mode=offline: refusing network request to {original}"
        ))),
        ResolvedMirror::Mirror(m) if m.is_official => Ok(original.to_string()),
        ResolvedMirror::Mirror(m) => {
            // Generic proxy scheme: mirror base + "<host>/<path>?<query>"
            // This allows a mirror to implement simple HTTP reverse-proxying without runtime-specific rules.
            let u = Url::parse(original)
                .map_err(|e| EnvrError::Validation(format!("invalid url: {e}")))?;
            let base = mirror_base_url(&m)?;
            let host = u
                .host_str()
                .ok_or_else(|| EnvrError::Validation("url missing host".into()))?;
            let mut rel = format!("{host}{}", u.path());
            if let Some(q) = u.query() {
                rel.push('?');
                rel.push_str(q);
            }
            let joined = envr_mirror::strategy::join_url(&base, &rel)?;
            Ok(joined.to_string())
        }
    }
}
