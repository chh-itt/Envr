use crate::cli::{ExecRunSharedArgs, GlobalArgs, OutputFormat};
use crate::run_context::CliPathProfile;
use crate::commands::child_env;
use crate::commands::cli_install_progress;
use crate::CommandOutcome;
use crate::commands::dry_run_env;
use crate::commands::env_overrides;
use crate::output::{self, fmt_template};

use envr_config::project_config::{ProjectConfig, RustEnforceMode};
use envr_core::runtime::service::RuntimeService;
use envr_domain::runtime::{VersionSpec, parse_runtime_kind};
use envr_error::{EnvrError, EnvrResult};
use envr_shim_core::ShimContext;
use serde_json::json;
use std::collections::HashMap;
use std::fs::OpenOptions;
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
) -> EnvrResult<()> {
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
    if let Some(want) = want_channel
        && current_channel.as_deref() != Some(&want.to_ascii_lowercase()) {
            problems.push(format!(
                "rust channel mismatch: want {want}, got {}",
                current_channel.as_deref().unwrap_or("(unknown)")
            ));
        }
    if let Some(pref) = want_prefix
        && !current_rustc
            .as_deref()
            .is_some_and(|v| v.starts_with(pref))
        {
            problems.push(format!(
                "rustc version mismatch: want prefix {pref}, got {}",
                current_rustc.as_deref().unwrap_or("(unknown)")
            ));
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
        RustEnforceMode::Error => Err(EnvrError::Validation(msg)),
    }
}

fn collect_exec_env_maybe_install(
    g: &GlobalArgs,
    ctx: &ShimContext,
    lang: &str,
    spec: &Option<String>,
    install_if_missing: bool,
    service: &RuntimeService,
    pc: Option<&ProjectConfig>,
) -> EnvrResult<(
    std::collections::HashMap<String, String>,
    Vec<serde_json::Value>,
)> {
    let spec_deref = spec.as_deref();
    match child_env::collect_exec_env(ctx, lang, spec_deref, pc) {
        Ok(m) => Ok((m, Vec::new())),
        Err(e) if install_if_missing && lang != "rust" => {
            let install_spec =
                child_env::effective_install_spec_for_exec(ctx, lang, spec_deref, pc)?;
            let Some(ref spec_str) = install_spec else {
                return Err(e);
            };
            if !child_env::runtime_error_might_install_fix(&e) {
                return Err(e);
            }
            let headline = fmt_template(
                &envr_core::i18n::tr_key(
                    "cli.exec.installing_missing",
                    "envr：正在安装缺失的运行时 {lang} {version}…",
                    "envr: installing missing runtime {lang} {version}…",
                ),
                &[
                    ("lang", lang),
                    ("version", spec_str.as_str()),
                ],
            );
            let kind = parse_runtime_kind(lang)?;
            let use_prog = cli_install_progress::wants_cli_download_progress(g);
            let (request, guard) = cli_install_progress::install_request_with_progress(
                g,
                VersionSpec(spec_str.clone()),
                headline.clone(),
            );
            if !use_prog
                && !g.quiet
                && matches!(
                    g.effective_output_format(),
                    OutputFormat::Text
                )
            {
                eprintln!("{headline}");
            }
            let installed = service.install(kind, &request)?;
            guard.finish();
            let meta = json!({
                "kind": lang,
                "version": installed.0,
            });
            let m = child_env::collect_exec_env(ctx, lang, spec_deref, pc)?;
            Ok((m, vec![meta]))
        }
        Err(e) => Err(e),
    }
}

fn emit_dry_run_exec(
    g: &GlobalArgs,
    env_map: &HashMap<String, String>,
    command: &str,
    args: &[String],
) -> i32 {
    let mut keys: Vec<_> = env_map.keys().cloned().collect();
    keys.sort();
    let mut env_obj = serde_json::Map::new();
    for k in &keys {
        if let Some(v) = env_map.get(k) {
            env_obj.insert(k.clone(), json!(v));
        }
    }
    let data = json!({
        "command": command,
        "args": args,
        "env": env_obj,
    });
    output::emit_ok(g, "dry_run", data, || {
        if !g.quiet {
            println!(
                "{}",
                envr_core::i18n::tr_key(
                    "cli.dry_run.would_run",
                    "将执行：",
                    "Would run:",
                )
            );
            println!("  {} {}", command, shell_words_join(args));
            println!();
            for k in &keys {
                if let Some(v) = env_map.get(k) {
                    println!("{k}={v}");
                }
            }
        }
    })
}

