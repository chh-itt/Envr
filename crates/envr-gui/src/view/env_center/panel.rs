//! Node / Python / Java / Go env center: lists, install, use, uninstall via `RuntimeService`.

use envr_config::settings::{
    BunRuntimeSettings, DenoDownloadSource, DenoRuntimeSettings, DotnetRuntimeSettings,
    ElixirRuntimeSettings, ErlangRuntimeSettings, GoDownloadSource, GoProxyMode, GoRuntimeSettings, JavaDistro,
    JavaDownloadSource, JavaRuntimeSettings, NodeDownloadSource, NodeRuntimeSettings,
    NpmRegistryMode, PhpDownloadSource, PhpRuntimeSettings, PhpWindowsBuildFlavor, PipRegistryMode,
    PythonDownloadSource, PythonRuntimeSettings, RubyRuntimeSettings, RuntimeSettings, RustDownloadSource,
    CrystalRuntimeSettings, JuliaRuntimeSettings, LuaRuntimeSettings, NimRuntimeSettings,
    RlangRuntimeSettings,
    RustRuntimeSettings, ZigRuntimeSettings,
};
use envr_domain::runtime::{
    MajorVersionRecord, RuntimeKind, RuntimeVersion, major_line_remote_install_blocked,
    runtime_descriptor, version_line_key_for_kind,
};
use envr_ui::theme::ThemeTokens;
use iced::alignment::Horizontal;
use iced::widget::{button, column, container, mouse_area, row, rule, space, text, text_input, toggler};
use iced::{Alignment, Element, Length, Padding, Theme};

use std::collections::{HashMap, HashSet};

use crate::app::Message;
use crate::icons::Lucide;
use crate::theme as gui_theme;
use crate::view::empty_state::{EmptyTone, illustrative_block_compact};
use crate::view::loading::loading_skeleton;
use crate::widget_styles::{
    ButtonVariant, SegmentPosition, button_content_centered, button_style, card_container_style,
    contrast_text_on, segmented_button_style, setting_row, text_input_style,
};

type EnvCenterDataLoad =
    Result<(Vec<RuntimeVersion>, Option<RuntimeVersion>, Option<bool>), String>;

#[derive(Debug, Clone)]
pub enum EnvCenterMsg {
    PickKind(RuntimeKind),
    InstallInput(String),
    DirectInstallInput(String),
    DataLoaded(EnvCenterDataLoad),
    UnifiedMajorRowsCached(RuntimeKind, Result<Vec<MajorVersionRecord>, String>),
    UnifiedMajorRowsRefreshed(RuntimeKind, Result<Vec<MajorVersionRecord>, String>),
    UnifiedChildrenCached(RuntimeKind, String, Result<Vec<RuntimeVersion>, String>),
    UnifiedChildrenRefreshed(RuntimeKind, String, Result<Vec<RuntimeVersion>, String>),
    ToggleUnifiedMajorExpanded(String),
    ElixirPrereqChecked(Result<(), String>),
    SubmitInstall(String),
    SubmitInstallAndUse(String),
    SubmitDirectInstall,
    SubmitDirectInstallAndUse,
    InstallFinished(Result<RuntimeVersion, String>),
    SubmitUse(String),
    UseFinished(Result<(), String>),
    SubmitUninstall(String),
    UninstallFinished(Result<(), String>),
    /// Fold/unfold Node-only settings (download mirror, npm registry, PATH proxy).
    ToggleRuntimeSettings,
    SetNodeDownloadSource(NodeDownloadSource),
    SetNpmRegistryMode(NpmRegistryMode),
    SetNodePathProxy(bool),
    SetPythonDownloadSource(PythonDownloadSource),
    SetPipRegistryMode(PipRegistryMode),
    SetPythonPathProxy(bool),
    SetJavaDistro(JavaDistro),
    SetJavaDownloadSource(JavaDownloadSource),
    SetJavaPathProxy(bool),
    SetGoDownloadSource(GoDownloadSource),
    SetGoProxyMode(GoProxyMode),
    SetGoPathProxy(bool),
    SetGoProxyCustomDraft(String),
    SetGoPrivatePatternsDraft(String),
    ApplyGoNetworkSettings,
    SetRustDownloadSource(RustDownloadSource),
    SetPhpDownloadSource(PhpDownloadSource),
    SetPhpWindowsBuild(PhpWindowsBuildFlavor),
    SetPhpPathProxy(bool),
    SetDenoDownloadSource(DenoDownloadSource),
    SetDenoPackageSource(NpmRegistryMode),
    SetDenoPathProxy(bool),
    SetBunPackageSource(NpmRegistryMode),
    SetBunPathProxy(bool),
    SetDotnetPathProxy(bool),
    SetZigPathProxy(bool),
    SetJuliaPathProxy(bool),
    SetLuaPathProxy(bool),
    SetNimPathProxy(bool),
    SetCrystalPathProxy(bool),
    SetRLangPathProxy(bool),
    SetRubyPathProxy(bool),
    SetElixirPathProxy(bool),
    SetErlangPathProxy(bool),
    BunGlobalBinDirEdit(String),
    ApplyBunGlobalBinDir,

    // Rust page (specialized).
    RustRefresh,
    RustStatusLoaded(Result<RustStatus, String>),
    RustSelectTab(RustTab),
    RustChannelInstallOrSwitch(String),
    RustUpdateCurrent,
    RustManagedInstallStable,
    RustManagedUninstall,
    RustComponentsLoaded(Result<Vec<(String, bool, bool)>, String>),
    RustTargetsLoaded(Result<Vec<(String, bool, bool)>, String>),
    RustComponentToggle(String, bool),
    RustTargetToggle(String, bool),
    RustOpFinished(Result<(), String>),
    SyncShimsFinished(Result<(), String>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RustTab {
    Components,
    Targets,
}

#[derive(Debug, Clone)]
pub struct RustStatus {
    /// "system" | "managed" | "none"
    pub mode: String,
    pub active_toolchain: Option<String>,
    pub rustc_version: Option<String>,
    pub managed_install_available: bool,
    pub managed_installed: bool,
}

#[derive(Debug)]
pub struct EnvCenterState {
    pub kind: RuntimeKind,
    pub install_input: String,
    pub installed: Vec<RuntimeVersion>,
    pub current: Option<RuntimeVersion>,
    /// Global PHP build for `current` (`true` = TS). Only used when [`Self::kind`] is PHP.
    pub php_global_current_want_ts: Option<bool>,
    pub busy: bool,
    /// Non-fatal remote fetch/parse error shown inline (keeps global UI usable).
    pub remote_error: Option<String>,
    /// Elixir prerequisites check result (Erlang/OTP runtime).
    pub elixir_prereq_error: Option<String>,
    /// Optional version spec for direct install (right of search).
    pub direct_install_input: String,
    /// 0..1 phase for skeleton shimmer (`tasks_gui.md` GUI-041).
    pub skeleton_phase: f32,
    /// Runtime: whether the settings strip is visible (`03-gui-设计.md`).
    pub runtime_settings_expanded: bool,
    /// Synthetic job shown in downloads panel for current env-center operation.
    pub op_job_id: Option<u64>,
    /// Draft for `runtime.go.proxy_custom` (applied via [`EnvCenterMsg::ApplyGoNetworkSettings`]).
    pub go_proxy_custom_draft: String,
    /// Draft for `runtime.go.private_patterns`.
    pub go_private_patterns_draft: String,
    /// Draft for `runtime.bun.global_bin_dir` (applied via [`EnvCenterMsg::ApplyBunGlobalBinDir`]).
    pub bun_global_bin_dir_draft: String,

    /// Unified list VM (phase-2 rollout: Node first, other runtimes keep legacy list path).
    pub unified_major_rows_by_kind: HashMap<RuntimeKind, Vec<MajorVersionRecord>>,
    pub unified_children_rows_by_kind_major: HashMap<(RuntimeKind, String), Vec<RuntimeVersion>>,
    pub unified_expanded_major_keys: HashSet<String>,

    // Rust page state.
    pub rust_status: Option<RustStatus>,
    pub rust_tab: RustTab,
    pub rust_components: Vec<(String, bool, bool)>,
    pub rust_targets: Vec<(String, bool, bool)>,
}

impl Default for EnvCenterState {
    fn default() -> Self {
        Self {
            kind: RuntimeKind::Node,
            install_input: String::new(),
            installed: Vec::new(),
            current: None,
            php_global_current_want_ts: None,
            busy: false,
            remote_error: None,
            elixir_prereq_error: None,
            direct_install_input: String::new(),
            skeleton_phase: 0.0,
            runtime_settings_expanded: false,
            op_job_id: None,
            go_proxy_custom_draft: String::new(),
            go_private_patterns_draft: String::new(),
            bun_global_bin_dir_draft: String::new(),

            unified_major_rows_by_kind: HashMap::new(),
            unified_children_rows_by_kind_major: HashMap::new(),
            unified_expanded_major_keys: HashSet::new(),

            rust_status: None,
            rust_tab: RustTab::Components,
            rust_components: Vec::new(),
            rust_targets: Vec::new(),
        }
    }
}

// (scroll_y is clamped locally during rendering; no persistent clamping helper needed)

pub(crate) fn kind_label(kind: RuntimeKind) -> &'static str {
    runtime_descriptor(kind).label_en
}

/// Display name for download-panel install tasks (Chinese UI copy).
pub(crate) fn kind_label_zh(kind: RuntimeKind) -> &'static str {
    runtime_descriptor(kind).label_zh
}

fn node_runtime_settings_section(
    node: &NodeRuntimeSettings,
    tokens: ThemeTokens,
) -> Element<'static, Message> {
    let ty = tokens.typography();
    let sp = tokens.space();
    let muted = gui_theme::to_color(tokens.colors.text_muted);

    let mut dl_buttons = row![].spacing(-1.0);
    for src in [
        NodeDownloadSource::Auto,
        NodeDownloadSource::Domestic,
        NodeDownloadSource::Official,
    ] {
        let lab = match src {
            NodeDownloadSource::Auto => envr_core::i18n::tr_key(
                "gui.runtime.node.ds.auto",
                "自动（随区域语言）",
                "Auto (locale)",
            ),
            NodeDownloadSource::Domestic => {
                envr_core::i18n::tr_key("gui.runtime.node.ds.domestic", "国内镜像", "China mirror")
            }
            NodeDownloadSource::Official => {
                envr_core::i18n::tr_key("gui.runtime.node.ds.official", "官方", "Official")
            }
        };
        let variant = if src == node.download_source {
            ButtonVariant::Primary
        } else {
            ButtonVariant::Secondary
        };
        let h = if src == node.download_source {
            tokens.control_height_primary
        } else {
            tokens.control_height_secondary
        };
        let b = button(button_content_centered(text(lab).into()))
            .on_press(Message::EnvCenter(EnvCenterMsg::SetNodeDownloadSource(src)))
            .width(Length::Shrink)
            .height(Length::Fixed(h))
            .padding([sp.sm as f32, (sp.sm + 2) as f32])
            .style(button_style(tokens, variant));
        dl_buttons = dl_buttons.push(b);
    }
    let dl_row = setting_row(
        tokens,
        envr_core::i18n::tr_key(
            "gui.runtime.node.download_source",
            "Node 下载源",
            "Node download source",
        ),
        None,
        dl_buttons.into(),
    );

    let mut npm_buttons = row![].spacing(-1.0);
    for mode in [
        NpmRegistryMode::Auto,
        NpmRegistryMode::Domestic,
        NpmRegistryMode::Official,
        NpmRegistryMode::Restore,
    ] {
        let lab = match mode {
            NpmRegistryMode::Auto => envr_core::i18n::tr_key(
                "gui.runtime.node.npm.auto",
                "自动（随区域语言）",
                "Auto (locale)",
            ),
            NpmRegistryMode::Domestic => {
                envr_core::i18n::tr_key("gui.runtime.node.npm.domestic", "国内镜像", "China mirror")
            }
            NpmRegistryMode::Official => {
                envr_core::i18n::tr_key("gui.runtime.node.npm.official", "官方", "Official")
            }
            NpmRegistryMode::Restore => envr_core::i18n::tr_key(
                "gui.runtime.node.npm.restore",
                "还原（不修改 npm）",
                "Restore (leave npm as-is)",
            ),
        };
        let variant = if mode == node.npm_registry_mode {
            ButtonVariant::Primary
        } else {
            ButtonVariant::Secondary
        };
        let h = if mode == node.npm_registry_mode {
            tokens.control_height_primary
        } else {
            tokens.control_height_secondary
        };
        let b = button(button_content_centered(text(lab).into()))
            .on_press(Message::EnvCenter(EnvCenterMsg::SetNpmRegistryMode(mode)))
            .width(Length::Shrink)
            .height(Length::Fixed(h))
            .padding([sp.sm as f32, (sp.sm + 2) as f32])
            .style(button_style(tokens, variant));
        npm_buttons = npm_buttons.push(b);
    }
    let npm_row = setting_row(
        tokens,
        envr_core::i18n::tr_key("gui.runtime.node.npm_registry", "npm 源", "npm registry"),
        None,
        npm_buttons.into(),
    );

    let proxy_toggle = setting_row(
        tokens,
        envr_core::i18n::tr_key(
            "gui.runtime.node.path_proxy.label",
            "PATH 代理",
            "PATH proxy",
        ),
        Some(envr_core::i18n::tr_key(
            "gui.runtime.node.path_proxy.hint_short",
            "开启时由 envr 接管 node/npm/npx；关闭时 shim 透传到系统 PATH。",
            "When on, envr manages node/npm/npx; when off, shims passthrough to system PATH.",
        )),
        toggler(node.path_proxy_enabled)
            .label("")
            .size(20.0)
            .spacing(0.0)
            .on_toggle(|v| Message::EnvCenter(EnvCenterMsg::SetNodePathProxy(v)))
            .into(),
    );
    let proxy_note = text(envr_core::i18n::tr_key(
        "gui.runtime.node.path_proxy.note",
        "关闭时无法使用「切换」「安装并切换」。",
        "When off, Use / Install & Use are disabled.",
    ))
    .size(ty.micro)
    .color(muted);

    container(
        column![dl_row, npm_row, proxy_toggle, proxy_note]
            .spacing(sp.sm as f32)
            .width(Length::Fill),
    )
    .padding(Padding::from([sp.md as f32, sp.md as f32]))
    .style(card_container_style(tokens, 1))
    .into()
}

