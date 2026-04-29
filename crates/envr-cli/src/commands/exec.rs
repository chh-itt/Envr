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
use envr_core::runtime::service::RuntimeService;
use envr_domain::runtime::{VersionSpec, parse_runtime_kind};
use envr_error::{EnvrError, EnvrResult};
use envr_shim_core::{ShimContext, core_tool_executable, parse_core_command};
use serde_json::json;
use std::fs::OpenOptions;
use std::path::PathBuf;

fn inject_elixir_erlang_env_from_runtime_root(
    env_map: &mut std::collections::HashMap<String, String>,
) {
    let Some(root) = std::env::var_os("ENVR_RUNTIME_ROOT") else {
        return;
    };
    let cur = PathBuf::from(root)
        .join("runtimes")
        .join("erlang")
        .join("current");
    if !cur.exists() {
        return;
    }
    let home = if let Ok(target) = std::fs::read_link(&cur) {
        if target.is_relative() {
            cur.parent().map(|p| p.join(&target)).unwrap_or(target)
        } else {
            target
        }
    } else if let Ok(s) = std::fs::read_to_string(&cur) {
        let t = s.trim();
        if t.is_empty() {
            return;
        }
        PathBuf::from(t)
    } else {
        return;
    };
    let home = std::fs::canonicalize(&home).unwrap_or(home);
    let erlang_home = envr_platform::path_norm::normalize_fs_path_string_lossy(&home);
    let mut erts = envr_platform::path_norm::normalize_fs_path_string_lossy(&home.join("bin"));
    if cfg!(windows) && !erts.ends_with('\\') {
        erts.push('\\');
    }
    env_map.insert("ERLANG_HOME".into(), erlang_home);
    env_map.insert("ERTS_BIN".into(), erts);
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
                &[("lang", lang), ("version", spec_str.as_str())],
            );
            let kind = parse_runtime_kind(lang)?;
            let use_prog = cli_install_progress::wants_cli_download_progress(g);
            let (request, guard) = cli_install_progress::install_request_with_progress(
                g,
                VersionSpec(spec_str.clone()),
                headline.clone(),
            );
            if !use_prog && CliUxPolicy::from_global(g).human_text_decorated() {
                eprintln!("{headline}");
            }
            let installer = service.installer_port(kind)?;
            let installed = installer.install(&request)?;
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

/// Body for [`crate::commands::dispatch`]; errors are finished at the dispatch boundary.
pub(crate) fn run_inner(
    g: &GlobalArgs,
    lang: String,
    spec: Option<String>,
    shared: ExecRunSharedArgs,
    output: Option<PathBuf>,
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

    let lang = lang.trim().to_ascii_lowercase();
    parse_runtime_kind(&lang)?;

    let rex = CliPathProfile::new(path, profile).load_run_exec()?;
    let ctx = rex.ctx();
    let pc = rex.project_config();

    let text_out = matches!(g.effective_output_format(), OutputFormat::Text);

    if dry_run || dry_run_diff {
        let mut env_map = child_env::collect_exec_env(ctx, &lang, spec.as_deref(), pc)?;
        env_overrides::apply_env_overrides(&mut env_map, &env_files, &env_pairs)?;
        if verbose
            && let Ok(line) = child_env::describe_exec_resolution(ctx, &lang, spec.as_deref(), pc)
        {
            crate::commands::common::emit_verbose_lines(
                g,
                text_out,
                &[line],
                "cli.exec.verbose_using",
            );
        }
        if dry_run_diff {
            let parent = dry_run_env::parent_env_snapshot();
            return Ok(dry_run_env::emit_dry_run_diff(
                g, &parent, &env_map, &command, &args,
            ));
        }
        return Ok(dry_run_env::emit_dry_run_snapshot(
            g, &env_map, &command, &args,
        ));
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
    if lang == "elixir" {
        inject_elixir_erlang_env_from_runtime_root(&mut env_map);
    }

    if verbose
        && let Ok(line) = child_env::describe_exec_resolution(ctx, &lang, spec.as_deref(), pc)
    {
        crate::commands::common::emit_verbose_lines(g, text_out, &[line], "cli.exec.verbose_using");
    }

    if let Some(cfg) = pc {
        crate::commands::common::enforce_rust_constraints(&env_map, cfg, &ctx.working_dir)?;
    }

    // On Windows, executable lookup may happen before applying the child's environment block
    // (including PATH). Prefer an absolute core tool path when it matches the selected runtime.
    let mut resolved_cmd = command.clone();
    if let Some(core_cmd) = parse_core_command(&command)
        && core_cmd.project_runtime_key() == lang
        && let Ok(home) = child_env::resolve_exec_home_for_lang(ctx, &lang, spec.as_deref(), pc)
    {
        let home = std::fs::canonicalize(&home).unwrap_or(home);
        if let Ok(p) = core_tool_executable(&home, core_cmd) {
            resolved_cmd = p.display().to_string();
        }
    }

    let mut child = crate::commands::common::build_child_command(
        &resolved_cmd,
        &args,
        &env_map,
        &ctx.working_dir,
    );
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

    let status = child.status().map_err(|e| {
        EnvrError::Runtime(envr_platform::process::classify_spawn_failure_message(
            parse_runtime_kind(&lang).ok(),
            "exec command",
            &e,
        ))
    })?;
    let exit = status.code().unwrap_or(1);
    let env_file_s: Vec<String> = env_files.iter().map(|p| p.display().to_string()).collect();
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
    Ok(crate::commands::common::emit_child_process_outcome(
        g, data, exit,
    ))
}
