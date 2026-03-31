use envr_error::{EnvrError, EnvrResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MirrorId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mirror {
    pub id: MirrorId,
    pub name: String,
    pub base_url: String,
    pub is_official: bool,
}

#[derive(Debug, Default, Clone)]
pub struct MirrorRegistry {
    mirrors: HashMap<MirrorId, Mirror>,
}

impl MirrorRegistry {
    pub fn with_presets() -> EnvrResult<Self> {
        let mut reg = Self::default();

        // Official (placeholder base; runtime modules will append concrete paths later)
        reg.register(Mirror {
            id: MirrorId("official".to_string()),
            name: "Official".to_string(),
            base_url: "https://example.com/envr/".to_string(),
            is_official: true,
        })?;

        // Domestic presets (examples; can be replaced with real mirrors later)
        reg.register(Mirror {
            id: MirrorId("cn-1".to_string()),
            name: "CN Mirror 1".to_string(),
            base_url: "https://cn-mirror-1.example.com/envr/".to_string(),
            is_official: false,
        })?;
        reg.register(Mirror {
            id: MirrorId("cn-2".to_string()),
            name: "CN Mirror 2".to_string(),
            base_url: "https://cn-mirror-2.example.com/envr/".to_string(),
            is_official: false,
        })?;

        Ok(reg)
    }

    pub fn register(&mut self, mirror: Mirror) -> EnvrResult<()> {
        validate_mirror_url(&mirror.base_url)?;
        if self.mirrors.contains_key(&mirror.id) {
            return Err(EnvrError::Validation(format!(
                "mirror id already exists: {}",
                mirror.id.0
            )));
        }
        self.mirrors.insert(mirror.id.clone(), mirror);
        Ok(())
    }

    pub fn get(&self, id: &MirrorId) -> Option<&Mirror> {
        self.mirrors.get(id)
    }

    pub fn list(&self) -> Vec<&Mirror> {
        let mut v = self.mirrors.values().collect::<Vec<_>>();
        v.sort_by_key(|m| m.id.0.clone());
        v
    }
}

pub fn validate_mirror_url(url: &str) -> EnvrResult<()> {
    let parsed = reqwest::Url::parse(url)
        .map_err(|e| EnvrError::Validation(format!("invalid mirror url: {e}")))?;

    match parsed.scheme() {
        "https" | "http" => {}
        other => {
            return Err(EnvrError::Validation(format!(
                "unsupported mirror url scheme: {other}"
            )));
        }
    }
    if parsed.username() != "" || parsed.password().is_some() {
        return Err(EnvrError::Validation(
            "mirror url must not contain credentials".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_scheme() {
        let err = validate_mirror_url("file:///tmp/x").expect_err("should reject");
        assert!(matches!(err, EnvrError::Validation(_)));
    }

    #[test]
    fn presets_contain_official() {
        let reg = MirrorRegistry::with_presets().expect("presets");
        let official = reg
            .get(&MirrorId("official".to_string()))
            .expect("official");
        assert!(official.is_official);
    }
}