fn python_runtime_settings_section(
    py: &PythonRuntimeSettings,
    tokens: ThemeTokens,
) -> Element<'static, Message> {
    let ty = tokens.typography();
    let sp = tokens.space();
    let muted = gui_theme::to_color(tokens.colors.text_muted);

    let mut dl_buttons = row![].spacing(-1.0);
    for src in [
        PythonDownloadSource::Auto,
        PythonDownloadSource::Domestic,
        PythonDownloadSource::Official,
    ] {
        let lab = match src {
            PythonDownloadSource::Auto => envr_core::i18n::tr_key(
                "gui.runtime.python.ds.auto",
                "自动（随区域语言）",
                "Auto (locale)",
            ),
            PythonDownloadSource::Domestic => envr_core::i18n::tr_key(
                "gui.runtime.python.ds.domestic",
                "国内镜像",
                "China mirror",
            ),
            PythonDownloadSource::Official => {
                envr_core::i18n::tr_key("gui.runtime.python.ds.official", "官方", "Official")
            }
        };
        let variant = if src == py.download_source {
            ButtonVariant::Primary
        } else {
            ButtonVariant::Secondary
        };
        let h = if src == py.download_source {
            tokens.control_height_primary
        } else {
            tokens.control_height_secondary
        };
        dl_buttons = dl_buttons.push(
            button(button_content_centered(text(lab).into()))
                .on_press(Message::EnvCenter(EnvCenterMsg::SetPythonDownloadSource(
                    src,
                )))
                .width(Length::Shrink)
                .height(Length::Fixed(h))
                .padding([sp.sm as f32, (sp.sm + 2) as f32])
                .style(button_style(tokens, variant)),
        );
    }
    let dl_row = setting_row(
        tokens,
        envr_core::i18n::tr_key(
            "gui.runtime.python.download_source",
            "Python 下载源",
            "Python download source",
        ),
        Some(envr_core::i18n::tr_key(
            "gui.runtime.python.download_source_hint",
            "影响 Python 安装包与 get-pip.py 下载来源。",
            "Affects Python artifacts and get-pip.py source.",
        )),
        dl_buttons.into(),
    );

    let mut pip_buttons = row![].spacing(-1.0);
    for mode in [
        PipRegistryMode::Auto,
        PipRegistryMode::Domestic,
        PipRegistryMode::Official,
        PipRegistryMode::Restore,
    ] {
        let lab = match mode {
            PipRegistryMode::Auto => envr_core::i18n::tr_key(
                "gui.runtime.python.pip.auto",
                "自动（按区域）",
                "Auto (locale)",
            ),
            PipRegistryMode::Domestic => envr_core::i18n::tr_key(
                "gui.runtime.python.pip.domestic",
                "国内镜像",
                "China mirror",
            ),
            PipRegistryMode::Official => {
                envr_core::i18n::tr_key("gui.runtime.python.pip.official", "官方", "Official")
            }
            PipRegistryMode::Restore => envr_core::i18n::tr_key(
                "gui.runtime.python.pip.restore",
                "还原（不改源）",
                "Restore (no change)",
            ),
        };
        let variant = if mode == py.pip_registry_mode {
            ButtonVariant::Primary
        } else {
            ButtonVariant::Secondary
        };
        let h = if mode == py.pip_registry_mode {
            tokens.control_height_primary
        } else {
            tokens.control_height_secondary
        };
        pip_buttons = pip_buttons.push(
            button(button_content_centered(text(lab).into()))
                .on_press(Message::EnvCenter(EnvCenterMsg::SetPipRegistryMode(mode)))
                .width(Length::Shrink)
                .height(Length::Fixed(h))
                .padding([sp.sm as f32, (sp.sm + 2) as f32])
                .style(button_style(tokens, variant)),
        );
    }
    let pip_row = setting_row(
        tokens,
        envr_core::i18n::tr_key(
            "gui.runtime.python.pip_registry",
            "pip 引导源",
            "pip bootstrap",
        ),
        None,
        pip_buttons.into(),
    );

    let proxy_toggle = setting_row(
        tokens,
        envr_core::i18n::tr_key("gui.runtime.python.path_proxy", "PATH 代理", "PATH proxy"),
        Some(envr_core::i18n::tr_key(
            "gui.runtime.python.path_proxy.hint_short",
            "开启时由 envr 接管 python/pip；关闭时 shim 透传到系统 PATH。",
            "When on, envr manages python/pip; when off, shims passthrough to system PATH.",
        )),
        toggler(py.path_proxy_enabled)
            .label("")
            .size(20.0)
            .spacing(0.0)
            .on_toggle(|v| Message::EnvCenter(EnvCenterMsg::SetPythonPathProxy(v)))
            .into(),
    );
    let proxy_note = text(envr_core::i18n::tr_key(
        "gui.runtime.python.path_proxy.note",
        "关闭时无法使用「切换」「安装并切换」。",
        "While off, Use / Install & Use are disabled.",
    ))
    .size(ty.micro)
    .color(muted);

    let cache_note = text(envr_core::i18n::tr_key(
        "gui.runtime.python.getpip.cache",
        "get-pip.py 会缓存在用户目录并按天刷新；download_source 也会决定其下载地址。",
        "get-pip.py is cached under user data and refreshed daily; download_source also affects where it is fetched from.",
    ))
    .size(ty.micro)
    .color(muted);

    container(
        column![dl_row, pip_row, proxy_toggle, proxy_note, cache_note,]
            .spacing(sp.sm as f32)
            .width(Length::Fill),
    )
    .padding(Padding::from([sp.md as f32, sp.md as f32]))
    .style(card_container_style(tokens, 1))
    .into()
}

fn java_runtime_settings_section(
    java: &JavaRuntimeSettings,
    tokens: ThemeTokens,
) -> Element<'static, Message> {
    let sp = tokens.space();

    let sources = [
        JavaDownloadSource::Auto,
        JavaDownloadSource::Domestic,
        JavaDownloadSource::Official,
    ];
    let mut dl_buttons = row![].spacing(-1.0);
    for (idx, src) in sources.iter().copied().enumerate() {
        let label = match src {
            JavaDownloadSource::Auto => {
                envr_core::i18n::tr_key("gui.runtime.java.ds.auto", "自动", "Auto")
            }
            JavaDownloadSource::Domestic => {
                envr_core::i18n::tr_key("gui.runtime.java.ds.domestic", "国内优先", "China-first")
            }
            JavaDownloadSource::Official => {
                envr_core::i18n::tr_key("gui.runtime.java.ds.official", "官方", "Official")
            }
        };
        let variant = if src == java.download_source {
            ButtonVariant::Primary
        } else {
            ButtonVariant::Secondary
        };
        let pos = if sources.len() == 1 {
            SegmentPosition::Single
        } else if idx == 0 {
            SegmentPosition::Start
        } else if idx + 1 == sources.len() {
            SegmentPosition::End
        } else {
            SegmentPosition::Middle
        };
        dl_buttons = dl_buttons.push(
            button(button_content_centered(text(label).into()))
                .on_press(Message::EnvCenter(EnvCenterMsg::SetJavaDownloadSource(src)))
                .width(Length::Shrink)
                .height(Length::Fixed(tokens.control_height_secondary))
                .padding([sp.sm as f32, (sp.sm + 2) as f32])
                .style(segmented_button_style(tokens, variant, pos)),
        );
    }
    let dl_row = setting_row(
        tokens,
        envr_core::i18n::tr_key(
            "gui.runtime.java.download_source",
            "Java 下载源",
            "Java source",
        ),
        Some(envr_core::i18n::tr_key(
            "gui.runtime.java.download_source_hint",
            "仅支持 LTS（8/11/17/21/25）；部分发行版可能固定官方源。",
            "LTS only (8/11/17/21/25); some distros may always use official upstream.",
        )),
        dl_buttons.into(),
    );

    let proxy_row = setting_row(
        tokens,
        envr_core::i18n::tr_key("gui.runtime.java.path_proxy", "PATH 代理", "PATH proxy"),
        Some(envr_core::i18n::tr_key(
            "gui.runtime.java.path_proxy_hint_short",
            "开启时由 envr 接管 java/javac；关闭时 shim 透传到系统 PATH。",
            "When on, envr manages java/javac; when off, shims passthrough to system PATH.",
        )),
        toggler(java.path_proxy_enabled)
            .on_toggle(|v| Message::EnvCenter(EnvCenterMsg::SetJavaPathProxy(v)))
            .size(20.0)
            .spacing(sp.sm as f32)
            .into(),
    );

    container(column![dl_row, proxy_row].spacing(sp.md as f32))
        .padding(Padding::from([sp.sm as f32, sp.sm as f32]))
        .style(card_container_style(tokens, 1))
        .into()
}

fn go_runtime_settings_section(
    go: &GoRuntimeSettings,
    drafts: (&str, &str),
    tokens: ThemeTokens,
) -> Element<'static, Message> {
    let ty = tokens.typography();
    let sp = tokens.space();
    let muted = gui_theme::to_color(tokens.colors.text_muted);
    let (proxy_draft, private_draft) = drafts;

    let dl_sources = [
        GoDownloadSource::Auto,
        GoDownloadSource::Domestic,
        GoDownloadSource::Official,
    ];
    let mut dl_buttons = row![].spacing(-1.0);
    for (idx, src) in dl_sources.iter().copied().enumerate() {
        let lab = match src {
            GoDownloadSource::Auto => envr_core::i18n::tr_key(
                "gui.runtime.go.ds.auto",
                "自动（随区域语言）",
                "Auto (locale)",
            ),
            GoDownloadSource::Domestic => envr_core::i18n::tr_key(
                "gui.runtime.go.ds.domestic",
                "国内（golang.google.cn）",
                "China (golang.google.cn)",
            ),
            GoDownloadSource::Official => envr_core::i18n::tr_key(
                "gui.runtime.go.ds.official",
                "官方（go.dev）",
                "Official (go.dev)",
            ),
        };
        let variant = if src == go.download_source {
            ButtonVariant::Primary
        } else {
            ButtonVariant::Secondary
        };
        let h = if src == go.download_source {
            tokens.control_height_primary
        } else {
            tokens.control_height_secondary
        };
        let pos = if dl_sources.len() == 1 {
            SegmentPosition::Single
        } else if idx == 0 {
            SegmentPosition::Start
        } else if idx + 1 == dl_sources.len() {
            SegmentPosition::End
        } else {
            SegmentPosition::Middle
        };
        dl_buttons = dl_buttons.push(
            button(button_content_centered(text(lab).into()))
                .on_press(Message::EnvCenter(EnvCenterMsg::SetGoDownloadSource(src)))
                .width(Length::Shrink)
                .height(Length::Fixed(h))
                .padding([sp.sm as f32, (sp.sm + 2) as f32])
                .style(segmented_button_style(tokens, variant, pos)),
        );
    }
    let dl_row = setting_row(
        tokens,
        envr_core::i18n::tr_key(
            "gui.runtime.go.download_source",
            "Go 下载源",
            "Go download source",
        ),
        None,
        dl_buttons.into(),
    );

    let gp_modes = [
        GoProxyMode::Auto,
        GoProxyMode::Domestic,
        GoProxyMode::Official,
        GoProxyMode::Direct,
        GoProxyMode::Custom,
    ];
    let mut gp_buttons = row![].spacing(-1.0);
    for (idx, mode) in gp_modes.iter().copied().enumerate() {
        let lab = match mode {
            GoProxyMode::Auto => envr_core::i18n::tr_key("gui.runtime.go.gp.auto", "自动", "Auto"),
            GoProxyMode::Domestic => {
                envr_core::i18n::tr_key("gui.runtime.go.gp.domestic", "国内", "China")
            }
            GoProxyMode::Official => {
                envr_core::i18n::tr_key("gui.runtime.go.gp.official", "官方", "Official")
            }
            GoProxyMode::Direct => {
                envr_core::i18n::tr_key("gui.runtime.go.gp.direct", "直连", "Direct")
            }
            GoProxyMode::Custom => {
                envr_core::i18n::tr_key("gui.runtime.go.gp.custom", "自定义", "Custom")
            }
        };
        let variant = if mode == go.proxy_mode {
            ButtonVariant::Primary
        } else {
            ButtonVariant::Secondary
        };
        let h = if mode == go.proxy_mode {
            tokens.control_height_primary
        } else {
            tokens.control_height_secondary
        };
        let pos = if gp_modes.len() == 1 {
            SegmentPosition::Single
        } else if idx == 0 {
            SegmentPosition::Start
        } else if idx + 1 == gp_modes.len() {
            SegmentPosition::End
        } else {
            SegmentPosition::Middle
        };
        gp_buttons = gp_buttons.push(
            button(button_content_centered(text(lab).into()))
                .on_press(Message::EnvCenter(EnvCenterMsg::SetGoProxyMode(mode)))
                .width(Length::Shrink)
                .height(Length::Fixed(h))
                .padding([sp.sm as f32, (sp.sm + 2) as f32])
                .style(segmented_button_style(tokens, variant, pos)),
        );
    }
    let gp_row = setting_row(
        tokens,
        envr_core::i18n::tr_key(
            "gui.runtime.go.goproxy_mode",
            "GOPROXY（模块代理）",
            "GOPROXY (module proxy)",
        ),
        None,
        gp_buttons.into(),
    );

    let custom_block: Element<'static, Message> = if go.proxy_mode == GoProxyMode::Custom {
        row![
            container(
                text_input("https://goproxy.cn,direct", proxy_draft)
                    .on_input(|s| Message::EnvCenter(EnvCenterMsg::SetGoProxyCustomDraft(s)))
                    .padding(sp.sm)
                    .width(Length::FillPortion(3))
                    .style(text_input_style(tokens)),
            )
            .width(Length::FillPortion(3))
            .height(Length::Fixed(tokens.control_height_secondary))
            .align_y(iced::alignment::Vertical::Center),
            button(button_content_centered(
                text(envr_core::i18n::tr_key(
                    "gui.runtime.go.apply_network",
                    "应用",
                    "Apply"
                ))
                .into(),
            ))
            .on_press(Message::EnvCenter(EnvCenterMsg::ApplyGoNetworkSettings))
            .width(Length::FillPortion(1))
            .height(Length::Fixed(tokens.control_height_secondary))
            .padding([sp.sm as f32, sp.sm as f32])
            .style(button_style(tokens, ButtonVariant::Secondary)),
        ]
        .spacing(sp.sm as f32)
        .align_y(Alignment::Center)
        .into()
    } else {
        column![].into()
    };

    let private_row = row![
        container(
            text_input("e.g. *.corp.example.com,github.com/myorg", private_draft)
                .on_input(|s| Message::EnvCenter(EnvCenterMsg::SetGoPrivatePatternsDraft(s)))
                .padding(sp.sm)
                .width(Length::FillPortion(3))
                .style(text_input_style(tokens)),
        )
        .width(Length::FillPortion(3))
        .height(Length::Fixed(tokens.control_height_secondary))
        .align_y(iced::alignment::Vertical::Center),
        button(button_content_centered(
            text(envr_core::i18n::tr_key(
                "gui.runtime.go.apply_private",
                "应用私有规则",
                "Apply private",
            ))
            .into(),
        ))
        .on_press(Message::EnvCenter(EnvCenterMsg::ApplyGoNetworkSettings))
        .width(Length::FillPortion(1))
        .height(Length::Fixed(tokens.control_height_secondary))
        .padding([sp.sm as f32, sp.sm as f32])
        .style(button_style(tokens, ButtonVariant::Secondary)),
    ]
    .spacing(sp.sm as f32)
    .align_y(Alignment::Center);

    let private_hint = text(envr_core::i18n::tr_key(
        "gui.runtime.go.private.hint",
        "非空时注入 GOPRIVATE / GONOSUMDB / GONOPROXY（逗号分隔域名/通配）。GOSUMDB 仍遵循 Go 默认或你本机环境。",
        "When non-empty, sets GOPRIVATE/GONOSUMDB/GONOPROXY (comma-separated). GOSUMDB stays Go default unless set in your environment.",
    ))
    .size(ty.micro)
    .color(muted);

    let proxy_toggle = setting_row(
        tokens,
        envr_core::i18n::tr_key("gui.runtime.go.path_proxy", "PATH 代理", "PATH proxy"),
        Some(envr_core::i18n::tr_key(
            "gui.runtime.go.path_proxy.hint",
            "开启时由 envr 接管 go/gofmt；关闭时 shim 透传到系统 PATH。",
            "When on, envr manages go/gofmt; when off, shims delegate to your system PATH.",
        )),
        toggler(go.path_proxy_enabled)
            .on_toggle(|v| Message::EnvCenter(EnvCenterMsg::SetGoPathProxy(v)))
            .into(),
    );

    let proxy_note = text(envr_core::i18n::tr_key(
        "gui.runtime.go.path_proxy.note",
        "关闭时无法使用「切换」「安装并切换」。",
        "While off, Use / Install & Use are disabled.",
    ))
    .size(ty.micro)
    .color(muted);

    container(
        column![
            dl_row,
            gp_row,
            custom_block,
            text(envr_core::i18n::tr_key(
                "gui.runtime.go.private_title",
                "私有模块（可选）",
                "Private modules (optional)",
            ))
            .size(ty.body),
            private_row,
            private_hint,
            proxy_toggle,
            proxy_note,
        ]
        .spacing(sp.sm as f32)
        .width(Length::Fill),
    )
    .padding(Padding::from([sp.md as f32, sp.md as f32]))
    .style(card_container_style(tokens, 1))
    .into()
}

