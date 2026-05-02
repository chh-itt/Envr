use crate::CliExit;
use crate::CliUxPolicy;
use crate::cli::GlobalArgs;
use crate::output::{self, fmt_template};

use envr_config::project_config::{
    PROJECT_CONFIG_FILE, ProjectConfig, load_project_config_disk_only, parse_project_config,
    parse_tool_versions_compat_str, render_tool_versions, save_project_config,
};
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};

const TOOL_VERSIONS_FILE: &str = ".tool-versions";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImportExportFormat {
    EnvrToml,
    ToolVersions,
}

/// Body for [`crate::commands::dispatch`]; errors are finished at the dispatch boundary.
pub(crate) fn import_run_inner(
    g: &GlobalArgs,
    file: Option<PathBuf>,
    path: PathBuf,
    format: String,
    dry_run: bool,
) -> EnvrResult<CliExit> {
    let format = parse_import_format(&format, file.as_deref())?;
    let file = file.unwrap_or_else(|| default_import_file(format));
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

    let dest = path.join(PROJECT_CONFIG_FILE);
    let mut merged = if dest.is_file() {
        parse_project_config(&dest)?
    } else {
        ProjectConfig::default()
    };
    let (imported, import_warnings) = match format {
        ImportExportFormat::EnvrToml => (parse_project_config(&file)?, Vec::new()),
        ImportExportFormat::ToolVersions => parse_tool_versions_file(&file)?,
    };
    merged.runtimes.extend(imported.runtimes);
    merged.compat.asdf.names.extend(imported.compat.asdf.names);
    merged.env.extend(imported.env);
    merged.scripts.extend(imported.scripts);
    merged.profiles.extend(imported.profiles);

    let rendered = toml::to_string_pretty(&merged).map_err(|e| {
        EnvrError::with_source(ErrorCode::Config, "serialize project config toml", e)
    })?;

    if !dry_run {
        save_project_config(&dest, &merged)?;
    }

    let data = json!({
        "dest": dest.to_string_lossy(),
        "source": file.to_string_lossy(),
        "format": format.label(),
        "dry_run": dry_run,
        "warnings": import_warnings,
        "toml": rendered,
    });
    Ok(output::emit_ok(
        g,
        crate::codes::ok::CONFIG_IMPORTED,
        data,
        || {
            if CliUxPolicy::from_global(g).human_text_primary() {
                if dry_run {
                    print!("{rendered}");
                    if !rendered.ends_with('\n') {
                        println!();
                    }
                } else {
                    println!(
                        "{}",
                        fmt_template(
                            &envr_core::i18n::tr_key(
                                "cli.import.merged",
                                "已合并到 {path}",
                                "merged into {path}",
                            ),
                            &[("path", &dest.display().to_string())],
                        )
                    );
                    for warning in &import_warnings {
                        println!("{}", warning);
                    }
                }
            }
        },
    ))
}

/// Body for [`crate::commands::dispatch`]; errors are finished at the dispatch boundary.
pub(crate) fn export_run_inner(
    g: &GlobalArgs,
    path: PathBuf,
    output: Option<PathBuf>,
    format: String,
) -> EnvrResult<CliExit> {
    let format = parse_export_format(&format)?;
    let loaded = load_project_config_disk_only(&path)?;
    let Some((cfg, loc)) = loaded else {
        return Err(EnvrError::Validation(fmt_template(
            &envr_core::i18n::tr_key(
                "cli.err.no_project_config",
                "自 {path} 向上未找到 `.envr.toml` 或 `.envr.local.toml`",
                "no `.envr.toml` or `.envr.local.toml` found searching upward from {path}",
            ),
            &[("path", &path.display().to_string())],
        )));
    };

    let rendered = match format {
        ImportExportFormat::EnvrToml => toml::to_string_pretty(&cfg).map_err(|e| {
            EnvrError::with_source(ErrorCode::Config, "serialize project config toml", e)
        })?,
        ImportExportFormat::ToolVersions => render_tool_versions(&cfg),
    };

    if let Some(out_path) = output {
        fs::write(&out_path, &rendered)?;
        let data = json!({
            "config_dir": loc.dir.to_string_lossy(),
            "written": out_path.to_string_lossy(),
            "format": format.label(),
            "toml": rendered,
        });
        Ok(output::emit_ok(
            g,
            crate::codes::ok::CONFIG_EXPORTED,
            data,
            || {
                if CliUxPolicy::from_global(g).human_text_primary() {
                    println!(
                        "{}",
                        fmt_template(
                            &envr_core::i18n::tr_key(
                                "cli.export.wrote",
                                "已写入 {path}",
                                "wrote {path}",
                            ),
                            &[("path", &out_path.display().to_string())],
                        )
                    );
                }
            },
        ))
    } else {
        let data = json!({
            "config_dir": loc.dir.to_string_lossy(),
            "format": format.label(),
            "toml": rendered,
        });
        Ok(output::emit_ok(
            g,
            crate::codes::ok::CONFIG_EXPORTED,
            data,
            || {
                if CliUxPolicy::from_global(g).human_text_primary() {
                    print!("{rendered}");
                    if !rendered.ends_with('\n') {
                        println!();
                    }
                }
            },
        ))
    }
}

