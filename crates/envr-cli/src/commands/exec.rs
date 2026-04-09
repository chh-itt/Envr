use crate::cli::{GlobalArgs, OutputFormat};
use crate::commands::child_env;
use crate::commands::common;
use crate::output::{self, fmt_template};

use envr_config::project_config::{RustEnforceMode, load_project_config_profile};
use envr_domain::runtime::parse_runtime_kind;
use envr_shim_core::ShimContext;
use serde_json::json;
use std::path::PathBuf;
use std::process::Command;

fn parse_rust_channel_from_toolchain(toolchain: &str) -> Option<String> {
    let t = toolchain.trim();
    if t.is_empty() {
        return None;
    }
    let first = t.split_whitespace().next().unwrap_or("").trim();
    let chan = first
        .split('-')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    match chan.as_str() {
        "stable" | "beta" | "nightly" => Some(chan),
        _ => None,
    }
}

fn rustc_version_from_output(out: &str) -> Option<String> {
    let s = out.trim();
    let mut it = s.split_whitespace();
    let _ = it.next()?; // "rustc"
    let v = it.next()?.trim();
    if v.is_empty() {
        None
    } else {
        Some(v.to_string())
    }
}

fn enforce_rust_constraints(
    env_map: &std::collections::HashMap<String, String>,
    cfg: &envr_config::project_config::ProjectConfig,
    working_dir: &std::path::Path,
) -> Result<(), envr_error::EnvrError> {
    let Some(r) = cfg.runtimes.get("rust") else {
        return Ok(());
    };
    let want_channel = r
        .channel
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let want_prefix = r
        .version_prefix
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    if want_channel.is_none() && want_prefix.is_none() {
        return Ok(());
    }
    let mode = r.enforce.unwrap_or(RustEnforceMode::Warn);

    let current_channel = (|| -> Option<String> {
        let o = Command::new("rustup")
            .args(["show", "active-toolchain"])
            .env_clear()
            .envs(env_map)
            .current_dir(working_dir)
            .output()
            .ok()?;
        if !o.status.success() {
            return None;
        }
        let s = String::from_utf8_lossy(&o.stdout);
        parse_rust_channel_from_toolchain(&s)
    })();

    let current_rustc = (|| -> Option<String> {
        let o = Command::new("rustc")
            .arg("-V")
            .env_clear()
            .envs(env_map)
            .current_dir(working_dir)
            .output()
            .ok()?;
        if !o.status.success() {
            return None;
        }
        let s = String::from_utf8_lossy(&o.stdout);
        rustc_version_from_output(&s)
    })();

    let mut problems = Vec::new();
    if let Some(want) = want_channel {
        if current_channel.as_deref() != Some(&want.to_ascii_lowercase()) {
            problems.push(format!(
                "rust channel mismatch: want {want}, got {}",
                current_channel.as_deref().unwrap_or("(unknown)")
            ));
        }
    }
    if let Some(pref) = want_prefix {
        if !current_rustc
            .as_deref()
            .is_some_and(|v| v.starts_with(pref))
        {
            problems.push(format!(
                "rustc version mismatch: want prefix {pref}, got {}",
                current_rustc.as_deref().unwrap_or("(unknown)")
            ));
        }
    }
    if problems.is_empty() {
        return Ok(());
    }

    let msg = format!("Rust constraints not satisfied: {}", problems.join("; "));
    match mode {
        RustEnforceMode::Warn => {
            eprintln!("envr: warning: {msg}");
            Ok(())
        }
        RustEnforceMode::Error => Err(envr_error::EnvrError::Validation(msg)),
    }
}

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

    if let Ok(Some((cfg, _loc))) =
        load_project_config_profile(&ctx.working_dir, ctx.profile.as_deref())
    {
        if let Err(e) = enforce_rust_constraints(&env_map, &cfg, &ctx.working_dir) {
            return common::print_envr_error(g, e);
        }
    }

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
