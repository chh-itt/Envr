use crate::CliExit;
use crate::CliUxPolicy;
use crate::cli::{ExecRunSharedArgs, GlobalArgs, OutputFormat};
use crate::commands::child_env;
use crate::commands::cli_install_progress;
use crate::commands::dry_run_env;
use crate::commands::env_overrides;
use crate::output::fmt_template;
use crate::run_context::CliPathProfile;

use envr_config::project_config::ProjectConfig;
use envr_domain::runtime::{RuntimeVersion, VersionSpec, parse_runtime_kind};
use envr_error::{EnvrError, EnvrResult};
use serde_json::json;
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
        && let Some(script) = cfg.scripts.get(command)
    {
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
    "v",
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
    let ux = CliUxPolicy::from_global(g);
    if ran_as_script || !ux.human_text_primary() || ux.wants_porcelain_lines() {
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

/// Body for [`crate::commands::dispatch`]; errors are finished at the dispatch boundary.
pub(crate) fn run_inner(
    g: &GlobalArgs,
    shared: ExecRunSharedArgs,
    command: String,
    args: Vec<String>,
) -> EnvrResult<CliExit> {
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

    let text_out = matches!(g.effective_output_format(), OutputFormat::Text);

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
                if !use_prog && CliUxPolicy::from_global(g).human_text_decorated() {
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

    if verbose && let Ok(lines) = child_env::collect_run_verbose_lines(ctx, install_if_missing, pc)
    {
        crate::commands::common::emit_verbose_lines(g, text_out, &lines, "cli.run.verbose_using");
    }

    if dry_run_diff {
        let parent = dry_run_env::parent_env_snapshot();
        return Ok(dry_run_env::emit_dry_run_diff(
            g, &parent, &env_map, &exe, &exe_args,
        ));
    }
    if dry_run {
        return Ok(dry_run_env::emit_dry_run_snapshot(
            g, &env_map, &exe, &exe_args,
        ));
    }

    if let Some((cfg, _loc)) = proj_loaded {
        crate::commands::common::enforce_rust_constraints(&env_map, cfg, &ctx.working_dir)?;
    }

    let mut child =
        crate::commands::common::build_child_command(&exe, &exe_args, &env_map, &ctx.working_dir);

    let status = child.status().map_err(EnvrError::from)?;
    let exit = status.code().unwrap_or(1);
    let env_file_s: Vec<String> = env_files.iter().map(|p| p.display().to_string()).collect();
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
    Ok(crate::commands::common::emit_child_process_outcome(
        g, data, exit,
    ))
}
