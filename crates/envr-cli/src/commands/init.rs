use crate::cli::GlobalArgs;
use crate::CommandOutcome;
use crate::output::{self, fmt_template};

use envr_config::project_config::PROJECT_CONFIG_FILE;
use envr_error::{EnvrError, EnvrResult};
use std::fs;
use std::io::{self, BufRead, IsTerminal, Write};
use std::path::PathBuf;

const INIT_TEMPLATE: &str = r#"# envr project configuration
# See `refactor docs/04-shim-设计.md`. Uncomment a version to pin this repo.

[env]

[runtimes.node]
# version = "20"

[runtimes.python]
# version = "3.12"

[runtimes.java]
# version = "21"
"#;

const INIT_TEMPLATE_FULL: &str = r#"# envr project configuration
# `envr init --full` — commented examples for [env] and [profiles].

# -----------------------------------------------------------------------------
# 项目级环境变量（合并进 envr run / exec / shell）
# -----------------------------------------------------------------------------
[env]
# MY_TOOL_HOME = "/opt/my-tool"
# PATH = "/extra/bin:${PATH}"

# -----------------------------------------------------------------------------
# 默认运行时 pin（取消注释一行即可固定版本）
# -----------------------------------------------------------------------------
[runtimes.node]
# version = "20"

[runtimes.python]
# version = "3.12"

[runtimes.java]
# version = "21"

# -----------------------------------------------------------------------------
# 命名 profile：用 ENVR_PROFILE=name 或 envr run --profile name 激活
# -----------------------------------------------------------------------------
# [profiles.ci.env]
# CI = "1"

# [profiles.ci.runtimes.node]
# version = "22"

# [profiles.old_lts.runtimes.node]
# version = "18"
"#;

fn trim_version_input(s: &str) -> String {
    s.trim()
        .trim_matches(|c: char| c == '"' || c == '\'')
        .to_string()
}

fn read_line(stdin: &mut dyn BufRead) -> String {
    let mut buf = String::new();
    let _ = stdin.read_line(&mut buf);
    buf
}

fn parse_yes(line: &str, default_yes: bool) -> bool {
    let t = line.trim().to_ascii_lowercase();
    if t.is_empty() {
        return default_yes;
    }
    matches!(t.as_str(), "y" | "yes" | "1" | "true" | "t")
}

fn prompt_version(stdin: &mut dyn BufRead, default: &str) -> String {
    eprint!("  version [{default}]: ");
    let _ = io::stderr().flush();
    let line = read_line(stdin);
    let v = trim_version_input(&line);
    if v.is_empty() {
        default.to_string()
    } else {
        v
    }
}

fn maybe_pin_runtime(
    stdin: &mut dyn BufRead,
    lang: &str,
    default_version: &str,
    default_pin: bool,
) -> Option<String> {
    let def = if default_pin { "Y/n" } else { "y/N" };
    eprint!("Pin {lang}? [{def}] ");
    let _ = io::stderr().flush();
    let line = read_line(stdin);
    let pin = parse_yes(&line, default_pin);
    if !pin {
        return None;
    }
    Some(prompt_version(stdin, default_version))
}

fn runtime_block(kind: &str, v: &str) -> EnvrResult<String> {
    if v.chars()
        .any(|c| matches!(c, '"' | '\\' | '\n' | '\r' | '[' | ']'))
    {
        return Err(EnvrError::Validation(
            envr_core::i18n::tr_key(
                "cli.err.init_version_chars",
                "版本字符串包含不允许的字符（请使用字母、数字和 .-+_ 等）。",
                "version string contains disallowed characters (use letters, digits, and .-+_ etc.).",
            ),
        ));
    }
    Ok(format!("[runtimes.{kind}]\nversion = \"{v}\"\n\n"))
}