fn php_runtime_settings_section(
    php: &PhpRuntimeSettings,
    tokens: ThemeTokens,
) -> Element<'static, Message> {
    let ty = tokens.typography();
    let sp = tokens.space();
    let muted = gui_theme::to_color(tokens.colors.text_muted);

    let ds_sources = [
        PhpDownloadSource::Auto,
        PhpDownloadSource::Domestic,
        PhpDownloadSource::Official,
    ];
    let mut ds_buttons = row![].spacing(-1.0);
    for (idx, src) in ds_sources.iter().copied().enumerate() {
        let label = match src {
            PhpDownloadSource::Auto => {
                envr_core::i18n::tr_key("gui.runtime.php.ds.auto", "自动", "Auto")
            }
            PhpDownloadSource::Domestic => {
                envr_core::i18n::tr_key("gui.runtime.php.ds.domestic", "国内镜像", "China mirror")
            }
            PhpDownloadSource::Official => {
                envr_core::i18n::tr_key("gui.runtime.php.ds.official", "官方", "Official")
            }
        };
        let active = src == php.download_source;
        let variant = if active {
            ButtonVariant::Primary
        } else {
            ButtonVariant::Secondary
        };
        let pos = if ds_sources.len() == 1 {
            SegmentPosition::Single
        } else if idx == 0 {
            SegmentPosition::Start
        } else if idx + 1 == ds_sources.len() {
            SegmentPosition::End
        } else {
            SegmentPosition::Middle
        };
        ds_buttons = ds_buttons.push(
            button(button_content_centered(text(label).into()))
                .on_press(Message::EnvCenter(EnvCenterMsg::SetPhpDownloadSource(src)))
                .width(Length::Shrink)
                .height(Length::Fixed(if active {
                    tokens.control_height_primary
                } else {
                    tokens.control_height_secondary
                }))
                .padding([sp.sm as f32, (sp.sm + 2) as f32])
                .style(segmented_button_style(tokens, variant, pos)),
        );
    }
    let ds_row = setting_row(
        tokens,
        envr_core::i18n::tr_key(
            "gui.runtime.php.download_source",
            "PHP 下载源",
            "PHP download source",
        ),
        None,
        ds_buttons.into(),
    );

    let proxy_toggle = setting_row(
        tokens,
        envr_core::i18n::tr_key("gui.runtime.php.path_proxy", "PATH 代理", "PATH proxy"),
        Some(envr_core::i18n::tr_key(
            "gui.runtime.php.path_proxy_hint",
            "开启时由 envr 接管 php；关闭时 shim 透传到系统 PATH。",
            "When on, envr manages php; when off, shims passthrough to system PATH.",
        )),
        toggler(php.path_proxy_enabled)
            .on_toggle(|v| Message::EnvCenter(EnvCenterMsg::SetPhpPathProxy(v)))
            .into(),
    );

    let unix_blurb = text(
        envr_core::i18n::tr_key(
            "gui.runtime.php.unix_settings_blurb",
            "Unix：通过本机已安装的 PHP（如 Homebrew / 发行版包）发现并注册到 envr；此处不下载安装包，亦无 NTS/TS 构建切换。",
            "Unix: envr discovers existing PHP installs (e.g. Homebrew or distro packages) and registers them; there is no zip download or NTS/TS toggle here.",
        ),
    )
    .size(ty.micro)
    .color(muted);

    let main_col = if cfg!(windows) {
        column![
            ds_row,
            proxy_toggle,
            text("关闭时无法使用「切换」「安装并切换」。")
                .size(ty.micro)
                .color(muted),
        ]
    } else {
        column![
            unix_blurb,
            proxy_toggle,
            text("关闭时无法使用「切换」「安装并切换」。")
                .size(ty.micro)
                .color(muted),
        ]
    };

    container(main_col.spacing(sp.sm as f32).width(Length::Fill))
        .padding(Padding::from([sp.md as f32, sp.md as f32]))
        .style(card_container_style(tokens, 1))
        .into()
}

fn php_windows_build_row(
    php: &PhpRuntimeSettings,
    tokens: ThemeTokens,
) -> Option<Element<'static, Message>> {
    if !cfg!(windows) {
        return None;
    }
    let sp = tokens.space();
    let builds = [PhpWindowsBuildFlavor::Nts, PhpWindowsBuildFlavor::Ts];
    let mut build_buttons = row![].spacing(-1.0);
    for (idx, flavor) in builds.iter().copied().enumerate() {
        let label = match flavor {
            PhpWindowsBuildFlavor::Nts => "NTS",
            PhpWindowsBuildFlavor::Ts => "TS",
        };
        let active = flavor == php.windows_build;
        let variant = if active {
            ButtonVariant::Primary
        } else {
            ButtonVariant::Secondary
        };
        let pos = if idx == 0 {
            SegmentPosition::Start
        } else if idx + 1 == builds.len() {
            SegmentPosition::End
        } else {
            SegmentPosition::Middle
        };
        build_buttons = build_buttons.push(
            button(button_content_centered(text(label).into()))
                .on_press(Message::EnvCenter(EnvCenterMsg::SetPhpWindowsBuild(flavor)))
                .width(Length::Shrink)
                .height(Length::Fixed(if active {
                    tokens.control_height_primary
                } else {
                    tokens.control_height_secondary
                }))
                .padding([sp.sm as f32, (sp.sm + 2) as f32])
                .style(segmented_button_style(tokens, variant, pos)),
        );
    }
    Some(
        setting_row(
            tokens,
            envr_core::i18n::tr_key("gui.runtime.php.windows_build", "Windows 构建", "Windows build"),
            Some(envr_core::i18n::tr_key(
                "gui.runtime.php.windows_build_hint",
                "切换后列表会刷新（NTS/TS 独立）。",
                "Switching refreshes the list (NTS/TS are independent).",
            )),
            build_buttons.into(),
        )
        .into(),
    )
}

fn deno_runtime_settings_section(
    deno: &DenoRuntimeSettings,
    tokens: ThemeTokens,
) -> Element<'static, Message> {
    let ty = tokens.typography();
    let sp = tokens.space();
    let muted = gui_theme::to_color(tokens.colors.text_muted);

    let dl_sources = [
        DenoDownloadSource::Auto,
        DenoDownloadSource::Domestic,
        DenoDownloadSource::Official,
    ];
    let mut dl_buttons = row![].spacing(-1.0);
    for (idx, src) in dl_sources.iter().copied().enumerate() {
        let lab = match src {
            DenoDownloadSource::Auto => envr_core::i18n::tr_key(
                "gui.runtime.deno.ds.auto",
                "自动（随区域语言）",
                "Auto (locale)",
            ),
            DenoDownloadSource::Domestic => {
                envr_core::i18n::tr_key("gui.runtime.deno.ds.domestic", "国内镜像", "China mirror")
            }
            DenoDownloadSource::Official => {
                envr_core::i18n::tr_key("gui.runtime.deno.ds.official", "官方", "Official")
            }
        };
        let variant = if src == deno.download_source {
            ButtonVariant::Primary
        } else {
            ButtonVariant::Secondary
        };
        let h = if src == deno.download_source {
            tokens.control_height_primary
        } else {
            tokens.control_height_secondary
        };
        let pos = if dl_sources.len() == 1 {
            SegmentPosition::Single
        } else if idx == 0 {
            SegmentPosition::Start
        } else if idx + 1 == dl_sources.len() {
            SegmentPosition::End
        } else {
            SegmentPosition::Middle
        };
        dl_buttons = dl_buttons.push(
            button(button_content_centered(text(lab).into()))
                .on_press(Message::EnvCenter(EnvCenterMsg::SetDenoDownloadSource(src)))
                .width(Length::Shrink)
                .height(Length::Fixed(h))
                .padding([sp.sm as f32, (sp.sm + 2) as f32])
                .style(segmented_button_style(tokens, variant, pos)),
        );
    }
    let dl_row = setting_row(
        tokens,
        envr_core::i18n::tr_key(
            "gui.runtime.deno.download_source",
            "Deno 下载源",
            "Deno source",
        ),
        Some(envr_core::i18n::tr_key(
            "gui.runtime.deno.download_source_hint",
            "控制二进制 zip 来源（dl.deno.land / npmmirror）。",
            "Controls where the release zip is downloaded from (dl.deno.land / npmmirror).",
        )),
        dl_buttons.into(),
    );

    let pkg_modes = [
        NpmRegistryMode::Auto,
        NpmRegistryMode::Domestic,
        NpmRegistryMode::Official,
        NpmRegistryMode::Restore,
    ];
    let mut pkg_buttons = row![].spacing(-1.0);
    for (idx, mode) in pkg_modes.iter().copied().enumerate() {
        let lab = match mode {
            NpmRegistryMode::Auto => envr_core::i18n::tr_key(
                "gui.runtime.deno.pkg.auto",
                "自动（随区域语言）",
                "Auto (locale)",
            ),
            NpmRegistryMode::Domestic => {
                envr_core::i18n::tr_key("gui.runtime.deno.pkg.domestic", "国内镜像", "China mirror")
            }
            NpmRegistryMode::Official => {
                envr_core::i18n::tr_key("gui.runtime.deno.pkg.official", "官方", "Official")
            }
            NpmRegistryMode::Restore => envr_core::i18n::tr_key(
                "gui.runtime.deno.pkg.restore",
                "还原（不注入）",
                "Restore (leave as-is)",
            ),
        };
        let variant = if mode == deno.package_source {
            ButtonVariant::Primary
        } else {
            ButtonVariant::Secondary
        };
        let h = if mode == deno.package_source {
            tokens.control_height_primary
        } else {
            tokens.control_height_secondary
        };
        let pos = if pkg_modes.len() == 1 {
            SegmentPosition::Single
        } else if idx == 0 {
            SegmentPosition::Start
        } else if idx + 1 == pkg_modes.len() {
            SegmentPosition::End
        } else {
            SegmentPosition::Middle
        };
        pkg_buttons = pkg_buttons.push(
            button(button_content_centered(text(lab).into()))
                .on_press(Message::EnvCenter(EnvCenterMsg::SetDenoPackageSource(mode)))
                .width(Length::Shrink)
                .height(Length::Fixed(h))
                .padding([sp.sm as f32, (sp.sm + 2) as f32])
                .style(segmented_button_style(tokens, variant, pos)),
        );
    }
    let pkg_row = setting_row(
        tokens,
        envr_core::i18n::tr_key(
            "gui.runtime.deno.package_source",
            "包源（npm + JSR）",
            "Package source (npm + JSR)",
        ),
        Some(envr_core::i18n::tr_key(
            "gui.runtime.deno.package_source_hint",
            "同时设置 NPM_CONFIG_REGISTRY 与 JSR_URL。",
            "Sets both NPM_CONFIG_REGISTRY and JSR_URL.",
        )),
        pkg_buttons.into(),
    );

    let proxy_toggle = setting_row(
        tokens,
        envr_core::i18n::tr_key("gui.runtime.deno.path_proxy", "PATH 代理", "PATH proxy"),
        Some(envr_core::i18n::tr_key(
            "gui.runtime.deno.path_proxy.label",
            "开启时由 envr 接管 deno；关闭时 shim 透传到系统 PATH。",
            "When on, envr manages deno; when off, shims passthrough to system PATH.",
        )),
        toggler(deno.path_proxy_enabled)
            .on_toggle(|v| Message::EnvCenter(EnvCenterMsg::SetDenoPathProxy(v)))
            .into(),
    );

    let main_col = column![
        dl_row,
        pkg_row,
        proxy_toggle,
        text(envr_core::i18n::tr_key(
            "gui.runtime.deno.path_proxy.off_hint",
            "关闭时无法使用「切换」「安装并切换」。",
            "When off, you can't Use / Install & Use.",
        ))
        .size(ty.micro)
        .color(muted),
    ];

    container(main_col.spacing(sp.sm as f32).width(Length::Fill))
        .padding(Padding::from([sp.md as f32, sp.md as f32]))
        .style(card_container_style(tokens, 1))
        .into()
}

