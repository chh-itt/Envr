//! Unified CLI output: JSON envelope (`schema_version`, `success`, `code`, `message`, `data`, `diagnostics`) and exit codes.
//! Envelope vs per-`message` `data` versioning: `docs/cli/output-contract.md` and `docs/schemas/README.md`.
//!
//! Human vs machine and quiet/porcelain/color rules live in [`crate::presenter::CliUxPolicy`]; this module implements
//! envelope builders and emitters using that policy.

use crate::cli::{GlobalArgs, OutputFormat};
use crate::codes;
use crate::command_outcome::CliExit;
use crate::presenter::{CliPersona, CliUxPolicy};

use envr_error::{EnvrError, ErrorCode};
use serde_json::{Map, Value, json};
use std::collections::HashMap;
use std::sync::OnceLock;

fn ok_message_for_code(code: &str) -> String {
    // Message is human-facing text. Automation must use `code`.
    let key = format!("cli.ok.{code}");
    let s = envr_core::i18n::tr_key(&key, "", "");
    if s.is_empty() {
        envr_core::i18n::tr_key("cli.ok._default", "ok", "ok")
    } else {
        s
    }
}

/// Top-level `schema_version` integer for every `--format json` envelope line.
/// Bump when making breaking changes to the envelope or documented `data` shapes.
/// JSON Schema sources for the envelope and selected `data` blobs: `schemas/cli/` (see integration tests).
pub const CLI_JSON_SCHEMA_VERSION: u32 = 3;
const ERROR_KIND_MAP_JSON: &str = include_str!("../../../schemas/cli/error-kind-map.json");

struct ErrorKindSpec {
    default: &'static str,
    mappings: HashMap<String, &'static str>,
}

fn error_kind_spec() -> &'static ErrorKindSpec {
    static SPEC: OnceLock<ErrorKindSpec> = OnceLock::new();
    SPEC.get_or_init(|| {
        let parsed: Value =
            serde_json::from_str(ERROR_KIND_MAP_JSON).expect("error-kind-map.json must be valid JSON");
        let default_raw = parsed
            .get("default")
            .and_then(Value::as_str)
            .expect("error-kind-map.json default must be string");
        let mappings_obj = parsed
            .get("map")
            .and_then(Value::as_object)
            .expect("error-kind-map.json map must be object");
        let mut mappings = HashMap::new();
        for (code, kind) in mappings_obj {
            let kind = kind
                .as_str()
                .expect("error-kind-map.json map values must be strings");
            let leaked_kind: &'static str = Box::leak(kind.to_string().into_boxed_str());
            mappings.insert(code.to_string(), leaked_kind);
        }
        let default: &'static str = Box::leak(default_raw.to_string().into_boxed_str());
        ErrorKindSpec { default, mappings }
    })
}

/// Replace `{name}` placeholders in a localized template (order-independent).
pub fn fmt_template(tmpl: &str, vars: &[(&str, &str)]) -> String {
    let mut s = tmpl.to_string();
    for (k, v) in vars {
        s = s.replace(&format!("{{{k}}}"), v);
    }
    s
}

/// Stable snake_case code strings aligned with [`ErrorCode`] serialization.
pub fn error_code_token(code: ErrorCode) -> &'static str {
    match code {
        ErrorCode::Unknown => "unknown",
        ErrorCode::Io => "io",
        ErrorCode::Config => "config",
        ErrorCode::Validation => "validation",
        ErrorCode::Runtime => "runtime",
        ErrorCode::Platform => "platform",
        ErrorCode::Download => "download",
        ErrorCode::Mirror => "mirror",
    }
}

/// Stable token for metrics/logging output mode fields.
pub fn output_mode_token(mode: OutputFormat) -> &'static str {
    match mode {
        OutputFormat::Text => "text",
        OutputFormat::Json => "json",
    }
}

/// Fallback metrics token for failures that return a non-zero process code without a typed [`EnvrError`].
#[inline]
pub fn metrics_error_code_for_exit(exit_code: i32) -> Option<&'static str> {
    if exit_code == 0 {
        None
    } else {
        Some("nonzero_exit")
    }
}

