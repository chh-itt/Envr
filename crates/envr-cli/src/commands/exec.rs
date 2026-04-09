use crate::cli::{GlobalArgs, OutputFormat};
use crate::commands::child_env;
use crate::commands::common;
use crate::output::{self, fmt_template};

use envr_domain::runtime::parse_runtime_kind;
use envr_shim_core::ShimContext;
use serde_json::json;
use std::path::PathBuf;
use std::process::Command;

fn go_tool_executable(home: &std::path::Path, tool: &str) -> Option<std::path::PathBuf> {
    let bin = home.join("bin");
    #[cfg(windows)]
    {
        match tool {
            "go" => Some(bin.join("go.exe")),
            "gofmt" => Some(bin.join("gofmt.exe")),
            _ => None,
        }
    }
    #[cfg(not(windows))]
    {
        match tool {
            "go" => Some(bin.join("go")),
            "gofmt" => Some(bin.join("gofmt")),
            _ => None,
        }
    }
}

pub fn run(
    g: &GlobalArgs,
    lang: String,
    spec: Option<String>,
    path: PathBuf,
    profile: Option<String>,
    command: String,
    args: Vec<String>,
) -> i32 {
    let lang = lang.trim().to_ascii_lowercase();
    if let Err(e) = parse_runtime_kind(&lang) {
        return common::print_envr_error(g, e);
    }

    let mut ctx = match ShimContext::from_process_env() {
        Ok(c) => c,
        Err(e) => return common::print_envr_error(g, e),
    };
    ctx.working_dir = std::fs::canonicalize(&path).unwrap_or(path);
    if let Some(p) = profile.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        ctx.profile = Some(p.to_string());
    }

    let env_map = match child_env::collect_exec_env(&ctx, &lang, spec.as_deref()) {
        Ok(m) => m,
        Err(e) => return common::print_envr_error(g, e),
    };

    // On Windows, executable lookup may happen before applying the child's environment block
    // (including PATH). Prefer an absolute core tool path when we can derive it from the runtime home.
    let mut resolved_cmd = command.clone();
    if lang == "go" && (command == "go" || command == "gofmt") {
        if let Ok(home) = child_env::resolve_exec_home_for_lang(&ctx, &lang, spec.as_deref()) {
            let home = std::fs::canonicalize(&home).unwrap_or(home);
            if let Some(p) = go_tool_executable(&home, &command) {
                resolved_cmd = p.display().to_string();
            }
        }
    }

    let mut child = Command::new(&resolved_cmd);
    child.args(&args);
    child.env_clear();
    for (k, v) in &env_map {
        child.env(k, v);
    }
    child.current_dir(&ctx.working_dir);

    let status = match child.status() {
        Ok(s) => s,
        Err(e) => return common::print_envr_error(g, e.into()),
    };
    let exit = status.code().unwrap_or(1);
    let data = json!({
        "exit_code": exit,
        "command": command,
        "args": args,
    });
    if exit == 0 {
        output::emit_ok(g, "child_completed", data, || {})
    } else {
        let msg = fmt_template(
            &envr_core::i18n::tr_key(
                "cli.child.exit_nonzero",
                "子进程退出，代码 {exit}",
                "child process exited with code {exit}",
            ),
            &[("exit", &exit.to_string())],
        );
        match g.output_format.unwrap_or(OutputFormat::Text) {
            OutputFormat::Json => {
                output::write_envelope(false, Some("child_exit"), &msg, data, &[]);
            }
            OutputFormat::Text => {
                eprintln!("envr: {msg}");
            }
        }
        exit
    }
}