fn bun_runtime_settings_section(
    bun: &BunRuntimeSettings,
    global_bin_dir_draft: &str,
    tokens: ThemeTokens,
) -> Element<'static, Message> {
    let ty = tokens.typography();
    let sp = tokens.space();
    let muted = gui_theme::to_color(tokens.colors.text_muted);

    let bin_dir_row = {
        let input = container(
            text_input("runtime.bun.global_bin_dir", global_bin_dir_draft)
                .on_input(|s| Message::EnvCenter(EnvCenterMsg::BunGlobalBinDirEdit(s)))
                .padding(sp.sm)
                .width(Length::Fixed(240.0))
                .style(text_input_style(tokens)),
        )
        .height(Length::Fixed(tokens.control_height_secondary))
        .align_y(iced::alignment::Vertical::Center);
        let apply = button(button_content_centered(
            text(envr_core::i18n::tr_key("gui.action.apply", "应用", "Apply")).into(),
        ))
        .on_press(Message::EnvCenter(EnvCenterMsg::ApplyBunGlobalBinDir))
        .height(Length::Fixed(tokens.control_height_secondary))
        .padding([sp.sm as f32, sp.md as f32])
        .style(button_style(tokens, ButtonVariant::Secondary));
        setting_row(
            tokens,
            envr_core::i18n::tr_key(
                "gui.runtime.bun.global_bin_dir",
                "全局 bin 目录",
                "Global bin dir",
            ),
            Some(envr_core::i18n::tr_key(
                "gui.runtime.bun.global_bin_dir_hint",
                "可选：覆盖 `bun pm bin -g`，用于 shim 同步全局 Bun 可执行文件。",
                "Optional: overrides `bun pm bin -g` result for syncing global Bun executables.",
            )),
            row![input, apply].spacing(sp.sm as f32).into(),
        )
    };

    let pkg_modes = [
        NpmRegistryMode::Auto,
        NpmRegistryMode::Domestic,
        NpmRegistryMode::Official,
        NpmRegistryMode::Restore,
    ];
    let mut pkg_buttons = row![].spacing(-1.0);
    for (idx, mode) in pkg_modes.iter().copied().enumerate() {
        let lab = match mode {
            NpmRegistryMode::Auto => envr_core::i18n::tr_key(
                "gui.runtime.bun.pkg.auto",
                "自动（随区域语言）",
                "Auto (locale)",
            ),
            NpmRegistryMode::Domestic => {
                envr_core::i18n::tr_key("gui.runtime.bun.pkg.domestic", "国内镜像", "China mirror")
            }
            NpmRegistryMode::Official => {
                envr_core::i18n::tr_key("gui.runtime.bun.pkg.official", "官方", "Official")
            }
            NpmRegistryMode::Restore => envr_core::i18n::tr_key(
                "gui.runtime.bun.pkg.restore",
                "还原（不注入）",
                "Restore (leave as-is)",
            ),
        };
        let variant = if mode == bun.package_source {
            ButtonVariant::Primary
        } else {
            ButtonVariant::Secondary
        };
        let h = if mode == bun.package_source {
            tokens.control_height_primary
        } else {
            tokens.control_height_secondary
        };
        let pos = if pkg_modes.len() == 1 {
            SegmentPosition::Single
        } else if idx == 0 {
            SegmentPosition::Start
        } else if idx + 1 == pkg_modes.len() {
            SegmentPosition::End
        } else {
            SegmentPosition::Middle
        };
        pkg_buttons = pkg_buttons.push(
            button(button_content_centered(text(lab).into()))
                .on_press(Message::EnvCenter(EnvCenterMsg::SetBunPackageSource(mode)))
                .width(Length::Shrink)
                .height(Length::Fixed(h))
                .padding([sp.sm as f32, (sp.sm + 2) as f32])
                .style(segmented_button_style(tokens, variant, pos)),
        );
    }
    let pkg_row = setting_row(
        tokens,
        envr_core::i18n::tr_key(
            "gui.runtime.bun.package_source",
            "包源（npm）",
            "Package source (npm)",
        ),
        Some(envr_core::i18n::tr_key(
            "gui.runtime.bun.package_source_hint",
            "设置 NPM_CONFIG_REGISTRY（restore 时不注入）。",
            "Sets NPM_CONFIG_REGISTRY (no injection on restore).",
        )),
        pkg_buttons.into(),
    );

    let proxy_toggle = setting_row(
        tokens,
        envr_core::i18n::tr_key("gui.runtime.bun.path_proxy", "PATH 代理", "PATH proxy"),
        Some(envr_core::i18n::tr_key(
            "gui.runtime.bun.path_proxy.label",
            "开启时由 envr 接管 bun/bunx；关闭时 shim 透传到系统 PATH。",
            "When on, envr manages bun/bunx; when off, shims passthrough to system PATH.",
        )),
        toggler(bun.path_proxy_enabled)
            .on_toggle(|v| Message::EnvCenter(EnvCenterMsg::SetBunPathProxy(v)))
            .into(),
    );

    let main_col = column![
        bin_dir_row,
        pkg_row,
        text(envr_core::i18n::tr_key(
            "gui.runtime.bun.win_support_note",
            "Windows 仅支持 Bun 1.x+（0.x 无官方 Windows 发布资产，已在列表中隐藏）。",
            "Windows supports Bun 1.x+ only (0.x has no official Windows release assets and is hidden).",
        ))
        .size(ty.micro)
        .color(muted),
        proxy_toggle,
        text(envr_core::i18n::tr_key(
            "gui.runtime.bun.path_proxy.off_hint",
            "关闭时无法使用「切换」「安装并切换」。",
            "When off, you can't Use / Install & Use.",
        ))
        .size(ty.micro)
        .color(muted),
    ];

    container(main_col.spacing(sp.sm as f32).width(Length::Fill))
        .padding(Padding::from([sp.md as f32, sp.md as f32]))
        .style(card_container_style(tokens, 1))
        .into()
}

fn remote_error_inline(tokens: ThemeTokens, error: &str) -> Element<'static, Message> {
    let ty = tokens.typography();
    let muted = gui_theme::to_color(tokens.colors.text_muted);
    let warn = gui_theme::to_color(tokens.colors.warning);
    let sp = tokens.space();
    container(
        row![
            Lucide::CircleAlert.view(16.0, warn),
            text(format!(
                "{}: {}",
                envr_core::i18n::tr_key(
                    "gui.runtime.remote_error",
                    "远程列表不可用",
                    "Remote list unavailable"
                ),
                error
            ))
            .size(ty.caption)
            .color(muted)
            .width(Length::Fill),
        ]
        .spacing(sp.sm as f32)
        .align_y(Alignment::Center),
    )
    .padding(Padding::from([sp.sm as f32, sp.md as f32]))
    .style(card_container_style(tokens, 1))
    .into()
}

fn ruby_runtime_settings_section(
    ruby: &RubyRuntimeSettings,
    tokens: ThemeTokens,
) -> Element<'static, Message> {
    let ty = tokens.typography();
    let sp = tokens.space();
    let muted = gui_theme::to_color(tokens.colors.text_muted);

    let proxy_toggle = setting_row(
        tokens,
        envr_core::i18n::tr_key("gui.runtime.ruby.path_proxy", "PATH 代理", "PATH proxy"),
        Some(envr_core::i18n::tr_key(
            "gui.runtime.ruby.path_proxy.hint",
            "开启时由 envr 接管 ruby/gem/bundle/irb；关闭时 shim 透传到系统 PATH。",
            "When on, envr manages ruby/gem/bundle/irb; when off, shim passthrough goes to system PATH.",
        )),
        toggler(ruby.path_proxy_enabled)
            .label("")
            .size(20.0)
            .spacing(0.0)
            .on_toggle(|v| Message::EnvCenter(EnvCenterMsg::SetRubyPathProxy(v)))
            .into(),
    );

    let proxy_note = text(envr_core::i18n::tr_key(
        "gui.runtime.ruby.path_proxy.note",
        "关闭时无法使用「切换」「安装并切换」。",
        "When off, Use / Install & Use are disabled.",
    ))
    .size(ty.micro)
    .color(muted);

    container(
        column![proxy_toggle, proxy_note]
            .spacing(sp.sm as f32)
            .width(Length::Fill),
    )
    .padding(Padding::from([sp.md as f32, sp.md as f32]))
    .style(card_container_style(tokens, 1))
    .into()
}

fn dotnet_runtime_settings_section(
    dotnet: &DotnetRuntimeSettings,
    tokens: ThemeTokens,
) -> Element<'static, Message> {
    let ty = tokens.typography();
    let sp = tokens.space();
    let muted = gui_theme::to_color(tokens.colors.text_muted);

    let proxy_toggle = setting_row(
        tokens,
        envr_core::i18n::tr_key("gui.runtime.dotnet.path_proxy", "PATH 代理", "PATH proxy"),
        Some(envr_core::i18n::tr_key(
            "gui.runtime.dotnet.path_proxy.hint",
            "开启时由 envr 接管 dotnet；关闭时 shim 透传到系统 PATH。",
            "When on, envr manages dotnet; when off, shim passthrough goes to system PATH.",
        )),
        toggler(dotnet.path_proxy_enabled)
            .label("")
            .size(20.0)
            .spacing(0.0)
            .on_toggle(|v| Message::EnvCenter(EnvCenterMsg::SetDotnetPathProxy(v)))
            .into(),
    );
    let proxy_note = text(envr_core::i18n::tr_key(
        "gui.runtime.dotnet.path_proxy.note",
        "关闭时无法使用「切换」「安装并切换」。",
        "When off, Use / Install & Use are disabled.",
    ))
    .size(ty.micro)
    .color(muted);

    container(
        column![proxy_toggle, proxy_note]
            .spacing(sp.sm as f32)
            .width(Length::Fill),
    )
    .padding(Padding::from([sp.md as f32, sp.md as f32]))
    .style(card_container_style(tokens, 1))
    .into()
}

fn julia_runtime_settings_section(
    julia: &JuliaRuntimeSettings,
    tokens: ThemeTokens,
) -> Element<'static, Message> {
    let ty = tokens.typography();
    let sp = tokens.space();
    let muted = gui_theme::to_color(tokens.colors.text_muted);

    let proxy_toggle = setting_row(
        tokens,
        envr_core::i18n::tr_key("gui.runtime.julia.path_proxy", "PATH 代理", "PATH proxy"),
        Some(envr_core::i18n::tr_key(
            "gui.runtime.julia.path_proxy.hint",
            "开启时由 envr 接管 julia；关闭时 shim 透传到系统 PATH。",
            "When on, envr manages julia; when off, shim passthrough goes to system PATH.",
        )),
        toggler(julia.path_proxy_enabled)
            .label("")
            .size(20.0)
            .spacing(0.0)
            .on_toggle(|v| Message::EnvCenter(EnvCenterMsg::SetJuliaPathProxy(v)))
            .into(),
    );
    let proxy_note = text(envr_core::i18n::tr_key(
        "gui.runtime.julia.path_proxy.note",
        "关闭时无法使用「切换」「安装并切换」。",
        "When off, Use / Install & Use are disabled.",
    ))
    .size(ty.micro)
    .color(muted);

    container(
        column![proxy_toggle, proxy_note]
            .spacing(sp.sm as f32)
            .width(Length::Fill),
    )
    .padding(Padding::from([sp.md as f32, sp.md as f32]))
    .style(card_container_style(tokens, 1))
    .into()
}

fn lua_runtime_settings_section(
    lua: &LuaRuntimeSettings,
    tokens: ThemeTokens,
) -> Element<'static, Message> {
    let ty = tokens.typography();
    let sp = tokens.space();
    let muted = gui_theme::to_color(tokens.colors.text_muted);

    let proxy_toggle = setting_row(
        tokens,
        envr_core::i18n::tr_key("gui.runtime.lua.path_proxy", "PATH 代理", "PATH proxy"),
        Some(envr_core::i18n::tr_key(
            "gui.runtime.lua.path_proxy.hint",
            "开启时由 envr 接管 lua / luac；关闭时 shim 透传到系统 PATH。",
            "When on, envr manages lua/luac; when off, shim passthrough goes to system PATH.",
        )),
        toggler(lua.path_proxy_enabled)
            .label("")
            .size(20.0)
            .spacing(0.0)
            .on_toggle(|v| Message::EnvCenter(EnvCenterMsg::SetLuaPathProxy(v)))
            .into(),
    );
    let proxy_note = text(envr_core::i18n::tr_key(
        "gui.runtime.lua.path_proxy.note",
        "关闭时无法使用「切换」「安装并切换」。",
        "When off, Use / Install & Use are disabled.",
    ))
    .size(ty.micro)
    .color(muted);

    container(
        column![proxy_toggle, proxy_note]
            .spacing(sp.sm as f32)
            .width(Length::Fill),
    )
    .padding(Padding::from([sp.md as f32, sp.md as f32]))
    .style(card_container_style(tokens, 1))
    .into()
}

fn rlang_runtime_settings_section(
    rlang: &RlangRuntimeSettings,
    tokens: ThemeTokens,
) -> Element<'static, Message> {
    let ty = tokens.typography();
    let sp = tokens.space();
    let muted = gui_theme::to_color(tokens.colors.text_muted);

    let proxy_toggle = setting_row(
        tokens,
        envr_core::i18n::tr_key("gui.runtime.r.path_proxy", "PATH 代理", "PATH proxy"),
        Some(envr_core::i18n::tr_key(
            "gui.runtime.r.path_proxy.hint",
            "开启时由 envr 接管 R / Rscript；关闭时 shim 透传到系统 PATH。",
            "When on, envr manages R/Rscript; when off, shim passthrough goes to system PATH.",
        )),
        toggler(rlang.path_proxy_enabled)
            .label("")
            .size(20.0)
            .spacing(0.0)
            .on_toggle(|v| Message::EnvCenter(EnvCenterMsg::SetRLangPathProxy(v)))
            .into(),
    );
    let proxy_note = text(envr_core::i18n::tr_key(
        "gui.runtime.r.path_proxy.note",
        "关闭时无法使用「切换」「安装并切换」。",
        "When off, Use / Install & Use are disabled.",
    ))
    .size(ty.micro)
    .color(muted);

    container(
        column![proxy_toggle, proxy_note]
            .spacing(sp.sm as f32)
            .width(Length::Fill),
    )
    .padding(Padding::from([sp.md as f32, sp.md as f32]))
    .style(card_container_style(tokens, 1))
    .into()
}

fn crystal_runtime_settings_section(
    crystal: &CrystalRuntimeSettings,
    tokens: ThemeTokens,
) -> Element<'static, Message> {
    let ty = tokens.typography();
    let sp = tokens.space();
    let muted = gui_theme::to_color(tokens.colors.text_muted);

    let proxy_toggle = setting_row(
        tokens,
        envr_core::i18n::tr_key("gui.runtime.crystal.path_proxy", "PATH 代理", "PATH proxy"),
        Some(envr_core::i18n::tr_key(
            "gui.runtime.crystal.path_proxy.hint",
            "开启时由 envr 接管 crystal；关闭时 shim 透传到系统 PATH。",
            "When on, envr manages crystal; when off, shim passthrough goes to system PATH.",
        )),
        toggler(crystal.path_proxy_enabled)
            .label("")
            .size(20.0)
            .spacing(0.0)
            .on_toggle(|v| Message::EnvCenter(EnvCenterMsg::SetCrystalPathProxy(v)))
            .into(),
    );
    let proxy_note = text(envr_core::i18n::tr_key(
        "gui.runtime.crystal.path_proxy.note",
        "关闭时无法使用「切换」「安装并切换」。",
        "When off, Use / Install & Use are disabled.",
    ))
    .size(ty.micro)
    .color(muted);

    container(
        column![proxy_toggle, proxy_note]
            .spacing(sp.sm as f32)
            .width(Length::Fill),
    )
    .padding(Padding::from([sp.md as f32, sp.md as f32]))
    .style(card_container_style(tokens, 1))
    .into()
}