fn interactive_toml() -> EnvrResult<String> {
    if !io::stdin().is_terminal() {
        return Err(EnvrError::Validation(
            envr_core::i18n::tr_key(
                "cli.err.init_interactive_tty",
                "`envr init --interactive` 需要交互式终端（TTY）。",
                "`envr init --interactive` requires an interactive terminal (TTY).",
            ),
        ));
    }
    let mut stdin = io::stdin().lock();
    eprintln!(
        "{}",
        envr_core::i18n::tr_key(
            "cli.init.interactive_intro",
            "通过几个问答生成 `.envr.toml`（直接回车使用方括号中的默认值）。",
            "Answer a few prompts to generate `.envr.toml` (Enter accepts [defaults] in brackets).",
        )
    );

    let mut out = String::from("# envr project configuration (`envr init --interactive`)\n\n[env]\n\n");

    if let Some(v) = maybe_pin_runtime(&mut stdin, "node", "20", true) {
        out.push_str(&runtime_block("node", &v)?);
    } else {
        out.push_str("[runtimes.node]\n# version = \"20\"\n\n");
    }

    if let Some(v) = maybe_pin_runtime(&mut stdin, "python", "3.12", false) {
        out.push_str(&runtime_block("python", &v)?);
    } else {
        out.push_str("[runtimes.python]\n# version = \"3.12\"\n\n");
    }

    if let Some(v) = maybe_pin_runtime(&mut stdin, "java", "21", false) {
        out.push_str(&runtime_block("java", &v)?);
    } else {
        out.push_str("[runtimes.java]\n# version = \"21\"\n\n");
    }

    eprint!(
        "{} ",
        envr_core::i18n::tr_key(
            "cli.init.interactive_full_snippet",
            "是否附加 [env] / [profiles] 注释示例块？ [y/N]",
            "Append commented `[env]` / `[profiles]` example blocks? [y/N]",
        )
    );
    let _ = io::stderr().flush();
    let line = read_line(&mut stdin);
    if parse_yes(&line, false) {
        out.push_str(
            r#"
# --- Examples (commented) ---
# [env]
# CI = "1"
#
# [profiles.ci.env]
# CI = "1"
#
# [profiles.ci.runtimes.node]
# version = "22"
"#,
        );
    }

    Ok(out)
}

pub fn run(
    g: &GlobalArgs,
    path: PathBuf,
    force: bool,
    full: bool,
    interactive: bool,
) -> i32 {
    CommandOutcome::from_result(run_inner(g, path, force, full, interactive)).finish(g)
}

fn run_inner(
    g: &GlobalArgs,
    path: PathBuf,
    force: bool,
    full: bool,
    interactive: bool,
) -> EnvrResult<i32> {
    if interactive
        && matches!(
            g.effective_output_format(),
            crate::cli::OutputFormat::Json
        ) {
            return Err(EnvrError::Validation(
                envr_core::i18n::tr_key(
                    "cli.err.init_interactive_format",
                    "`envr init --interactive` 不能与 `--format json` 同时使用。",
                    "`envr init --interactive` cannot be used with `--format json`.",
                ),
            ));
        }

    if !path.is_dir() {
        return Err(EnvrError::Validation(fmt_template(
            &envr_core::i18n::tr_key(
                "cli.err.not_a_directory",
                "不是目录：{path}",
                "not a directory: {path}",
            ),
            &[("path", &path.display().to_string())],
        )));
    }
    let target = path.join(PROJECT_CONFIG_FILE);
    if target.exists() && !force {
        return Err(EnvrError::Config(fmt_template(
            &envr_core::i18n::tr_key(
                "cli.err.init_exists",
                "{path} 已存在（使用 --force 覆盖）",
                "{path} already exists (use --force to overwrite)",
            ),
            &[("path", &target.display().to_string())],
        )));
    }

    let body = if interactive {
        interactive_toml()?
    } else if full {
        INIT_TEMPLATE_FULL.to_string()
    } else {
        INIT_TEMPLATE.to_string()
    };

    fs::write(&target, &body).map_err(EnvrError::from)?;
    let data = serde_json::json!({
        "path": target.to_string_lossy(),
        "interactive": interactive,
    });
    Ok(output::emit_ok(g, "project_config_init", data, || {
        if !g.quiet {
            println!(
                "{}",
                fmt_template(
                    &envr_core::i18n::tr_key("cli.init.wrote", "已写入 {path}", "wrote {path}",),
                    &[("path", &target.display().to_string())],
                )
            );
        }
    }))
}
