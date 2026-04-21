use envr_error::{EnvrError, EnvrResult};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MirrorMode {
    Official,
    Auto,
    Manual,
    Offline,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DownloadSettings {
    #[serde(default = "defaults::max_concurrent_downloads")]
    pub max_concurrent_downloads: u32,

    #[serde(default = "defaults::retry_max")]
    pub retry_max: u32,
}

impl Default for DownloadSettings {
    fn default() -> Self {
        Self {
            max_concurrent_downloads: defaults::max_concurrent_downloads(),
            retry_max: defaults::retry_max(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MirrorSettings {
    #[serde(default = "defaults::mirror_mode")]
    pub mode: MirrorMode,

    #[serde(default)]
    pub manual_id: Option<String>,
}

impl Default for MirrorSettings {
    fn default() -> Self {
        Self {
            mode: defaults::mirror_mode(),
            manual_id: None,
        }
    }
}

/// Persistent overrides for install layout (GUI + CLI read the same file).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PathSettings {
    /// If set (non-empty after trim), used as runtime root unless `ENVR_RUNTIME_ROOT` is set.
    #[serde(default)]
    pub runtime_root: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BehaviorSettings {
    /// Remove staging/temp artifacts after a successful install (providers may adopt later).
    #[serde(default)]
    pub cleanup_downloads_after_install: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FontMode {
    Auto,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    FollowSystem,
    Light,
    Dark,
}

impl Default for ThemeMode {
    fn default() -> Self {
        defaults::theme_mode()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocaleMode {
    FollowSystem,
    ZhCn,
    EnUs,
}

impl Default for LocaleMode {
    fn default() -> Self {
        defaults::locale_mode()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FontSettings {
    #[serde(default = "defaults::font_mode")]
    pub mode: FontMode,

    /// Used only when `mode = "custom"`.
    #[serde(default)]
    pub family: Option<String>,
}

impl Default for FontSettings {
    fn default() -> Self {
        Self {
            mode: defaults::font_mode(),
            family: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AppearanceSettings {
    #[serde(default)]
    pub font: FontSettings,

    #[serde(default = "defaults::theme_mode")]
    pub theme_mode: ThemeMode,

    /// Optional brand accent `#RGB` / `#RRGGBB`; merged into theme primary when valid (GUI-003).
    #[serde(default)]
    pub accent_color: Option<String>,
}

/// Order and visibility for runtime hub + dashboard overview (string keys = `RuntimeDescriptor::key`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RuntimeLayoutSettings {
    /// Permutation of runtime keys; empty means built-in default order at resolve time.
    #[serde(default)]
    pub order: Vec<String>,
    /// Keys hidden from the runtime hub and shown only in the dashboard “hidden” region.
    #[serde(default)]
    pub hidden: Vec<String>,
}

/// GUI-only state persisted in `settings.toml` so window layout/UX preferences survive restarts.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct GuiSettings {
    #[serde(default)]
    pub downloads_panel: DownloadsPanelSettings,

    #[serde(default)]
    pub runtime_layout: RuntimeLayoutSettings,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DownloadsPanelSettings {
    /// Whether the floating downloads panel is visible.
    #[serde(default = "defaults::downloads_panel_visible")]
    pub visible: bool,
    /// Whether the panel is expanded (shows job list).
    #[serde(default = "defaults::downloads_panel_expanded")]
    pub expanded: bool,
    /// Left offset in pixels from the window's left edge.
    #[serde(default = "defaults::downloads_panel_x")]
    pub x: i32,
    /// Bottom offset in pixels from the window's bottom edge.
    #[serde(default = "defaults::downloads_panel_y")]
    pub y: i32,
    /// Normalized horizontal inset: `x ≈ x_frac * (client_w - 2*pad - panel_w)` (`tasks_gui.md` GUI-061).
    #[serde(default)]
    pub x_frac: Option<f32>,
    /// Normalized bottom inset: `y ≈ y_frac * (client_h - 2*pad)` (`tasks_gui.md` GUI-061).
    #[serde(default)]
    pub y_frac: Option<f32>,
}

impl DownloadsPanelSettings {
    /// Pixel insets for the panel, using fractional coords when present (DPI / resize stable).
    pub fn pixel_insets(
        &self,
        client_w: f32,
        client_h: f32,
        content_pad: f32,
        panel_w: f32,
    ) -> (i32, i32) {
        let inner_w = (client_w - 2.0 * content_pad).max(1.0);
        let inner_h = (client_h - 2.0 * content_pad).max(1.0);
        let avail_x = (inner_w - panel_w).max(1.0);
        if let (Some(xf), Some(yf)) = (self.x_frac, self.y_frac) {
            let x = (xf.clamp(0.0, 1.0) * avail_x).round() as i32;
            let y = (yf.clamp(0.0, 1.0) * inner_h).round() as i32;
            (x.max(0), y.max(0))
        } else {
            (self.x.max(0), self.y.max(0))
        }
    }

    /// Writes [`Self::x_frac`] / [`Self::y_frac`] from current pixel offsets (for persistence).
    pub fn sync_frac_from_pixels(
        &mut self,
        x: i32,
        y: i32,
        client_w: f32,
        client_h: f32,
        content_pad: f32,
        panel_w: f32,
    ) {
        let inner_w = (client_w - 2.0 * content_pad).max(1.0);
        let inner_h = (client_h - 2.0 * content_pad).max(1.0);
        let avail_x = (inner_w - panel_w).max(1.0);
        self.x = x.max(0);
        self.y = y.max(0);
        self.x_frac = Some((self.x as f32 / avail_x).clamp(0.0, 1.0));
        self.y_frac = Some((self.y as f32 / inner_h).clamp(0.0, 1.0));
    }
}

impl Default for DownloadsPanelSettings {
    fn default() -> Self {
        Self {
            visible: defaults::downloads_panel_visible(),
            expanded: defaults::downloads_panel_expanded(),
            x: defaults::downloads_panel_x(),
            y: defaults::downloads_panel_y(),
            x_frac: None,
            y_frac: None,
        }
    }
}

/// Node.js distribution index (`index.json`) selection for installs / remote lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NodeDownloadSource {
    /// Prefer npmmirror when UI locale suggests China, else nodejs.org.
    #[default]
    Auto,
    /// npmmirror.com mirror (China).
    Domestic,
    /// nodejs.org official.
    Official,
}

/// How GUI manages `npm config registry` (Restore leaves user `.npmrc` untouched).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum NpmRegistryMode {
    #[default]
    Auto,
    Domestic,
    Official,
    /// Do not run `npm config set`; user may use a custom registry.
    Restore,
}

/// Python bootstrap source choice for `get-pip.py` retrieval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PythonDownloadSource {
    #[default]
    Auto,
    Domestic,
    Official,
}

/// How `pip` bootstrap should resolve package index during `get-pip.py`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PipRegistryMode {
    #[default]
    Auto,
    Domestic,
    Official,
    /// Do not force `--index-url` during bootstrap.
    Restore,
}

/// Go toolchain download source preference (go.dev vs China mirror).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GoDownloadSource {
    #[default]
    Auto,
    Domestic,
    Official,
}

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

/// PHP download source preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PhpDownloadSource {
    #[default]
    Auto,
    Domestic,
    Official,
}

/// Deno binary zip source (`dl.deno.land` vs npmmirror binary mirror).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DenoDownloadSource {
    /// Prefer npmmirror when UI locale suggests China, else official.
    #[default]
    Auto,
    Domestic,
    Official,
}

/// Windows PHP build flavor preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PhpWindowsBuildFlavor {
    #[default]
    Nts,
    Ts,
}

/// How `GOPROXY` should be injected in `envr env`/`run`/`exec` when Go is in scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GoProxyMode {
    #[default]
    Auto,
    Domestic,
    Official,
    /// Disable module proxy (`GOPROXY=direct`).
    Direct,
    /// Use user-provided `runtime.go.proxy_custom`.
    Custom,
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
pub struct NodeRuntimeSettings {
    #[serde(default)]
    pub download_source: NodeDownloadSource,
    #[serde(default)]
    pub npm_registry_mode: NpmRegistryMode,
    /// When false, Node/npm/npx shims resolve to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::node_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for NodeRuntimeSettings {
    fn default() -> Self {
        Self {
            download_source: NodeDownloadSource::default(),
            npm_registry_mode: NpmRegistryMode::default(),
            path_proxy_enabled: defaults::node_path_proxy_enabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PythonRuntimeSettings {
    #[serde(default)]
    pub download_source: PythonDownloadSource,
    /// Windows distribution choice: `auto` (prefer full NuGet), `nuget`, or `embeddable`.
    #[serde(default)]
    pub windows_distribution: PythonWindowsDistribution,
    #[serde(default)]
    pub pip_registry_mode: PipRegistryMode,
    /// When false, python/pip shims resolve to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::python_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for PythonRuntimeSettings {
    fn default() -> Self {
        Self {
            download_source: PythonDownloadSource::default(),
            windows_distribution: PythonWindowsDistribution::default(),
            pip_registry_mode: PipRegistryMode::default(),
            path_proxy_enabled: defaults::python_path_proxy_enabled(),
        }
    }
}

/// Windows distribution for CPython installs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PythonWindowsDistribution {
    /// Prefer full NuGet packages on Windows, fall back to embeddable zip when needed.
    #[default]
    Auto,
    /// Full Python from NuGet (`python`, `pythonx86`, `pythonarm64`).
    Nuget,
    /// python.org embeddable zip (may lack some stdlib modules such as `venv`).
    Embeddable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JavaRuntimeSettings {
    #[serde(default)]
    pub current_distro: JavaDistro,
    #[serde(default)]
    pub download_source: JavaDownloadSource,
    /// When false, java/javac shims resolve to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::java_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for JavaRuntimeSettings {
    fn default() -> Self {
        Self {
            current_distro: JavaDistro::default(),
            download_source: JavaDownloadSource::default(),
            path_proxy_enabled: defaults::java_path_proxy_enabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KotlinRuntimeSettings {
    /// When false, kotlin/kotlinc shims resolve to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::kotlin_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for KotlinRuntimeSettings {
    fn default() -> Self {
        Self {
            path_proxy_enabled: defaults::kotlin_path_proxy_enabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScalaRuntimeSettings {
    /// When false, scala/scalac shims resolve to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::scala_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for ScalaRuntimeSettings {
    fn default() -> Self {
        Self {
            path_proxy_enabled: defaults::scala_path_proxy_enabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClojureRuntimeSettings {
    /// When false, clojure/clj shims resolve to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::clojure_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for ClojureRuntimeSettings {
    fn default() -> Self {
        Self {
            path_proxy_enabled: defaults::clojure_path_proxy_enabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroovyRuntimeSettings {
    /// When false, groovy/groovyc shims resolve to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::groovy_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for GroovyRuntimeSettings {
    fn default() -> Self {
        Self {
            path_proxy_enabled: defaults::groovy_path_proxy_enabled(),
        }
    }
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
    pub lua: LuaRuntimeSettings,

    #[serde(default)]
    pub nim: NimRuntimeSettings,

    #[serde(default)]
    pub crystal: CrystalRuntimeSettings,

    #[serde(default)]
    pub perl: PerlRuntimeSettings,

    #[serde(default)]
    pub r: RlangRuntimeSettings,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RustRuntimeSettings {
    /// Rust toolchain download source choice (used for `rustup` env injection).
    #[serde(default)]
    pub download_source: RustDownloadSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RubyRuntimeSettings {
    /// When false, ruby/gem/bundle/irb shims resolve to the next matching binary on PATH
    /// outside envr shims.
    #[serde(default = "defaults::ruby_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for RubyRuntimeSettings {
    fn default() -> Self {
        Self {
            path_proxy_enabled: defaults::ruby_path_proxy_enabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ElixirRuntimeSettings {
    /// When false, elixir/mix/iex shims resolve to the next matching binary on PATH
    /// outside envr shims.
    #[serde(default = "defaults::elixir_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for ElixirRuntimeSettings {
    fn default() -> Self {
        Self {
            path_proxy_enabled: defaults::elixir_path_proxy_enabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErlangRuntimeSettings {
    /// When false, erl/erlc/escript shims resolve to the next matching binary on PATH
    /// outside envr shims.
    #[serde(default = "defaults::erlang_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for ErlangRuntimeSettings {
    fn default() -> Self {
        Self {
            path_proxy_enabled: defaults::erlang_path_proxy_enabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhpRuntimeSettings {
    #[serde(default)]
    pub download_source: PhpDownloadSource,
    #[serde(default)]
    pub windows_build: PhpWindowsBuildFlavor,
    /// When false, php shim resolves to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::php_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for PhpRuntimeSettings {
    fn default() -> Self {
        Self {
            download_source: PhpDownloadSource::default(),
            windows_build: PhpWindowsBuildFlavor::default(),
            path_proxy_enabled: defaults::php_path_proxy_enabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DenoRuntimeSettings {
    #[serde(default)]
    pub download_source: DenoDownloadSource,
    /// Single preset for both `NPM_CONFIG_REGISTRY` and `JSR_URL` (see `deno_package_registry_env`).
    #[serde(default)]
    pub package_source: NpmRegistryMode,
    /// When false, `deno` shim resolves to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::deno_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for DenoRuntimeSettings {
    fn default() -> Self {
        Self {
            download_source: DenoDownloadSource::default(),
            package_source: NpmRegistryMode::default(),
            path_proxy_enabled: defaults::deno_path_proxy_enabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoRuntimeSettings {
    /// Go toolchain download source choice.
    #[serde(default)]
    pub download_source: GoDownloadSource,
    /// `GOPROXY` injection mode.
    #[serde(default)]
    pub proxy_mode: GoProxyMode,
    /// Custom `GOPROXY` value (only when `proxy_mode = custom`).
    #[serde(default)]
    pub proxy_custom: Option<String>,
    /// Optional private module patterns (comma-separated). When set, envr injects:
    /// - `GOPRIVATE`
    /// - `GONOSUMDB`
    /// - `GONOPROXY`
    #[serde(default)]
    pub private_patterns: Option<String>,
    /// When false, go/gofmt shims resolve to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::go_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
    /// Backward compatibility: older settings used a direct `goproxy` value.
    ///
    /// When `proxy_mode` is `auto` and this is set, it takes precedence.
    /// When `proxy_mode` is `custom` and `proxy_custom` is empty, this is used as fallback.
    #[serde(default)]
    pub goproxy: Option<String>,
}

impl Default for GoRuntimeSettings {
    fn default() -> Self {
        Self {
            download_source: GoDownloadSource::default(),
            proxy_mode: GoProxyMode::default(),
            proxy_custom: None,
            private_patterns: None,
            path_proxy_enabled: defaults::go_path_proxy_enabled(),
            goproxy: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BunRuntimeSettings {
    /// Single preset for Bun package source env injection.
    #[serde(default)]
    pub package_source: NpmRegistryMode,
    /// When false, bun/bunx shims resolve to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::bun_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
    /// Optional override for Bun global bin directory (defaults to `bun pm bin -g`).
    ///
    /// This affects shim sync for global Bun executables.
    #[serde(default)]
    pub global_bin_dir: Option<String>,
}

impl Default for BunRuntimeSettings {
    fn default() -> Self {
        Self {
            package_source: NpmRegistryMode::default(),
            path_proxy_enabled: defaults::bun_path_proxy_enabled(),
            global_bin_dir: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DotnetRuntimeSettings {
    /// When false, dotnet shim resolves to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::dotnet_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for DotnetRuntimeSettings {
    fn default() -> Self {
        Self {
            path_proxy_enabled: defaults::dotnet_path_proxy_enabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZigRuntimeSettings {
    /// When false, the zig shim resolves to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::zig_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for ZigRuntimeSettings {
    fn default() -> Self {
        Self {
            path_proxy_enabled: defaults::zig_path_proxy_enabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JuliaRuntimeSettings {
    /// When false, the julia shim resolves to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::julia_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for JuliaRuntimeSettings {
    fn default() -> Self {
        Self {
            path_proxy_enabled: defaults::julia_path_proxy_enabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LuaRuntimeSettings {
    /// When false, lua/luac shims resolve to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::lua_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for LuaRuntimeSettings {
    fn default() -> Self {
        Self {
            path_proxy_enabled: defaults::lua_path_proxy_enabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NimRuntimeSettings {
    /// When false, the nim shim resolves to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::nim_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for NimRuntimeSettings {
    fn default() -> Self {
        Self {
            path_proxy_enabled: defaults::nim_path_proxy_enabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrystalRuntimeSettings {
    /// When false, the crystal shim resolves to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::crystal_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for CrystalRuntimeSettings {
    fn default() -> Self {
        Self {
            path_proxy_enabled: defaults::crystal_path_proxy_enabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PerlRuntimeSettings {
    /// When false, the perl shim resolves to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::perl_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for PerlRuntimeSettings {
    fn default() -> Self {
        Self {
            path_proxy_enabled: defaults::perl_path_proxy_enabled(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RlangRuntimeSettings {
    /// When false, the `R` / `Rscript` shims resolve to the next matching binary on PATH outside envr shims.
    #[serde(default = "defaults::rlang_path_proxy_enabled")]
    pub path_proxy_enabled: bool,
}

impl Default for RlangRuntimeSettings {
    fn default() -> Self {
        Self {
            path_proxy_enabled: defaults::rlang_path_proxy_enabled(),
        }
    }
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

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct I18nSettings {
    #[serde(default = "defaults::locale_mode")]
    pub locale: LocaleMode,
}

impl Settings {
    pub fn validate(&self) -> EnvrResult<()> {
        if let Some(ref root) = self.paths.runtime_root
            && root.trim().is_empty()
        {
            return Err(EnvrError::Validation(
                "paths.runtime_root must not be whitespace-only".to_string(),
            ));
        }

        if self.download.max_concurrent_downloads == 0 {
            return Err(EnvrError::Validation(
                "download.max_concurrent_downloads must be >= 1".to_string(),
            ));
        }

        if self.mirror.mode == MirrorMode::Manual {
            let id_ok = self
                .mirror
                .manual_id
                .as_deref()
                .is_some_and(|s| !s.trim().is_empty());
            if !id_ok {
                return Err(EnvrError::Validation(
                    "mirror.manual_id is required when mirror.mode = manual".to_string(),
                ));
            }
        }

        if self.appearance.font.mode == FontMode::Custom {
            let ok = self
                .appearance
                .font
                .family
                .as_deref()
                .is_some_and(|s| !s.trim().is_empty());
            if !ok {
                return Err(EnvrError::Validation(
                    "appearance.font.family is required when appearance.font.mode = custom"
                        .to_string(),
                ));
            }
        }

        if let Some(ref gp) = self.runtime.go.goproxy
            && gp.trim().is_empty()
        {
            return Err(EnvrError::Validation(
                "runtime.go.goproxy must not be whitespace-only".to_string(),
            ));
        }

        if let Some(ref v) = self.runtime.go.proxy_custom
            && v.trim().is_empty()
        {
            return Err(EnvrError::Validation(
                "runtime.go.proxy_custom must not be whitespace-only".to_string(),
            ));
        }
        if self.runtime.go.proxy_mode == GoProxyMode::Custom
            && self
                .runtime
                .go
                .proxy_custom
                .as_deref()
                .is_none_or(|s| s.trim().is_empty())
            && self
                .runtime
                .go
                .goproxy
                .as_deref()
                .is_none_or(|s| s.trim().is_empty())
        {
            return Err(EnvrError::Validation(
                "runtime.go.proxy_custom is required when runtime.go.proxy_mode = custom"
                    .to_string(),
            ));
        }
        if let Some(ref v) = self.runtime.go.private_patterns
            && v.trim().is_empty()
        {
            return Err(EnvrError::Validation(
                "runtime.go.private_patterns must not be whitespace-only".to_string(),
            ));
        }

        if let Some(ref dir) = self.runtime.bun.global_bin_dir
            && dir.trim().is_empty()
        {
            return Err(EnvrError::Validation(
                "runtime.bun.global_bin_dir must not be whitespace-only".to_string(),
            ));
        }

        if self.gui.downloads_panel.x < 0 || self.gui.downloads_panel.y < 0 {
            return Err(EnvrError::Validation(
                "gui.downloads_panel x/y must be >= 0".to_string(),
            ));
        }

        if let Some(xf) = self.gui.downloads_panel.x_frac
            && (!xf.is_finite() || !(0.0..=1.0).contains(&xf))
        {
            return Err(EnvrError::Validation(
                "gui.downloads_panel x_frac must be in [0, 1]".to_string(),
            ));
        }
        if let Some(yf) = self.gui.downloads_panel.y_frac
            && (!yf.is_finite() || !(0.0..=1.0).contains(&yf))
        {
            return Err(EnvrError::Validation(
                "gui.downloads_panel y_frac must be in [0, 1]".to_string(),
            ));
        }

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
            .map_err(|e| EnvrError::Runtime(format!("toml encode: {e}")))?;
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

/// True when the primary language subtag of a BCP-47–style tag is `zh` (e.g. `zh-CN`, `zh_TW.UTF-8`).
fn bcp47_primary_language_is_zh(tag: &str) -> bool {
    let t = tag.trim();
    let first = t
        .split(['-', '_'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    first == "zh"
}

/// POSIX `LANG` / `LC_*` hints (Unix shells, CI, WSL); secondary to [`sys_locale::get_locale`].
fn env_locale_vars_suggest_chinese() -> bool {
    for key in ["LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE"] {
        if let Ok(v) = std::env::var(key) {
            let l = v.to_ascii_lowercase();
            if l.contains("zh_cn")
                || l.contains("zh-cn")
                || l.contains("zh_hans")
                || l.starts_with("zh.")
                || bcp47_primary_language_is_zh(&v)
            {
                return true;
            }
        }
    }
    false
}

/// Heuristic: OS or environment suggests a Chinese locale (used when [`LocaleMode::FollowSystem`]).
///
/// Order: [`sys_locale::get_locale`] (cross-platform OS API), then `LC_*` / `LANG` / `LANGUAGE`.
pub fn system_locale_suggests_chinese() -> bool {
    if let Some(tag) = sys_locale::get_locale()
        && bcp47_primary_language_is_zh(&tag)
    {
        return true;
    }
    env_locale_vars_suggest_chinese()
}

pub fn prefer_china_mirror_locale(settings: &Settings) -> bool {
    match settings.i18n.locale {
        LocaleMode::ZhCn => true,
        LocaleMode::EnUs => false,
        LocaleMode::FollowSystem => system_locale_suggests_chinese(),
    }
}

pub fn node_index_json_url(settings: &Settings) -> String {
    if prefer_domestic_source(
        settings,
        matches!(
            settings.runtime.node.download_source,
            NodeDownloadSource::Domestic
        ),
        matches!(
            settings.runtime.node.download_source,
            NodeDownloadSource::Auto
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
        NpmRegistryMode::Auto => Some(if prefer_china_mirror_locale(settings) {
            NPM_REGISTRY_DOMESTIC
        } else {
            NPM_REGISTRY_OFFICIAL
        }),
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
        _ => Err(EnvrError::Platform(format!(
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
            if prefer_china_mirror_locale(settings) {
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
            if prefer_china_mirror_locale(settings) {
                NPM_REGISTRY_DOMESTIC
            } else {
                NPM_REGISTRY_OFFICIAL
            }
        }
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
        PipRegistryMode::Auto => {
            if prefer_china_mirror_locale(settings) {
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

fn prefer_domestic_source(settings: &Settings, explicit_domestic: bool, is_auto: bool) -> bool {
    explicit_domestic || (is_auto && prefer_china_mirror_locale(settings))
}

/// Read [`NodeRuntimeSettings::path_proxy_enabled`] from disk; on error defaults to `true`.
pub fn node_path_proxy_enabled_from_disk() -> bool {
    let Ok(platform) = envr_platform::paths::current_platform_paths() else {
        return true;
    };
    let path = settings_path_from_platform(&platform);
    Settings::load_or_default_from(&path)
        .map(|s| s.runtime.node.path_proxy_enabled)
        .unwrap_or(true)
}

/// Read [`PythonRuntimeSettings::path_proxy_enabled`] from disk; on error defaults to `true`.
pub fn python_path_proxy_enabled_from_disk() -> bool {
    let Ok(platform) = envr_platform::paths::current_platform_paths() else {
        return true;
    };
    let path = settings_path_from_platform(&platform);
    Settings::load_or_default_from(&path)
        .map(|s| s.runtime.python.path_proxy_enabled)
        .unwrap_or(true)
}

/// Read [`JavaRuntimeSettings::path_proxy_enabled`] from disk; on error defaults to `true`.
pub fn java_path_proxy_enabled_from_disk() -> bool {
    let Ok(platform) = envr_platform::paths::current_platform_paths() else {
        return true;
    };
    let path = settings_path_from_platform(&platform);
    Settings::load_or_default_from(&path)
        .map(|s| s.runtime.java.path_proxy_enabled)
        .unwrap_or(true)
}

/// Read [`GoRuntimeSettings::path_proxy_enabled`] from disk; on error defaults to `true`.
pub fn go_path_proxy_enabled_from_disk() -> bool {
    let Ok(platform) = envr_platform::paths::current_platform_paths() else {
        return true;
    };
    let path = settings_path_from_platform(&platform);
    Settings::load_or_default_from(&path)
        .map(|s| s.runtime.go.path_proxy_enabled)
        .unwrap_or(true)
}

/// Read [`PhpRuntimeSettings::path_proxy_enabled`] from disk; on error defaults to `true`.
pub fn php_path_proxy_enabled_from_disk() -> bool {
    let Ok(platform) = envr_platform::paths::current_platform_paths() else {
        return true;
    };
    let path = settings_path_from_platform(&platform);
    Settings::load_or_default_from(&path)
        .map(|s| s.runtime.php.path_proxy_enabled)
        .unwrap_or(true)
}

/// Read [`DenoRuntimeSettings::path_proxy_enabled`] from disk; on error defaults to `true`.
pub fn deno_path_proxy_enabled_from_disk() -> bool {
    let Ok(platform) = envr_platform::paths::current_platform_paths() else {
        return true;
    };
    let path = settings_path_from_platform(&platform);
    Settings::load_or_default_from(&path)
        .map(|s| s.runtime.deno.path_proxy_enabled)
        .unwrap_or(true)
}

/// Read [`BunRuntimeSettings::path_proxy_enabled`] from disk; on error defaults to `true`.
pub fn bun_path_proxy_enabled_from_disk() -> bool {
    let Ok(platform) = envr_platform::paths::current_platform_paths() else {
        return true;
    };
    let path = settings_path_from_platform(&platform);
    Settings::load_or_default_from(&path)
        .map(|s| s.runtime.bun.path_proxy_enabled)
        .unwrap_or(true)
}

/// Read [`DotnetRuntimeSettings::path_proxy_enabled`] from disk; on error defaults to `true`.
pub fn dotnet_path_proxy_enabled_from_disk() -> bool {
    let Ok(platform) = envr_platform::paths::current_platform_paths() else {
        return true;
    };
    let path = settings_path_from_platform(&platform);
    Settings::load_or_default_from(&path)
        .map(|s| s.runtime.dotnet.path_proxy_enabled)
        .unwrap_or(true)
}

/// Read [`JuliaRuntimeSettings::path_proxy_enabled`] from disk; on error defaults to `true`.
pub fn julia_path_proxy_enabled_from_disk() -> bool {
    let Ok(platform) = envr_platform::paths::current_platform_paths() else {
        return true;
    };
    let path = settings_path_from_platform(&platform);
    Settings::load_or_default_from(&path)
        .map(|s| s.runtime.julia.path_proxy_enabled)
        .unwrap_or(true)
}

/// Read [`LuaRuntimeSettings::path_proxy_enabled`] from disk; on error defaults to `true`.
pub fn lua_path_proxy_enabled_from_disk() -> bool {
    let Ok(platform) = envr_platform::paths::current_platform_paths() else {
        return true;
    };
    let path = settings_path_from_platform(&platform);
    Settings::load_or_default_from(&path)
        .map(|s| s.runtime.lua.path_proxy_enabled)
        .unwrap_or(true)
}

/// Read [`PerlRuntimeSettings::path_proxy_enabled`] from disk; on error defaults to `true`.
pub fn perl_path_proxy_enabled_from_disk() -> bool {
    let Ok(platform) = envr_platform::paths::current_platform_paths() else {
        return true;
    };
    let path = settings_path_from_platform(&platform);
    Settings::load_or_default_from(&path)
        .map(|s| s.runtime.perl.path_proxy_enabled)
        .unwrap_or(true)
}

/// Read [`CrystalRuntimeSettings::path_proxy_enabled`] from disk; on error defaults to `true`.
pub fn crystal_path_proxy_enabled_from_disk() -> bool {
    let Ok(platform) = envr_platform::paths::current_platform_paths() else {
        return true;
    };
    let path = settings_path_from_platform(&platform);
    Settings::load_or_default_from(&path)
        .map(|s| s.runtime.crystal.path_proxy_enabled)
        .unwrap_or(true)
}

/// Read [`NimRuntimeSettings::path_proxy_enabled`] from disk; on error defaults to `true`.
pub fn nim_path_proxy_enabled_from_disk() -> bool {
    let Ok(platform) = envr_platform::paths::current_platform_paths() else {
        return true;
    };
    let path = settings_path_from_platform(&platform);
    Settings::load_or_default_from(&path)
        .map(|s| s.runtime.nim.path_proxy_enabled)
        .unwrap_or(true)
}

/// Read [`RlangRuntimeSettings::path_proxy_enabled`] from disk; on error defaults to `true`.
pub fn rlang_path_proxy_enabled_from_disk() -> bool {
    let Ok(platform) = envr_platform::paths::current_platform_paths() else {
        return true;
    };
    let path = settings_path_from_platform(&platform);
    Settings::load_or_default_from(&path)
        .map(|s| s.runtime.r.path_proxy_enabled)
        .unwrap_or(true)
}

/// Read [`ZigRuntimeSettings::path_proxy_enabled`] from disk; on error defaults to `true`.
pub fn zig_path_proxy_enabled_from_disk() -> bool {
    let Ok(platform) = envr_platform::paths::current_platform_paths() else {
        return true;
    };
    let path = settings_path_from_platform(&platform);
    Settings::load_or_default_from(&path)
        .map(|s| s.runtime.zig.path_proxy_enabled)
        .unwrap_or(true)
}

/// Read [`VRuntimeSettings::path_proxy_enabled`] from disk; on error defaults to `true`.
pub fn v_path_proxy_enabled_from_disk() -> bool {
    let Ok(platform) = envr_platform::paths::current_platform_paths() else {
        return true;
    };
    let path = settings_path_from_platform(&platform);
    Settings::load_or_default_from(&path)
        .map(|s| s.runtime.v.path_proxy_enabled)
        .unwrap_or(true)
}

/// Read [`DartRuntimeSettings::path_proxy_enabled`] from disk; on error defaults to `true`.
pub fn dart_path_proxy_enabled_from_disk() -> bool {
    let Ok(platform) = envr_platform::paths::current_platform_paths() else {
        return true;
    };
    let path = settings_path_from_platform(&platform);
    Settings::load_or_default_from(&path)
        .map(|s| s.runtime.dart.path_proxy_enabled)
        .unwrap_or(true)
}

/// Read [`FlutterRuntimeSettings::path_proxy_enabled`] from disk; on error defaults to `true`.
pub fn flutter_path_proxy_enabled_from_disk() -> bool {
    let Ok(platform) = envr_platform::paths::current_platform_paths() else {
        return true;
    };
    let path = settings_path_from_platform(&platform);
    Settings::load_or_default_from(&path)
        .map(|s| s.runtime.flutter.path_proxy_enabled)
        .unwrap_or(true)
}

/// Read [`RubyRuntimeSettings::path_proxy_enabled`] from disk; on error defaults to `true`.
pub fn ruby_path_proxy_enabled_from_disk() -> bool {
    let Ok(platform) = envr_platform::paths::current_platform_paths() else {
        return true;
    };
    let path = settings_path_from_platform(&platform);
    Settings::load_or_default_from(&path)
        .map(|s| s.runtime.ruby.path_proxy_enabled)
        .unwrap_or(true)
}

/// Read [`ElixirRuntimeSettings::path_proxy_enabled`] from disk; on error defaults to `true`.
pub fn elixir_path_proxy_enabled_from_disk() -> bool {
    let Ok(platform) = envr_platform::paths::current_platform_paths() else {
        return true;
    };
    let path = settings_path_from_platform(&platform);
    Settings::load_or_default_from(&path)
        .map(|s| s.runtime.elixir.path_proxy_enabled)
        .unwrap_or(true)
}

/// Read [`ErlangRuntimeSettings::path_proxy_enabled`] from disk; on error defaults to `true`.
pub fn erlang_path_proxy_enabled_from_disk() -> bool {
    let Ok(platform) = envr_platform::paths::current_platform_paths() else {
        return true;
    };
    let path = settings_path_from_platform(&platform);
    Settings::load_or_default_from(&path)
        .map(|s| s.runtime.erlang.path_proxy_enabled)
        .unwrap_or(true)
}

/// Read [`PhpRuntimeSettings::windows_build`] from disk: `true` = TS, `false` = NTS.
pub fn php_windows_build_want_ts_from_disk() -> bool {
    let Ok(platform) = envr_platform::paths::current_platform_paths() else {
        return false;
    };
    let path = settings_path_from_platform(&platform);
    Settings::load_or_default_from(&path)
        .map(|s| matches!(s.runtime.php.windows_build, PhpWindowsBuildFlavor::Ts))
        .unwrap_or(false)
}

fn file_mtime(path: &Path) -> EnvrResult<SystemTime> {
    let meta = fs::metadata(path).map_err(EnvrError::from)?;
    meta.modified()
        .map_err(|e| EnvrError::Io(std::io::Error::other(e)))
}

fn backup_corrupted_file(path: &Path) -> EnvrResult<()> {
    let ts = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|e| EnvrError::Runtime(format!("time error: {e}")))?
        .as_secs();
    let bad = path.with_extension(format!("toml.bad.{ts}"));
    let _ = fs::rename(path, bad);
    Ok(())
}

mod defaults {
    use super::{FontMode, LocaleMode, MirrorMode, ThemeMode};

    pub fn max_concurrent_downloads() -> u32 {
        4
    }

    pub fn retry_max() -> u32 {
        3
    }

    pub fn mirror_mode() -> MirrorMode {
        MirrorMode::Auto
    }

    pub fn font_mode() -> FontMode {
        FontMode::Auto
    }

    pub fn theme_mode() -> ThemeMode {
        ThemeMode::FollowSystem
    }

    pub fn locale_mode() -> LocaleMode {
        LocaleMode::FollowSystem
    }

    pub fn downloads_panel_visible() -> bool {
        true
    }

    pub fn downloads_panel_expanded() -> bool {
        true
    }

    pub fn downloads_panel_x() -> i32 {
        12
    }

    pub fn downloads_panel_y() -> i32 {
        12
    }

    pub fn node_path_proxy_enabled() -> bool {
        true
    }

    pub fn python_path_proxy_enabled() -> bool {
        true
    }

    pub fn java_path_proxy_enabled() -> bool {
        true
    }

    pub fn kotlin_path_proxy_enabled() -> bool {
        true
    }

    pub fn scala_path_proxy_enabled() -> bool {
        true
    }

    pub fn clojure_path_proxy_enabled() -> bool {
        true
    }

    pub fn groovy_path_proxy_enabled() -> bool {
        true
    }

    pub fn terraform_path_proxy_enabled() -> bool {
        true
    }

    pub fn v_path_proxy_enabled() -> bool {
        true
    }

    pub fn dart_path_proxy_enabled() -> bool {
        true
    }

    pub fn flutter_path_proxy_enabled() -> bool {
        true
    }

    pub fn go_path_proxy_enabled() -> bool {
        true
    }

    pub fn php_path_proxy_enabled() -> bool {
        true
    }

    pub fn deno_path_proxy_enabled() -> bool {
        true
    }

    pub fn bun_path_proxy_enabled() -> bool {
        true
    }

    pub fn dotnet_path_proxy_enabled() -> bool {
        true
    }

    pub fn zig_path_proxy_enabled() -> bool {
        true
    }

    pub fn julia_path_proxy_enabled() -> bool {
        true
    }

    pub fn lua_path_proxy_enabled() -> bool {
        true
    }

    pub fn nim_path_proxy_enabled() -> bool {
        true
    }

    pub fn crystal_path_proxy_enabled() -> bool {
        true
    }

    pub fn perl_path_proxy_enabled() -> bool {
        true
    }

    pub fn rlang_path_proxy_enabled() -> bool {
        true
    }

    pub fn ruby_path_proxy_enabled() -> bool {
        true
    }

    pub fn elixir_path_proxy_enabled() -> bool {
        true
    }

    pub fn erlang_path_proxy_enabled() -> bool {
        true
    }
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
                retry_max: 5,
            },
            mirror: MirrorSettings {
                mode: MirrorMode::Manual,
                manual_id: Some("cn-fast".to_string()),
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
                lua: LuaRuntimeSettings::default(),
                nim: NimRuntimeSettings::default(),
                crystal: CrystalRuntimeSettings::default(),
                perl: PerlRuntimeSettings::default(),
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
        assert!(super::bcp47_primary_language_is_zh("zh-CN"));
        assert!(super::bcp47_primary_language_is_zh("zh_TW.UTF-8"));
        assert!(!super::bcp47_primary_language_is_zh("en-US"));
        assert!(!super::bcp47_primary_language_is_zh(""));
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