fn nim_runtime_settings_section(
    nim: &NimRuntimeSettings,
    tokens: ThemeTokens,
) -> Element<'static, Message> {
    let ty = tokens.typography();
    let sp = tokens.space();
    let muted = gui_theme::to_color(tokens.colors.text_muted);

    let proxy_toggle = setting_row(
        tokens,
        envr_core::i18n::tr_key("gui.runtime.nim.path_proxy", "PATH 代理", "PATH proxy"),
        Some(envr_core::i18n::tr_key(
            "gui.runtime.nim.path_proxy.hint",
            "开启时由 envr 接管 nim；关闭时 shim 透传到系统 PATH。",
            "When on, envr manages nim; when off, shim passthrough goes to system PATH.",
        )),
        toggler(nim.path_proxy_enabled)
            .label("")
            .size(20.0)
            .spacing(0.0)
            .on_toggle(|v| Message::EnvCenter(EnvCenterMsg::SetNimPathProxy(v)))
            .into(),
    );
    let proxy_note = text(envr_core::i18n::tr_key(
        "gui.runtime.nim.path_proxy.note",
        "关闭时无法使用「切换」「安装并切换」。",
        "When off, Use / Install & Use are disabled.",
    ))
    .size(ty.micro)
    .color(muted);

    container(
        column![proxy_toggle, proxy_note]
            .spacing(sp.sm as f32)
            .width(Length::Fill),
    )
    .padding(Padding::from([sp.md as f32, sp.md as f32]))
    .style(card_container_style(tokens, 1))
    .into()
}

fn zig_runtime_settings_section(
    zig: &ZigRuntimeSettings,
    tokens: ThemeTokens,
) -> Element<'static, Message> {
    let ty = tokens.typography();
    let sp = tokens.space();
    let muted = gui_theme::to_color(tokens.colors.text_muted);

    let proxy_toggle = setting_row(
        tokens,
        envr_core::i18n::tr_key("gui.runtime.zig.path_proxy", "PATH 代理", "PATH proxy"),
        Some(envr_core::i18n::tr_key(
            "gui.runtime.zig.path_proxy.hint",
            "开启时由 envr 接管 zig；关闭时 shim 透传到系统 PATH。",
            "When on, envr manages zig; when off, shim passthrough goes to system PATH.",
        )),
        toggler(zig.path_proxy_enabled)
            .label("")
            .size(20.0)
            .spacing(0.0)
            .on_toggle(|v| Message::EnvCenter(EnvCenterMsg::SetZigPathProxy(v)))
            .into(),
    );
    let proxy_note = text(envr_core::i18n::tr_key(
        "gui.runtime.zig.path_proxy.note",
        "关闭时无法使用「切换」「安装并切换」。",
        "When off, Use / Install & Use are disabled.",
    ))
    .size(ty.micro)
    .color(muted);

    container(
        column![proxy_toggle, proxy_note]
            .spacing(sp.sm as f32)
            .width(Length::Fill),
    )
    .padding(Padding::from([sp.md as f32, sp.md as f32]))
    .style(card_container_style(tokens, 1))
    .into()
}

fn elixir_runtime_settings_section(
    elixir: &ElixirRuntimeSettings,
    tokens: ThemeTokens,
) -> Element<'static, Message> {
    let ty = tokens.typography();
    let sp = tokens.space();
    let muted = gui_theme::to_color(tokens.colors.text_muted);

    let proxy_toggle = setting_row(
        tokens,
        envr_core::i18n::tr_key("gui.runtime.elixir.path_proxy", "PATH 代理", "PATH proxy"),
        Some(envr_core::i18n::tr_key(
            "gui.runtime.elixir.path_proxy.hint",
            "开启时由 envr 接管 elixir/mix/iex；关闭时 shim 透传到系统 PATH。",
            "When on, envr manages elixir/mix/iex; when off, shim passthrough goes to system PATH.",
        )),
        toggler(elixir.path_proxy_enabled)
            .label("")
            .size(20.0)
            .spacing(0.0)
            .on_toggle(|v| Message::EnvCenter(EnvCenterMsg::SetElixirPathProxy(v)))
            .into(),
    );
    let proxy_note = text(envr_core::i18n::tr_key(
        "gui.runtime.elixir.path_proxy.note",
        "关闭时无法使用「切换」「安装并切换」。",
        "When off, Use / Install & Use are disabled.",
    ))
    .size(ty.micro)
    .color(muted);

    container(
        column![proxy_toggle, proxy_note]
            .spacing(sp.sm as f32)
            .width(Length::Fill),
    )
    .padding(Padding::from([sp.md as f32, sp.md as f32]))
    .style(card_container_style(tokens, 1))
    .into()
}

fn erlang_runtime_settings_section(
    erlang: &ErlangRuntimeSettings,
    tokens: ThemeTokens,
) -> Element<'static, Message> {
    let ty = tokens.typography();
    let sp = tokens.space();
    let muted = gui_theme::to_color(tokens.colors.text_muted);

    let proxy_toggle = setting_row(
        tokens,
        envr_core::i18n::tr_key("gui.runtime.erlang.path_proxy", "PATH 代理", "PATH proxy"),
        Some(envr_core::i18n::tr_key(
            "gui.runtime.erlang.path_proxy.hint",
            "开启时由 envr 接管 erl/erlc/escript；关闭时 shim 透传到系统 PATH。",
            "When on, envr manages erl/erlc/escript; when off, shim passthrough goes to system PATH.",
        )),
        toggler(erlang.path_proxy_enabled)
            .label("")
            .size(20.0)
            .spacing(0.0)
            .on_toggle(|v| Message::EnvCenter(EnvCenterMsg::SetErlangPathProxy(v)))
            .into(),
    );
    let proxy_note = text(envr_core::i18n::tr_key(
        "gui.runtime.erlang.path_proxy.note",
        "关闭时无法使用「切换」「安装并切换」。",
        "When off, Use / Install & Use are disabled.",
    ))
    .size(ty.micro)
    .color(muted);

    container(
        column![proxy_toggle, proxy_note]
            .spacing(sp.sm as f32)
            .width(Length::Fill),
    )
    .padding(Padding::from([sp.md as f32, sp.md as f32]))
    .style(card_container_style(tokens, 1))
    .into()
}

