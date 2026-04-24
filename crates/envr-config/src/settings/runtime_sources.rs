use envr_error::EnvrResult;

use super::{
    DENO_NPMIRROR_BINARY_BASE, DenoDownloadSource, GET_PIP_URL_DOMESTIC, GET_PIP_URL_OFFICIAL,
    JSR_REGISTRY_DOMESTIC, JSR_REGISTRY_OFFICIAL, NODE_INDEX_JSON_DOMESTIC,
    NODE_INDEX_JSON_OFFICIAL, NPM_REGISTRY_DOMESTIC, NPM_REGISTRY_OFFICIAL, NpmRegistryMode,
    PHP_WINDOWS_RELEASES_JSON_DOMESTIC, PHP_WINDOWS_RELEASES_JSON_OFFICIAL, PIP_INDEX_DOMESTIC,
    PIP_INDEX_DOMESTIC_FALLBACK, PIP_INDEX_OFFICIAL, PYTHON_FTP_DOMESTIC, PYTHON_FTP_OFFICIAL,
    PhpDownloadSource, PipRegistryMode, PythonDownloadSource, Settings, prefer_china_mirrors,
};

pub fn node_index_json_url(settings: &Settings) -> String {
    if prefer_domestic_source(
        settings,
        matches!(
            settings.runtime.node.download_source,
            super::NodeDownloadSource::Domestic
        ),
        matches!(
            settings.runtime.node.download_source,
            super::NodeDownloadSource::Auto
        ),
    ) {
        NODE_INDEX_JSON_DOMESTIC.to_string()
    } else {
        NODE_INDEX_JSON_OFFICIAL.to_string()
    }
}

/// Resolved registry URL to pass to `npm config set registry`, or `None` for [`NpmRegistryMode::Restore`].
pub fn npm_registry_url_to_apply(settings: &Settings) -> Option<&'static str> {
    match settings.runtime.node.npm_registry_mode {
        NpmRegistryMode::Restore => None,
        NpmRegistryMode::Official => Some(NPM_REGISTRY_OFFICIAL),
        NpmRegistryMode::Domestic => Some(NPM_REGISTRY_DOMESTIC),
        NpmRegistryMode::Custom => None,
        NpmRegistryMode::Auto => Some(if prefer_china_mirrors(settings) {
            NPM_REGISTRY_DOMESTIC
        } else {
            NPM_REGISTRY_OFFICIAL
        }),
    }
}

/// Owned version: supports `custom` URLs.
pub fn npm_registry_url_to_apply_owned(settings: &Settings) -> Option<String> {
    match settings.runtime.node.npm_registry_mode {
        NpmRegistryMode::Restore => None,
        NpmRegistryMode::Official => Some(NPM_REGISTRY_OFFICIAL.to_string()),
        NpmRegistryMode::Domestic => Some(NPM_REGISTRY_DOMESTIC.to_string()),
        NpmRegistryMode::Auto => Some(
            if prefer_china_mirrors(settings) {
                NPM_REGISTRY_DOMESTIC
            } else {
                NPM_REGISTRY_OFFICIAL
            }
            .to_string(),
        ),
        NpmRegistryMode::Custom => settings
            .runtime
            .node
            .npm_registry_url_custom
            .as_deref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string()),
    }
}

fn deno_host_tuple() -> EnvrResult<&'static str> {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    match (os, arch) {
        ("windows", "x86_64") => Ok("x86_64-pc-windows-msvc"),
        ("windows", "aarch64") => Ok("aarch64-pc-windows-msvc"),
        ("linux", "x86_64") => Ok("x86_64-unknown-linux-gnu"),
        ("linux", "aarch64") => Ok("aarch64-unknown-linux-gnu"),
        ("macos", "x86_64") => Ok("x86_64-apple-darwin"),
        ("macos", "aarch64") => Ok("aarch64-apple-darwin"),
        _ => Err(envr_error::EnvrError::Platform(format!(
            "unsupported host for deno install: {os}-{arch}"
        ))),
    }
}

