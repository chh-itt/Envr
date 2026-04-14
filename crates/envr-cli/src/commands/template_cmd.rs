use crate::cli::{GlobalArgs, OutputFormat, ProjectPathProfileArgs};
use crate::CliPathProfile;
use crate::commands::child_env;
use crate::commands::env_overrides;
use crate::output;

use envr_error::{EnvrError, EnvrResult};
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Replace `${VAR}` where `VAR` is `[A-Za-z_][A-Za-z0-9_]*`. Missing vars become empty strings.
fn render_template(input: &str, vars: &HashMap<String, String>) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(pos) = rest.find("${") {
        out.push_str(&rest[..pos]);
        rest = &rest[pos + 2..];
        let Some(end) = rest.find('}') else {
            out.push_str("${");
            break;
        };
        let name = &rest[..end];
        rest = &rest[end + 1..];
        let ok = !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_');
        if ok {
            out.push_str(vars.get(name).map(|s| s.as_str()).unwrap_or(""));
        } else {
            out.push_str("${");
            out.push_str(name);
            out.push('}');
        }
    }
    out.push_str(rest);
    out
}

/// Body for [`crate::commands::dispatch`]; errors are finished at the dispatch boundary.
pub(crate) fn run_inner(
    g: &GlobalArgs,
    file: PathBuf,
    project: ProjectPathProfileArgs,
    env_files: Vec<PathBuf>,
    env_pairs: Vec<String>,
) -> EnvrResult<i32> {
    let ProjectPathProfileArgs { path, profile } = project;
    let session = CliPathProfile::new(path, profile).load_project()?;
    let mut vars =
        child_env::collect_run_env_for_template(&session.ctx, session.project_config())?;
    env_overrides::apply_env_overrides(&mut vars, &env_files, &env_pairs)?;

    let raw = fs::read_to_string(&file).map_err(|e| {
        EnvrError::Io(std::io::Error::new(
            e.kind(),
            format!("{}: {e}", file.display()),
        ))
    })?;
    let rendered = render_template(&raw, &vars);
    let file_s = file.display().to_string();
    let data = json!({
        "file": file_s,
        "rendered": rendered,
    });

    match g.effective_output_format() {
        OutputFormat::Json => {
            output::write_envelope(true, None, "template_rendered", data, &[]);
            Ok(0)
        }
        OutputFormat::Text => {
            print!("{rendered}");
            Ok(0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subst_missing_empty() {
        let mut m = HashMap::new();
        m.insert("A".into(), "1".into());
        assert_eq!(render_template("x${A}${B}", &m), "x1");
    }
}
