use envr_error::{EnvrError, EnvrResult, ErrorCode};
use envr_platform::paths::EnvrPaths;
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
    time::SystemTime,
};

mod defaults;
mod general_config;
mod locale_mirror;
mod runtime_disk_flags;
mod runtime_js_py_go;
mod runtime_jvm;
mod runtime_lang_core;
mod runtime_long_tail;
mod runtime_sources;
mod runtime_web_tooling;
mod ui_config;
mod validation;
pub use general_config::{
    BehaviorSettings, DownloadSettings, MirrorMode, MirrorSettings, PathSettings,
};
pub use locale_mirror::{
    prefer_china_mirror_locale, prefer_china_mirrors, system_locale_suggests_chinese,
};
pub use runtime_disk_flags::{
    bun_path_proxy_enabled_from_disk, crystal_path_proxy_enabled_from_disk,
    dart_path_proxy_enabled_from_disk, deno_path_proxy_enabled_from_disk,
    dotnet_path_proxy_enabled_from_disk, elixir_path_proxy_enabled_from_disk,
    erlang_path_proxy_enabled_from_disk, flutter_path_proxy_enabled_from_disk,
    go_path_proxy_enabled_from_disk, java_path_proxy_enabled_from_disk,
    julia_path_proxy_enabled_from_disk, lua_path_proxy_enabled_from_disk,
    nim_path_proxy_enabled_from_disk, node_path_proxy_enabled_from_disk,
    perl_path_proxy_enabled_from_disk, php_path_proxy_enabled_from_disk,
    php_windows_build_want_ts_from_disk, python_path_proxy_enabled_from_disk,
    rlang_path_proxy_enabled_from_disk, ruby_path_proxy_enabled_from_disk,
    v_path_proxy_enabled_from_disk, zig_path_proxy_enabled_from_disk,
};
pub use runtime_js_py_go::{
    GoDownloadSource, GoProxyMode, GoRuntimeSettings, NodeDownloadSource, NodeRuntimeSettings,
    NpmRegistryMode, PipRegistryMode, PythonDownloadSource, PythonRuntimeSettings,
    PythonWindowsDistribution,
};
pub use runtime_jvm::{
    ClojureRuntimeSettings, GroovyRuntimeSettings, JavaRuntimeSettings, KotlinRuntimeSettings,
    ScalaRuntimeSettings,
};
pub use runtime_lang_core::{
    ElixirRuntimeSettings, ErlangRuntimeSettings, RubyRuntimeSettings, RustRuntimeSettings,
};
pub use runtime_long_tail::{
    BabashkaRuntimeSettings, C3RuntimeSettings, CrystalRuntimeSettings, HaxeRuntimeSettings,
    JanetRuntimeSettings, JuliaRuntimeSettings, LuaRuntimeSettings, NimRuntimeSettings,
    PerlRuntimeSettings, RlangRuntimeSettings, SbclRuntimeSettings, UnisonRuntimeSettings,
    ZigRuntimeSettings,
};
pub use runtime_sources::{
    bun_package_registry_env, deno_official_release_zip_url, deno_package_registry_env,
    deno_release_zip_url, node_index_json_url, npm_registry_url_to_apply,
    npm_registry_url_to_apply_owned, php_windows_releases_json_url,
    pip_index_url_for_bootstrap_owned, pip_registry_url_for_bootstrap,
    pip_registry_urls_for_bootstrap, python_download_url_candidates, python_get_pip_url,
};
pub use runtime_web_tooling::{
    BunRuntimeSettings, DenoDownloadSource, DenoRuntimeSettings, DotnetRuntimeSettings,
    PhpDownloadSource, PhpRuntimeSettings, PhpWindowsBuildFlavor,
};
pub use ui_config::{
    AppearanceSettings, DownloadsPanelSettings, FontMode, FontSettings, GuiSettings, I18nSettings,
    LocaleMode, RuntimeLayoutSettings, ThemeMode,
};
use validation::{validate_core_settings, validate_runtime_settings};