/// Resolved Deno release zip URL (official `dl.deno.land` vs npmmirror binary mirror).
pub fn deno_release_zip_url(settings: &Settings, version: &str) -> EnvrResult<String> {
    let tuple = deno_host_tuple()?;
    let prefer_domestic = prefer_domestic_source(
        settings,
        matches!(
            settings.runtime.deno.download_source,
            DenoDownloadSource::Domestic
        ),
        matches!(
            settings.runtime.deno.download_source,
            DenoDownloadSource::Auto
        ),
    );
    if prefer_domestic {
        Ok(format!(
            "{DENO_NPMIRROR_BINARY_BASE}/v{version}/deno-{tuple}.zip"
        ))
    } else {
        Ok(format!(
            "https://dl.deno.land/release/v{version}/deno-{tuple}.zip"
        ))
    }
}

/// Official Deno release zip URL (always `dl.deno.land`).
pub fn deno_official_release_zip_url(version: &str) -> EnvrResult<String> {
    let tuple = deno_host_tuple()?;
    Ok(format!(
        "https://dl.deno.land/release/v{version}/deno-{tuple}.zip"
    ))
}

/// `NPM_CONFIG_REGISTRY` and `JSR_URL` for managed Deno child processes. Empty when
/// [`NpmRegistryMode::Restore`] (do not override user environment).
pub fn deno_package_registry_env(settings: &Settings) -> Vec<(String, String)> {
    match settings.runtime.deno.package_source {
        NpmRegistryMode::Restore => vec![],
        NpmRegistryMode::Official => vec![
            (
                "NPM_CONFIG_REGISTRY".into(),
                NPM_REGISTRY_OFFICIAL.to_string(),
            ),
            ("JSR_URL".into(), JSR_REGISTRY_OFFICIAL.to_string()),
        ],
        NpmRegistryMode::Domestic => vec![
            (
                "NPM_CONFIG_REGISTRY".into(),
                NPM_REGISTRY_DOMESTIC.to_string(),
            ),
            ("JSR_URL".into(), JSR_REGISTRY_DOMESTIC.to_string()),
        ],
        NpmRegistryMode::Auto => {
            if prefer_china_mirrors(settings) {
                vec![
                    (
                        "NPM_CONFIG_REGISTRY".into(),
                        NPM_REGISTRY_DOMESTIC.to_string(),
                    ),
                    ("JSR_URL".into(), JSR_REGISTRY_DOMESTIC.to_string()),
                ]
            } else {
                vec![
                    (
                        "NPM_CONFIG_REGISTRY".into(),
                        NPM_REGISTRY_OFFICIAL.to_string(),
                    ),
                    ("JSR_URL".into(), JSR_REGISTRY_OFFICIAL.to_string()),
                ]
            }
        }
        // Not supported for Deno yet; fall back to official.
        NpmRegistryMode::Custom => vec![
            (
                "NPM_CONFIG_REGISTRY".into(),
                NPM_REGISTRY_OFFICIAL.to_string(),
            ),
            ("JSR_URL".into(), JSR_REGISTRY_OFFICIAL.to_string()),
        ],
    }
}

/// `NPM_CONFIG_REGISTRY` for managed Bun child processes. Empty when
/// [`NpmRegistryMode::Restore`] (do not override user environment).
pub fn bun_package_registry_env(settings: &Settings) -> Vec<(String, String)> {
    let npm = match settings.runtime.bun.package_source {
        NpmRegistryMode::Restore => return vec![],
        NpmRegistryMode::Official => NPM_REGISTRY_OFFICIAL,
        NpmRegistryMode::Domestic => NPM_REGISTRY_DOMESTIC,
        NpmRegistryMode::Auto => {
            if prefer_china_mirrors(settings) {
                NPM_REGISTRY_DOMESTIC
            } else {
                NPM_REGISTRY_OFFICIAL
            }
        }
        // Not supported for Bun yet; fall back to official.
        NpmRegistryMode::Custom => NPM_REGISTRY_OFFICIAL,
    };
    vec![("NPM_CONFIG_REGISTRY".into(), npm.to_string())]
}