#[allow(clippy::too_many_arguments)]
pub fn env_center_view(
    state: &EnvCenterState,
    node_runtime: Option<&NodeRuntimeSettings>,
    python_runtime: Option<&PythonRuntimeSettings>,
    java_runtime: Option<&JavaRuntimeSettings>,
    go_runtime: Option<&GoRuntimeSettings>,
    rust_runtime: Option<&RustRuntimeSettings>,
    ruby_runtime: Option<&RubyRuntimeSettings>,
    elixir_runtime: Option<&ElixirRuntimeSettings>,
    erlang_runtime: Option<&ErlangRuntimeSettings>,
    php_runtime: Option<&PhpRuntimeSettings>,
    deno_runtime: Option<&DenoRuntimeSettings>,
    bun_runtime: Option<&BunRuntimeSettings>,
    dotnet_runtime: Option<&DotnetRuntimeSettings>,
    zig_runtime: Option<&ZigRuntimeSettings>,
    julia_runtime: Option<&JuliaRuntimeSettings>,
    lua_runtime: Option<&LuaRuntimeSettings>,
    nim_runtime: Option<&NimRuntimeSettings>,
    crystal_runtime: Option<&CrystalRuntimeSettings>,
    r_runtime: Option<&RlangRuntimeSettings>,
    runtime_settings: &RuntimeSettings,
    tokens: ThemeTokens,
) -> Element<'static, Message> {
    let ty = tokens.typography();
    let sp = tokens.space();
    let busy = state.busy;
    let card_s = card_container_style(tokens, 1);

    if state.kind == RuntimeKind::Rust {
        return rust_env_center_view(state, rust_runtime, tokens);
    }

    let path_proxy_on = runtime_settings
        .path_proxy_enabled_for_kind(state.kind)
        .unwrap_or(true);

    let cur_line = match &state.current {
        Some(v) => {
            let mut s = format!(
                "{} {}",
                envr_core::i18n::tr_key("gui.runtime.current", "当前：", "Current:"),
                v.0
            );
            if state.kind == RuntimeKind::Php
                && let Some(ts) = state.php_global_current_want_ts
            {
                s.push_str(if ts { " · TS" } else { " · NTS" });
            }
            s
        }
        None => envr_core::i18n::tr_key(
            "gui.runtime.current_none",
            "当前：(未设置)",
            "Current: (not set)",
        ),
    };

    let header_title = format!("{}设置", kind_label(state.kind));
    let show_runtime_fold =
        envr_domain::runtime::unified_major_list_rollout_enabled(state.kind);
    let toggle_lbl = if state.runtime_settings_expanded {
        envr_core::i18n::tr_key("gui.action.collapse", "折叠", "Collapse")
    } else {
        envr_core::i18n::tr_key("gui.action.expand", "展开", "Expand")
    };

    let toggle_btn = button(button_content_centered(
        row![
            Lucide::ChevronsUpDown.view(16.0, gui_theme::to_color(tokens.colors.text)),
            text(toggle_lbl),
        ]
        .spacing(sp.xs as f32)
        .align_y(Alignment::Center)
        .into(),
    ))
    .on_press_maybe(
        show_runtime_fold.then_some(Message::EnvCenter(EnvCenterMsg::ToggleRuntimeSettings)),
    )
    .height(Length::Fixed(tokens.control_height_secondary))
    .style(button_style(tokens, ButtonVariant::Secondary));

    let cur_el = text(cur_line)
        .size(ty.caption)
        .color(gui_theme::to_color(tokens.colors.text_muted));

    let header_content = if show_runtime_fold {
        row![text(header_title).size(ty.section), cur_el, toggle_btn,]
            .spacing(sp.sm as f32)
            .align_y(Alignment::Center)
            .width(Length::Fill)
    } else {
        row![text(header_title).size(ty.section), cur_el]
            .spacing(sp.sm as f32)
            .align_y(Alignment::Center)
            .width(Length::Fill)
    };

    let header = container(header_content)
        .padding(Padding::from([sp.md as f32, sp.md as f32]))
        .style(move |theme: &Theme| card_s(theme));

    let txt = gui_theme::to_color(tokens.colors.text);

    let prereq_hint: Option<Element<'static, Message>> = if state.kind == RuntimeKind::Elixir {
        state.elixir_prereq_error.as_ref().map(|msg| {
            let msg = msg.clone();
            let ty = tokens.typography();
            let muted = gui_theme::to_color(tokens.colors.text_muted);
            let warn = gui_theme::to_color(tokens.colors.warning);
            let title = text(envr_core::i18n::tr_key(
                "gui.runtime.elixir.prereq.title",
                "前置依赖：Erlang/OTP",
                "Prerequisite: Erlang/OTP",
            ))
            .size(ty.caption)
            .color(warn);
            let body = text(msg).size(ty.caption).color(muted);
            container(column![title, body].spacing(sp.xs as f32))
                .padding(Padding::from([sp.sm as f32, sp.md as f32]))
                .style(card_container_style(tokens, 1))
                .into()
        })
    } else {
        None
    };

    let runtime_settings_block: Element<'static, Message> = if !state.runtime_settings_expanded {
        column![].into()
    } else if state.kind == RuntimeKind::Node {
        node_runtime
            .map(|n| node_runtime_settings_section(n, tokens))
            .unwrap_or_else(|| column![].into())
    } else if state.kind == RuntimeKind::Python {
        python_runtime
            .map(|p| python_runtime_settings_section(p, tokens))
            .unwrap_or_else(|| column![].into())
    } else if state.kind == RuntimeKind::Java {
        java_runtime
            .map(|j| java_runtime_settings_section(j, tokens))
            .unwrap_or_else(|| column![].into())
    } else if state.kind == RuntimeKind::Go {
        go_runtime
            .map(|g| {
                go_runtime_settings_section(
                    g,
                    (
                        &state.go_proxy_custom_draft,
                        &state.go_private_patterns_draft,
                    ),
                    tokens,
                )
            })
            .unwrap_or_else(|| column![].into())
    } else if state.kind == RuntimeKind::Ruby {
        ruby_runtime
            .map(|r| ruby_runtime_settings_section(r, tokens))
            .unwrap_or_else(|| column![].into())
    } else if state.kind == RuntimeKind::Elixir {
        elixir_runtime
            .map(|r| elixir_runtime_settings_section(r, tokens))
            .unwrap_or_else(|| column![].into())
    } else if state.kind == RuntimeKind::Erlang {
        erlang_runtime
            .map(|r| erlang_runtime_settings_section(r, tokens))
            .unwrap_or_else(|| column![].into())
    } else if state.kind == RuntimeKind::Php {
        php_runtime
            .map(|p| php_runtime_settings_section(p, tokens))
            .unwrap_or_else(|| column![].into())
    } else if state.kind == RuntimeKind::Deno {
        deno_runtime
            .map(|d| deno_runtime_settings_section(d, tokens))
            .unwrap_or_else(|| column![].into())
    } else if state.kind == RuntimeKind::Bun {
        bun_runtime
            .map(|b| bun_runtime_settings_section(b, &state.bun_global_bin_dir_draft, tokens))
            .unwrap_or_else(|| column![].into())
    } else if state.kind == RuntimeKind::Dotnet {
        dotnet_runtime
            .map(|d| dotnet_runtime_settings_section(d, tokens))
            .unwrap_or_else(|| column![].into())
    } else if state.kind == RuntimeKind::Zig {
        zig_runtime
            .map(|z| zig_runtime_settings_section(z, tokens))
            .unwrap_or_else(|| column![].into())
    } else if state.kind == RuntimeKind::Julia {
        julia_runtime
            .map(|j| julia_runtime_settings_section(j, tokens))
            .unwrap_or_else(|| column![].into())
    } else if state.kind == RuntimeKind::Lua {
        lua_runtime
            .map(|l| lua_runtime_settings_section(l, tokens))
            .unwrap_or_else(|| column![].into())
    } else if state.kind == RuntimeKind::Nim {
        nim_runtime
            .map(|n| nim_runtime_settings_section(n, tokens))
            .unwrap_or_else(|| column![].into())
    } else if state.kind == RuntimeKind::Crystal {
        crystal_runtime
            .map(|c| crystal_runtime_settings_section(c, tokens))
            .unwrap_or_else(|| column![].into())
    } else if state.kind == RuntimeKind::RLang {
        r_runtime
            .map(|r| rlang_runtime_settings_section(r, tokens))
            .unwrap_or_else(|| column![].into())
    } else {
        column![].into()
    };

    // Search/filter text (we reuse `install_input` field). Actual grouping/sorting is precomputed
    // in `update()` and stored on the state to keep `view()` fast.
    let query = state.install_input.trim();
    let query_norm = query.strip_prefix('v').unwrap_or(query);
    let unified_major_rows = state.unified_major_rows_by_kind.get(&state.kind);
    let unified_major_latest_by_key: HashMap<String, RuntimeVersion> = unified_major_rows
        .map(|rows| {
            rows.iter()
                .filter_map(|r| {
                    r.latest_installable
                        .as_ref()
                        .map(|v| (r.major_key.clone(), v.clone()))
                })
                .collect()
        })
        .unwrap_or_default();

    let matches_empty_hint = || -> Element<'static, Message> {
        let (title, body) = if query_norm.is_empty() {
            (
                envr_core::i18n::tr_key(
                    "gui.empty.title.no_installed_versions",
                    "这里还没有已安装版本",
                    "No installed versions here",
                ),
                envr_core::i18n::tr_key(
                    "gui.empty.hint.no_installed_versions",
                    "左侧筛选列表；右侧输入精确版本安装。远程列表会先读本地缓存再静默更新。",
                    "Filter on the left; enter an exact version on the right to install. Remote rows load from cache first, then refresh quietly.",
                ),
            )
        } else {
            (
                envr_core::i18n::tr_key(
                    "gui.empty.title.no_versions",
                    "没有匹配的版本",
                    "No matching versions",
                ),
                envr_core::i18n::tr_key(
                    "gui.empty.body.no_versions",
                    "换个关键字试试，或清空搜索以查看完整列表。",
                    "Try another keyword, or clear search to see the full list.",
                ),
            )
        };
        container(
            container(illustrative_block_compact(
                tokens,
                EmptyTone::Neutral,
                Lucide::Package,
                36.0,
                title,
                body,
                None,
            ))
            .align_x(Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center)
            .width(Length::Fill)
            .height(Length::Fill),
        )
        .width(Length::Fill)
        .into()
    };

    // Use spacing instead of dense horizontal rules for a calmer UI.
    let mut list_col = column![].spacing(sp.sm as f32).width(Length::Fill);

    let render_keys: Vec<String> = if envr_domain::runtime::unified_major_list_rollout_enabled(state.kind)
        && unified_major_rows.is_some_and(|rows| !rows.is_empty())
    {
        let mut keys: HashSet<String> = unified_major_rows
            .into_iter()
            .flatten()
            .map(|r| r.major_key.clone())
            .collect();
        for v in &state.installed {
            if let Some(k) = version_line_key_for_kind(state.kind, &v.0) {
                keys.insert(k);
            }
        }
        if let Some(cur) = state.current.as_ref()
            && let Some(k) = version_line_key_for_kind(state.kind, &cur.0)
        {
            keys.insert(k);
        }
        let mut keys: Vec<String> = keys.into_iter().collect();
        keys.retain(|k| {
            !major_line_remote_install_blocked(state.kind, k)
                || state.installed.iter().any(|v| {
                    version_line_key_for_kind(state.kind, &v.0).as_deref() == Some(k.as_str())
                })
                || state
                    .current
                    .as_ref()
                    .and_then(|c| version_line_key_for_kind(state.kind, &c.0))
                    .as_deref()
                    == Some(k.as_str())
        });
        keys.sort_by(|a, b| parse_python_key_sort(b).cmp(&parse_python_key_sort(a)));
        if query_norm.is_empty() {
            keys
        } else if query_norm.contains('.') {
            keys.into_iter()
                .filter(|k| k.starts_with(query_norm))
                .collect()
        } else {
            keys.into_iter()
                .filter(|k| {
                    let mut it = k.split('.');
                    let major = it.next().unwrap_or("");
                    let minor = it.next().unwrap_or("");
                    major == query_norm || minor == query_norm || k.contains(query_norm)
                })
                .collect()
        }
    } else {
        Vec::new()
    };

    let waiting_remote =
        runtime_descriptor(state.kind).supports_remote_latest && unified_major_rows.is_none();

    if let Some(err) = state.remote_error.as_deref()
        && runtime_descriptor(state.kind).supports_remote_latest
    {
        list_col = list_col.push(remote_error_inline(tokens, err));
    }

    if (busy && render_keys.is_empty()) || (waiting_remote && render_keys.is_empty()) {
        list_col = list_col.push(loading_skeleton(
            tokens,
            state.skeleton_phase,
            tokens.list_skeleton_rows(),
        ));
    } else if render_keys.is_empty() {
        list_col = list_col.push(matches_empty_hint());
    } else {
        for key in render_keys.iter() {
            let mut installed_versions: Vec<RuntimeVersion> = state
                .installed
                .iter()
                .filter(|v| version_line_key_for_kind(state.kind, &v.0).as_deref() == Some(key))
                .cloned()
                .collect();
            installed_versions.sort_by(|a, b| semver_cmp_desc(&a.0, &b.0));
            let current_key = state
                .current
                .as_ref()
                .and_then(|v| version_line_key_for_kind(state.kind, &v.0));
            let is_active = current_key.as_deref() == Some(key.as_str());
            let flavor_matches_global = if state.kind != RuntimeKind::Php {
                true
            } else {
                match state.php_global_current_want_ts {
                    None => true,
                    Some(g) => {
                        let tab_want_ts = php_runtime
                            .is_some_and(|p| matches!(p.windows_build, PhpWindowsBuildFlavor::Ts));
                        g == tab_want_ts
                    }
                }
            };
            let show_as_active = is_active && path_proxy_on && flavor_matches_global;

            let highest_installed = installed_versions.first().cloned();
            let primary_installed: Option<RuntimeVersion> = installed_versions
                .iter()
                .find(|v| {
                    state.current.as_ref().is_some_and(|c| c.0 == v.0)
                        && path_proxy_on
                        && if state.kind == RuntimeKind::Php {
                            flavor_matches_global
                        } else {
                            true
                        }
                })
                .cloned()
                .or_else(|| highest_installed.clone());
            let extra_installed_count = installed_versions.len().saturating_sub(1);

            let (maj_row_ver_text, maj_row_is_current_exact): (Option<String>, bool) =
                if let Some(installed) = primary_installed.as_ref() {
                    let is_current_exact = state.current.as_ref().is_some_and(|c| c.0 == installed.0)
                        && path_proxy_on
                        && if state.kind == RuntimeKind::Php {
                            flavor_matches_global
                        } else {
                            true
                        };
                    let ver_core = if is_current_exact {
                        format!(
                            "{} {}",
                            installed.0,
                            envr_core::i18n::tr_key("gui.runtime.current_tag", "(当前)", "(current)")
                        )
                    } else {
                        installed.0.clone()
                    };
                    let ver_text = if extra_installed_count > 0 {
                        format!("{} (+{})", ver_core, extra_installed_count)
                    } else {
                        ver_core
                    };
                    (Some(ver_text), is_current_exact)
                } else {
                    (None, false)
                };

            let unified_major_mode = envr_domain::runtime::unified_major_list_rollout_enabled(state.kind)
                && unified_major_rows.is_some_and(|rows| !rows.is_empty());

            // Node list shows stable major keys; install spec still resolves latest patch elsewhere.
            let label_base = if state.kind == RuntimeKind::Node {
                format!("{} {}", kind_label(state.kind), key)
            } else if state.kind == RuntimeKind::Php {
                if cfg!(windows) {
                    let tag = php_runtime
                        .map(|p| match p.windows_build {
                            PhpWindowsBuildFlavor::Nts => "NTS",
                            PhpWindowsBuildFlavor::Ts => "TS",
                        })
                        .unwrap_or("?");
                    format!("{} {} · {}", kind_label(state.kind), key, tag)
                } else {
                    format!("{} {}", kind_label(state.kind), key)
                }
            } else {
                format!("{} {}", kind_label(state.kind), key)
            };

            // Avoid duplicating "(current)" on the title when the row shows an installed summary column.
            let left_text = if show_as_active && installed_versions.is_empty() {
                format!(
                    "{} {}",
                    label_base,
                    envr_core::i18n::tr_key("gui.runtime.current_tag", "(当前)", "(current)",)
                )
            } else {
                label_base
            };

            let install_spec = || -> String {
                if envr_domain::runtime::unified_major_list_rollout_enabled(state.kind)
                    && let Some(v) = unified_major_latest_by_key.get(key)
                {
                    return v.0.clone();
                }
                key.clone()
            };

            let action_btn: Element<'static, Message> = if let Some(installed) = primary_installed {
                let is_current_exact = maj_row_is_current_exact;
                let ver_text = maj_row_ver_text.clone().unwrap_or_default();

                let use_btn = button(button_content_centered(
                    row![
                        Lucide::Package.view(14.0, txt),
                        text(envr_core::i18n::tr_key("gui.action.use", "切换", "Use")),
                    ]
                    .spacing(sp.xs as f32)
                    .align_y(Alignment::Center)
                    .into(),
                ))
                .on_press_maybe((!is_current_exact && path_proxy_on).then_some(
                    Message::EnvCenter(EnvCenterMsg::SubmitUse(installed.0.clone())),
                ))
                .height(Length::Fixed(
                    tokens
                        .control_height_secondary
                        .max(tokens.min_click_target_px()),
                ))
                .padding([sp.sm as f32, sp.sm as f32])
                .style(button_style(tokens, ButtonVariant::Secondary));

                let uninstall_btn = button(button_content_centered(
                    row![
                        Lucide::X.view(
                            14.0,
                            contrast_text_on(gui_theme::to_color(tokens.colors.danger)),
                        ),
                        text(envr_core::i18n::tr_key(
                            "gui.action.uninstall",
                            "卸载",
                            "Uninstall",
                        )),
                    ]
                    .spacing(sp.xs as f32)
                    .align_y(Alignment::Center)
                    .into(),
                ))
                .on_press_maybe(Some(Message::EnvCenter(EnvCenterMsg::SubmitUninstall(
                    installed.0.clone(),
                ))))
                .height(Length::Fixed(
                    tokens
                        .control_height_secondary
                        .max(tokens.min_click_target_px()),
                ))
                .padding([sp.sm as f32, sp.sm as f32])
                .style(button_style(tokens, ButtonVariant::Danger));

                if unified_major_mode {
                    if is_current_exact {
                        space::horizontal().into()
                    } else {
                        container(
                            row![use_btn, uninstall_btn]
                                .spacing(sp.sm as f32)
                                .align_y(Alignment::Center),
                        )
                        .padding([sp.xs as f32, sp.md as f32])
                        .into()
                    }
                } else {
                    let line_row = if is_current_exact {
                        row![text(ver_text).width(Length::Fill)]
                            .spacing(sp.sm as f32)
                            .align_y(Alignment::Center)
                            .width(Length::Fill)
                    } else {
                        row![text(ver_text).width(Length::Fill), use_btn, uninstall_btn]
                            .spacing(sp.sm as f32)
                            .align_y(Alignment::Center)
                            .width(Length::Fill)
                    };

                    container(line_row.padding([sp.xs as f32, sp.md as f32]))
                        .width(Length::Fill)
                        .into()
                }
            } else {
                let spec = install_spec();
                let install_btn = button(button_content_centered(
                    row![
                        Lucide::Download.view(14.0, txt),
                        text(envr_core::i18n::tr_key(
                            "gui.action.install",
                            "安装",
                            "Install"
                        )),
                    ]
                    .spacing(sp.xs as f32)
                    .align_y(Alignment::Center)
                    .into(),
                ))
                .on_press_maybe(Some(Message::EnvCenter(EnvCenterMsg::SubmitInstall(
                    spec.clone(),
                ))))
                .height(Length::Fixed(
                    tokens
                        .control_height_secondary
                        .max(tokens.min_click_target_px()),
                ))
                .padding([sp.sm as f32, sp.sm as f32])
                .style(button_style(tokens, ButtonVariant::Primary));

                let install_and_use_btn = button(button_content_centered(
                    row![
                        Lucide::RefreshCw.view(14.0, txt),
                        text(envr_core::i18n::tr_key(
                            "gui.action.install_use",
                            "安装并切换",
                            "Install & Use"
                        )),
                    ]
                    .spacing(sp.xs as f32)
                    .align_y(Alignment::Center)
                    .into(),
                ))
                .on_press_maybe(
                    path_proxy_on
                        .then_some(Message::EnvCenter(EnvCenterMsg::SubmitInstallAndUse(spec))),
                )
                .height(Length::Fixed(
                    tokens
                        .control_height_secondary
                        .max(tokens.min_click_target_px()),
                ))
                .padding([sp.sm as f32, sp.sm as f32])
                .style(button_style(tokens, ButtonVariant::Secondary));

                container(
                    row![install_btn, install_and_use_btn]
                        .spacing(sp.sm as f32)
                        .align_y(Alignment::Center),
                )
                .into()
            };

            let head_row: Element<'static, Message> = if unified_major_mode {
                let expanded = state.unified_expanded_major_keys.contains(key);
                let muted = gui_theme::to_color(tokens.colors.text_muted);
                let chevron_lbl = if expanded { "▾" } else { "▸" };
                let chevron_cell = container(
                    text(chevron_lbl)
                        .size(ty.caption)
                        .color(muted),
                )
                .width(Length::Fixed(22.0))
                .align_x(Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center);
                let mut tap_inner = row![
                    chevron_cell,
                    text(left_text.clone()).width(Length::Fill),
                ]
                .spacing(sp.sm as f32)
                .align_y(Alignment::Center);
                if let Some(v) = maj_row_ver_text.clone() {
                    tap_inner = tap_inner.push(text(v).width(Length::Fill));
                }
                let expand_strip = container(
                    mouse_area(tap_inner.width(Length::Fill))
                        .on_press(Message::EnvCenter(EnvCenterMsg::ToggleUnifiedMajorExpanded(
                            key.clone(),
                        )))
                        .interaction(iced::mouse::Interaction::Pointer),
                )
                .width(Length::Fill);
                row![expand_strip, action_btn]
                    .spacing(sp.sm as f32)
                    .align_y(Alignment::Center)
                    .width(Length::Fill)
                    .into()
            } else {
                row![text(left_text).width(Length::Fill), action_btn]
                    .spacing(sp.sm as f32)
                    .align_y(Alignment::Center)
                    .width(Length::Fill)
                    .into()
            };

            list_col = list_col.push(
                container(
                    head_row,
                )
                .padding([sp.xs as f32, sp.md as f32])
                .style(card_container_style(tokens, 1)),
            );

            if envr_domain::runtime::unified_major_list_rollout_enabled(state.kind)
                && state.unified_expanded_major_keys.contains(key)
            {
                let child_rows = state
                    .unified_children_rows_by_kind_major
                    .get(&(state.kind, key.clone()))
                    .cloned()
                    .unwrap_or_default();
                for child in child_rows {
                    let spec = child.0.clone();
                    let is_installed = state.installed.iter().any(|v| v.0 == spec);
                    let is_current = state.current.as_ref().is_some_and(|c| c.0 == spec);
                    let child_label = if is_current {
                        format!(
                            "{} {}",
                            spec,
                            envr_core::i18n::tr_key("gui.runtime.current_tag", "(当前)", "(current)")
                        )
                    } else {
                        spec.clone()
                    };

                    let child_actions: Element<'static, Message> = if is_installed {
                        let use_btn = button(button_content_centered(
                            row![
                                Lucide::Package.view(14.0, txt),
                                text(envr_core::i18n::tr_key("gui.action.use", "切换", "Use")),
                            ]
                            .spacing(sp.xs as f32)
                            .align_y(Alignment::Center)
                            .into(),
                        ))
                        .on_press_maybe((!is_current && path_proxy_on).then_some(
                            Message::EnvCenter(EnvCenterMsg::SubmitUse(spec.clone())),
                        ))
                        .height(Length::Fixed(
                            tokens
                                .control_height_secondary
                                .max(tokens.min_click_target_px()),
                        ))
                        .padding([sp.sm as f32, sp.sm as f32])
                        .style(button_style(tokens, ButtonVariant::Secondary));

                        let uninstall_btn = button(button_content_centered(
                            row![
                                Lucide::X.view(
                                    14.0,
                                    contrast_text_on(gui_theme::to_color(tokens.colors.danger)),
                                ),
                                text(envr_core::i18n::tr_key(
                                    "gui.action.uninstall",
                                    "卸载",
                                    "Uninstall",
                                )),
                            ]
                            .spacing(sp.xs as f32)
                            .align_y(Alignment::Center)
                            .into(),
                        ))
                        .on_press_maybe(Some(Message::EnvCenter(EnvCenterMsg::SubmitUninstall(
                            spec.clone(),
                        ))))
                        .height(Length::Fixed(
                            tokens
                                .control_height_secondary
                                .max(tokens.min_click_target_px()),
                        ))
                        .padding([sp.sm as f32, sp.sm as f32])
                        .style(button_style(tokens, ButtonVariant::Danger));
                        row![use_btn, uninstall_btn]
                            .spacing(sp.sm as f32)
                            .align_y(Alignment::Center)
                            .into()
                    } else {
                        let install_btn = button(button_content_centered(
                            row![
                                Lucide::Download.view(14.0, txt),
                                text(envr_core::i18n::tr_key(
                                    "gui.action.install",
                                    "安装",
                                    "Install"
                                )),
                            ]
                            .spacing(sp.xs as f32)
                            .align_y(Alignment::Center)
                            .into(),
                        ))
                        .on_press_maybe(Some(Message::EnvCenter(EnvCenterMsg::SubmitInstall(
                            spec.clone(),
                        ))))
                        .height(Length::Fixed(
                            tokens
                                .control_height_secondary
                                .max(tokens.min_click_target_px()),
                        ))
                        .padding([sp.sm as f32, sp.sm as f32])
                        .style(button_style(tokens, ButtonVariant::Primary));

                        let install_and_use_btn = button(button_content_centered(
                            row![
                                Lucide::RefreshCw.view(14.0, txt),
                                text(envr_core::i18n::tr_key(
                                    "gui.action.install_use",
                                    "安装并切换",
                                    "Install & Use"
                                )),
                            ]
                            .spacing(sp.xs as f32)
                            .align_y(Alignment::Center)
                            .into(),
                        ))
                        .on_press_maybe(path_proxy_on.then_some(Message::EnvCenter(
                            EnvCenterMsg::SubmitInstallAndUse(spec.clone()),
                        )))
                        .height(Length::Fixed(
                            tokens
                                .control_height_secondary
                                .max(tokens.min_click_target_px()),
                        ))
                        .padding([sp.sm as f32, sp.sm as f32])
                        .style(button_style(tokens, ButtonVariant::Secondary));
                        row![install_btn, install_and_use_btn]
                            .spacing(sp.sm as f32)
                            .align_y(Alignment::Center)
                            .into()
                    };

                    list_col = list_col.push(
                        container(
                            row![
                                container(space().width(Length::Fixed((sp.xl + sp.sm) as f32)))
                                    .width(Length::Fixed((sp.xl + sp.sm) as f32)),
                                text(child_label).width(Length::Fill),
                                child_actions
                            ]
                            .spacing(sp.sm as f32)
                            .align_y(Alignment::Center)
                            .width(Length::Fill),
                        )
                        .padding([sp.xs as f32, sp.md as f32])
                        .style(card_container_style(tokens, 1)),
                    );
                }
            }
        }
    }

    let search_ph = if state.kind == RuntimeKind::Python {
        envr_core::i18n::tr_key(
            "gui.runtime.search_placeholder.python",
            "筛选主版本（例如 3.14）",
            "Filter by major.minor (e.g. 3.14)",
        )
    } else {
        envr_core::i18n::tr_key(
            "gui.runtime.search_placeholder.default",
            "筛选主版本（例如 24）",
            "Filter by major (e.g. 24)",
        )
    };

    let direct_ph = envr_core::i18n::tr_key(
        "gui.runtime.direct_install_placeholder",
        "指定版本安装",
        "Exact version to install",
    );

    let ctrl_h = tokens
        .control_height_secondary
        .max(tokens.min_click_target_px());

    let search: Element<'static, Message> = container(
        text_input(&search_ph, &state.install_input)
            .on_input(|s| Message::EnvCenter(EnvCenterMsg::InstallInput(s)))
            .padding(sp.sm)
            .width(Length::Fill)
            .style(text_input_style(tokens)),
    )
    .width(Length::Fill)
    .height(Length::Fixed(ctrl_h))
    .align_y(iced::alignment::Vertical::Center)
    .into();

    let direct_input_el: Element<'static, Message> = container(
        text_input(&direct_ph, &state.direct_install_input)
            .on_input(|s| Message::EnvCenter(EnvCenterMsg::DirectInstallInput(s)))
            .padding(sp.sm)
            .width(Length::Fill)
            .style(text_input_style(tokens)),
    )
    .width(Length::Fill)
    .height(Length::Fixed(ctrl_h))
    .align_y(iced::alignment::Vertical::Center)
    .into();

    let direct_spec_nonempty = !state.direct_install_input.trim().is_empty();
    let bun_spec_blocked = state.kind == RuntimeKind::Bun
        && bun_direct_spec_blocked_on_windows(&state.direct_install_input);
    let deno_spec_blocked =
        state.kind == RuntimeKind::Deno && deno_direct_spec_blocked(&state.direct_install_input);
    let direct_spec_ready = direct_spec_nonempty && !bun_spec_blocked && !deno_spec_blocked;
    let direct_install_btn = button(button_content_centered(
        row![
            Lucide::Download.view(14.0, txt),
            text(envr_core::i18n::tr_key(
                "gui.action.install",
                "安装",
                "Install"
            )),
        ]
        .spacing(sp.xs as f32)
        .align_y(Alignment::Center)
        .into(),
    ))
    .on_press_maybe(
        direct_spec_ready.then_some(Message::EnvCenter(EnvCenterMsg::SubmitDirectInstall)),
    )
    .height(Length::Fixed(ctrl_h))
    .padding([sp.sm as f32, sp.md as f32])
    .style(button_style(tokens, ButtonVariant::Primary));

    let direct_install_use_btn = button(button_content_centered(
        row![
            Lucide::RefreshCw.view(14.0, txt),
            text(envr_core::i18n::tr_key(
                "gui.action.install_use",
                "安装并切换",
                "Install & Use",
            )),
        ]
        .spacing(sp.xs as f32)
        .align_y(Alignment::Center)
        .into(),
    ))
    .on_press_maybe(
        (direct_spec_ready && path_proxy_on)
            .then_some(Message::EnvCenter(EnvCenterMsg::SubmitDirectInstallAndUse)),
    )
    .height(Length::Fixed(ctrl_h))
    .padding([sp.sm as f32, sp.md as f32])
    .style(button_style(tokens, ButtonVariant::Secondary));

    let filter_row = row![
        container(search)
            .width(Length::FillPortion(3))
            .height(Length::Fixed(ctrl_h)),
        container(direct_input_el)
            .width(Length::FillPortion(2))
            .height(Length::Fixed(ctrl_h)),
        direct_install_btn,
        direct_install_use_btn,
    ]
    .spacing(sp.sm as f32)
    .align_y(Alignment::Center);

    let bun_direct_spec_hint = if bun_spec_blocked {
        Some(
            text(envr_core::i18n::tr_key(
                "gui.runtime.bun.win_0x_blocked",
                "Windows 不支持 Bun 0.x，请输入 1.x 及以上版本。",
                "Bun 0.x is unavailable on Windows. Please enter version 1.x or newer.",
            ))
            .size(ty.micro)
            .color(gui_theme::to_color(tokens.colors.warning)),
        )
    } else if deno_spec_blocked {
        Some(
            text(envr_core::i18n::tr_key(
                "gui.runtime.deno.0x_blocked",
                "Deno 0.x 不受托管安装支持，请输入 1.x/2.x 版本。",
                "Deno 0.x is not supported for managed install. Please enter a 1.x/2.x version.",
            ))
            .size(ty.micro)
            .color(gui_theme::to_color(tokens.colors.warning)),
        )
    } else {
        None
    };

    let java_distro_row = if state.kind == RuntimeKind::Java {
        let current_distro = java_runtime
            .map(|j| j.current_distro)
            .map(|d| {
                if d == JavaDistro::OpenJdk {
                    JavaDistro::Temurin
                } else {
                    d
                }
            })
            .unwrap_or(JavaDistro::Temurin);
        let mut r = row![text("Java 发行版").size(ty.body)].spacing(sp.sm as f32);
        for d in [
            JavaDistro::Temurin,
            JavaDistro::AzulZulu,
            JavaDistro::AlibabaDragonwell,
            JavaDistro::OracleOpenJdk,
            JavaDistro::AmazonCorretto,
            JavaDistro::Microsoft,
            JavaDistro::OracleJdk,
        ] {
            let label = match d {
                JavaDistro::Temurin => "Temurin",
                JavaDistro::AzulZulu => "Zulu",
                JavaDistro::AlibabaDragonwell => "Dragonwell",
                JavaDistro::OracleOpenJdk => "Oracle OpenJDK",
                JavaDistro::AmazonCorretto => "Corretto",
                JavaDistro::Microsoft => "Microsoft",
                JavaDistro::OracleJdk => "Oracle JDK",
                JavaDistro::OpenJdk => "OpenJDK",
            };
            let variant = if d == current_distro {
                ButtonVariant::Primary
            } else {
                ButtonVariant::Secondary
            };
            r = r.push(
                button(button_content_centered(text(label).into()))
                    .on_press(Message::EnvCenter(EnvCenterMsg::SetJavaDistro(d)))
                    .width(Length::FillPortion(1))
                    .height(Length::Fixed(ctrl_h))
                    .padding([sp.sm as f32, sp.sm as f32])
                    .style(button_style(tokens, variant)),
            );
        }
        Some(r)
    } else {
        None
    };

    let muted = gui_theme::to_color(tokens.colors.text_muted);
    let php_build_row = if state.kind == RuntimeKind::Php {
        php_runtime.and_then(|p| php_windows_build_row(p, tokens))
    } else {
        None
    };
    let mut col = if state.kind == RuntimeKind::Java {
        if let Some(distro_row) = java_distro_row {
            column![header, runtime_settings_block, distro_row]
        } else {
            column![header, runtime_settings_block]
        }
    } else if state.kind == RuntimeKind::Php {
        if let Some(build_row) = php_build_row {
            column![header, runtime_settings_block, build_row, filter_row]
        } else {
            column![header, runtime_settings_block, filter_row]
        }
    } else if let Some(hint) = bun_direct_spec_hint {
        column![header, runtime_settings_block, filter_row, hint]
    } else {
        column![header, runtime_settings_block, filter_row]
    }
    .spacing(sp.sm as f32)
    .width(Length::Fill);

    if let Some(hint) = prereq_hint {
        col = col.push(hint);
    }
    if busy {
        col = col.push(
            text(envr_core::i18n::tr_key(
                "gui.app.loading",
                "正在加载…",
                "Loading…",
            ))
            .size(ty.micro)
            .color(muted),
        );
    }
    col.push(list_col).into()
}