/// Rust toolchain download source preference for `rustup` (`RUSTUP_DIST_SERVER` / `RUSTUP_UPDATE_ROOT`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RustDownloadSource {
    /// Prefer domestic mirror when UI locale suggests China; otherwise official.
    #[default]
    Auto,
    Domestic,
    Official,
}

/// Java distribution choice for runtime installation and listing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum JavaDistro {
    #[default]
    Temurin,
    OracleOpenJdk,
    AmazonCorretto,
    Microsoft,
    OracleJdk,
    /// Azul Zulu builds.
    #[serde(alias = "zulu")]
    #[serde(alias = "azul_zulu")]
    AzulZulu,
    /// Alibaba Dragonwell builds.
    #[serde(alias = "dragonwell")]
    #[serde(alias = "alibaba_dragonwell")]
    AlibabaDragonwell,
    /// Backward compatibility for older settings values (maps to Temurin in runtime).
    #[serde(alias = "open_jdk")]
    OpenJdk,
}

impl JavaDistro {
    /// LTS majors offered in the GUI for this distribution (newest first).
    ///
    /// Kept in sync with install-time checks in `envr-runtime-java`.
    pub fn supported_lts_major_strs(self) -> &'static [&'static str] {
        match self {
            JavaDistro::Temurin | JavaDistro::OpenJdk => &["25", "21", "17", "11", "8"],
            JavaDistro::OracleOpenJdk => &["25", "21", "17"],
            JavaDistro::AmazonCorretto => &["21", "17", "11", "8"],
            JavaDistro::Microsoft => &["25", "21", "17", "11"],
            JavaDistro::OracleJdk => &["25", "21"],
            JavaDistro::AzulZulu | JavaDistro::AlibabaDragonwell => &["25", "21", "17", "11", "8"],
        }
    }
}

