use crate::registry::{Mirror, MirrorId, MirrorRegistry};
use crate::strategy::{ResolvedMirror, mirror_base_url};
use envr_config::settings::{MirrorMode, Settings};
use envr_error::{EnvrError, EnvrResult, ErrorCode};
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
        .map_err(|e| EnvrError::with_source(ErrorCode::Config, "invalid probe cache json", e))
}

fn save_cache(path: &PathBuf, cache: &ProbeCache) -> EnvrResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(EnvrError::from)?;
    }
    let content = serde_json::to_string_pretty(cache)
        .map_err(|e| EnvrError::with_source(ErrorCode::Runtime, "serialize probe cache", e))?;
    std::fs::write(path, content).map_err(EnvrError::from)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{Mirror, MirrorId, MirrorRegistry};
    use crate::strategy::ResolvedMirror;
    use envr_config::settings::{MirrorMode, MirrorSettings, Settings};
    use reqwest::Client;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    static ENVR_ROOT_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    struct EnvRestore {
        key: &'static str,
        had_value: bool,
        old: Option<std::ffi::OsString>,
    }

    impl EnvRestore {
        fn set(key: &'static str, value: &std::path::Path) -> Self {
            let old = std::env::var_os(key);
            let had_value = old.is_some();
            // SAFETY: `ENVR_ROOT_TEST_LOCK` serializes tests that touch this env var.
            unsafe {
                std::env::set_var(key, value.as_os_str());
            }
            Self {
                key,
                had_value,
                old,
            }
        }
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            // SAFETY: same as `EnvRestore::set`.
            unsafe {
                if self.had_value {
                    if let Some(v) = &self.old {
                        std::env::set_var(self.key, v);
                    }
                } else {
                    std::env::remove_var(self.key);
                }
            }
        }
    }

    #[test]
    fn probe_config_default_values() {
        let c = ProbeConfig::default();
        assert_eq!(c.timeout_ms, 1500);
        assert_eq!(c.cache_ttl_ms, 5 * 60 * 1000);
    }

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
        assert_eq!(cmp_result(&b, &a), std::cmp::Ordering::Greater);
        assert_eq!(cmp_result(&a, &a), std::cmp::Ordering::Equal);
    }

    #[test]
    fn cmp_treats_missing_latency_as_worst() {
        let slow = MirrorProbeResult {
            mirror_id: "x".to_string(),
            ok: true,
            status: Some(200),
            latency_ms: None,
            checked_at_epoch_ms: 1,
        };
        let fast = MirrorProbeResult {
            mirror_id: "y".to_string(),
            ok: true,
            status: Some(200),
            latency_ms: Some(5),
            checked_at_epoch_ms: 1,
        };
        assert_eq!(cmp_result(&slow, &fast), std::cmp::Ordering::Greater);
    }

    #[test]
    fn save_and_load_probe_cache_roundtrip() {
        let dir = tempfile::tempdir().expect("tmp");
        let path = dir.path().join("mirror-probe-cache.json");
        let mut cache = ProbeCache::default();
        cache.results.insert(
            "m1".to_string(),
            MirrorProbeResult {
                mirror_id: "m1".to_string(),
                ok: true,
                status: Some(200),
                latency_ms: Some(1),
                checked_at_epoch_ms: 42,
            },
        );
        save_cache(&path, &cache).expect("save");
        let loaded = load_cache_if_fresh(&path, u64::MAX).expect("load");
        assert_eq!(loaded.results.len(), 1);
        assert_eq!(loaded.results.get("m1").unwrap().checked_at_epoch_ms, 42);
    }

    #[test]
    fn load_probe_cache_rejects_invalid_json() {
        let dir = tempfile::tempdir().expect("tmp");
        let path = dir.path().join("bad.json");
        std::fs::write(&path, b"{ not json").expect("write");
        let err = load_cache_if_fresh(&path, u64::MAX).expect_err("bad json");
        assert_eq!(err.code(), ErrorCode::Config);
    }

    #[tokio::test]
    async fn probe_mirror_head_success_and_404_count_as_ok() {
        let server = MockServer::start().await;
        Mock::given(method("HEAD"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let mirror = Mirror {
            id: MirrorId("t".into()),
            name: "t".into(),
            base_url: format!("{}/", server.uri()),
            is_official: false,
        };
        let r = probe_mirror(&Client::new(), &mirror, 3000)
            .await
            .expect("probe");
        assert!(r.ok);
        assert_eq!(r.status, Some(200));

        let server404 = MockServer::start().await;
        Mock::given(method("HEAD"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server404)
            .await;
        let m404 = Mirror {
            id: MirrorId("n".into()),
            name: "n".into(),
            base_url: format!("{}/", server404.uri()),
            is_official: false,
        };
        let r404 = probe_mirror(&Client::new(), &m404, 3000)
            .await
            .expect("probe");
        assert!(r404.ok);
        assert_eq!(r404.status, Some(404));
    }

    #[tokio::test]
    async fn resolve_mirror_auto_requires_auto_mode() {
        let reg = MirrorRegistry::with_presets().expect("reg");
        let settings = Settings {
            mirror: MirrorSettings {
                mode: MirrorMode::Official,
                manual_id: None,
                prefer_china_mirrors: false,
            },
            ..Default::default()
        };
        let err = resolve_mirror_auto(&settings, &reg, &Client::new(), &ProbeConfig::default())
            .await
            .expect_err("not auto");
        assert!(matches!(err, EnvrError::Validation(_)));
    }

    #[tokio::test]
    async fn resolve_mirror_auto_picks_fastest_reachable_mirror() {
        let _lock = ENVR_ROOT_TEST_LOCK.lock().await;
        let tmp = tempfile::tempdir().expect("tmp");
        let _env = EnvRestore::set("ENVR_ROOT", tmp.path());

        let server = MockServer::start().await;
        Mock::given(method("HEAD"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let mut reg = MirrorRegistry::default();
        reg.register(Mirror {
            id: MirrorId("official".into()),
            name: "Official".into(),
            base_url: "https://example.invalid/envr/".into(),
            is_official: true,
        })
        .expect("official");
        reg.register(Mirror {
            id: MirrorId("local".into()),
            name: "Local".into(),
            base_url: format!("{}/", server.uri()),
            is_official: false,
        })
        .expect("local");

        let settings = Settings {
            mirror: MirrorSettings {
                mode: MirrorMode::Auto,
                manual_id: None,
                prefer_china_mirrors: false,
            },
            ..Default::default()
        };

        let resolved =
            resolve_mirror_auto(&settings, &reg, &Client::new(), &ProbeConfig::default())
                .await
                .expect("resolve");

        match resolved {
            ResolvedMirror::Mirror(m) => {
                assert_eq!(m.id.0, "local");
            }
            ResolvedMirror::Offline => panic!("expected mirror"),
        }
    }
}