/// Human-oriented primary detail for stderr (no `validation error:`-style wrapper; the bracket tag carries the class).
pub fn envr_error_line_message(err: &EnvrError) -> String {
    match err {
        EnvrError::Io(e) => e.to_string(),
        EnvrError::Config(s)
        | EnvrError::Validation(s)
        | EnvrError::Runtime(s)
        | EnvrError::Platform(s)
        | EnvrError::Download(s)
        | EnvrError::Mirror(s)
        | EnvrError::Unknown(s) => s.clone(),
    }
}

/// Maps a JSON envelope `code` string (snake_case) to a grep-friendly bracket label, e.g. `validation` → `[E_VALIDATION]`.
pub fn error_bracket_label(json_code: &str) -> String {
    let u = json_code.to_ascii_uppercase();
    format!("[E_{u}]")
}

/// Stable coarse-grained error category used by `data.error.kind`.
pub fn error_kind_token(code: &str) -> &'static str {
    let spec = error_kind_spec();
    spec.mappings.get(code).copied().unwrap_or(spec.default)
}

/// Unified stderr error line: always `envr:` prefix; `json_code` matches `--format json` envelope `code` when applicable.
pub fn print_error_text(json_code: &str, message: &str) {
    let tag = error_bracket_label(json_code);
    eprintln!("envr: {tag} {message}");
}

fn trim_failure_for_quiet_json(
    policy: CliUxPolicy,
    code: &str,
    message: &str,
    data: Value,
    diagnostics: &[String],
) -> (String, Value, Vec<String>) {
    if !policy.quiet_json_failure_trim() {
        return (message.to_string(), data, diagnostics.to_vec());
    }
    (error_bracket_label(code), Value::Null, vec![])
}

fn add_error_object_if_possible(code: &str, message: &str, mut data: Value, diagnostics: &[String]) -> Value {
    let Value::Object(ref mut m) = data else {
        return data;
    };
    if m.contains_key("error") {
        return Value::Object(m.clone());
    }
    m.insert(
        "error".into(),
        json!({
            "code": code,
            "kind": error_kind_token(code),
            "message": message,
            "diagnostics_len": diagnostics.len(),
            "source_chain": diagnostics.iter().map(|d| json!({ "message": d })).collect::<Vec<Value>>(),
        }),
    );
    data
}

pub(crate) fn build_failure_envelope_value(
    g: &GlobalArgs,
    code: &str,
    message: &str,
    data: Value,
    diagnostics: &[String],
) -> Value {
    let policy = CliUxPolicy::from_global(g);
    let data = if policy.quiet {
        data
    } else {
        add_error_object_if_possible(code, message, data, diagnostics)
    };
    let (msg, data, diags) = trim_failure_for_quiet_json(policy, code, message, data, diagnostics);
    build_envelope_value(false, code, &msg, data, &diags)
}

/// Failure with a stable string `code` (child exit, project checks, etc.): same envelope shape as other CLI errors.
/// With `--quiet`, `message` is replaced by `[E_CODE]` and `diagnostics` are omitted from JSON.
pub fn emit_failure_envelope(
    g: &GlobalArgs,
    code: &'static str,
    message: &str,
    data: Value,
    diagnostics: &[String],
    exit_code: i32,
) -> CliExit {
    let policy = CliUxPolicy::from_global(g);
    match policy.format {
        OutputFormat::Json => {
            let v = build_failure_envelope_value(g, code, message, data, diagnostics);
            println!("{}", serde_json::to_string(&v).unwrap_or_else(|_| "{}".to_string()));
        }
        OutputFormat::Text => {
            if policy.quiet {
                eprintln!("envr: {}", error_bracket_label(code));
            } else {
                print_error_text(code, message);
            }
        }
    }
    CliExit {
        exit_code,
        error_code: Some(code),
    }
}

/// Exit-code policy table keyed by stable [`ErrorCode`] class.
///
/// - `1`: user/business/config/runtime class failures
/// - `2`: external/system/network class failures (`io`, `download`, `mirror`)
#[inline]
pub fn exit_code_for_error_code(code: ErrorCode) -> i32 {
    match code {
        ErrorCode::Io | ErrorCode::Download | ErrorCode::Mirror => 2,
        _ => 1,
    }
}

/// Per design doc: 1 = user/business, 2 = external (I/O, network fetch, mirror).
#[inline]
pub fn exit_code_for_error(err: &EnvrError) -> i32 {
    exit_code_for_error_code(err.code())
}

