use crate::cli::{ExecRunSharedArgs, GlobalArgs, OutputFormat};
use crate::run_context::CliPathProfile;
use crate::commands::child_env;
use crate::commands::cli_install_progress;
use crate::CommandOutcome;
use crate::commands::dry_run_env;
use crate::commands::env_overrides;
use crate::output::{self, fmt_template};

use envr_config::project_config::{ProjectConfig, RustEnforceMode};
use envr_domain::runtime::{RuntimeVersion, VersionSpec, parse_runtime_kind};
use envr_error::{EnvrError, EnvrResult};
use serde_json::json;
use std::collections::HashMap;
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

fn emit_dry_run_run(
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

fn escape_windows_cmd_token(arg: &str) -> String {
    if arg.is_empty() {
        return "\"\"".to_string();
    }
    if !arg
        .chars()
        .any(|c| c.is_whitespace() || matches!(c, '&' | '|' | '<' | '>' | '^' | '"' | '%'))
    {
        return arg.to_string();
    }
    format!("\"{}\"", arg.replace('"', r#"\""#))
}

/// When `command` matches `[scripts]` in the project config, run the one-liner via a shell so
/// extra CLI args are forwarded (`"$@"` on Unix, appended on Windows).
fn resolve_run_command(
    command: &str,
    args: &[String],
    cfg: Option<&ProjectConfig>,
) -> (String, Vec<String>, bool) {
    if let Some(cfg) = cfg
        && let Some(script) = cfg.scripts.get(command) {
            let (exe, a) = script_shell_invocation(script, args);
            return (exe, a, true);
        }
    (command.to_string(), args.to_vec(), false)
}

fn normalized_run_token(command: &str) -> String {
    let mut t = command.trim().to_ascii_lowercase();
    #[cfg(windows)]
    if t.ends_with(".exe") {
        t.truncate(t.len().saturating_sub(4));
    }
    t
}

/// True when the first token looks like a `[scripts]` task name (single segment, no path).
fn looks_like_script_task_token(command: &str) -> bool {
    let s = command.trim();
    if s.is_empty() || s.len() > 64 {
        return false;
    }
    if s.contains('/') || s.contains('\\') {
        return false;
    }
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_alphabetic() && first != '_' {
        return false;
    }
    s.chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

const COMMON_RUN_FIRST_TOKEN: &[&str] = &[
    "ansible",
    "apt",
    "awk",
    "bash",
    "brew",
    "bun",
    "bundle",
    "c++",
    "cargo",
    "cc",
    "clang",
    "clang++",
    "clippy-driver",
    "cmake",
    "cmd",
    "composer",
    "corepack",
    "cp",
    "curl",
    "deno",
    "docker",
    "dotnet",
    "echo",
    "elixir",
    "emacs",
    "env",
    "erl",
    "false",
    "fish",
    "g++",
    "gcc",
    "gem",
    "git",
    "go",
    "gofmt",
    "gradle",
    "gradlew",
    "helm",
    "hg",
    "irb",
    "jar",
    "java",
    "javac",
    "kubectl",
    "ls",
    "make",
    "meson",
    "mix",
    "mvn",
    "mvnw",
    "mv",
    "nano",
    "ninja",
    "node",
    "npm",
    "npx",
    "nu",
    "openssl",
    "perl",
    "php",
    "pip",
    "pip3",
    "pnpm",
    "podman",
    "powershell",
    "pwsh",
    "py",
    "python",
    "python3",
    "rbenv",
    "rebar3",
    "rm",
    "ruby",
    "rustc",
    "rustfmt",
    "rustup",
    "scp",
    "sed",
    "sh",
    "ssh",
    "svn",
    "tar",
    "terraform",
    "test",
    "true",
    "vim",
    "wasmtime",
    "wasmer",
    "wget",
    "yarn",
    "yarnpkg",
    "yum",
    "zsh",
];

fn is_common_toolchain_binary(command: &str) -> bool {
    let n = normalized_run_token(command);
    COMMON_RUN_FIRST_TOKEN.iter().any(|&x| x == n)
}

fn maybe_emit_run_script_miss_hint(
    g: &GlobalArgs,
    command: &str,
    cfg: Option<&ProjectConfig>,
    ran_as_script: bool,
) {
    if ran_as_script || g.quiet || output::wants_porcelain(g) {
        return;
    }
    if !matches!(
        g.effective_output_format(),
        OutputFormat::Text
    ) {
        return;
    }
    let Some(cfg) = cfg else {
        return;
    };
    if cfg.scripts.is_empty() {
        return;
    }
    if !looks_like_script_task_token(command) {
        return;
    }
    if is_common_toolchain_binary(command) {
        return;
    }
    let msg = fmt_template(
        &envr_core::i18n::tr_key(
            "cli.run.script_miss_hint",
            "本项目定义了 `[scripts]`，但 `{cmd}` 不是脚本名。若要直接调用语言工具链，可尝试：`envr exec --lang <种类> -- {cmd} ...`",
            "This project defines `[scripts]`, but `{cmd}` is not a script name. To run a language toolchain directly, try: `envr exec --lang <kind> -- {cmd} ...`",
        ),
        &[("cmd", command)],
    );
    eprintln!("envr: {msg}");
}

#[cfg(unix)]
fn script_shell_invocation(script: &str, tail_args: &[String]) -> (String, Vec<String>) {
    let body = format!("'{}' \"$@\"", script.replace('\'', "'\\''"));
    let mut v = vec!["-c".to_string(), body, "_".to_string()];
    v.extend(tail_args.iter().cloned());
    ("sh".to_string(), v)
}

#[cfg(windows)]
fn script_shell_invocation(script: &str, tail_args: &[String]) -> (String, Vec<String>) {
    let mut line = script.to_string();
    for a in tail_args {
        line.push(' ');
        line.push_str(&escape_windows_cmd_token(a));
    }
    (
        "cmd.exe".to_string(),
        vec!["/d".to_string(), "/s".to_string(), "/c".to_string(), line],
    )
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

pub fn run(
    g: &GlobalArgs,
    shared: ExecRunSharedArgs,
    command: String,
    args: Vec<String>,
) -> i32 {
    CommandOutcome::from_result(run_inner(g, shared, command, args)).finish(g)
}

fn run_inner(
    g: &GlobalArgs,
    shared: ExecRunSharedArgs,
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

    let rex = CliPathProfile::new(path, profile).load_run_exec()?;
    let ctx = rex.ctx();
    let pc = rex.project_config();

    let text_out = matches!(
        g.effective_output_format(),
        OutputFormat::Text
    );

    let mut auto_installed: Vec<serde_json::Value> = Vec::new();
    if install_if_missing && !dry_run && !dry_run_diff {
        let plan = child_env::plan_missing_pinned_runtimes_for_run(ctx, pc)?;
        let mut seen = std::collections::HashSet::<String>::new();
        let mut uniq: Vec<(String, String)> = Vec::new();
        for (lang, spec) in plan {
            if seen.insert(lang.clone()) {
                uniq.push((lang, spec));
            }
        }
        if !uniq.is_empty() {
            let service = rex.service();
            for (lang, spec) in uniq {
                let kind = parse_runtime_kind(&lang)?;
                let headline = fmt_template(
                    &envr_core::i18n::tr_key(
                        "cli.run.installing_missing",
                        "envr：正在安装缺失的运行时 {lang} {version}…",
                        "envr: installing missing runtime {lang} {version}…",
                    ),
                    &[("lang", &lang), ("version", &spec)],
                );
                let use_prog = cli_install_progress::wants_cli_download_progress(g);
                let (request, guard) = cli_install_progress::install_request_with_progress(
                    g,
                    VersionSpec(spec.clone()),
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
                let installed: RuntimeVersion = service.install(kind, &request)?;
                guard.finish();
                auto_installed.push(json!({
                    "kind": lang,
                    "version": installed.0,
                }));
            }
        }
    }

    let mut env_map = child_env::collect_run_env(ctx, install_if_missing, pc)?;
    env_overrides::apply_env_overrides(&mut env_map, &env_files, &env_pairs)?;

    let proj_loaded = rex.project();
    let (exe, exe_args, ran_as_script) =
        resolve_run_command(&command, &args, proj_loaded.as_ref().map(|(c, _)| c));
    maybe_emit_run_script_miss_hint(g, &command, pc, ran_as_script);

    if verbose && !g.quiet && text_out
        && let Ok(lines) = child_env::collect_run_verbose_lines(ctx, install_if_missing, pc) {
            for line in lines {
                let msg = fmt_template(
                    &envr_core::i18n::tr_key(
                        "cli.run.verbose_using",
                        "Using {detail}",
                        "Using {detail}",
                    ),
                    &[("detail", &line)],
                );
                eprintln!("envr: {msg}");
            }
        }

    if dry_run_diff {
        let parent = dry_run_env::parent_env_snapshot();
        return Ok(dry_run_env::emit_dry_run_diff(
            g, &parent, &env_map, &exe, &exe_args,
        ));
    }
    if dry_run {
        return Ok(emit_dry_run_run(g, &env_map, &exe, &exe_args));
    }

    if let Some((cfg, _loc)) = proj_loaded {
        enforce_rust_constraints(&env_map, cfg, &ctx.working_dir)?;
    }

    let mut child = Command::new(&exe);
    child.args(&exe_args);
    child.env_clear();
    for (k, v) in &env_map {
        child.env(k, v);
    }
    child.current_dir(&ctx.working_dir);

    let status = child.status().map_err(EnvrError::from)?;
    let exit = status.code().unwrap_or(1);
    let env_file_s: Vec<String> = env_files
        .iter()
        .map(|p| p.display().to_string())
        .collect();
    let data = json!({
        "exit_code": exit,
        "command": exe,
        "args": exe_args,
        "install_if_missing": install_if_missing,
        "dry_run": false,
        "verbose": verbose,
        "auto_installed": auto_installed,
        "env_files": env_file_s,
        "env_overrides": env_pairs,
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
