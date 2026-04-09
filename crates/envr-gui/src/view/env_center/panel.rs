//! Node / Python / Java / Go env center: lists, install, use, uninstall via `RuntimeService`.

use envr_config::settings::{
    JavaDistro, JavaDownloadSource, JavaRuntimeSettings, NodeDownloadSource, NodeRuntimeSettings,
    NpmRegistryMode, PipRegistryMode, PythonDownloadSource, PythonRuntimeSettings,
};
use envr_domain::runtime::{RuntimeKind, RuntimeVersion};
use envr_ui::theme::ThemeTokens;
use iced::alignment::Horizontal;
use iced::widget::{button, column, container, row, rule, space, text, text_input, toggler};
use iced::{Alignment, Element, Length, Padding, Theme};

use std::collections::{HashMap, HashSet};

use crate::app::Message;
use crate::icons::Lucide;
use crate::theme as gui_theme;
use crate::view::empty_state::{EmptyTone, illustrative_block_compact};
use crate::widget_styles::{
    card_container_style, ButtonVariant, button_content_centered, button_style, text_input_style,
};

#[derive(Debug, Clone)]
pub enum EnvCenterMsg {
    PickKind(RuntimeKind),
    InstallInput(String),
    DirectInstallInput(String),
    DataLoaded(Result<(Vec<RuntimeVersion>, Option<RuntimeVersion>), String>),
    RemoteLatestDiskSnapshot(RuntimeKind, Vec<RuntimeVersion>),
    RemoteLatestRefreshed(RuntimeKind, Result<Vec<RuntimeVersion>, String>),
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
    SyncShimsFinished(Result<(), String>),
}

#[derive(Debug)]
pub struct EnvCenterState {
    pub kind: RuntimeKind,
    pub install_input: String,
    pub installed: Vec<RuntimeVersion>,
    pub current: Option<RuntimeVersion>,
    pub busy: bool,
    /// Non-fatal remote fetch/parse error shown inline (keeps global UI usable).
    pub remote_error: Option<String>,
    /// Node: latest patch per major from cache/network (see `list_remote_latest_per_major`).
    pub node_remote_latest: Vec<RuntimeVersion>,
    /// Node: background refresh of `node_remote_latest` (TTL / index) is in flight.
    pub node_remote_refreshing: bool,
    /// Python: latest patch per major from cache/network (see `list_remote_latest_per_major`).
    pub python_remote_latest: Vec<RuntimeVersion>,
    /// Python: background refresh of `python_remote_latest` (TTL / index) is in flight.
    pub python_remote_refreshing: bool,
    /// Java: latest patch per LTS major from cache/network.
    pub java_remote_latest: Vec<RuntimeVersion>,
    /// Java: background refresh is in flight.
    pub java_remote_refreshing: bool,
    /// Optional version spec for direct install (right of search).
    pub direct_install_input: String,
    /// 0..1 phase for skeleton shimmer (`tasks_gui.md` GUI-041).
    pub skeleton_phase: f32,
    /// Runtime: whether the settings strip is visible (`03-gui-设计.md`).
    pub runtime_settings_expanded: bool,
    /// Synthetic job shown in downloads panel for current env-center operation.
    pub op_job_id: Option<u64>,
}

impl Default for EnvCenterState {
    fn default() -> Self {
        Self {
            kind: RuntimeKind::Node,
            install_input: String::new(),
            installed: Vec::new(),
            current: None,
            busy: false,
            remote_error: None,
            node_remote_latest: Vec::new(),
            node_remote_refreshing: false,
            python_remote_latest: Vec::new(),
            python_remote_refreshing: false,
            java_remote_latest: Vec::new(),
            java_remote_refreshing: false,
            direct_install_input: String::new(),
            skeleton_phase: 0.0,
            runtime_settings_expanded: false,
            op_job_id: None,
        }
    }
}

// (scroll_y is clamped locally during rendering; no persistent clamping helper needed)

pub(crate) fn kind_label(kind: RuntimeKind) -> &'static str {
    match kind {
        RuntimeKind::Node => "Node",
        RuntimeKind::Python => "Python",
        RuntimeKind::Java => "Java",
        RuntimeKind::Go => "Go",
        RuntimeKind::Rust => "Rust",
        RuntimeKind::Php => "PHP",
        RuntimeKind::Deno => "Deno",
        RuntimeKind::Bun => "Bun",
    }
}

