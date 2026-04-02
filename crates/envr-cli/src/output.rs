//! Unified CLI output: JSON envelope (`success`, `code`, `message`, `data`, `diagnostics`) and exit codes.

use crate::cli::{GlobalArgs, OutputFormat};

use envr_error::{EnvrError, ErrorCode};
use serde_json::{Map, Value, json};

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

/// Per design doc: 1 = user/business, 2 = external (I/O, network fetch, mirror).
pub fn exit_code_for_error(err: &EnvrError) -> i32 {
    match err.code() {
        ErrorCode::Io | ErrorCode::Download | ErrorCode::Mirror => 2,
        _ => 1,
    }
}

/// Print one JSON line with the standard envelope (design doc §4).
pub fn write_envelope(
    success: bool,
    code: Option<&str>,
    message: &str,
    data: Value,
    diagnostics: &[String],
) {
    let mut m = Map::new();
    m.insert("success".into(), json!(success));
    m.insert("code".into(), code.map(|c| json!(c)).unwrap_or(Value::Null));
    m.insert("message".into(), json!(message));
    m.insert("data".into(), data);
    m.insert(
        "diagnostics".into(),
        serde_json::to_value(diagnostics).unwrap_or_else(|_| json!([])),
    );
    println!("{}", Value::Object(m));
}

pub fn emit_validation(g: &GlobalArgs, cmd: &str, example: &str) -> i32 {
    let tmpl = envr_core::i18n::tr_key(
        "cli.validation.missing_args",
        "`{cmd}` 缺少参数（示例：{example}）",
        "missing arguments for `{cmd}` (example: {example})",
    );
    let msg = fmt_template(&tmpl, &[("cmd", cmd), ("example", example)]);
    match g.output_format.unwrap_or(OutputFormat::Text) {
        OutputFormat::Json => {
            write_envelope(false, Some("validation"), &msg, Value::Null, &[]);
        }
        OutputFormat::Text => {
            eprintln!("envr: {msg}");
        }
    }
    1
}

pub fn emit_envr_error(g: &GlobalArgs, err: EnvrError) -> i32 {
    let code = error_code_token(err.code());
    let payload = err.to_payload();
    let diags = payload.chain;
    match g.output_format.unwrap_or(OutputFormat::Text) {
        OutputFormat::Json => {
            write_envelope(false, Some(code), &payload.message, Value::Null, &diags);
        }
        OutputFormat::Text => {
            eprintln!("envr: {err}");
        }
    }
    exit_code_for_error(&err)
}

pub fn emit_ok<F: FnOnce()>(g: &GlobalArgs, message: &str, data: Value, text: F) -> i32 {
    match g.output_format.unwrap_or(OutputFormat::Text) {
        OutputFormat::Json => {
            write_envelope(true, None, message, data, &[]);
        }
        OutputFormat::Text => {
            text();
        }
    }
    0
}

pub fn emit_doctor(
    g: &GlobalArgs,
    success: bool,
    message: &str,
    code_if_fail: Option<&str>,
    data: Value,
    text: impl FnOnce(),
) -> i32 {
    match g.output_format.unwrap_or(OutputFormat::Text) {
        OutputFormat::Json => {
            write_envelope(success, code_if_fail, message, data, &[]);
        }
        OutputFormat::Text => {
            text();
        }
    }
    if success { 0 } else { 1 }
}
