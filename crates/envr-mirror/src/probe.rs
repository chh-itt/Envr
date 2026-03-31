use crate::registry::{Mirror, MirrorId, MirrorRegistry};
use crate::strategy::{ResolvedMirror, mirror_base_url};
use envr_config::settings::{MirrorMode, Settings};
use envr_error::{EnvrError, EnvrResult};
use envr_platform::paths::current_platform_paths;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeConfig {
    pub timeout_ms: u64,
    pub cache_ttl_ms: u64,
}

impl Default for ProbeConfig {
    fn default() -> Self {
        Self {
            timeout_ms: 1500,
            cache_ttl_ms: 5 * 60 * 1000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MirrorProbeResult {
    pub mirror_id: String,
    pub ok: bool,
    pub status: Option<u16>,
    pub latency_ms: Option<u64>,
    pub checked_at_epoch_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ProbeCache {
    pub results: HashMap<String, MirrorProbeResult>,
}

pub async fn resolve_mirror_auto(
    settings: &Settings,
    registry: &MirrorRegistry,
    client: &Client,
    config: &ProbeConfig,
) -> EnvrResult<ResolvedMirror> {
    if settings.mirror.mode != MirrorMode::Auto {
        return Err(EnvrError::Validation(
            "resolve_mirror_auto requires mirror.mode = auto".to_string(),
        ));
    }

    let cache_path = default_cache_path()?;
    let cache = load_cache_if_fresh(&cache_path, config.cache_ttl_ms).unwrap_or_default();

    // Probe all non-official mirrors first; fallback to official.
    let mut candidates = registry.list();
    candidates.retain(|m| !m.is_official);
    let mut best: Option<(Mirror, MirrorProbeResult)> = None;

    for m in candidates {
        let result = match cache.results.get(&m.id.0) {
            Some(r) => r.clone(),
            None => probe_mirror(client, m, config.timeout_ms).await?,
        };

        if result.ok {
            let better = match &best {
                None => true,
                Some((_, b)) => cmp_result(&result, b) == std::cmp::Ordering::Less,
            };
            if better {
                best = Some((m.clone(), result));
            }
        }
    }

    if let Some((m, r)) = best {
        let mut new_cache = cache;
        new_cache.results.insert(m.id.0.clone(), r);
        let _ = save_cache(&cache_path, &new_cache);
        return Ok(ResolvedMirror::Mirror(m));
    }

    // Fallback: official
    let official = registry
        .get(&MirrorId("official".to_string()))
        .cloned()
        .ok_or_else(|| EnvrError::Config("official mirror missing".to_string()))?;
    Ok(ResolvedMirror::Mirror(official))
}

pub async fn probe_mirror(
    client: &Client,
    mirror: &Mirror,
    timeout_ms: u64,
) -> EnvrResult<MirrorProbeResult> {
    let base = mirror_base_url(mirror)?;
    let url = base.clone();

    let start = tokio::time::Instant::now();
    let resp = client
        .head(url.clone())
        .timeout(Duration::from_millis(timeout_ms))
        .send()
        .await;

    let checked_at_epoch_ms = now_epoch_ms();

    match resp {
        Ok(r) => {
            let status = r.status();
            let latency_ms = start.elapsed().as_millis() as u64;
            let ok = status.is_success() || status == StatusCode::NOT_FOUND;
            Ok(MirrorProbeResult {
                mirror_id: mirror.id.0.clone(),
                ok,
                status: Some(status.as_u16()),
                latency_ms: Some(latency_ms),
                checked_at_epoch_ms,
            })
        }
        Err(_) => Ok(MirrorProbeResult {
            mirror_id: mirror.id.0.clone(),
            ok: false,
            status: None,
            latency_ms: None,
            checked_at_epoch_ms,
        }),
    }
}

fn cmp_result(a: &MirrorProbeResult, b: &MirrorProbeResult) -> std::cmp::Ordering {
    // ok already filtered; compare latency (lower better), missing treated as worst
    let la = a.latency_ms.unwrap_or(u64::MAX);
    let lb = b.latency_ms.unwrap_or(u64::MAX);
    la.cmp(&lb)
}

fn default_cache_path() -> EnvrResult<PathBuf> {
    let paths = current_platform_paths()?;
    Ok(paths.cache_dir.join("mirror-probe-cache.json"))
}

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn load_cache_if_fresh(path: &PathBuf, ttl_ms: u64) -> EnvrResult<ProbeCache> {
    let meta = std::fs::metadata(path).map_err(EnvrError::from)?;
    let modified = meta
        .modified()
        .map_err(|e| EnvrError::Io(std::io::Error::other(e)))?;
    let age_ms = SystemTime::now()
        .duration_since(modified)
        .unwrap_or_default()
        .as_millis() as u64;
    if age_ms > ttl_ms {
        return Err(EnvrError::Validation("probe cache expired".to_string()));
    }
    let content = std::fs::read_to_string(path).map_err(EnvrError::from)?;
    serde_json::from_str(&content)
        .map_err(|e| EnvrError::Config(format!("invalid probe cache json: {e}")))
}

fn save_cache(path: &PathBuf, cache: &ProbeCache) -> EnvrResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(EnvrError::from)?;
    }
    let content = serde_json::to_string_pretty(cache)
        .map_err(|e| EnvrError::Runtime(format!("serialize probe cache: {e}")))?;
    std::fs::write(path, content).map_err(EnvrError::from)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cmp_prefers_lower_latency() {
        let a = MirrorProbeResult {
            mirror_id: "a".to_string(),
            ok: true,
            status: Some(200),
            latency_ms: Some(10),
            checked_at_epoch_ms: 1,
        };
        let b = MirrorProbeResult {
            mirror_id: "b".to_string(),
            ok: true,
            status: Some(200),
            latency_ms: Some(20),
            checked_at_epoch_ms: 1,
        };
        assert_eq!(cmp_result(&a, &b), std::cmp::Ordering::Less);
    }
}