fn rust_env_center_view(
    state: &EnvCenterState,
    rust_runtime: Option<&RustRuntimeSettings>,
    tokens: ThemeTokens,
) -> Element<'static, Message> {
    let ty = tokens.typography();
    let sp = tokens.space();
    let txt = gui_theme::to_color(tokens.colors.text);
    let muted = gui_theme::to_color(tokens.colors.text_muted);

    let header = container(
        row![
            text(envr_core::i18n::tr_key(
                "gui.runtime.rust.title",
                "Rust 设置",
                "Rust"
            ))
            .size(ty.section),
            container(space()).width(Length::Fill),
            button(button_content_centered(
                row![
                    Lucide::RefreshCw.view(14.0, txt),
                    text(envr_core::i18n::tr_key(
                        "gui.action.refresh",
                        "刷新",
                        "Refresh"
                    )),
                ]
                .spacing(sp.xs as f32)
                .align_y(Alignment::Center)
                .into(),
            ))
            .on_press(Message::EnvCenter(EnvCenterMsg::RustRefresh))
            .height(Length::Fixed(tokens.control_height_secondary))
            .style(button_style(tokens, ButtonVariant::Secondary)),
        ]
        .spacing(sp.sm as f32)
        .align_y(Alignment::Center),
    )
    .padding(Padding::from([sp.md as f32, sp.md as f32]))
    .style(card_container_style(tokens, 1));

    let status = state.rust_status.as_ref();
    let mode = status.map(|s| s.mode.as_str()).unwrap_or("none");
    let mode_label = match mode {
        "system" => envr_core::i18n::tr_key(
            "gui.runtime.rust.mode.system",
            "系统 rustup",
            "System rustup",
        ),
        "managed" => envr_core::i18n::tr_key(
            "gui.runtime.rust.mode.managed",
            "托管 rustup",
            "Managed rustup",
        ),
        _ => envr_core::i18n::tr_key(
            "gui.runtime.rust.mode.none",
            "未安装 rustup",
            "rustup not installed",
        ),
    };
    let active = status
        .and_then(|s| s.active_toolchain.clone())
        .unwrap_or_else(|| envr_core::i18n::tr_key("gui.runtime.rust.none", "(无)", "(none)"));
    let mode_hint = match mode {
        "system" => envr_core::i18n::tr_key(
            "gui.runtime.rust.mode_hint.system",
            "检测到系统 rustup。可以安装/切换工具链，并管理组件与目标。",
            "System rustup detected. You can install/switch toolchains and manage components/targets.",
        ),
        "managed" => envr_core::i18n::tr_key(
            "gui.runtime.rust.mode_hint.managed",
            "当前使用 envr 托管 rustup。可在此更新工具链，或卸载托管安装。",
            "Using envr-managed rustup. You can update toolchains here or uninstall the managed installation.",
        ),
        _ => envr_core::i18n::tr_key(
            "gui.runtime.rust.mode_hint.none",
            "未检测到可用 rustup。先点击“安装 stable”完成托管安装，再管理组件/目标。",
            "No rustup detected. Click \"Install stable\" first, then manage components/targets.",
        ),
    };
    let rustc = status
        .and_then(|s| s.rustc_version.clone())
        .unwrap_or_else(|| envr_core::i18n::tr_key("gui.runtime.rust.unknown", "未知", "unknown"));

    let status_card = container(
        column![
            text(mode_label).size(ty.body_small),
            text(format!(
                "{} {}",
                envr_core::i18n::tr_key("gui.runtime.current", "当前：", "Current:"),
                active
            ))
            .size(ty.caption)
            .color(muted),
            text(format!("rustc {rustc}")).size(ty.caption).color(muted),
            text(mode_hint).size(ty.micro).color(muted),
        ]
        .spacing(sp.xs as f32),
    )
    .padding(sp.md)
    .style(card_container_style(tokens, 1));

    let ds = rust_runtime.map(|r| r.download_source).unwrap_or_default();
    let mut ds_row = row![
        text(envr_core::i18n::tr_key(
            "gui.runtime.rust.download_source",
            "Rust 下载源",
            "Rust download source"
        ))
        .size(ty.body)
    ]
    .spacing(sp.sm as f32);
    for src in [
        RustDownloadSource::Auto,
        RustDownloadSource::Domestic,
        RustDownloadSource::Official,
    ] {
        let lab = match src {
            RustDownloadSource::Auto => envr_core::i18n::tr_key(
                "gui.runtime.rust.ds.auto",
                "自动（随区域语言）",
                "Auto (locale)",
            ),
            RustDownloadSource::Domestic => {
                envr_core::i18n::tr_key("gui.runtime.rust.ds.domestic", "国内镜像", "China mirror")
            }
            RustDownloadSource::Official => {
                envr_core::i18n::tr_key("gui.runtime.rust.ds.official", "官方", "Official")
            }
        };
        let variant = if src == ds {
            ButtonVariant::Primary
        } else {
            ButtonVariant::Secondary
        };
        ds_row = ds_row.push(
            button(button_content_centered(text(lab).into()))
                .on_press(Message::EnvCenter(EnvCenterMsg::SetRustDownloadSource(src)))
                .width(Length::FillPortion(1))
                .height(Length::Fixed(
                    tokens
                        .control_height_secondary
                        .max(tokens.min_click_target_px()),
                ))
                .padding([sp.sm as f32, sp.sm as f32])
                .style(button_style(tokens, variant)),
        );
    }
    let settings_card = container(
        column![
            ds_row,
            text(envr_core::i18n::tr_key(
                "gui.runtime.rust.download_source_hint",
                "下载源同时影响 rustup-init 与 Rust 工具链下载。",
                "Download source affects both rustup-init and Rust toolchain downloads.",
            ))
            .size(ty.micro)
            .color(muted),
        ]
        .spacing(sp.xs as f32),
    )
    .padding(sp.md)
    .style(card_container_style(tokens, 1));

    let show_managed_install =
        mode == "none" || status.is_some_and(|s| s.managed_install_available);
    let managed_install_btn = if show_managed_install {
        Some(
            button(button_content_centered(
                row![
                    Lucide::Download.view(14.0, txt),
                    text(envr_core::i18n::tr_key(
                        "gui.runtime.rust.install",
                        "安装 stable",
                        "Install stable"
                    )),
                ]
                .spacing(sp.xs as f32)
                .align_y(Alignment::Center)
                .into(),
            ))
            .on_press(Message::EnvCenter(EnvCenterMsg::RustManagedInstallStable))
            .height(Length::Fixed(
                tokens
                    .control_height_secondary
                    .max(tokens.min_click_target_px()),
            ))
            .padding([sp.sm as f32, sp.md as f32])
            .style(button_style(tokens, ButtonVariant::Primary)),
        )
    } else {
        None
    };

    let managed_uninstall_btn = if status.is_some_and(|s| s.managed_installed) && mode == "managed"
    {
        Some(
            button(button_content_centered(
                row![
                    Lucide::X.view(
                        14.0,
                        contrast_text_on(gui_theme::to_color(tokens.colors.danger)),
                    ),
                    text(envr_core::i18n::tr_key(
                        "gui.action.uninstall",
                        "卸载",
                        "Uninstall"
                    )),
                ]
                .spacing(sp.xs as f32)
                .align_y(Alignment::Center)
                .into(),
            ))
            .on_press(Message::EnvCenter(EnvCenterMsg::RustManagedUninstall))
            .height(Length::Fixed(
                tokens
                    .control_height_secondary
                    .max(tokens.min_click_target_px()),
            ))
            .padding([sp.sm as f32, sp.md as f32])
            .style(button_style(tokens, ButtonVariant::Danger)),
        )
    } else {
        None
    };

    let mut ops_col = column![].spacing(sp.sm as f32).width(Length::Fill);
    if mode != "none" {
        let channel_row = row![
            rust_channel_btn(tokens, "stable"),
            rust_channel_btn(tokens, "beta"),
            rust_channel_btn(tokens, "nightly"),
            button(button_content_centered(
                row![
                    Lucide::RefreshCw.view(14.0, txt),
                    text(envr_core::i18n::tr_key(
                        "gui.runtime.rust.update",
                        "更新",
                        "Update"
                    )),
                ]
                .spacing(sp.xs as f32)
                .align_y(Alignment::Center)
                .into(),
            ))
            .on_press(Message::EnvCenter(EnvCenterMsg::RustUpdateCurrent))
            .height(Length::Fixed(
                tokens
                    .control_height_secondary
                    .max(tokens.min_click_target_px()),
            ))
            .padding([sp.sm as f32, sp.sm as f32])
            .style(button_style(tokens, ButtonVariant::Secondary)),
        ]
        .spacing(sp.sm as f32)
        .align_y(Alignment::Center);
        ops_col = ops_col.push(channel_row);
    }

    let mut managed_row = row![].spacing(sp.sm as f32).align_y(Alignment::Center);
    let mut managed_row_has_actions = false;
    if let Some(b) = managed_install_btn {
        managed_row_has_actions = true;
        managed_row = managed_row.push(b);
    }
    if let Some(b) = managed_uninstall_btn {
        managed_row_has_actions = true;
        managed_row = managed_row.push(b);
    }
    if managed_row_has_actions {
        ops_col = ops_col.push(managed_row);
    }
    if mode == "none" {
        ops_col = ops_col.push(
            text(envr_core::i18n::tr_key(
                "gui.runtime.rust.install_hint",
                "安装后会默认使用 stable 工具链；你仍可切换到 beta/nightly。",
                "Install uses stable by default; you can still switch to beta/nightly later.",
            ))
            .size(ty.micro)
            .color(muted),
        );
    }
    let ops_card = container(ops_col)
        .padding(sp.md)
        .style(card_container_style(tokens, 1));

    let tab_row = row![
        rust_tab_btn(tokens, RustTab::Components, state.rust_tab),
        rust_tab_btn(tokens, RustTab::Targets, state.rust_tab),
    ]
    .spacing(sp.sm as f32);

    let list = if state.rust_tab == RustTab::Components {
        rust_kv_list(tokens, &state.rust_components, true, state.busy)
    } else {
        rust_kv_list(tokens, &state.rust_targets, false, state.busy)
    };

    column![header, status_card, settings_card, ops_card, tab_row, list]
        .spacing(sp.sm as f32)
        .width(Length::Fill)
        .into()
}