pub fn python_get_pip_url(settings: &Settings) -> &'static str {
    if prefer_domestic_source(
        settings,
        matches!(
            settings.runtime.python.download_source,
            PythonDownloadSource::Domestic
        ),
        matches!(
            settings.runtime.python.download_source,
            PythonDownloadSource::Auto
        ),
    ) {
        GET_PIP_URL_DOMESTIC
    } else {
        GET_PIP_URL_OFFICIAL
    }
}

/// Candidate download URLs for Python artifacts (first is preferred, later entries are fallbacks).
///
/// `original_url` usually comes from python.org release APIs. In `auto` / `domestic`, when the URL
/// is under official Python FTP, a TUNA mirror URL is prepended and official is kept as fallback.
pub fn python_download_url_candidates(settings: &Settings, original_url: &str) -> Vec<String> {
    let prefer_domestic = prefer_domestic_source(
        settings,
        matches!(
            settings.runtime.python.download_source,
            PythonDownloadSource::Domestic
        ),
        matches!(
            settings.runtime.python.download_source,
            PythonDownloadSource::Auto
        ),
    );
    if !prefer_domestic {
        return vec![original_url.to_string()];
    }
    if let Some(rest) = original_url.strip_prefix(PYTHON_FTP_OFFICIAL) {
        return vec![
            format!("{PYTHON_FTP_DOMESTIC}{rest}"),
            original_url.to_string(),
        ];
    }
    vec![original_url.to_string()]
}

/// Resolved `pip` index URL for bootstrap `get-pip.py`, or `None` to keep interpreter defaults.
pub fn pip_registry_url_for_bootstrap(settings: &Settings) -> Option<&'static str> {
    pip_registry_urls_for_bootstrap(settings).into_iter().next()
}

/// Candidate `pip` index URLs (ordered) for bootstrap and runtime-managed pip config.
pub fn pip_registry_urls_for_bootstrap(settings: &Settings) -> Vec<&'static str> {
    match settings.runtime.python.pip_registry_mode {
        PipRegistryMode::Restore => vec![],
        PipRegistryMode::Official => vec![PIP_INDEX_OFFICIAL],
        PipRegistryMode::Domestic => vec![
            PIP_INDEX_DOMESTIC,
            PIP_INDEX_DOMESTIC_FALLBACK,
            PIP_INDEX_OFFICIAL,
        ],
        PipRegistryMode::Custom => vec![PIP_INDEX_OFFICIAL],
        PipRegistryMode::Auto => {
            if prefer_china_mirrors(settings) {
                vec![
                    PIP_INDEX_DOMESTIC,
                    PIP_INDEX_DOMESTIC_FALLBACK,
                    PIP_INDEX_OFFICIAL,
                ]
            } else {
                vec![PIP_INDEX_OFFICIAL]
            }
        }
    }
}

/// Owned version: supports `custom` URLs.
pub fn pip_index_url_for_bootstrap_owned(settings: &Settings) -> Option<String> {
    match settings.runtime.python.pip_registry_mode {
        PipRegistryMode::Restore => None,
        PipRegistryMode::Official => Some(PIP_INDEX_OFFICIAL.to_string()),
        PipRegistryMode::Domestic => Some(PIP_INDEX_DOMESTIC.to_string()),
        PipRegistryMode::Auto => Some(
            if prefer_china_mirrors(settings) {
                PIP_INDEX_DOMESTIC
            } else {
                PIP_INDEX_OFFICIAL
            }
            .to_string(),
        ),
        PipRegistryMode::Custom => settings
            .runtime
            .python
            .pip_index_url_custom
            .as_deref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string()),
    }
}

pub fn php_windows_releases_json_url(settings: &Settings) -> &'static str {
    if prefer_domestic_source(
        settings,
        matches!(
            settings.runtime.php.download_source,
            PhpDownloadSource::Domestic
        ),
        matches!(
            settings.runtime.php.download_source,
            PhpDownloadSource::Auto
        ),
    ) {
        PHP_WINDOWS_RELEASES_JSON_DOMESTIC
    } else {
        PHP_WINDOWS_RELEASES_JSON_OFFICIAL
    }
}

pub(super) fn prefer_domestic_source(
    settings: &Settings,
    explicit_domestic: bool,
    is_auto: bool,
) -> bool {
    explicit_domestic || (is_auto && prefer_china_mirrors(settings))
}
