use crate::CliExit;
use crate::CliUxPolicy;
use crate::cli::GlobalArgs;
use crate::output::{self, fmt_template};

use envr_config::project_config::{
    parse_project_config, parse_tool_versions_compat_str, render_tool_versions,
};
use envr_error::{EnvrError, EnvrResult};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};

const TOOL_VERSIONS_FILE: &str = ".tool-versions";

/// Body for [`crate::commands::dispatch`]; errors are finished at the dispatch boundary.
pub(crate) fn run_inner(
    g: &GlobalArgs,
    file: Option<PathBuf>,
    path: PathBuf,
    format: String,
) -> EnvrResult<CliExit> {
    let format = parse_format(&format, file.as_deref())?;
    let file = file.unwrap_or_else(|| default_file(format));
    let file = if file.is_absolute() { file } else { path.join(file) };
    if !file.is_file() {
        return Err(EnvrError::Validation(fmt_template(
            &envr_core::i18n::tr_key(
                "cli.err.not_a_file",
                "不是文件：{path}",
                "not a file: {path}",
            ),
            &[("path", &file.display().to_string())],
        )));
    }

    let data = match format {
        ImportExportFormat::EnvrToml => {
            let cfg = parse_project_config(&file)?;
            json!({
                "source": file.to_string_lossy(),
                "format": format.label(),
                "config": cfg,
            })
        }
        ImportExportFormat::ToolVersions => {
            let content = fs::read_to_string(&file)?;
            let (cfg, warnings) = parse_tool_versions_compat_str(&content)?;
            json!({
                "source": file.to_string_lossy(),
                "format": format.label(),
                "warnings": warnings,
                "config": cfg,
                "rendered_tool_versions": render_tool_versions(&cfg),
            })
        }
    };

    Ok(output::emit_ok(
        g,
        crate::codes::ok::CONFIG_IMPORTED,
        data,
        || {
            if CliUxPolicy::from_global(g).human_text_primary() {
                println!(
                    "{}",
                    fmt_template(
                        &envr_core::i18n::tr_key(
                            "cli.read.done",
                            "已读取 {path}",
                            "read {path}",
                        ),
                        &[("path", &file.display().to_string())],
                    )
                );
            }
        },
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImportExportFormat {
    EnvrToml,
    ToolVersions,
}

impl ImportExportFormat {
    fn label(self) -> &'static str {
        match self {
            ImportExportFormat::EnvrToml => "envr-toml",
            ImportExportFormat::ToolVersions => "tool-versions",
        }
    }
}

fn parse_format(raw: &str, file: Option<&Path>) -> EnvrResult<ImportExportFormat> {
    let trimmed = raw.trim();
    if trimmed.eq_ignore_ascii_case("auto") {
        if file
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .is_some_and(|name| name == TOOL_VERSIONS_FILE)
        {
            return Ok(ImportExportFormat::ToolVersions);
        }
        return Ok(ImportExportFormat::EnvrToml);
    }
    match trimmed {
        "envr-toml" | "toml" => Ok(ImportExportFormat::EnvrToml),
        "tool-versions" | "asdf" => Ok(ImportExportFormat::ToolVersions),
        other => Err(EnvrError::Validation(format!(
            "unsupported read format `{other}`; expected `envr-toml` or `tool-versions`"
        ))),
    }
}

fn default_file(format: ImportExportFormat) -> PathBuf {
    match format {
        ImportExportFormat::EnvrToml => PathBuf::from(".envr.toml"),
        ImportExportFormat::ToolVersions => PathBuf::from(TOOL_VERSIONS_FILE),
    }
}
