use crate::cli::{GlobalArgs, OutputFormat};
use crate::commands::child_env;
use crate::commands::common;
use crate::commands::env_overrides;
use crate::output;

use envr_error::EnvrError;
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

pub fn run(
    g: &GlobalArgs,
    file: PathBuf,
    path: PathBuf,
    profile: Option<String>,
    env_files: Vec<PathBuf>,
    env_pairs: Vec<String>,
) -> i32 {
    let ctx = match common::shim_context_for(path, profile) {
        Ok(c) => c,
        Err(e) => return common::print_envr_error(g, e),
    };

    let mut vars = match child_env::collect_run_env(&ctx, false) {
        Ok(m) => m,
        Err(e) => return common::print_envr_error(g, e),
    };
    match child_env::template_extension_vars(&ctx) {
        Ok(ext) => {
            for (k, v) in ext {
                vars.insert(k, v);
            }
        }
        Err(e) => return common::print_envr_error(g, e),
    }
    if let Err(e) = env_overrides::apply_env_overrides(&mut vars, &env_files, &env_pairs) {
        return common::print_envr_error(g, e);
    }

    let raw = match fs::read_to_string(&file) {
        Ok(s) => s,
        Err(e) => {
            return common::print_envr_error(
                g,
                EnvrError::Io(std::io::Error::new(
                    e.kind(),
                    format!("{}: {e}", file.display()),
                )),
            );
        }
    };
    let rendered = render_template(&raw, &vars);
    let file_s = file.display().to_string();
    let data = json!({
        "file": file_s,
        "rendered": rendered,
    });

    match g.output_format.unwrap_or(OutputFormat::Text) {
        OutputFormat::Json => {
            output::write_envelope(true, None, "template_rendered", data, &[]);
        }
        OutputFormat::Text => {
            print!("{rendered}");
        }
    }
    0
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