/// Display name for download-panel install tasks (Chinese UI copy).
pub(crate) fn kind_label_zh(kind: RuntimeKind) -> &'static str {
    match kind {
        RuntimeKind::Node => "Node.js",
        RuntimeKind::Python => "Python",
        RuntimeKind::Java => "Java",
        RuntimeKind::Go => "Go",
        RuntimeKind::Rust => "Rust",
        RuntimeKind::Php => "PHP",
        RuntimeKind::Deno => "Deno",
        RuntimeKind::Bun => "Bun",
    }
}

fn node_runtime_settings_section(
    node: &NodeRuntimeSettings,
    tokens: ThemeTokens,
) -> Element<'static, Message> {
    let ty = tokens.typography();
    let sp = tokens.space();
    let muted = gui_theme::to_color(tokens.colors.text_muted);

    let dl_title = text(envr_core::i18n::tr_key(
        "gui.runtime.node.download_source",
        "Node 下载源",
        "Node download source",
    ))
    .size(ty.body);

    let mut dl_row = row![dl_title].spacing(sp.sm as f32);
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
            NodeDownloadSource::Domestic => envr_core::i18n::tr_key(
                "gui.runtime.node.ds.domestic",
                "国内镜像",
                "China mirror",
            ),
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
            .width(Length::FillPortion(1))
            .height(Length::Fixed(h))
            .padding([sp.sm as f32, sp.sm as f32])
            .style(button_style(tokens, variant));
        dl_row = dl_row.push(b);
    }

    let npm_title = text(envr_core::i18n::tr_key(
        "gui.runtime.node.npm_registry",
        "npm 源",
        "npm registry",
    ))
    .size(ty.body);

    let mut npm_row = row![npm_title].spacing(sp.sm as f32);
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
            NpmRegistryMode::Domestic => envr_core::i18n::tr_key(
                "gui.runtime.node.npm.domestic",
                "国内镜像",
                "China mirror",
            ),
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
            .width(Length::FillPortion(1))
            .height(Length::Fixed(h))
            .padding([sp.sm as f32, sp.sm as f32])
            .style(button_style(tokens, variant));
        npm_row = npm_row.push(b);
    }

    let proxy_label = text(envr_core::i18n::tr_key(
        "gui.runtime.node.path_proxy",
        "PATH 代理",
        "PATH proxy",
    ))
    .size(ty.body);

    let proxy_toggle = toggler(node.path_proxy_enabled)
        .label(envr_core::i18n::tr_key(
            "gui.runtime.node.path_proxy.hint",
            "开启时由 envr 接管 node/npm/npx；关闭时 shim 透传到系统 PATH。",
            "When on, envr manages node/npm/npx; when off, shims delegate to your system PATH.",
        ))
        .on_toggle(|v| Message::EnvCenter(EnvCenterMsg::SetNodePathProxy(v)));

    let proxy_note = text(envr_core::i18n::tr_key(
        "gui.runtime.node.path_proxy.note",
        "关闭时无法使用「切换」「安装并切换」；再次开启后不会自动恢复上次版本，需手动切换。",
        "While off, Use / Install & Use are disabled; turning on again does not auto-restore the previous version.",
    ))
    .size(ty.micro)
    .color(muted);

    let proxy_block = column![proxy_label, proxy_toggle, proxy_note]
        .spacing((sp.xs + 2) as f32)
        .width(Length::Fill);

    container(
        column![
            dl_row,
            npm_row,
            proxy_block,
        ]
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

    let dl_title = text(envr_core::i18n::tr_key(
        "gui.runtime.python.download_source",
        "Python 下载源（影响 Python 安装包与 get-pip.py）",
        "Python download source (affects Python artifacts + get-pip.py)",
    ))
    .size(ty.body);
    let mut dl_row = row![dl_title].spacing(sp.sm as f32);
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
        dl_row = dl_row.push(
            button(button_content_centered(text(lab).into()))
                .on_press(Message::EnvCenter(EnvCenterMsg::SetPythonDownloadSource(src)))
                .width(Length::FillPortion(1))
                .height(Length::Fixed(h))
                .padding([sp.sm as f32, sp.sm as f32])
                .style(button_style(tokens, variant)),
        );
    }

    let pip_title = text(envr_core::i18n::tr_key(
        "gui.runtime.python.pip_registry",
        "pip 引导源",
        "pip bootstrap index",
    ))
    .size(ty.body);
    let mut pip_row = row![pip_title].spacing(sp.sm as f32);
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
        pip_row = pip_row.push(
            button(button_content_centered(text(lab).into()))
                .on_press(Message::EnvCenter(EnvCenterMsg::SetPipRegistryMode(mode)))
                .width(Length::FillPortion(1))
                .height(Length::Fixed(h))
                .padding([sp.sm as f32, sp.sm as f32])
                .style(button_style(tokens, variant)),
        );
    }

    let proxy_toggle = toggler(py.path_proxy_enabled)
        .label(envr_core::i18n::tr_key(
            "gui.runtime.python.path_proxy.hint",
            "开启时由 envr 接管 python/pip；关闭时 shim 透传到系统 PATH。",
            "When on, envr manages python/pip; when off, shims delegate to your system PATH.",
        ))
        .on_toggle(|v| Message::EnvCenter(EnvCenterMsg::SetPythonPathProxy(v)));

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
        column![
            dl_row,
            pip_row,
            proxy_toggle,
            proxy_note,
            cache_note,
        ]
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
    let ty = tokens.typography();
    let sp = tokens.space();
    let muted = gui_theme::to_color(tokens.colors.text_muted);

    let mut dl_row = row![text("Java 下载源").size(ty.body)].spacing(sp.sm as f32);
    for src in [
        JavaDownloadSource::Auto,
        JavaDownloadSource::Domestic,
        JavaDownloadSource::Official,
    ] {
        let label = match src {
            JavaDownloadSource::Auto => "自动（随区域语言）",
            JavaDownloadSource::Domestic => "国内优先（可回退）",
            JavaDownloadSource::Official => "官方",
        };
        let variant = if src == java.download_source {
            ButtonVariant::Primary
        } else {
            ButtonVariant::Secondary
        };
        dl_row = dl_row.push(
            button(button_content_centered(text(label).into()))
                .on_press(Message::EnvCenter(EnvCenterMsg::SetJavaDownloadSource(src)))
                .width(Length::FillPortion(1))
                .height(Length::Fixed(tokens.control_height_secondary))
                .padding([sp.sm as f32, sp.sm as f32])
                .style(button_style(tokens, variant)),
        );
    }

    let proxy_row = row![
        text("PATH 代理").size(ty.body),
        toggler(java.path_proxy_enabled)
            .on_toggle(|v| Message::EnvCenter(EnvCenterMsg::SetJavaPathProxy(v)))
            .size(20.0)
            .spacing(sp.sm as f32),
    ]
    .spacing(sp.sm as f32)
    .align_y(Alignment::Center);

    container(
        column![
            dl_row,
            proxy_row,
            text("仅支持 LTS（8/11/17/21/25）；JAVA_HOME 仅写入用户环境变量。")
                .size(ty.micro)
                .color(muted),
            text("镜像仅对 Temurin 与 Oracle OpenJDK 提供；其他发行版固定使用官方源。")
                .size(ty.micro)
                .color(muted),
        ]
        .spacing(sp.sm as f32),
    )
    .padding(Padding::from([sp.sm as f32, sp.sm as f32]))
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
                envr_core::i18n::tr_key("gui.runtime.remote_error", "远程列表不可用", "Remote list unavailable"),
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