/// Localized mirror/network suggestion when the error is classified as download, mirror, or I/O that looks network-related.
fn mirror_network_hint_message(err: &EnvrError) -> Option<String> {
    let line = envr_error_line_message(err).to_ascii_lowercase();
    let io_looks_network = matches!(err.code(), ErrorCode::Io)
        && (line.contains("http")
            || line.contains("tls")
            || line.contains("connection")
            || line.contains("timed out")
            || line.contains("timeout")
            || line.contains("dns")
            || line.contains("network"));
    if !matches!(err.code(), ErrorCode::Download | ErrorCode::Mirror) && !io_looks_network {
        return None;
    }
    Some(envr_core::i18n::tr_key(
        "cli.hint.network_mirror",
        "若下载较慢或失败（例如在中国大陆），可尝试：envr config set mirror.mode auto",
        "If downloads are slow or fail (e.g. in mainland China), try: `envr config set mirror.mode auto`",
    ))
}

fn mirror_network_json_error_data(g: &GlobalArgs, err: &EnvrError) -> Value {
    if CliUxPolicy::from_global(g).quiet {
        return Value::Null;
    }
    mirror_network_hint_message(err)
        .map(|h| json!({ "hints": [h] }))
        .unwrap_or(Value::Null)
}

fn common_error_hints(err: &EnvrError) -> Vec<String> {
    let line = envr_error_line_message(err).to_ascii_lowercase();
    let mut hints = Vec::new();
    if line.contains("timed out")
        || line.contains("timeout")
        || line.contains("connection refused")
        || line.contains("dns")
        || line.contains("tls")
    {
        hints.push(envr_core::i18n::tr_key(
            "cli.hint.network_retry_or_mirror",
            "网络请求失败：请检查网络/代理，或稍后重试；若在中国大陆可尝试镜像模式。",
            "Network request failed: check network/proxy and retry; mirror mode may help in restricted regions.",
        ));
    }
    if line.contains("permission denied") || line.contains("access is denied") {
        hints.push(envr_core::i18n::tr_key(
            "cli.hint.permission",
            "权限不足：请检查 ENVR_RUNTIME_ROOT 目录可写性，必要时切换到用户可写路径。",
            "Permission denied: ensure ENVR_RUNTIME_ROOT is writable, or switch to a user-writable path.",
        ));
    }
    if line.contains("not found")
        || line.contains("no such file")
        || line.contains("cannot find the path")
    {
        hints.push(envr_core::i18n::tr_key(
            "cli.hint.missing_file",
            "文件或目录不存在：可先执行 `envr doctor` 与 `envr cache index sync` 进行修复与预热。",
            "File/directory not found: run `envr doctor` and `envr cache index sync` to repair and warm caches.",
        ));
    }
    hints
}

/// Build the JSON object emitted by [`write_envelope`] (compact `serde_json` is one line).
pub(crate) fn build_envelope_value(
    success: bool,
    code: &str,
    message: &str,
    data: Value,
    diagnostics: &[String],
) -> Value {
    let mut m = Map::new();
    m.insert(
        "schema_version".into(),
        json!(CLI_JSON_SCHEMA_VERSION),
    );
    m.insert("success".into(), json!(success));
    m.insert("code".into(), json!(code));
    m.insert("message".into(), json!(message));
    m.insert("data".into(), data);
    m.insert(
        "diagnostics".into(),
        serde_json::to_value(diagnostics).unwrap_or_else(|_| json!([])),
    );
    Value::Object(m)
}

/// Print one JSON line with the standard envelope (design doc §4).
///
/// Returns the envelope `code` when `success` is false and `code` is [`Some`] (for embedding in [`CliExit`]).
pub fn write_envelope(
    success: bool,
    code: &'static str,
    message: &str,
    data: Value,
    diagnostics: &[String],
) -> Option<&'static str> {
    let v = build_envelope_value(success, code, message, data, diagnostics);
    println!("{}", serde_json::to_string(&v).unwrap_or_else(|_| "{}".to_string()));
    (!success).then_some(code)
}

