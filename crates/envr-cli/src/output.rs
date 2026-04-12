//! Unified CLI output: JSON envelope (`schema_version`, `success`, `code`, `message`, `data`, `diagnostics`) and exit codes.

use crate::cli::{GlobalArgs, OutputFormat};

use envr_error::{EnvrError, ErrorCode};
use serde_json::{Map, Value, json};

/// Top-level `schema_version` integer for every `--format json` envelope line.
/// Bump when making breaking changes to the envelope or documented `data` shapes.
/// JSON Schema sources for the envelope and selected `data` blobs: `schemas/cli/` (see integration tests).
pub const CLI_JSON_SCHEMA_VERSION: u32 = 2;

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

/// Unified stderr error line: always `envr:` prefix; `json_code` matches `--format json` envelope `code` when applicable.
pub fn print_error_text(json_code: &str, message: &str) {
    let tag = error_bracket_label(json_code);
    eprintln!("envr: {tag} {message}");
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
    let json_message = if g.quiet {
        error_bracket_label(code)
    } else {
        message.to_string()
    };
    let empty_diags: &[String] = &[];
    let eff_diags: &[String] = if g.quiet {
        empty_diags
    } else {
        diagnostics
    };
    match g.output_format.unwrap_or(OutputFormat::Text) {
        OutputFormat::Json => {
            write_envelope(
                false,
                Some(code),
                &json_message,
                data,
                eff_diags,
            );
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
            if g.quiet {
                let tag = error_bracket_label("validation");
                write_envelope(false, Some("validation"), &tag, Value::Null, &[]);
            } else {
                write_envelope(false, Some("validation"), &msg, Value::Null, &[]);
            }
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
    let mut line = envr_error_line_message(&err);
    let l = line.to_ascii_lowercase();
    let io_looks_network = matches!(err.code(), ErrorCode::Io)
        && (l.contains("http")
            || l.contains("tls")
            || l.contains("connection")
            || l.contains("timed out")
            || l.contains("timeout")
            || l.contains("dns")
            || l.contains("network"));
    let hint_mirror = matches!(err.code(), ErrorCode::Download | ErrorCode::Mirror) || io_looks_network;
    if !g.quiet
        && matches!(
            g.output_format.unwrap_or(OutputFormat::Text),
            OutputFormat::Text
        )
        && hint_mirror
    {
        line.push('\n');
        line.push_str(&envr_core::i18n::tr_key(
            "cli.hint.network_mirror",
            "若下载较慢或失败（例如在中国大陆），可尝试：envr config set mirror.mode auto",
            "If downloads are slow or fail (e.g. in mainland China), try: `envr config set mirror.mode auto`",
        ));
    }
    match g.output_format.unwrap_or(OutputFormat::Text) {
        OutputFormat::Json => {
            if g.quiet {
                let tag = error_bracket_label(code);
                write_envelope(false, Some(code), &tag, Value::Null, &[]);
            } else {
                write_envelope(false, Some(code), &payload.message, Value::Null, &diags);
            }
        }
        OutputFormat::Text => {
            if g.quiet {
                eprintln!("envr: {}", error_bracket_label(code));
            } else {
                print_error_text(code, &line);
            }
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

pub fn wants_porcelain(g: &GlobalArgs) -> bool {
    g.porcelain && !matches!(g.output_format.unwrap_or(OutputFormat::Text), OutputFormat::Json)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{GlobalArgs, OutputFormat};
    use envr_error::EnvrError;

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
}