fn node_remote_latest_for_key(state: &EnvCenterState, key: &str) -> Option<RuntimeVersion> {
    state
        .node_remote_latest
        .iter()
        .find(|v| parse_node_major_key(&v.0).as_deref() == Some(key))
        .cloned()
}

fn python_remote_latest_for_key(
    state: &EnvCenterState,
    key: &str,
) -> Option<RuntimeVersion> {
    state
        .python_remote_latest
        .iter()
        .find(|v| parse_python_major_minor_key(&v.0).as_deref() == Some(key))
        .cloned()
}

fn java_remote_latest_for_key(state: &EnvCenterState, key: &str) -> Option<RuntimeVersion> {
    state
        .java_remote_latest
        .iter()
        .find(|v| parse_java_major_key(&v.0).as_deref() == Some(key))
        .cloned()
}

pub fn env_center_view(
    state: &EnvCenterState,
    node_runtime: Option<&NodeRuntimeSettings>,
    python_runtime: Option<&PythonRuntimeSettings>,
    java_runtime: Option<&JavaRuntimeSettings>,
    tokens: ThemeTokens,
) -> Element<'static, Message> {
    let ty = tokens.typography();
    let sp = tokens.space();
    let busy = state.busy;
    let card_s = card_container_style(tokens, 1);

    let path_proxy_on = match state.kind {
        RuntimeKind::Node => node_runtime.map(|n| n.path_proxy_enabled).unwrap_or(true),
        RuntimeKind::Python => python_runtime.map(|p| p.path_proxy_enabled).unwrap_or(true),
        RuntimeKind::Java => java_runtime.map(|j| j.path_proxy_enabled).unwrap_or(true),
        _ => true,
    };

    let cur_line = match &state.current {
        Some(v) => format!(
            "{} {}",
            envr_core::i18n::tr_key("gui.runtime.current", "当前：", "Current:"),
            v.0
        ),
        None => envr_core::i18n::tr_key(
            "gui.runtime.current_none",
            "当前：(未设置)",
            "Current: (not set)",
        ),
    };

    let header_title = format!("{}设置", kind_label(state.kind));
    let show_runtime_fold = matches!(
        state.kind,
        RuntimeKind::Node | RuntimeKind::Python | RuntimeKind::Java
    );
    let toggle_lbl = if state.runtime_settings_expanded {
        envr_core::i18n::tr_key("gui.action.collapse", "折叠", "Collapse")
    } else {
        envr_core::i18n::tr_key("gui.action.expand", "展开", "Expand")
    };

    let toggle_btn = button(
        button_content_centered(
            row![
                Lucide::ChevronsUpDown.view(16.0, gui_theme::to_color(tokens.colors.text)),
                text(toggle_lbl),
            ]
            .spacing(sp.xs as f32)
            .align_y(Alignment::Center)
            .into()
        ),
    )
    .on_press_maybe(
        show_runtime_fold.then_some(Message::EnvCenter(EnvCenterMsg::ToggleRuntimeSettings)),
    )
    .height(Length::Fixed(tokens.control_height_secondary))
    .style(button_style(tokens, ButtonVariant::Secondary));

    let cur_el = text(cur_line)
        .size(ty.caption)
        .color(gui_theme::to_color(tokens.colors.text_muted));

    let header_content = if show_runtime_fold {
        row![
            text(header_title).size(ty.section),
            cur_el,
            toggle_btn,
        ]
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
    } else {
        column![].into()
    };

    // Search/filter text (we reuse `install_input` field).
    let query = state.install_input.trim();
    let query_norm = query.strip_prefix('v').unwrap_or(query);
    let query_key = match state.kind {
        RuntimeKind::Python => parse_python_major_minor_only(query_norm),
        RuntimeKind::Java => parse_major_only(query_norm),
        _ => parse_major_only(query_norm),
    };

    // Group installed versions by key (Node: major, Python: major.minor, else: major).
    let mut installed_by_key: HashMap<String, Vec<RuntimeVersion>> = HashMap::new();
    for v in &state.installed {
        let key = match state.kind {
            RuntimeKind::Python => parse_python_major_minor_key(&v.0),
            RuntimeKind::Node => parse_node_major_key(&v.0),
            RuntimeKind::Java => parse_java_major_key(&v.0),
            _ => parse_major_from_ver(&v.0),
        };
        if let Some(k) = key {
            installed_by_key.entry(k).or_default().push(v.clone());
        }
    }
    for (_k, versions) in installed_by_key.iter_mut() {
        versions.sort_by(|a, b| semver_cmp_desc(&a.0, &b.0));
    }

    // Merge remote keys when available (so empty installs still show suggestions).
    let mut keys_set: HashSet<String> = installed_by_key.keys().cloned().collect();
    if state.kind == RuntimeKind::Node {
        for v in &state.node_remote_latest {
            if let Some(k) = parse_node_major_key(&v.0) {
                keys_set.insert(k);
            }
        }
    } else if state.kind == RuntimeKind::Python {
        for v in &state.python_remote_latest {
            if let Some(k) = parse_python_major_minor_key(&v.0) {
                keys_set.insert(k);
            }
        }
    } else if state.kind == RuntimeKind::Java {
        for v in &state.java_remote_latest {
            if let Some(k) = parse_java_major_key(&v.0) {
                keys_set.insert(k);
            }
        }
    }
    let mut keys: Vec<String> = keys_set.into_iter().collect();
    if state.kind == RuntimeKind::Python {
        keys.sort_by(|a, b| parse_python_key_sort(a).cmp(&parse_python_key_sort(b)).reverse());
    } else if state.kind == RuntimeKind::Java {
        // Rows follow the curated per-distro matrix (not a global 8/11/17/21/25 list).
        let distro = java_runtime
            .map(|j| j.current_distro)
            .unwrap_or(JavaDistro::Temurin);
        keys = distro
            .supported_lts_major_strs()
            .iter()
            .map(|s| (*s).to_string())
            .collect();
    } else {
        keys.sort_by(|a, b| parse_major_num(a).cmp(&parse_major_num(b)).reverse());
    }

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

    let mut show_keys: Vec<String> = if query_norm.is_empty() {
        keys.clone()
    } else {
        let needle = query_key.as_deref().unwrap_or(query_norm);
        match state.kind {
            RuntimeKind::Node => keys
                .into_iter()
                .filter(|k| k.contains(needle))
                .collect(),
            RuntimeKind::Python => {
                if needle.contains('.') {
                    keys.into_iter().filter(|k| k.starts_with(needle)).collect()
                } else {
                    keys.into_iter()
                        .filter(|k| {
                            let mut it = k.split('.');
                            let major = it.next().unwrap_or("");
                            let minor = it.next().unwrap_or("");
                            major == needle || minor == needle
                        })
                        .collect()
                }
            }
            _ => keys.into_iter().filter(|k| k.starts_with(needle)).collect(),
        }
    };
    if state.kind == RuntimeKind::Python {
        show_keys.sort_by(|a, b| parse_python_key_sort(a).cmp(&parse_python_key_sort(b)).reverse());
    } else {
        show_keys.sort_by(|a, b| parse_major_num(a).cmp(&parse_major_num(b)).reverse());
    }

    let node_waiting_remote = state.kind == RuntimeKind::Node
        && state.node_remote_latest.is_empty()
        && state.node_remote_refreshing;
    let python_waiting_remote = state.kind == RuntimeKind::Python
        && state.python_remote_latest.is_empty()
        && state.python_remote_refreshing;
    let java_waiting_remote = state.kind == RuntimeKind::Java
        && state.java_remote_latest.is_empty()
        && state.java_remote_refreshing;

    if let Some(err) = state.remote_error.as_deref() {
        if matches!(state.kind, RuntimeKind::Node | RuntimeKind::Python | RuntimeKind::Java) {
            list_col = list_col.push(remote_error_inline(tokens, err));
        }
    }

    if (busy && show_keys.is_empty())
        || (node_waiting_remote && show_keys.is_empty())
        || (python_waiting_remote && show_keys.is_empty())
        || (java_waiting_remote && show_keys.is_empty())
    {
        list_col = list_col.push(list_loading_skeleton(tokens, state.skeleton_phase));
    } else if show_keys.is_empty() {
        list_col = list_col.push(matches_empty_hint());
    } else {
        for key in show_keys.iter() {
            let installed_versions = installed_by_key
                .get(key)
                .cloned()
                .unwrap_or_default();
            let current_key = state.current.as_ref().and_then(|v| match state.kind {
                RuntimeKind::Python => parse_python_major_minor_key(&v.0),
                RuntimeKind::Node => parse_node_major_key(&v.0),
                RuntimeKind::Java => parse_java_major_key(&v.0),
                _ => parse_major_from_ver(&v.0),
            });
            let is_active = current_key.as_deref() == Some(key.as_str());
            let show_as_active = is_active && path_proxy_on;

            let highest_installed = installed_versions.first().cloned();

            let label_base = if state.kind == RuntimeKind::Node {
                if let Some(_rv) = node_remote_latest_for_key(state, key) {
                    // Keep list stable: show `Node <major>` but use latest patch for install spec.
                    format!("{} {}", kind_label(state.kind), key)
                } else {
                    format!("{} {}", kind_label(state.kind), key)
                }
            } else if state.kind == RuntimeKind::Python {
                format!("{} {}", kind_label(state.kind), key)
            } else {
                format!("{} {}", kind_label(state.kind), key)
            };

            let left_text = if show_as_active {
                format!(
                    "{} {}",
                    label_base,
                    envr_core::i18n::tr_key(
                        "gui.runtime.current_tag",
                        "(当前)",
                        "(current)",
                    )
                )
            } else {
                label_base
            };

            let install_spec = || -> String {
                if state.kind == RuntimeKind::Node {
                    node_remote_latest_for_key(state, key)
                        .map(|v| v.0)
                        .unwrap_or_else(|| key.clone())
                } else if state.kind == RuntimeKind::Python {
                    python_remote_latest_for_key(state, key)
                        .map(|v| v.0)
                        .unwrap_or_else(|| key.clone())
                } else if state.kind == RuntimeKind::Java {
                    java_remote_latest_for_key(state, key)
                        .map(|v| v.0)
                        .unwrap_or_else(|| key.clone())
                } else {
                    key.clone()
                }
            };

            let action_btn: Element<'static, Message> = if show_as_active {
                container(space()).into()
            } else if let Some(highest) = highest_installed {
                let use_btn = button(
                    button_content_centered(
                        row![
                            Lucide::Package.view(14.0, txt),
                            text(envr_core::i18n::tr_key("gui.action.use", "切换", "Use")),
                        ]
                        .spacing(sp.xs as f32)
                        .align_y(Alignment::Center)
                        .into(),
                    ),
                )
                .on_press_maybe(path_proxy_on.then_some(Message::EnvCenter(
                    EnvCenterMsg::SubmitUse(highest.0.clone()),
                )))
                .height(Length::Fixed(
                    tokens
                        .control_height_secondary
                        .max(tokens.min_click_target_px()),
                ))
                .padding([sp.sm as f32, sp.sm as f32])
                .style(button_style(tokens, ButtonVariant::Secondary));

                let uninstall_btn = button(
                    button_content_centered(
                        row![
                            Lucide::X.view(14.0, gui_theme::to_color(tokens.colors.danger)),
                            text(envr_core::i18n::tr_key(
                                "gui.action.uninstall",
                                "卸载",
                                "Uninstall",
                            )),
                        ]
                        .spacing(sp.xs as f32)
                        .align_y(Alignment::Center)
                        .into(),
                    ),
                )
                .on_press_maybe(Some(Message::EnvCenter(EnvCenterMsg::SubmitUninstall(
                    highest.0.clone(),
                ))))
                .height(Length::Fixed(
                    tokens
                        .control_height_secondary
                        .max(tokens.min_click_target_px()),
                ))
                .padding([sp.sm as f32, sp.sm as f32])
                .style(button_style(tokens, ButtonVariant::Danger));

                container(
                    row![use_btn, uninstall_btn]
                        .spacing(sp.sm as f32)
                        .align_y(Alignment::Center),
                )
                .into()
            } else {
                let spec = install_spec();
                let install_btn = button(
                    button_content_centered(
                        row![
                            Lucide::Download.view(14.0, txt),
                            text(envr_core::i18n::tr_key("gui.action.install", "安装", "Install")),
                        ]
                        .spacing(sp.xs as f32)
                        .align_y(Alignment::Center)
                        .into(),
                    ),
                )
                .on_press_maybe(Some(Message::EnvCenter(EnvCenterMsg::SubmitInstall(
                    spec.clone(),
                ))))
                .style(button_style(tokens, ButtonVariant::Primary));

                let install_and_use_btn = button(
                    button_content_centered(
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
                    ),
                )
                .on_press_maybe(path_proxy_on.then_some(Message::EnvCenter(
                    EnvCenterMsg::SubmitInstallAndUse(spec),
                )))
                .style(button_style(tokens, ButtonVariant::Secondary));

                container(
                    row![install_btn, install_and_use_btn]
                        .spacing(sp.sm as f32)
                        .align_y(Alignment::Center),
                )
                .into()
            };

            list_col = list_col.push(
                container(
                    row![
                        text(left_text).width(Length::Fill),
                        action_btn,
                    ]
                    .spacing(sp.sm as f32)
                    .align_y(Alignment::Center)
                    .height(Length::Fixed(tokens.list_row_height())),
                )
                .style(card_container_style(tokens, 1)),
            );
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
            "gui.runtime.search_placeholder",
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
    let direct_install_btn = button(
        button_content_centered(
            row![
                Lucide::Download.view(14.0, txt),
                text(envr_core::i18n::tr_key("gui.action.install", "安装", "Install")),
            ]
            .spacing(sp.xs as f32)
            .align_y(Alignment::Center)
            .into(),
        ),
    )
    .on_press_maybe(direct_spec_nonempty.then_some(Message::EnvCenter(
        EnvCenterMsg::SubmitDirectInstall,
    )))
    .height(Length::Fixed(ctrl_h))
    .padding([sp.sm as f32, sp.md as f32])
    .style(button_style(tokens, ButtonVariant::Primary));

    let direct_install_use_btn = button(
        button_content_centered(
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
        ),
    )
    .on_press_maybe(
        (direct_spec_nonempty && path_proxy_on).then_some(Message::EnvCenter(
            EnvCenterMsg::SubmitDirectInstallAndUse,
        )),
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
    let mut col = if state.kind == RuntimeKind::Java {
        if let Some(distro_row) = java_distro_row {
            column![header, runtime_settings_block, distro_row]
        } else {
            column![header, runtime_settings_block]
        }
    } else {
        column![header, runtime_settings_block, filter_row]
    }
    .spacing(sp.sm as f32)
    .width(Length::Fill);
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

fn semver_parts(s: &str) -> Option<(u64, u64, u64)> {
    let t = s.trim().strip_prefix('v').unwrap_or(s.trim());
    let mut it = t.split('.');
    let a: u64 = it.next()?.parse().ok()?;
    let b: u64 = it.next()?.parse().ok()?;
    let c: u64 = it.next()?.parse().ok()?;
    if it.next().is_some() {
        return None;
    }
    Some((a, b, c))
}

fn semver_cmp_desc(a: &str, b: &str) -> std::cmp::Ordering {
    match (semver_parts(a), semver_parts(b)) {
        (Some((ma, m1, p)), Some((mb, n1, q))) => (mb, n1, q).cmp(&(ma, m1, p)),
        _ => b.cmp(a),
    }
}

fn parse_major_num(major: &str) -> u64 {
    major.parse::<u64>().unwrap_or(0)
}

fn parse_major_from_ver(ver: &str) -> Option<String> {
    semver_parts(ver).map(|(ma, _mi, _patch)| ma.to_string())
}

fn parse_node_major_key(ver: &str) -> Option<String> {
    parse_major_from_ver(ver)
}

fn parse_java_major_key(ver: &str) -> Option<String> {
    let t = ver.trim().strip_prefix('v').unwrap_or(ver.trim());
    let major = t.split(['.', '+', '-']).next().unwrap_or("").trim();
    if major.is_empty() || !major.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(major.to_string())
}

fn parse_python_major_minor_key(ver: &str) -> Option<String> {
    semver_parts(ver).map(|(ma, mi, _patch)| format!("{ma}.{mi}"))
}

fn parse_major_only(s: &str) -> Option<String> {
    let t = s.trim().strip_prefix('v').unwrap_or(s.trim());
    if t.is_empty() {
        return None;
    }
    if t.chars().all(|c| c.is_ascii_digit()) {
        Some(t.to_string())
    } else {
        None
    }
}

fn parse_python_major_minor_only(s: &str) -> Option<String> {
    let t = s.trim().strip_prefix('v').unwrap_or(s.trim());
    if t.is_empty() {
        return None;
    }
    let parts: Vec<&str> = t.split('.').collect();
    if parts.len() != 2 {
        return None;
    }
    if parts[0].chars().all(|c| c.is_ascii_digit()) && parts[1].chars().all(|c| c.is_ascii_digit())
    {
        Some(format!("{}.{}", parts[0], parts[1]))
    } else {
        None
    }
}

fn parse_python_key_sort(k: &str) -> (u64, u64) {
    let mut it = k.split('.');
    let a = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let b = it.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    (a, b)
}

fn list_loading_skeleton(tokens: ThemeTokens, phase: f32) -> Element<'static, Message> {
    use iced::Background;

    let pulse = (phase * std::f32::consts::TAU).sin() * 0.5 + 0.5;
    let fill = gui_theme::to_color(tokens.colors.text_muted).scale_alpha(0.07 + 0.16 * pulse);
    let row_h = tokens.list_row_height();
    let bar_h = row_h * 0.42;
    let n = tokens.list_skeleton_rows();
    let mut col = column![].spacing(0);
    for i in 0..n {
        col = col.push(
            container(
                space()
                    .width(Length::Fill)
                    .height(Length::Fixed(bar_h)),
            )
            .width(Length::Fill)
            .height(Length::Fixed(row_h))
            .align_y(iced::alignment::Vertical::Center)
            .padding([0, tokens.space().md as u16])
            .style(move |_theme: &Theme| {
                iced::widget::container::Style::default().background(Background::Color(fill))
            }),
        );
        if i + 1 < n {
            col = col.push(rule::horizontal(1.0));
        }
    }
    col.into()
}
