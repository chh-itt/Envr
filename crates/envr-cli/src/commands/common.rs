use crate::cli::{GlobalArgs, OutputFormat};

use envr_core::runtime::service::RuntimeService;
use envr_domain::runtime::RuntimeKind;
use envr_error::EnvrError;
use std::path::PathBuf;

pub fn kind_label(kind: RuntimeKind) -> &'static str {
    match kind {
        RuntimeKind::Node => "node",
        RuntimeKind::Python => "python",
        RuntimeKind::Java => "java",
    }
}

pub fn runtime_service() -> Result<RuntimeService, EnvrError> {
    let root = runtime_root()?;
    RuntimeService::with_runtime_root(root)
}

fn runtime_root() -> Result<PathBuf, EnvrError> {
    if let Ok(p) = std::env::var("ENVR_RUNTIME_ROOT")
        && !p.is_empty()
    {
        return Ok(PathBuf::from(p));
    }
    Ok(envr_platform::paths::current_platform_paths()?.runtime_root)
}

pub fn print_envr_error(g: &GlobalArgs, err: EnvrError) -> i32 {
    match g.output_format.unwrap_or(OutputFormat::Text) {
        OutputFormat::Json => {
            let p = err.to_payload();
            println!(
                "{}",
                serde_json::json!({
                    "success": false,
                    "code": p.code,
                    "message": p.message,
                    "data": serde_json::Value::Null,
                    "diagnostics": p.chain,
                })
            );
        }
        OutputFormat::Text => {
            eprintln!("envr: {err}");
        }
    }
    1
}

pub fn missing_positional(g: &GlobalArgs, cmd: &str, example: &str) -> i32 {
    match g.output_format.unwrap_or(OutputFormat::Text) {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::json!({
                    "success": false,
                    "code": "validation",
                    "message": format!("missing arguments for `{cmd}` (example: {example})"),
                    "data": serde_json::Value::Null,
                    "diagnostics": [],
                })
            );
        }
        OutputFormat::Text => {
            eprintln!("envr: missing arguments for `{cmd}` (example: {example})");
        }
    }
    1
}