impl ImportExportFormat {
    fn label(self) -> &'static str {
        match self {
            ImportExportFormat::EnvrToml => "envr-toml",
            ImportExportFormat::ToolVersions => "tool-versions",
        }
    }
}

fn parse_import_format(raw: &str, file: Option<&Path>) -> EnvrResult<ImportExportFormat> {
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
    parse_named_format(trimmed)
}

fn parse_export_format(raw: &str) -> EnvrResult<ImportExportFormat> {
    parse_named_format(raw.trim())
}

fn parse_named_format(raw: &str) -> EnvrResult<ImportExportFormat> {
    match raw {
        "envr-toml" | "toml" => Ok(ImportExportFormat::EnvrToml),
        "tool-versions" | "asdf" => Ok(ImportExportFormat::ToolVersions),
        other => Err(EnvrError::Validation(format!(
            "unsupported import/export format `{other}`; expected `envr-toml` or `tool-versions`"
        ))),
    }
}

fn default_import_file(format: ImportExportFormat) -> PathBuf {
    match format {
        ImportExportFormat::EnvrToml => PathBuf::from(PROJECT_CONFIG_FILE),
        ImportExportFormat::ToolVersions => PathBuf::from(TOOL_VERSIONS_FILE),
    }
}

fn parse_tool_versions_file(path: &Path) -> EnvrResult<(ProjectConfig, Vec<String>)> {
    let content = fs::read_to_string(path)?;
    parse_tool_versions_str(&content)
}