/// Java artifact source preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum JavaDownloadSource {
    #[default]
    Auto,
    Domestic,
    Official,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerraformRuntimeSettings {
    /// When false, terraform shim resolves to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::terraform_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for TerraformRuntimeSettings {
    fn default() -> Self {
        Self {
            path_proxy_enabled: defaults::terraform_path_proxy_enabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VRuntimeSettings {
    /// When false, v shim resolves to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::v_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for VRuntimeSettings {
    fn default() -> Self {
        Self {
            path_proxy_enabled: defaults::v_path_proxy_enabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OdinRuntimeSettings {
    /// When false, odin shim resolves to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::odin_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for OdinRuntimeSettings {
    fn default() -> Self {
        Self {
            path_proxy_enabled: defaults::odin_path_proxy_enabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PurescriptRuntimeSettings {
    /// When false, purs shim resolves to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::purescript_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for PurescriptRuntimeSettings {
    fn default() -> Self {
        Self {
            path_proxy_enabled: defaults::purescript_path_proxy_enabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElmRuntimeSettings {
    /// When false, elm shim resolves to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::elm_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for ElmRuntimeSettings {
    fn default() -> Self {
        Self {
            path_proxy_enabled: defaults::elm_path_proxy_enabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GleamRuntimeSettings {
    /// When false, gleam shim resolves to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::gleam_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for GleamRuntimeSettings {
    fn default() -> Self {
        Self {
            path_proxy_enabled: defaults::gleam_path_proxy_enabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RacketRuntimeSettings {
    /// When false, racket/raco shims resolve to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::racket_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for RacketRuntimeSettings {
    fn default() -> Self {
        Self {
            path_proxy_enabled: defaults::racket_path_proxy_enabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DartRuntimeSettings {
    /// When false, dart shim resolves to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::dart_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for DartRuntimeSettings {
    fn default() -> Self {
        Self {
            path_proxy_enabled: defaults::dart_path_proxy_enabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlutterRuntimeSettings {
    /// When false, flutter shim resolves to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::flutter_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for FlutterRuntimeSettings {
    fn default() -> Self {
        Self {
            path_proxy_enabled: defaults::flutter_path_proxy_enabled(),
        }
    }
}

/// Official Node `index.json` URL.
pub const NODE_INDEX_JSON_OFFICIAL: &str = "https://nodejs.org/dist/index.json";
/// Common China mirror (npmmirror) `index.json`.
pub const NODE_INDEX_JSON_DOMESTIC: &str = "https://npmmirror.com/mirrors/node/index.json";

pub const NPM_REGISTRY_OFFICIAL: &str = "https://registry.npmjs.org/";
pub const NPM_REGISTRY_DOMESTIC: &str = "https://registry.npmmirror.com/";
pub const PYTHON_FTP_OFFICIAL: &str = "https://www.python.org/ftp/python/";
pub const PYTHON_FTP_DOMESTIC: &str = "https://mirrors.tuna.tsinghua.edu.cn/python/";
pub const GET_PIP_URL_OFFICIAL: &str = "https://bootstrap.pypa.io/get-pip.py";
pub const GET_PIP_URL_DOMESTIC: &str = "https://mirrors.aliyun.com/pypi/get-pip.py";
pub const PIP_INDEX_OFFICIAL: &str = "https://pypi.org/simple";
pub const PIP_INDEX_DOMESTIC: &str = "https://mirrors.aliyun.com/pypi/simple";
pub const PIP_INDEX_DOMESTIC_FALLBACK: &str = "https://pypi.tuna.tsinghua.edu.cn/simple";
pub const PHP_WINDOWS_RELEASES_JSON_OFFICIAL: &str =
    "https://downloads.php.net/~windows/releases/releases.json";
// Placeholder mirror URL for MVP; can be updated after mirror validation.
pub const PHP_WINDOWS_RELEASES_JSON_DOMESTIC: &str =
    "https://downloads.php.net/~windows/releases/releases.json";

/// npmmirror binary mirror for Deno release zips (`deno-{tuple}.zip` under `v{version}/`).
pub const DENO_NPMIRROR_BINARY_BASE: &str = "https://registry.npmmirror.com/-/binary/deno";

/// Default JSR origin (Deno reads `JSR_URL` in supported versions).
pub const JSR_REGISTRY_OFFICIAL: &str = "https://jsr.io/";
/// Domestic JSR: no widely agreed mirror yet; keep official until validated.
pub const JSR_REGISTRY_DOMESTIC: &str = "https://jsr.io/";

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RuntimeSettings {
    #[serde(default)]
    pub node: NodeRuntimeSettings,
    #[serde(default)]
    pub python: PythonRuntimeSettings,
    #[serde(default)]
    pub java: JavaRuntimeSettings,
    #[serde(default)]
    pub kotlin: KotlinRuntimeSettings,
    #[serde(default)]
    pub scala: ScalaRuntimeSettings,
    #[serde(default)]
    pub clojure: ClojureRuntimeSettings,
    #[serde(default)]
    pub groovy: GroovyRuntimeSettings,
    #[serde(default)]
    pub terraform: TerraformRuntimeSettings,
    #[serde(default)]
    pub v: VRuntimeSettings,
    #[serde(default)]
    pub odin: OdinRuntimeSettings,
    #[serde(default)]
    pub purescript: PurescriptRuntimeSettings,
    #[serde(default)]
    pub elm: ElmRuntimeSettings,
    #[serde(default)]
    pub gleam: GleamRuntimeSettings,
    #[serde(default)]
    pub racket: RacketRuntimeSettings,
    #[serde(default)]
    pub dart: DartRuntimeSettings,
    #[serde(default)]
    pub flutter: FlutterRuntimeSettings,
    #[serde(default)]
    pub go: GoRuntimeSettings,
    #[serde(default)]
    pub rust: RustRuntimeSettings,
    #[serde(default)]
    pub ruby: RubyRuntimeSettings,
    #[serde(default)]
    pub elixir: ElixirRuntimeSettings,
    #[serde(default)]
    pub erlang: ErlangRuntimeSettings,
    #[serde(default)]
    pub php: PhpRuntimeSettings,
    #[serde(default)]
    pub deno: DenoRuntimeSettings,
    #[serde(default)]
    pub bun: BunRuntimeSettings,
    #[serde(default)]
    pub dotnet: DotnetRuntimeSettings,

    #[serde(default)]
    pub zig: ZigRuntimeSettings,

    #[serde(default)]
    pub julia: JuliaRuntimeSettings,

    #[serde(default)]
    pub janet: JanetRuntimeSettings,

    #[serde(default)]
    pub c3: C3RuntimeSettings,

    #[serde(default)]
    pub babashka: BabashkaRuntimeSettings,

    #[serde(default)]
    pub sbcl: SbclRuntimeSettings,

    #[serde(default)]
    pub haxe: HaxeRuntimeSettings,

    #[serde(default)]
    pub lua: LuaRuntimeSettings,

    #[serde(default)]
    pub nim: NimRuntimeSettings,

    #[serde(default)]
    pub crystal: CrystalRuntimeSettings,

    #[serde(default)]
    pub perl: PerlRuntimeSettings,

    #[serde(default)]
    pub unison: UnisonRuntimeSettings,

    #[serde(default)]
    pub r: RlangRuntimeSettings,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub paths: PathSettings,

    #[serde(default)]
    pub behavior: BehaviorSettings,

    #[serde(default)]
    pub appearance: AppearanceSettings,

    #[serde(default)]
    pub gui: GuiSettings,

    #[serde(default)]
    pub download: DownloadSettings,

    #[serde(default)]
    pub mirror: MirrorSettings,

    #[serde(default)]
    pub i18n: I18nSettings,

    #[serde(default)]
    pub runtime: RuntimeSettings,
}

impl Settings {
    pub fn validate(&self) -> EnvrResult<()> {
        validate_core_settings(self)?;
        validate_runtime_settings(&self.runtime)?;

        Ok(())
    }

    pub fn load_from(path: impl AsRef<Path>) -> EnvrResult<Self> {
        let path = path.as_ref();
        let content = fs::read_to_string(path).map_err(EnvrError::from)?;
        let settings: Settings = toml::from_str(&content).map_err(|err| {
            EnvrError::Config(format!(
                "failed to parse {}: {}",
                path.display(),
                format_toml_settings_deser_error(&content, &err)
            ))
        })?;
        settings.validate()?;
        Ok(settings)
    }

    pub fn load_or_default_from(path: impl AsRef<Path>) -> EnvrResult<Self> {
        let path = path.as_ref().to_path_buf();
        let mtime = fs::metadata(&path).ok().and_then(|m| m.modified().ok());

        if let Some(s) = SETTINGS_FILE_CACHE.with(|c| {
            c.borrow().get(&path).and_then(
                |(m2, s)| {
                    if m2 == &mtime { Some(s.clone()) } else { None }
                },
            )
        }) {
            return Ok(s);
        }

        let loaded: Settings = match Self::load_from(&path) {
            Ok(v) => v,
            Err(_err) => {
                if path.exists() {
                    let _ = backup_corrupted_file(&path);
                }
                let defaults = Settings::default();
                defaults.validate()?;
                defaults
            }
        };

        SETTINGS_FILE_CACHE.with(|c| {
            c.borrow_mut().insert(path, (mtime, loaded.clone()));
        });
        Ok(loaded)
    }

    pub fn save_to(&self, path: impl AsRef<Path>) -> EnvrResult<()> {
        self.validate()?;

        let path = path.as_ref();
        let content = toml::to_string_pretty(self)
            .map_err(|e| EnvrError::with_source(ErrorCode::Runtime, "toml encode settings", e))?;
        envr_platform::fs_atomic::write_atomic_with_backup(path, content.as_bytes(), "bak")
            .map_err(EnvrError::from)?;
        let pb = path.to_path_buf();
        SETTINGS_FILE_CACHE.with(|c| {
            c.borrow_mut().remove(&pb);
        });
        RESOLVE_RUNTIME_ROOT_CACHE.with(|c| *c.borrow_mut() = None);
        Ok(())
    }
}

thread_local! {
    static SETTINGS_FILE_CACHE: RefCell<HashMap<PathBuf, (Option<SystemTime>, Settings)>> =
        RefCell::new(HashMap::new());
}

thread_local! {
    static RESOLVE_RUNTIME_ROOT_CACHE: RefCell<Option<(PathBuf, Option<SystemTime>, PathBuf)>> =
        const { RefCell::new(None) };
}

static PROCESS_RUNTIME_ROOT_OVERRIDE: OnceLock<PathBuf> = OnceLock::new();

/// Set a process-wide runtime root override (preferred over `ENVR_RUNTIME_ROOT` and `settings.toml`).
///
/// Intended for early startup configuration (CLI global `--runtime-root`) without mutating the
/// process environment.
///
/// Returns `true` when the override was set by this call; `false` when it was already set.
pub fn set_process_runtime_root_override(path: PathBuf) -> bool {
    let trimmed = path.to_string_lossy().trim().to_string();
    if trimmed.is_empty() {
        return false;
    }
    PROCESS_RUNTIME_ROOT_OVERRIDE
        .set(PathBuf::from(trimmed))
        .is_ok()
}

pub fn process_runtime_root_override() -> Option<&'static PathBuf> {
    PROCESS_RUNTIME_ROOT_OVERRIDE.get()
}

/// Clears in-process caches for [`Settings::load_or_default_from`] and [`resolve_runtime_root`].
pub fn reset_settings_load_caches() {
    SETTINGS_FILE_CACHE.with(|c| c.borrow_mut().clear());
    RESOLVE_RUNTIME_ROOT_CACHE.with(|c| *c.borrow_mut() = None);
}

fn format_toml_settings_deser_error(content: &str, e: &toml::de::Error) -> String {
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

/// Returns `RUSTUP_DIST_SERVER` when a non-official mirror is selected, otherwise `None`.
pub fn rustup_dist_server_from_settings(s: &Settings) -> Option<String> {
    if prefer_domestic_source(
        s,
        matches!(s.runtime.rust.download_source, RustDownloadSource::Domestic),
        matches!(s.runtime.rust.download_source, RustDownloadSource::Auto),
    ) {
        Some("https://mirrors.ustc.edu.cn/rust-static".to_string())
    } else {
        None
    }
}

/// Returns `RUSTUP_UPDATE_ROOT` when a non-official mirror is selected, otherwise `None`.
pub fn rustup_update_root_from_settings(s: &Settings) -> Option<String> {
    if prefer_domestic_source(
        s,
        matches!(s.runtime.rust.download_source, RustDownloadSource::Domestic),
        matches!(s.runtime.rust.download_source, RustDownloadSource::Auto),
    ) {
        Some("https://mirrors.ustc.edu.cn/rust-static/rustup".to_string())
    } else {
        None
    }
}

pub struct SettingsCache {
    path: PathBuf,
    cached: Settings,
    last_modified: Option<SystemTime>,
}

impl SettingsCache {
    pub fn new(path: impl Into<PathBuf>) -> EnvrResult<Self> {
        let path = path.into();
        let cached = Settings::load_or_default_from(&path)?;
        let last_modified = file_mtime(&path).ok();
        Ok(Self {
            path,
            cached,
            last_modified,
        })
    }

    pub fn get(&mut self) -> EnvrResult<&Settings> {
        let mtime = file_mtime(&self.path).ok();
        if mtime != self.last_modified {
            self.cached = Settings::load_or_default_from(&self.path)?;
            self.last_modified = mtime;
        }
        Ok(&self.cached)
    }

    /// Reread `settings.toml` from disk even when mtime is unchanged (e.g. after external CLI edit in same second).
    pub fn reload(&mut self) -> EnvrResult<&Settings> {
        self.cached = Settings::load_or_default_from(&self.path)?;
        self.last_modified = file_mtime(&self.path).ok();
        Ok(&self.cached)
    }

    pub fn set_and_persist(&mut self, settings: Settings) -> EnvrResult<()> {
        settings.save_to(&self.path)?;
        self.cached = settings;
        self.last_modified = file_mtime(&self.path).ok();
        Ok(())
    }

    /// Replace cached settings without any disk I/O.
    ///
    /// Useful for GUI async flows where the settings were already loaded/saved
    /// off the UI thread.
    pub fn set_cached(&mut self, settings: Settings) {
        self.cached = settings;
        // Keep mtime tracking consistent so `get()` can stay in-memory unless disk changed.
        self.last_modified = file_mtime(&self.path).ok();
    }

    /// In-memory settings (last load / [`Self::set_cached`] / [`Self::reload`]).
    ///
    /// Prefer this over [`Self::get`] when syncing UI immediately after [`Self::set_cached`]:
    /// `get()` may re-read disk if mtime differs slightly; a failed parse would replace the cache
    /// with defaults and wipe fields like `paths.runtime_root`.
    pub fn snapshot(&self) -> &Settings {
        &self.cached
    }
}

pub fn settings_path_from_platform(paths: &EnvrPaths) -> PathBuf {
    paths.settings_file.clone()
}

/// Effective runtime data root: `ENVR_RUNTIME_ROOT` wins, then `settings.toml` `paths.runtime_root`,
/// then the platform default (`EnvrPaths::runtime_root`).
pub fn resolve_runtime_root() -> EnvrResult<PathBuf> {
    if let Some(p) = process_runtime_root_override() {
        return Ok(p.clone());
    }
    if let Ok(p) = std::env::var("ENVR_RUNTIME_ROOT") {
        let t = p.trim();
        if !t.is_empty() {
            return Ok(PathBuf::from(t));
        }
    }

    let platform = envr_platform::paths::current_platform_paths()?;
    let settings_path = settings_path_from_platform(&platform);
    let mtime = fs::metadata(&settings_path)
        .ok()
        .and_then(|m| m.modified().ok());

    if let Some(root) = RESOLVE_RUNTIME_ROOT_CACHE.with(|c| {
        c.borrow().as_ref().and_then(|(p, m2, root)| {
            if p == &settings_path && m2 == &mtime {
                Some(root.clone())
            } else {
                None
            }
        })
    }) {
        return Ok(root);
    }

    let settings = Settings::load_or_default_from(&settings_path)?;
    let root = if let Some(ref r) = settings.paths.runtime_root {
        let t = r.trim();
        if !t.is_empty() {
            PathBuf::from(t)
        } else {
            platform.runtime_root.clone()
        }
    } else {
        platform.runtime_root.clone()
    };

    RESOLVE_RUNTIME_ROOT_CACHE.with(|c| {
        *c.borrow_mut() = Some((settings_path, mtime, root.clone()));
    });
    Ok(root)
}

fn prefer_domestic_source(settings: &Settings, explicit_domestic: bool, is_auto: bool) -> bool {
    explicit_domestic || (is_auto && prefer_china_mirrors(settings))
}

fn file_mtime(path: &Path) -> EnvrResult<SystemTime> {
    let meta = fs::metadata(path).map_err(EnvrError::from)?;
    meta.modified()
        .map_err(|e| EnvrError::Io(std::io::Error::other(e)))
}

fn backup_corrupted_file(path: &Path) -> EnvrResult<()> {
    let ts = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|e| EnvrError::with_source(ErrorCode::Runtime, "time error", e))?
        .as_secs();
    let bad = path.with_extension(format!("toml.bad.{ts}"));
    let _ = fs::rename(path, bad);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn downloads_panel_frac_roundtrip_stable() {
        let mut p = DownloadsPanelSettings::default();
        p.sync_frac_from_pixels(100, 48, 960.0, 600.0, 12.0, 320.0);
        let (x, y) = p.pixel_insets(960.0, 600.0, 12.0, 320.0);
        assert_eq!((x, y), (100, 48));
        let (x2, y2) = p.pixel_insets(1200.0, 720.0, 12.0, 320.0);
        assert!(x2 > x && y2 > y, "larger window should allow larger insets");
    }

    #[test]
    fn read_write_roundtrip_is_consistent() {
        let tmp = TempDir::new().expect("tmp");
        let path = tmp.path().join("settings.toml");

        let settings = Settings {
            paths: PathSettings {
                runtime_root: Some("/tmp/envr-rt".to_string()),
            },
            behavior: BehaviorSettings {
                cleanup_downloads_after_install: true,
                auto_sync_shims_on_use: true,
                auto_sync_globals_on_use: false,
                auto_sync_windows_path_mirror_on_use: cfg!(windows),
                cache_artifact_ttl_days: 30,
                cache_max_size_mb: 10 * 1024,
                cache_auto_prune_on_start: true,
            },
            appearance: AppearanceSettings {
                font: FontSettings {
                    mode: FontMode::Custom,
                    family: Some("Microsoft YaHei UI".to_string()),
                },
                theme_mode: ThemeMode::Dark,
                accent_color: None,
            },
            gui: GuiSettings {
                downloads_panel: DownloadsPanelSettings {
                    visible: true,
                    expanded: false,
                    x: 24,
                    y: 18,
                    x_frac: None,
                    y_frac: None,
                },
                runtime_layout: RuntimeLayoutSettings::default(),
            },
            download: DownloadSettings {
                max_concurrent_downloads: 8,
                max_bytes_per_sec: 0,
                retry_max: 5,
            },
            mirror: MirrorSettings {
                mode: MirrorMode::Manual,
                manual_id: Some("cn-fast".to_string()),
                prefer_china_mirrors: defaults::prefer_china_mirrors(),
            },
            i18n: I18nSettings {
                locale: LocaleMode::EnUs,
            },
            runtime: RuntimeSettings {
                node: NodeRuntimeSettings::default(),
                ruby: RubyRuntimeSettings::default(),
                elixir: ElixirRuntimeSettings::default(),
                erlang: ErlangRuntimeSettings::default(),
                python: PythonRuntimeSettings::default(),
                java: JavaRuntimeSettings::default(),
                kotlin: KotlinRuntimeSettings::default(),
                scala: ScalaRuntimeSettings::default(),
                clojure: ClojureRuntimeSettings::default(),
                groovy: GroovyRuntimeSettings::default(),
                terraform: TerraformRuntimeSettings::default(),
                v: VRuntimeSettings::default(),
                odin: OdinRuntimeSettings::default(),
                purescript: PurescriptRuntimeSettings::default(),
                elm: ElmRuntimeSettings::default(),
                gleam: GleamRuntimeSettings::default(),
                racket: RacketRuntimeSettings::default(),
                dart: DartRuntimeSettings::default(),
                flutter: FlutterRuntimeSettings::default(),
                go: GoRuntimeSettings {
                    goproxy: Some("https://proxy.golang.org,direct".to_string()),
                    ..Default::default()
                },
                rust: RustRuntimeSettings::default(),
                bun: BunRuntimeSettings {
                    package_source: NpmRegistryMode::default(),
                    path_proxy_enabled: true,
                    global_bin_dir: Some("/tmp/.bun/bin".to_string()),
                },
                dotnet: DotnetRuntimeSettings::default(),
                zig: ZigRuntimeSettings::default(),
                julia: JuliaRuntimeSettings::default(),
                janet: JanetRuntimeSettings::default(),
                c3: C3RuntimeSettings::default(),
                babashka: BabashkaRuntimeSettings::default(),
                sbcl: SbclRuntimeSettings::default(),
                haxe: HaxeRuntimeSettings::default(),
                lua: LuaRuntimeSettings::default(),
                nim: NimRuntimeSettings::default(),
                crystal: CrystalRuntimeSettings::default(),
                perl: PerlRuntimeSettings::default(),
                unison: UnisonRuntimeSettings::default(),
                r: RlangRuntimeSettings::default(),
                php: PhpRuntimeSettings::default(),
                deno: DenoRuntimeSettings::default(),
            },
        };

        settings.save_to(&path).expect("save");
        let loaded = Settings::load_from(&path).expect("load");
        assert_eq!(settings, loaded);
    }

    #[test]
    fn windows_style_runtime_root_roundtrips() {
        let tmp = TempDir::new().expect("tmp");
        let path = tmp.path().join("settings.toml");
        let mut s = Settings::default();
        s.paths.runtime_root = Some(r"D:\environment\runtimes".to_string());
        s.save_to(&path).expect("save");
        let loaded = Settings::load_from(&path).expect("load");
        assert_eq!(
            loaded.paths.runtime_root.as_deref(),
            Some(r"D:\environment\runtimes")
        );
    }

    #[test]
    fn corrupted_file_recovers_defaults() {
        let tmp = TempDir::new().expect("tmp");
        let path = tmp.path().join("settings.toml");

        fs::write(&path, "not = toml = =").expect("write");
        let loaded = Settings::load_or_default_from(&path).expect("load_or_default");
        assert_eq!(loaded, Settings::default());
    }

    #[test]
    fn invalid_manual_mode_is_rejected() {
        let tmp = TempDir::new().expect("tmp");
        let path = tmp.path().join("settings.toml");

        fs::write(
            &path,
            r#"
[mirror]
mode = "manual"
"#,
        )
        .expect("write");

        let loaded = Settings::load_or_default_from(&path).expect("load_or_default");
        assert_eq!(loaded, Settings::default());
    }

    #[test]
    fn invalid_download_limits_recover_defaults() {
        let tmp = TempDir::new().expect("tmp");
        let path = tmp.path().join("settings.toml");

        fs::write(
            &path,
            r#"
[download]
max_concurrent_downloads = 0
retry_max = -1
"#,
        )
        .expect("write");

        let loaded = Settings::load_or_default_from(&path).expect("load_or_default");
        assert_eq!(loaded, Settings::default());
    }

    #[test]
    fn bcp47_primary_language_zh() {
        assert!(super::locale_mirror::bcp47_primary_language_is_zh("zh-CN"));
        assert!(super::locale_mirror::bcp47_primary_language_is_zh(
            "zh_TW.UTF-8"
        ));
        assert!(!super::locale_mirror::bcp47_primary_language_is_zh("en-US"));
        assert!(!super::locale_mirror::bcp47_primary_language_is_zh(""));
    }

    #[test]
    fn cache_set_cached_updates_in_memory_without_disk_write() {
        let tmp = TempDir::new().expect("tmp");
        let path = tmp.path().join("settings.toml");
        Settings::default().save_to(&path).expect("save default");

        let mut cache = SettingsCache::new(&path).expect("cache");
        let mut in_mem = Settings::default();
        in_mem.mirror.mode = MirrorMode::Offline;
        cache.set_cached(in_mem.clone());

        let got = cache.get().expect("get").clone();
        assert_eq!(got.mirror.mode, MirrorMode::Offline);

        // Disk content remains unchanged until explicitly persisted.
        let from_disk = Settings::load_from(&path).expect("load disk");
        assert_eq!(from_disk.mirror.mode, Settings::default().mirror.mode);
    }
}
