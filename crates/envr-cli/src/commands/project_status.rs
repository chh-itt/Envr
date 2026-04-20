//! Shared logic for `envr status` and `envr hook prompt`.

use envr_config::project_config::{
    ProjectConfig, ProjectConfigLocation, load_project_config_profile,
};
use envr_domain::runtime::parse_runtime_kind;
use envr_error::EnvrResult;
use envr_shim_core::{
    CoreCommand, ShimContext, WhichRuntimeSource, resolve_core_shim_command, which_runtime_detail,
};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct RuntimeStatusRow {
    pub kind: String,
    pub pin: Option<String>,
    pub active_version: String,
    pub source: WhichRuntimeSource,
    pub ok: bool,
    pub detail: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ProjectStatus {
    pub working_dir: PathBuf,
    pub project_dir: Option<PathBuf>,
    pub profile: Option<String>,
    pub rows: Vec<RuntimeStatusRow>,
}

fn core_command_for_project_key(key: &str) -> Option<CoreCommand> {
    match key.trim().to_ascii_lowercase().as_str() {
        "node" => Some(CoreCommand::Node),
        "python" => Some(CoreCommand::Python),
        "java" => Some(CoreCommand::Java),
        "kotlin" => Some(CoreCommand::Kotlin),
        "scala" => Some(CoreCommand::Scala),
        "clojure" => Some(CoreCommand::Clojure),
        "groovy" => Some(CoreCommand::Groovy),
        "terraform" => Some(CoreCommand::Terraform),
        "v" => Some(CoreCommand::V),
        "dart" => Some(CoreCommand::Dart),
        "go" => Some(CoreCommand::Go),
        "php" => Some(CoreCommand::Php),
        "deno" => Some(CoreCommand::Deno),
        "bun" => Some(CoreCommand::Bun),
        _ => None,
    }
}

fn collect_lang_keys(cfg: Option<&ProjectConfig>) -> Vec<String> {
    let mut keys: BTreeSet<String> = BTreeSet::new();
    for k in [
        "node",
        "python",
        "java",
        "kotlin",
        "scala",
        "clojure",
        "groovy",
        "terraform",
    "v",
    "dart",
        "go",
        "deno",
        "bun",
        "php",
    ] {
        keys.insert(k.to_string());
    }
    if let Some(c) = cfg {
        for k in c.runtimes.keys() {
            keys.insert(k.trim().to_ascii_lowercase());
        }
    }
    keys.into_iter().collect()
}

fn pin_for_key(
    cfg: Option<&envr_config::project_config::ProjectConfig>,
    key: &str,
) -> Option<String> {
    cfg.and_then(|c| c.runtimes.get(key))
        .and_then(|r| r.version.as_deref())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

/// Build status using project config already loaded for this session (e.g. from [`crate::CliProjectContext`]).
pub fn build_project_status_from_loaded(
    ctx: &ShimContext,
    loaded: &Option<(ProjectConfig, ProjectConfigLocation)>,
) -> EnvrResult<ProjectStatus> {
    let (project_dir, cfg_ref) = loaded
        .as_ref()
        .map(|(c, loc)| (Some(loc.dir.clone()), Some(c)))
        .unwrap_or((None, None));

    let mut rows = Vec::new();
    for key in collect_lang_keys(cfg_ref) {
        if key == "rust" {
            continue;
        }
        if parse_runtime_kind(&key).is_err() {
            continue;
        }
        let Some(cmd) = core_command_for_project_key(&key) else {
            continue;
        };
        let pin = pin_for_key(cfg_ref, &key);
        match resolve_core_shim_command(cmd, ctx) {
            Ok(resolved) => match which_runtime_detail(cmd, ctx, &resolved.executable) {
                Ok(d) => rows.push(RuntimeStatusRow {
                    kind: key.clone(),
                    pin,
                    active_version: d.version,
                    source: d.source,
                    ok: true,
                    detail: None,
                }),
                Err(e) => rows.push(RuntimeStatusRow {
                    kind: key,
                    pin,
                    active_version: "?".into(),
                    source: WhichRuntimeSource::GlobalCurrent,
                    ok: false,
                    detail: Some(e.to_string()),
                }),
            },
            Err(e) => rows.push(RuntimeStatusRow {
                kind: key,
                pin,
                active_version: "?".into(),
                source: WhichRuntimeSource::GlobalCurrent,
                ok: false,
                detail: Some(e.to_string()),
            }),
        }
    }

    Ok(ProjectStatus {
        working_dir: ctx.working_dir.clone(),
        project_dir,
        profile: ctx.profile.clone(),
        rows,
    })
}

/// Load project config from disk, then build status (for callers that only have a [`ShimContext`]).
#[allow(dead_code)] // Kept for embedders/tests; CLI uses [`build_project_status_from_loaded`] + session.
pub fn build_project_status(ctx: &ShimContext) -> EnvrResult<ProjectStatus> {
    let loaded = load_project_config_profile(&ctx.working_dir, ctx.profile.as_deref())?;
    build_project_status_from_loaded(ctx, &loaded)
}

fn source_label(src: WhichRuntimeSource) -> &'static str {
    match src {
        WhichRuntimeSource::ProjectPin => "project_pin",
        WhichRuntimeSource::GlobalCurrent => "global_current",
        WhichRuntimeSource::PathProxyBypass => "path_proxy_bypass",
    }
}

pub fn status_to_json(st: &ProjectStatus) -> Value {
    let project = st.project_dir.as_ref().map(|d| {
        json!({
            "dir": d.to_string_lossy(),
        })
    });
    let rows: Vec<Value> = st
        .rows
        .iter()
        .map(|r| {
            json!({
                "kind": r.kind,
                "pin": r.pin,
                "active_version": r.active_version,
                "source": source_label(r.source),
                "ok": r.ok,
                "detail": r.detail,
            })
        })
        .collect();
    json!({
        "working_dir": st.working_dir.to_string_lossy(),
        "profile": st.profile,
        "project": project,
        "runtimes": rows,
    })
}

/// Single-line prompt fragment, e.g. `[node:20.10 python:3.12] ` (trailing space).
pub fn format_prompt_segment(st: &ProjectStatus) -> String {
    let mut parts: Vec<String> = Vec::new();
    for r in &st.rows {
        if !r.ok {
            continue;
        }
        if r.source == WhichRuntimeSource::PathProxyBypass {
            parts.push(format!("{}:sys", r.kind));
            continue;
        }
        if r.pin.is_some()
            || r.source == WhichRuntimeSource::ProjectPin
            || matches!(r.kind.as_str(), "node" | "python" | "java")
        {
            parts.push(format!("{}:{}", r.kind, r.active_version));
        }
    }
    if parts.is_empty() {
        return String::new();
    }
    format!("[{}] ", parts.join(" "))
}