fn shell_words_join(args: &[String]) -> String {
    args.iter()
        .map(|a| {
            if a.contains(char::is_whitespace) {
                format!("{a:?}")
            } else {
                a.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
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
    shared: ExecRunSharedArgs,
    output: Option<PathBuf>,
    command: String,
    args: Vec<String>,
) -> i32 {
    CommandOutcome::from_result(run_inner(
        g,
        lang,
        spec,
        shared,
        output,
        command,
        args,
    ))
    .finish(g)
}

fn run_inner(
    g: &GlobalArgs,
    lang: String,
    spec: Option<String>,
    shared: ExecRunSharedArgs,
    output: Option<PathBuf>,
    command: String,
    args: Vec<String>,
) -> EnvrResult<i32> {
    let ExecRunSharedArgs {
        install_if_missing,
        dry_run,
        dry_run_diff,
        verbose,
        path,
        profile,
        env: env_pairs,
        env_file: env_files,
    } = shared;

    let lang = lang.trim().to_ascii_lowercase();
    parse_runtime_kind(&lang)?;

    let rex = CliPathProfile::new(path, profile).load_run_exec()?;
    let ctx = rex.ctx();
    let pc = rex.project_config();

    let text_out = matches!(
        g.effective_output_format(),
        OutputFormat::Text
    );

    if dry_run || dry_run_diff {
        let mut env_map = child_env::collect_exec_env(ctx, &lang, spec.as_deref(), pc)?;
        env_overrides::apply_env_overrides(&mut env_map, &env_files, &env_pairs)?;
        if verbose && !g.quiet && text_out
            && let Ok(line) = child_env::describe_exec_resolution(ctx, &lang, spec.as_deref(), pc) {
                let msg = fmt_template(
                    &envr_core::i18n::tr_key(
                        "cli.exec.verbose_using",
                        "Using {detail}",
                        "Using {detail}",
                    ),
                    &[("detail", &line)],
                );
                eprintln!("envr: {msg}");
            }
        if dry_run_diff {
            let parent = dry_run_env::parent_env_snapshot();
            return Ok(dry_run_env::emit_dry_run_diff(
                g, &parent, &env_map, &command, &args,
            ));
        }
        return Ok(emit_dry_run_exec(g, &env_map, &command, &args));
    }

    let (mut env_map, auto_installed) = collect_exec_env_maybe_install(
        g,
        ctx,
        &lang,
        &spec,
        install_if_missing,
        rex.service(),
        pc,
    )?;
    env_overrides::apply_env_overrides(&mut env_map, &env_files, &env_pairs)?;

    if verbose && !g.quiet && text_out
        && let Ok(line) = child_env::describe_exec_resolution(ctx, &lang, spec.as_deref(), pc) {
            let msg = fmt_template(
                &envr_core::i18n::tr_key(
                    "cli.exec.verbose_using",
                    "Using {detail}",
                    "Using {detail}",
                ),
                &[("detail", &line)],
            );
            eprintln!("envr: {msg}");
        }

    if let Some(cfg) = pc {
        enforce_rust_constraints(&env_map, cfg, &ctx.working_dir)?;
    }

    // On Windows, executable lookup may happen before applying the child's environment block
    // (including PATH). Prefer an absolute core tool path when we can derive it from the runtime home.
    let mut resolved_cmd = command.clone();
    if lang == "go" && (command == "go" || command == "gofmt")
        && let Ok(home) = child_env::resolve_exec_home_for_lang(ctx, &lang, spec.as_deref(), pc) {
            let home = std::fs::canonicalize(&home).unwrap_or(home);
            if let Some(p) = go_tool_executable(&home, &command) {
                resolved_cmd = p.display().to_string();
            }
        }

    let mut child = Command::new(&resolved_cmd);
    child.args(&args);
    child.env_clear();
    for (k, v) in &env_map {
        child.env(k, v);
    }
    child.current_dir(&ctx.working_dir);
    if let Some(ref out_path) = output {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(out_path)
            .map_err(|e| {
                EnvrError::Io(std::io::Error::new(
                    e.kind(),
                    format!("{}: {e}", out_path.display()),
                ))
            })?;
        let f2 = file.try_clone().map_err(EnvrError::from)?;
        child.stdout(file);
        child.stderr(f2);
    }

    let status = child.status().map_err(EnvrError::from)?;
    let exit = status.code().unwrap_or(1);
    let env_file_s: Vec<String> = env_files
        .iter()
        .map(|p| p.display().to_string())
        .collect();
    let data = json!({
        "exit_code": exit,
        "command": command,
        "args": args,
        "lang": lang,
        "install_if_missing": install_if_missing,
        "dry_run": false,
        "verbose": verbose,
        "auto_installed": auto_installed,
        "env_files": env_file_s,
        "env_overrides": env_pairs,
        "output_file": output.as_ref().map(|p| p.display().to_string()),
    });
    Ok(if exit == 0 {
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
        output::emit_failure_envelope(g, "child_exit", &msg, data, &[], exit)
    })
}