pub fn emit_validation(g: &GlobalArgs, cmd: &str, example: &str) -> CliExit {
    let tmpl = envr_core::i18n::tr_key(
        "cli.validation.missing_args",
        "`{cmd}` 缺少参数（示例：{example}）",
        "missing arguments for `{cmd}` (example: {example})",
    );
    let msg = fmt_template(&tmpl, &[("cmd", cmd), ("example", example)]);
    let policy = CliUxPolicy::from_global(g);
    match policy.format {
        OutputFormat::Json => {
            let v = build_failure_envelope_value(g, codes::err::VALIDATION, &msg, Value::Null, &[]);
            println!("{}", serde_json::to_string(&v).unwrap_or_else(|_| "{}".to_string()));
        }
        OutputFormat::Text => {
            if policy.quiet {
                eprintln!("envr: {}", error_bracket_label(codes::err::VALIDATION));
            } else {
                print_error_text(codes::err::VALIDATION, &msg);
            }
        }
    }
    CliExit::failure(1, codes::err::VALIDATION)
}

pub fn emit_envr_error(g: &GlobalArgs, err: EnvrError) -> i32 {
    let code = error_code_token(err.code());
    let payload = err.to_payload();
    let mut diags = payload.chain;
    let hint_lines = common_error_hints(&err);
    diags.extend(hint_lines.iter().cloned());
    let exit = exit_code_for_error(&err);
    tracing::error!(
        target: "envr_cli",
        cli_error_kind = %code,
        cli_error_exit_code = exit,
        cli_error_diagnostics_len = diags.len(),
        "cli error"
    );
    let policy = CliUxPolicy::from_global(g);
    let mut line = envr_error_line_message(&err);
    if policy.human_text_primary()
        && let Some(ref h) = mirror_network_hint_message(&err)
    {
        line.push('\n');
        line.push_str(h);
    }
    let json_error_data = mirror_network_json_error_data(g, &err);
    let json_error_data = if policy.quiet || hint_lines.is_empty() {
        json_error_data
    } else if json_error_data.is_null() {
        json!({ "hints": hint_lines })
    } else {
        let mut obj = json_error_data.as_object().cloned().unwrap_or_default();
        obj.insert("hints".into(), json!(hint_lines));
        Value::Object(obj)
    };
    match policy.format {
        OutputFormat::Json => {
            let v = build_failure_envelope_value(g, code, &payload.message, json_error_data, &diags);
            println!("{}", serde_json::to_string(&v).unwrap_or_else(|_| "{}".to_string()));
        }
        OutputFormat::Text => {
            if policy.quiet {
                eprintln!("envr: {}", error_bracket_label(code));
            } else {
                print_error_text(code, &line);
            }
        }
    }
    exit
}

pub fn emit_ok<F: FnOnce()>(g: &GlobalArgs, message: &'static str, data: Value, text: F) -> CliExit {
    let policy = CliUxPolicy::from_global(g);
    match policy.format {
        OutputFormat::Json => {
            let human = ok_message_for_code(message);
            write_envelope(true, message, &human, data, &[]);
        }
        OutputFormat::Text => {
            text();
        }
    }
    CliExit::ok()
}

