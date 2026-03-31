use crate::registry::{Mirror, MirrorId, MirrorRegistry};
use envr_config::settings::{MirrorMode, Settings};
use envr_error::{EnvrError, EnvrResult};
use reqwest::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedMirror {
    Offline,
    Mirror(Mirror),
}

pub fn resolve_mirror(
    settings: &Settings,
    registry: &MirrorRegistry,
) -> EnvrResult<ResolvedMirror> {
    match settings.mirror.mode {
        MirrorMode::Offline => Ok(ResolvedMirror::Offline),
        MirrorMode::Official => {
            let m = registry
                .get(&MirrorId("official".to_string()))
                .cloned()
                .ok_or_else(|| EnvrError::Config("official mirror missing".to_string()))?;
            Ok(ResolvedMirror::Mirror(m))
        }
        MirrorMode::Manual => {
            let id = settings
                .mirror
                .manual_id
                .as_deref()
                .ok_or_else(|| EnvrError::Validation("manual mirror id missing".to_string()))?;
            let m = registry
                .get(&MirrorId(id.to_string()))
                .cloned()
                .ok_or_else(|| EnvrError::Validation(format!("unknown mirror id: {id}")))?;
            Ok(ResolvedMirror::Mirror(m))
        }
        MirrorMode::Auto => {
            // T014 will implement probing; for now pick the first non-official preset if exists, otherwise official.
            let mut candidates = registry.list();
            candidates.retain(|m| !m.is_official);
            if let Some(m) = candidates.first() {
                return Ok(ResolvedMirror::Mirror((*m).clone()));
            }
            let m = registry
                .get(&MirrorId("official".to_string()))
                .cloned()
                .ok_or_else(|| EnvrError::Config("official mirror missing".to_string()))?;
            Ok(ResolvedMirror::Mirror(m))
        }
    }
}

pub fn join_url(base: &Url, path: &str) -> EnvrResult<Url> {
    if path.starts_with('/') {
        return Err(EnvrError::Validation(
            "path must be relative without leading slash".to_string(),
        ));
    }
    base.join(path)
        .map_err(|e| EnvrError::Validation(format!("failed to join url: {e}")))
}

pub fn mirror_base_url(mirror: &Mirror) -> EnvrResult<Url> {
    Url::parse(&mirror.base_url)
        .map_err(|e| EnvrError::Validation(format!("invalid mirror base_url: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::MirrorRegistry;

    #[test]
    fn official_mode_selects_official() {
        let reg = MirrorRegistry::with_presets().expect("presets");
        let settings = Settings {
            mirror: envr_config::settings::MirrorSettings {
                mode: MirrorMode::Official,
                manual_id: None,
            },
            ..Default::default()
        };
        let resolved = resolve_mirror(&settings, &reg).expect("resolve");
        match resolved {
            ResolvedMirror::Mirror(m) => assert!(m.is_official),
            ResolvedMirror::Offline => panic!("expected mirror"),
        }
    }

    #[test]
    fn manual_mode_requires_existing_id() {
        let reg = MirrorRegistry::with_presets().expect("presets");
        let settings = Settings {
            mirror: envr_config::settings::MirrorSettings {
                mode: MirrorMode::Manual,
                manual_id: Some("does-not-exist".to_string()),
            },
            ..Default::default()
        };
        let err = resolve_mirror(&settings, &reg).expect_err("should fail");
        assert!(matches!(err, EnvrError::Validation(_)));
    }

    #[test]
    fn join_url_rejects_absolute_paths() {
        let base = Url::parse("https://example.com/envr/").unwrap();
        let err = join_url(&base, "/abs").expect_err("reject");
        assert!(matches!(err, EnvrError::Validation(_)));
    }
}