fn rust_channel_btn(tokens: ThemeTokens, channel: &'static str) -> Element<'static, Message> {
    let sp = tokens.space();
    let txt = gui_theme::to_color(tokens.colors.text);
    button(button_content_centered(
        row![Lucide::RefreshCw.view(14.0, txt), text(channel),]
            .spacing(sp.xs as f32)
            .align_y(Alignment::Center)
            .into(),
    ))
    .on_press(Message::EnvCenter(
        EnvCenterMsg::RustChannelInstallOrSwitch(channel.to_string()),
    ))
    .height(Length::Fixed(
        tokens
            .control_height_secondary
            .max(tokens.min_click_target_px()),
    ))
    .padding([sp.sm as f32, sp.sm as f32])
    .style(button_style(tokens, ButtonVariant::Secondary))
    .into()
}

fn rust_tab_btn(tokens: ThemeTokens, tab: RustTab, active: RustTab) -> Element<'static, Message> {
    let sp = tokens.space();
    let label = match tab {
        RustTab::Components => {
            envr_core::i18n::tr_key("gui.runtime.rust.components", "组件", "Components")
        }
        RustTab::Targets => envr_core::i18n::tr_key("gui.runtime.rust.targets", "目标", "Targets"),
    };
    let variant = if tab == active {
        ButtonVariant::Primary
    } else {
        ButtonVariant::Secondary
    };
    button(button_content_centered(text(label).into()))
        .on_press(Message::EnvCenter(EnvCenterMsg::RustSelectTab(tab)))
        .height(Length::Fixed(
            tokens
                .control_height_secondary
                .max(tokens.min_click_target_px()),
        ))
        .padding([sp.sm as f32, sp.sm as f32])
        .style(button_style(tokens, variant))
        .into()
}

fn rust_kv_list(
    tokens: ThemeTokens,
    items: &[(String, bool, bool)],
    is_component: bool,
    busy: bool,
) -> Element<'static, Message> {
    let ty = tokens.typography();
    let sp = tokens.space();
    let txt = gui_theme::to_color(tokens.colors.text);
    let muted = gui_theme::to_color(tokens.colors.text_muted);

    if items.is_empty() {
        return container(illustrative_block_compact(
            tokens,
            EmptyTone::Neutral,
            Lucide::Package,
            36.0,
            envr_core::i18n::tr_key("gui.empty.title.no_data", "暂无数据", "No data"),
            envr_core::i18n::tr_key(
                "gui.empty.body.no_data",
                "点击刷新或检查 rustup 是否可用。",
                "Refresh or check rustup availability.",
            ),
            None,
        ))
        .width(Length::Fill)
        .into();
    }

    let owned: Vec<(String, bool, bool)> = items.to_vec();
    let mut col = column![].spacing(0).width(Length::Fill);
    for (name, installed, available) in owned.into_iter() {
        let action: Element<'static, Message> = if installed {
            button(button_content_centered(
                row![
                    Lucide::X.view(
                        14.0,
                        contrast_text_on(gui_theme::to_color(tokens.colors.danger)),
                    ),
                    text(envr_core::i18n::tr_key(
                        "gui.action.uninstall",
                        "卸载",
                        "Uninstall"
                    )),
                ]
                .spacing(sp.xs as f32)
                .align_y(Alignment::Center)
                .into(),
            ))
            .on_press_maybe((!busy).then_some(Message::EnvCenter(if is_component {
                EnvCenterMsg::RustComponentToggle(name.clone(), false)
            } else {
                EnvCenterMsg::RustTargetToggle(name.clone(), false)
            })))
            .height(Length::Fixed(
                tokens
                    .control_height_secondary
                    .max(tokens.min_click_target_px()),
            ))
            .padding([sp.sm as f32, sp.sm as f32])
            .style(button_style(tokens, ButtonVariant::Danger))
            .into()
        } else if available {
            button(button_content_centered(
                row![
                    Lucide::Download.view(14.0, txt),
                    text(envr_core::i18n::tr_key(
                        "gui.action.install",
                        "安装",
                        "Install"
                    )),
                ]
                .spacing(sp.xs as f32)
                .align_y(Alignment::Center)
                .into(),
            ))
            .on_press_maybe((!busy).then_some(Message::EnvCenter(if is_component {
                EnvCenterMsg::RustComponentToggle(name.clone(), true)
            } else {
                EnvCenterMsg::RustTargetToggle(name.clone(), true)
            })))
            .height(Length::Fixed(
                tokens
                    .control_height_secondary
                    .max(tokens.min_click_target_px()),
            ))
            .padding([sp.sm as f32, sp.sm as f32])
            .style(button_style(tokens, ButtonVariant::Primary))
            .into()
        } else {
            text(envr_core::i18n::tr_key(
                "gui.runtime.rust.unavailable",
                "当前不可用",
                "Unavailable",
            ))
            .size(ty.micro)
            .color(muted)
            .into()
        };
        let row_el = row![
            column![
                text(name).size(ty.body_small),
                text(if installed {
                    envr_core::i18n::tr_key("gui.runtime.installed", "已安装", "Installed")
                } else if !available {
                    envr_core::i18n::tr_key(
                        "gui.runtime.rust.unavailable_hint",
                        "当前工具链/平台下不可安装。",
                        "Not installable for current toolchain/platform.",
                    )
                } else {
                    String::new()
                })
                .size(ty.micro)
                .color(muted),
            ]
            .spacing(0)
            .width(Length::Fill),
            action,
        ]
        .spacing(sp.sm as f32)
        .align_y(Alignment::Center);
        col = col.push(
            container(row_el)
                .padding([sp.sm as f32, sp.md as f32])
                .style(card_container_style(tokens, 1)),
        );
        col = col.push(rule::horizontal(1.0));
    }
    col.into()
}

fn semver_parts(s: &str) -> Option<(u64, u64, u64)> {
    let t = s.trim().strip_prefix('v').unwrap_or(s.trim());
    let mut it = t.split('.');
    let a: u64 = it.next()?.parse().ok()?;
    let b: u64 = it.next()?.parse().ok()?;
    let c: u64 = it.next()?.parse().ok()?;
    // Accept versions with extra numeric segments (e.g. Erlang `27.3.4.10`);
    // grouping only needs major/minor(/patch), so we safely ignore the rest.
    for seg in it {
        if seg.parse::<u64>().is_err() {
            return None;
        }
    }
    Some((a, b, c))
}

fn semver_cmp_desc(a: &str, b: &str) -> std::cmp::Ordering {
    match (semver_parts(a), semver_parts(b)) {
        (Some((ma, m1, p)), Some((mb, n1, q))) => (mb, n1, q).cmp(&(ma, m1, p)),
        _ => b.cmp(a),
    }
}

fn bun_direct_spec_blocked_on_windows(spec: &str) -> bool {
    if !cfg!(windows) {
        return false;
    }
    let t = spec.trim().trim_start_matches('v');
    t.starts_with("0.")
}

fn deno_direct_spec_blocked(spec: &str) -> bool {
    let t = spec.trim().trim_start_matches('v');
    t.starts_with("0.")
}

fn parse_python_key_sort(k: &str) -> (u64, u64) {
    let mut it = k.split('.');
    let a = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let b = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    (a, b)
}

/// Drop in-memory unified list VM when leaving the Runtime page; on-disk cache under runtime root is kept.
pub(crate) fn env_center_clear_unified_list_render_state(state: &mut EnvCenterState) {
    state.unified_major_rows_by_kind.clear();
    state.unified_children_rows_by_kind_major.clear();
    state.unified_expanded_major_keys.clear();
}

