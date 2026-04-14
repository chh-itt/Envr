//! Unified CLI output: JSON envelope (`schema_version`, `success`, `code`, `message`, `data`, `diagnostics`) and exit codes.
//! Envelope vs per-`message` `data` versioning: `docs/cli/output-contract.md` and `docs/schemas/README.md`.

use crate::cli::{GlobalArgs, OutputFormat};

use envr_error::{EnvrError, ErrorCode};
use serde_json::{Map, Value, json};
use std::collections::HashMap;
use std::cell::RefCell;
use std::sync::OnceLock;
thread_local! {
    static RECORDED_FAILURE_CODE: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Top-level `schema_version` integer for every `--format json` envelope line.
/// Bump when making breaking changes to the envelope or documented `data` shapes.
/// JSON Schema sources for the envelope and selected `data` blobs: `schemas/cli/` (see integration tests).
pub const CLI_JSON_SCHEMA_VERSION: u32 = 2;
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

/// Stores the last stable JSON failure `code` on this thread for [`crate::CommandOutcome::from_exit_code`].
#[inline]
pub(crate) fn record_failure_code_for_metrics(code: &str) {
    RECORDED_FAILURE_CODE.with(|slot| {
        *slot.borrow_mut() = Some(code.to_string());
    });
}

#[inline]
pub(crate) fn take_recorded_failure_code_for_exit(exit_code: i32) -> Option<String> {
    RECORDED_FAILURE_CODE.with(|slot| {
        let mut slot = slot.borrow_mut();
        if exit_code == 0 {
            *slot = None;
            return None;
        }
        slot.take()
    })
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

fn trim_failure_for_quiet_json(g: &GlobalArgs, code: &str, message: &str, data: Value, diagnostics: &[String]) -> (String, Value, Vec<String>) {
    if !g.quiet || !matches!(g.effective_output_format(), OutputFormat::Json) {
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
    let data = if g.quiet {
        data
    } else {
        add_error_object_if_possible(code, message, data, diagnostics)
    };
    let (msg, data, diags) = trim_failure_for_quiet_json(g, code, message, data, diagnostics);
    build_envelope_value(false, Some(code), &msg, data, &diags)
}

/// Failure with a stable string `code` (child exit, project checks, etc.): same envelope shape as other CLI errors.
/// With `--quiet`, `message` is replaced by `[E_CODE]` and `diagnostics` are omitted from JSON.
pub fn emit_failure_envelope(
    g: &GlobalArgs,
    code: &str,
    message: &str,
    data: Value,
    diagnostics: &[String],
    exit_code: i32,
) -> i32 {
    record_failure_code_for_metrics(code);
    match g.effective_output_format() {
        OutputFormat::Json => {
            let v = build_failure_envelope_value(g, code, message, data, diagnostics);
            println!("{}", serde_json::to_string(&v).unwrap_or_else(|_| "{}".to_string()));
        }
        OutputFormat::Text => {
            if g.quiet {
                eprintln!("envr: {}", error_bracket_label(code));
            } else {
                print_error_text(code, message);
            }
        }
    }
    exit_code
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
    if g.quiet {
        return Value::Null;
    }
    mirror_network_hint_message(err)
        .map(|h| json!({ "hints": [h] }))
        .unwrap_or(Value::Null)
}

/// Build the JSON object emitted by [`write_envelope`] (compact `serde_json` is one line).
pub(crate) fn build_envelope_value(
    success: bool,
    code: Option<&str>,
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
    m.insert("code".into(), code.map(|c| json!(c)).unwrap_or(Value::Null));
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
/// When `success` is false and `code` is [`Some`], registers that code for
/// [`take_recorded_failure_code_for_exit`] so dispatch/finish metrics match the envelope.
pub fn write_envelope(
    success: bool,
    code: Option<&str>,
    message: &str,
    data: Value,
    diagnostics: &[String],
) {
    if !success && let Some(c) = code {
        record_failure_code_for_metrics(c);
    }
    println!(
        "{}",
        build_envelope_value(success, code, message, data, diagnostics)
    );
}

pub fn emit_validation(g: &GlobalArgs, cmd: &str, example: &str) -> i32 {
    record_failure_code_for_metrics("validation");
    let tmpl = envr_core::i18n::tr_key(
        "cli.validation.missing_args",
        "`{cmd}` 缺少参数（示例：{example}）",
        "missing arguments for `{cmd}` (example: {example})",
    );
    let msg = fmt_template(&tmpl, &[("cmd", cmd), ("example", example)]);
    match g.effective_output_format() {
        OutputFormat::Json => {
            let v = build_failure_envelope_value(g, "validation", &msg, Value::Null, &[]);
            println!("{}", serde_json::to_string(&v).unwrap_or_else(|_| "{}".to_string()));
        }
        OutputFormat::Text => {
            if g.quiet {
                eprintln!("envr: {}", error_bracket_label("validation"));
            } else {
                print_error_text("validation", &msg);
            }
        }
    }
    1
}

pub fn emit_envr_error(g: &GlobalArgs, err: EnvrError) -> i32 {
    let code = error_code_token(err.code());
    let payload = err.to_payload();
    let diags = payload.chain;
    let exit = exit_code_for_error(&err);
    tracing::error!(
        target: "envr_cli",
        cli_error_kind = %code,
        cli_error_exit_code = exit,
        cli_error_diagnostics_len = diags.len(),
        "cli error"
    );
    let mut line = envr_error_line_message(&err);
    if !g.quiet
        && matches!(
            g.effective_output_format(),
            OutputFormat::Text
        )
        && let Some(ref h) = mirror_network_hint_message(&err) {
            line.push('\n');
            line.push_str(h);
        }
    let json_error_data = mirror_network_json_error_data(g, &err);
    match g.effective_output_format() {
        OutputFormat::Json => {
            let v = build_failure_envelope_value(g, code, &payload.message, json_error_data, &diags);
            println!("{}", serde_json::to_string(&v).unwrap_or_else(|_| "{}".to_string()));
        }
        OutputFormat::Text => {
            if g.quiet {
                eprintln!("envr: {}", error_bracket_label(code));
            } else {
                print_error_text(code, &line);
            }
        }
    }
    exit
}

pub fn emit_ok<F: FnOnce()>(g: &GlobalArgs, message: &str, data: Value, text: F) -> i32 {
    match g.effective_output_format() {
        OutputFormat::Json => {
            write_envelope(true, None, message, data, &[]);
        }
        OutputFormat::Text => {
            text();
        }
    }
    0
}

pub fn wants_porcelain(g: &GlobalArgs) -> bool {
    g.porcelain && !matches!(g.effective_output_format(), OutputFormat::Json)
}

/// Whether stdout may use ANSI styles (honours `--no-color` and a tty).
pub fn use_terminal_styles(g: &GlobalArgs) -> bool {
    use std::io::{self, IsTerminal};
    !g.no_color && io::stdout().is_terminal()
}

pub fn emit_doctor(
    g: &GlobalArgs,
    success: bool,
    message: &str,
    code_if_fail: Option<&str>,
    data: Value,
    text: impl FnOnce(),
) -> i32 {
    match g.effective_output_format() {
        OutputFormat::Json => {
            write_envelope(success, code_if_fail, message, data, &[]);
        }
        OutputFormat::Text => {
            text();
            if !success && let Some(code) = code_if_fail {
                record_failure_code_for_metrics(code);
            }
        }
    }
    if success { 0 } else { 1 }
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
    fn recorded_failure_code_is_taken_for_nonzero_exit_and_cleared() {
        record_failure_code_for_metrics("child_exit");
        assert_eq!(take_recorded_failure_code_for_exit(1), Some("child_exit".to_string()));
        assert_eq!(take_recorded_failure_code_for_exit(1), None);
    }

    #[test]
    fn emit_validation_records_code_for_metrics() {
        let g = GlobalArgs {
            output_format: Some(OutputFormat::Text),
            porcelain: false,
            quiet: true,
            no_color: true,
            debug: false,
            runtime_root: None,
        };
        let exit = emit_validation(&g, "which", "envr which node");
        assert_eq!(exit, 1);
        assert_eq!(
            take_recorded_failure_code_for_exit(1),
            Some("validation".to_string())
        );
    }

    #[test]
    fn emit_doctor_failure_records_code_for_metrics() {
        let g = GlobalArgs {
            output_format: Some(OutputFormat::Text),
            porcelain: false,
            quiet: true,
            no_color: true,
            debug: false,
            runtime_root: None,
        };
        let exit = emit_doctor(&g, false, "x", Some("doctor_issues"), Value::Null, || {});
        assert_eq!(exit, 1);
        assert_eq!(
            take_recorded_failure_code_for_exit(1),
            Some("doctor_issues".to_string())
        );
    }

    #[test]
    fn emit_doctor_json_failure_records_code_via_write_envelope() {
        let g = GlobalArgs {
            output_format: Some(OutputFormat::Json),
            porcelain: false,
            quiet: true,
            no_color: true,
            debug: false,
            runtime_root: None,
        };
        let exit = emit_doctor(&g, false, "x", Some("doctor_issues"), Value::Null, || {});
        assert_eq!(exit, 1);
        assert_eq!(
            take_recorded_failure_code_for_exit(1),
            Some("doctor_issues".to_string())
        );
    }

    #[test]
    fn write_envelope_failure_with_code_records_for_metrics() {
        write_envelope(false, Some("envelope_fail"), "m", Value::Null, &[]);
        assert_eq!(
            take_recorded_failure_code_for_exit(1),
            Some("envelope_fail".to_string())
        );
    }

    proptest! {
        #[test]
        fn envelope_json_serializes_to_single_line(
            success in proptest::bool::ANY,
            msg in "[a-zA-Z0-9._ -]{0,40}"
        ) {
            let v = build_envelope_value(success, None, &msg, Value::Null, &[]);
            let line = serde_json::to_string(&v).expect("serde");
            prop_assert!(
                line.lines().count() == 1,
                "multi-line envelope: {line}"
            );
        }
    }
}