fn parse_tool_versions_str(content: &str) -> EnvrResult<(ProjectConfig, Vec<String>)> {
    parse_tool_versions_compat_str(content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use envr_config::project_config::RuntimeConfig;

    #[test]
    fn parses_tool_versions_with_name_mapping() {
        let cfg = parse_tool_versions_str(
            r#"
# comment
nodejs 22.11.0
python 3.12.7 # inline comment
golang 1.23.2
dotnet-core 8.0.100
java temurin-21.0.4+7
ruby 3.3.5
terraform 1.9.8
"#,
        )
        .expect("parse");
        assert!(cfg.1.is_empty());
        let cfg = cfg.0;

        assert_eq!(
            cfg.runtimes.get("node").and_then(|r| r.version.as_deref()),
            Some("22.11.0")
        );
        assert_eq!(
            cfg.runtimes.get("go").and_then(|r| r.version.as_deref()),
            Some("1.23.2")
        );
        assert_eq!(
            cfg.runtimes
                .get("dotnet")
                .and_then(|r| r.version.as_deref()),
            Some("8.0.100")
        );
        assert_eq!(
            cfg.runtimes
                .get("python")
                .and_then(|r| r.version.as_deref()),
            Some("3.12.7")
        );
        assert_eq!(
            cfg.runtimes.get("java").and_then(|r| r.version.as_deref()),
            Some("temurin-21.0.4+7")
        );
        assert_eq!(
            cfg.runtimes.get("ruby").and_then(|r| r.version.as_deref()),
            Some("3.3.5")
        );
        assert_eq!(
            cfg.runtimes
                .get("terraform")
                .and_then(|r| r.version.as_deref()),
            Some("1.9.8")
        );
    }

    #[test]
    fn renders_tool_versions_with_asdf_names() {
        let mut cfg = ProjectConfig::default();
        cfg.runtimes.insert(
            "node".into(),
            RuntimeConfig {
                version: Some("22.11.0".into()),
                ..RuntimeConfig::default()
            },
        );
        cfg.runtimes.insert(
            "go".into(),
            RuntimeConfig {
                version: Some("1.23.2".into()),
                ..RuntimeConfig::default()
            },
        );
        cfg.runtimes.insert(
            "java".into(),
            RuntimeConfig {
                version: Some("temurin-21.0.4+7".into()),
                ..RuntimeConfig::default()
            },
        );
        cfg.runtimes.insert(
            "custom-runtime".into(),
            RuntimeConfig {
                version: Some("latest".into()),
                ..RuntimeConfig::default()
            },
        );

        let rendered = render_tool_versions(&cfg);
        assert!(rendered.contains("nodejs 22.11.0\n"));
        assert!(rendered.contains("golang 1.23.2\n"));
        assert!(rendered.contains("java temurin-21.0.4+7\n"));
        assert!(rendered.contains("custom-runtime latest\n"));
    }

    #[test]
    fn import_export_roundtrip_preserves_common_aliases() {
        let cfg = parse_tool_versions_str(
            r#"
nodejs 22.11.0
golang 1.23.2
java temurin-21.0.4+7
ruby 3.3.5
terraform 1.9.8
"#,
        )
        .expect("parse");
        assert!(cfg.1.is_empty());
        let cfg = cfg.0;

        let rendered = render_tool_versions(&cfg);
        assert!(rendered.contains("nodejs 22.11.0\n"));
        assert!(rendered.contains("golang 1.23.2\n"));
        assert!(rendered.contains("java temurin-21.0.4+7\n"));
        assert!(rendered.contains("ruby 3.3.5\n"));
        assert!(rendered.contains("terraform 1.9.8\n"));
    }

    #[test]
    fn import_export_roundtrip_supports_more_aliases() {
        let cfg = parse_tool_versions_str(
            r#"
rust 1.78.0
deno 2.0.0
bun 1.1.30
php 8.3.12
elixir 1.17.3
erlang 27.1
kotlin 2.0.21
scala 3.5.1
clojure 1.12.0
groovy 4.0.23
dart 3.5.4
flutter 3.24.3
"#,
        )
        .expect("parse");
        assert!(cfg.1.is_empty());
        let cfg = cfg.0;

        assert_eq!(
            cfg.runtimes.get("rust").and_then(|r| r.version.as_deref()),
            Some("1.78.0")
        );
        assert_eq!(
            cfg.runtimes.get("deno").and_then(|r| r.version.as_deref()),
            Some("2.0.0")
        );
        assert_eq!(
            cfg.runtimes.get("bun").and_then(|r| r.version.as_deref()),
            Some("1.1.30")
        );
        assert_eq!(
            cfg.runtimes.get("php").and_then(|r| r.version.as_deref()),
            Some("8.3.12")
        );
        assert_eq!(
            cfg.runtimes
                .get("elixir")
                .and_then(|r| r.version.as_deref()),
            Some("1.17.3")
        );
        assert_eq!(
            cfg.runtimes
                .get("erlang")
                .and_then(|r| r.version.as_deref()),
            Some("27.1")
        );
        assert_eq!(
            cfg.runtimes
                .get("kotlin")
                .and_then(|r| r.version.as_deref()),
            Some("2.0.21")
        );
        assert_eq!(
            cfg.runtimes.get("scala").and_then(|r| r.version.as_deref()),
            Some("3.5.1")
        );
        assert_eq!(
            cfg.runtimes
                .get("clojure")
                .and_then(|r| r.version.as_deref()),
            Some("1.12.0")
        );
        assert_eq!(
            cfg.runtimes
                .get("groovy")
                .and_then(|r| r.version.as_deref()),
            Some("4.0.23")
        );
        assert_eq!(
            cfg.runtimes.get("dart").and_then(|r| r.version.as_deref()),
            Some("3.5.4")
        );
        assert_eq!(
            cfg.runtimes
                .get("flutter")
                .and_then(|r| r.version.as_deref()),
            Some("3.24.3")
        );

        let rendered = render_tool_versions(&cfg);
        assert!(rendered.contains("rust 1.78.0\n"));
        assert!(rendered.contains("deno 2.0.0\n"));
        assert!(rendered.contains("bun 1.1.30\n"));
        assert!(rendered.contains("php 8.3.12\n"));
        assert!(rendered.contains("elixir 1.17.3\n"));
        assert!(rendered.contains("erlang 27.1\n"));
        assert!(rendered.contains("kotlin 2.0.21\n"));
        assert!(rendered.contains("scala 3.5.1\n"));
        assert!(rendered.contains("clojure 1.12.0\n"));
        assert!(rendered.contains("groovy 4.0.23\n"));
        assert!(rendered.contains("dart 3.5.4\n"));
        assert!(rendered.contains("flutter 3.24.3\n"));
    }
}