/// Add normalized actionable guidance for automation-friendly success payloads.
///
/// `next_steps` is a list of `{ "id", "text" }` objects to keep shape stable
/// across commands while allowing localized copy.
pub fn with_next_steps(data: Value, steps: Vec<(&'static str, String)>) -> Value {
    with_next_steps_for_persona(data, steps, CliPersona::from_env())
}

fn with_next_steps_for_persona(
    data: Value,
    steps: Vec<(&'static str, String)>,
    persona: CliPersona,
) -> Value {
    if steps.is_empty() {
        return data;
    }
    let mut obj = match data {
        Value::Object(m) => m,
        _ => return data,
    };
    let items: Vec<Value> = select_steps_for_persona(steps, persona)
        .into_iter()
        .map(|(id, text)| json!({ "id": id, "text": text }))
        .collect();
    obj.insert("next_steps".into(), Value::Array(items));
    Value::Object(obj)
}

fn select_steps_for_persona(
    steps: Vec<(&'static str, String)>,
    persona: CliPersona,
) -> Vec<(&'static str, String)> {
    let max = max_next_steps_for_persona(persona);
    if matches!(persona, CliPersona::Operator) {
        return steps.into_iter().take(max).collect();
    }

    let mut ranked: Vec<(usize, i32, (&'static str, String))> = steps
        .into_iter()
        .enumerate()
        .map(|(idx, step)| (idx, next_step_persona_rank(step.0, persona), step))
        .collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    ranked
        .into_iter()
        .take(max)
        .map(|(_, _, step)| step)
        .collect()
}

fn next_step_persona_rank(id: &str, persona: CliPersona) -> i32 {
    match persona {
        CliPersona::Operator => 0,
        CliPersona::Automation => {
            // Automation prefers deterministic probe/status checks over setup guidance.
            if id.starts_with("verify_")
                || id.starts_with("check_")
                || id.starts_with("resolve_")
                || id.contains("status")
            {
                30
            } else if id.starts_with("set_") || id.starts_with("sync_") {
                20
            } else if id.starts_with("run_") || id.starts_with("fix_") || id.starts_with("init_") {
                10
            } else {
                0
            }
        }
        CliPersona::Onboarding => {
            // Onboarding prioritizes setup and repair actions before deep inspection commands.
            if id.starts_with("init_") || id.starts_with("fix_") || id.starts_with("run_doctor") {
                30
            } else if id.starts_with("sync_") || id.starts_with("set_") {
                20
            } else if id.starts_with("check_") || id.starts_with("verify_") || id.contains("status") {
                10
            } else {
                0
            }
        }
    }
}

#[inline]
fn max_next_steps_for_persona(persona: CliPersona) -> usize {
    match persona {
        // Automation consumers should get concise guidance.
        CliPersona::Automation => 1,
        // Operator keeps backward-compatible full guidance.
        CliPersona::Operator => usize::MAX,
        // Onboarding gets a short curated list to reduce overload.
        CliPersona::Onboarding => 3,
    }
}

/// Script-friendly plain lines (`--porcelain`); see [`CliUxPolicy::wants_porcelain_lines`].
#[inline]
pub fn wants_porcelain(g: &GlobalArgs) -> bool {
    CliUxPolicy::from_global(g).wants_porcelain_lines()
}

/// Whether stdout may use ANSI styles; see [`CliUxPolicy::use_ansi_stdout`].
#[inline]
pub fn use_terminal_styles(g: &GlobalArgs) -> bool {
    CliUxPolicy::from_global(g).use_ansi_stdout()
}

pub fn emit_doctor(
    g: &GlobalArgs,
    success: bool,
    code: &'static str,
    message: &str,
    data: Value,
    text: impl FnOnce(),
) -> CliExit {
    let policy = CliUxPolicy::from_global(g);
    let err_code = match policy.format {
        OutputFormat::Json => write_envelope(success, code, message, data, &[]),
        OutputFormat::Text => {
            text();
            (!success).then_some(code)
        }
    };
    CliExit {
        exit_code: if success { 0 } else { 1 },
        error_code: err_code,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{GlobalArgs, OutputFormat};
    use envr_error::EnvrError;
    use proptest::prelude::*;

    #[test]
    fn error_bracket_uppercases_snake_code() {
        assert_eq!(error_bracket_label("validation"), "[E_VALIDATION]");
        assert_eq!(error_bracket_label("io"), "[E_IO]");
        assert_eq!(error_bracket_label("child_exit"), "[E_CHILD_EXIT]");
    }

    #[test]
    fn emit_envr_error_quiet_uses_bracket_only_in_text_mode() {
        let g = GlobalArgs {
            output_format: None,
            porcelain: false,
            quiet: true,
            no_color: true,
            debug: false,
            verbose: false,
            runtime_root: None,
        };
        let code = emit_envr_error(&g, EnvrError::Download("example failure".into()));
        assert_eq!(code, 2);
    }

    #[test]
    fn emit_envr_error_quiet_json_message_is_tag() {
        let g = GlobalArgs {
            output_format: Some(OutputFormat::Json),
            porcelain: false,
            quiet: true,
            no_color: true,
            debug: false,
            verbose: false,
            runtime_root: None,
        };
        let code = emit_envr_error(&g, EnvrError::Mirror("bad mirror".into()));
        assert_eq!(code, 2);
    }

    #[test]
    fn build_failure_envelope_value_quiet_json_trims_data_and_diagnostics() {
        let g = GlobalArgs {
            output_format: Some(OutputFormat::Json),
            porcelain: false,
            quiet: true,
            no_color: true,
            debug: false,
            verbose: false,
            runtime_root: None,
        };
        let v = build_failure_envelope_value(
            &g,
            "child_exit",
            "child failed",
            json!({"exit_code": 1}),
            &["x".to_string()],
        );
        assert_eq!(v["message"], "[E_CHILD_EXIT]");
        assert!(v["data"].is_null());
        assert!(v["diagnostics"].as_array().is_some_and(|a| a.is_empty()));
    }

    #[test]
    fn build_failure_envelope_value_non_quiet_adds_structured_error_fields() {
        let g = GlobalArgs {
            output_format: Some(OutputFormat::Json),
            porcelain: false,
            quiet: false,
            no_color: true,
            debug: false,
            verbose: false,
            runtime_root: None,
        };
        let v = build_failure_envelope_value(
            &g,
            "project_check_failed",
            "project check failed",
            json!({"config_dir":"/tmp/x","issues":["missing"]}),
            &["missing runtime".to_string()],
        );
        let err = &v["data"]["error"];
        assert_eq!(err["code"], "project_check_failed");
        assert_eq!(err["kind"], "validation");
        assert_eq!(err["diagnostics_len"], 1);
        assert!(err["source_chain"].is_array());
    }

    #[test]
    fn error_kind_token_maps_common_failure_codes() {
        assert_eq!(error_kind_token("project_check_failed"), "validation");
        assert_eq!(error_kind_token("child_exit"), "runtime");
        assert_eq!(error_kind_token("diagnostics_export_failed"), "io");
        assert_eq!(error_kind_token("mirror"), "network");
        assert_eq!(error_kind_token("unknown_new_code"), "unknown");
    }

    #[test]
    fn mirror_network_hint_some_for_download_and_mirror() {
        assert!(mirror_network_hint_message(&EnvrError::Download("x".into())).is_some());
        assert!(mirror_network_hint_message(&EnvrError::Mirror("m".into())).is_some());
    }

    #[test]
    fn mirror_network_hint_some_for_io_when_message_looks_network() {
        let e = std::io::Error::other("connection refused");
        assert!(mirror_network_hint_message(&EnvrError::from(e)).is_some());
    }

    #[test]
    fn mirror_network_hint_none_for_validation() {
        assert!(mirror_network_hint_message(&EnvrError::Validation("n".into())).is_none());
    }

    #[test]
    fn mirror_network_json_error_data_includes_hints_when_not_quiet() {
        let g = GlobalArgs {
            output_format: Some(OutputFormat::Json),
            porcelain: false,
            quiet: false,
            no_color: true,
            debug: false,
            verbose: false,
            runtime_root: None,
        };
        let v = mirror_network_json_error_data(&g, &EnvrError::Mirror("m".into()));
        let hints = v["hints"].as_array().expect("hints array");
        assert_eq!(hints.len(), 1);
        assert!(hints[0].as_str().is_some_and(|s| !s.is_empty()));
    }

    #[test]
    fn mirror_network_json_error_data_null_when_quiet_even_for_mirror() {
        let g = GlobalArgs {
            output_format: Some(OutputFormat::Json),
            porcelain: false,
            quiet: true,
            no_color: true,
            debug: false,
            verbose: false,
            runtime_root: None,
        };
        let v = mirror_network_json_error_data(&g, &EnvrError::Mirror("m".into()));
        assert!(v.is_null());
    }

    #[test]
    fn exit_code_policy_table_is_stable() {
        assert_eq!(exit_code_for_error_code(ErrorCode::Unknown), 1);
        assert_eq!(exit_code_for_error_code(ErrorCode::Config), 1);
        assert_eq!(exit_code_for_error_code(ErrorCode::Validation), 1);
        assert_eq!(exit_code_for_error_code(ErrorCode::Runtime), 1);
        assert_eq!(exit_code_for_error_code(ErrorCode::Platform), 1);
        assert_eq!(exit_code_for_error_code(ErrorCode::Io), 2);
        assert_eq!(exit_code_for_error_code(ErrorCode::Download), 2);
        assert_eq!(exit_code_for_error_code(ErrorCode::Mirror), 2);
    }

    #[test]
    fn metrics_error_code_for_exit_uses_stable_fallback() {
        assert_eq!(metrics_error_code_for_exit(0), None);
        assert_eq!(metrics_error_code_for_exit(1), Some("nonzero_exit"));
        assert_eq!(metrics_error_code_for_exit(42), Some("nonzero_exit"));
    }

    #[test]
    fn emit_validation_returns_cli_exit_with_validation_code() {
        let g = GlobalArgs {
            output_format: Some(OutputFormat::Text),
            porcelain: false,
            quiet: true,
            no_color: true,
            debug: false,
            verbose: false,
            runtime_root: None,
        };
        let exit = emit_validation(&g, "which", "envr which node");
        assert_eq!(
            exit,
            CliExit {
                exit_code: 1,
                error_code: Some(codes::err::VALIDATION),
            }
        );
    }

    #[test]
    fn emit_doctor_failure_returns_cli_exit_with_issue_code() {
        let g = GlobalArgs {
            output_format: Some(OutputFormat::Text),
            porcelain: false,
            quiet: true,
            no_color: true,
            debug: false,
            verbose: false,
            runtime_root: None,
        };
        let exit = emit_doctor(&g, false, codes::ok::DOCTOR_ISSUES, "x", Value::Null, || {});
        assert_eq!(
            exit,
            CliExit {
                exit_code: 1,
                error_code: Some(codes::ok::DOCTOR_ISSUES),
            }
        );
    }

    #[test]
    fn emit_doctor_json_failure_returns_same_cli_exit_code() {
        let g = GlobalArgs {
            output_format: Some(OutputFormat::Json),
            porcelain: false,
            quiet: true,
            no_color: true,
            debug: false,
            verbose: false,
            runtime_root: None,
        };
        let exit = emit_doctor(&g, false, codes::ok::DOCTOR_ISSUES, "x", Value::Null, || {});
        assert_eq!(
            exit,
            CliExit {
                exit_code: 1,
                error_code: Some(codes::ok::DOCTOR_ISSUES),
            }
        );
    }

    #[test]
    fn write_envelope_failure_returns_envelope_code() {
        assert_eq!(
            write_envelope(false, "envelope_fail", "m", Value::Null, &[]),
            Some("envelope_fail")
        );
    }

    #[test]
    fn with_next_steps_operator_keeps_all_steps() {
        let data = with_next_steps_for_persona(
            json!({ "ok": true }),
            vec![
                ("a", "A".to_string()),
                ("b", "B".to_string()),
                ("c", "C".to_string()),
            ],
            CliPersona::Operator,
        );
        let steps = data["next_steps"].as_array().expect("array");
        assert_eq!(steps.len(), 3);
    }

    #[test]
    fn with_next_steps_automation_is_truncated() {
        let data = with_next_steps_for_persona(
            json!({ "ok": true }),
            vec![
                ("a", "A".to_string()),
                ("b", "B".to_string()),
                ("c", "C".to_string()),
            ],
            CliPersona::Automation,
        );
        let steps = data["next_steps"].as_array().expect("array");
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0]["id"], "a");
    }

    #[test]
    fn with_next_steps_automation_prefers_verify_or_check_actions() {
        let data = with_next_steps_for_persona(
            json!({ "ok": true }),
            vec![
                ("run_doctor", "Run doctor".to_string()),
                ("verify_executable", "Verify executable".to_string()),
                ("set_current", "Set current".to_string()),
            ],
            CliPersona::Automation,
        );
        let steps = data["next_steps"].as_array().expect("array");
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0]["id"], "verify_executable");
    }

    #[test]
    fn with_next_steps_onboarding_prefers_fix_and_init_actions() {
        let data = with_next_steps_for_persona(
            json!({ "ok": true }),
            vec![
                ("check_status", "Check status".to_string()),
                ("fix_project_config", "Fix config".to_string()),
                ("init_project_config", "Init project".to_string()),
                ("verify_executable", "Verify executable".to_string()),
            ],
            CliPersona::Onboarding,
        );
        let steps = data["next_steps"].as_array().expect("array");
        let ids: Vec<&str> = steps
            .iter()
            .filter_map(|s| s.get("id").and_then(Value::as_str))
            .collect();
        assert_eq!(ids, vec!["fix_project_config", "init_project_config", "check_status"]);
    }

    proptest! {
        #[test]
        fn envelope_json_serializes_to_single_line(
            success in proptest::bool::ANY,
            msg in "[a-zA-Z0-9._ -]{0,40}"
        ) {
            let v = build_envelope_value(success, "ok", &msg, Value::Null, &[]);
            let line = serde_json::to_string(&v).expect("serde");
            prop_assert!(
                line.lines().count() == 1,
                "multi-line envelope: {line}"
            );
        }
    }
}
