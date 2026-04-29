use crate::CliUxPolicy;
use crate::cli::GlobalArgs;
use crate::commands::doctor::{DoctorReport, onboarding_checklist_lines};
#[cfg(windows)]
use crate::commands::doctor::powershell_append_user_path_snippet;
use crate::output::fmt_template;

use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub(crate) fn shims_path_needs_attention(report: &DoctorReport) -> Option<PathBuf> {
    let shims = report.root.join("shims");
    if !shims.is_dir() {
        return None;
    }
    let empty = std::fs::read_dir(&shims)
        .map(|mut d| d.next().is_none())
        .unwrap_or(true);
    if empty {
        return None;
    }
    if path_contains_dir(&shims) {
        return None;
    }
    Some(shims)
}

fn doctor_use_color(g: &GlobalArgs) -> bool {
    CliUxPolicy::from_global(g).use_rich_text_styles()
}

fn doctor_style_line(g: &GlobalArgs, tone: u8, line: &str) -> String {
    if !doctor_use_color(g) {
        return line.to_string();
    }
    const RESET: &str = "\x1b[0m";
    const RED: &str = "\x1b[31m";
    const YELLOW: &str = "\x1b[33m";
    const DIM: &str = "\x1b[2m";
    match tone {
        1 => format!("{RED}{line}{RESET}"),
        2 => format!("{YELLOW}{line}{RESET}"),
        3 => format!("{DIM}{line}{RESET}"),
        _ => line.to_string(),
    }
}

#[cfg(windows)]
fn run_powershell_user_path_snippet(shims: &Path) -> Result<(), String> {
    use std::process::Command;
    let script = powershell_append_user_path_snippet(shims);
    let status = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("powershell exited with {status}"))
    }
}

pub(crate) fn doctor_path_followup(
    g: &GlobalArgs,
    report: &DoctorReport,
    fix_path: bool,
    fix_path_apply: bool,
) {
    if CliUxPolicy::from_global(g).wants_porcelain_lines() {
        return;
    }
    let Some(shims) = shims_path_needs_attention(report) else {
        if fix_path_apply && CliUxPolicy::from_global(g).human_text_primary() {
            println!(
                "{}",
                envr_core::i18n::tr_key(
                    "cli.doctor.path_fix.apply_noop",
                    "无需应用 PATH 修复（shims 已在 PATH 或 shims 目录不可用）。",
                    "Nothing to apply for PATH (shims already on PATH or shims dir unavailable).",
                )
            );
        }
        return;
    };
    if !CliUxPolicy::from_global(g).human_text_decorated() {
        return;
    }

    if fix_path {
        println!();
        println!(
            "{}",
            doctor_style_line(
                g,
                2,
                &envr_core::i18n::tr_key(
                    "cli.doctor.path_fix.heading",
                    "永久加入 PATH（请自行审核后复制执行）：",
                    "Add shims to PATH permanently (review carefully, then copy/paste):",
                ),
            )
        );
        println!(
            "{}",
            doctor_style_line(
                g,
                2,
                &envr_core::i18n::tr_key(
                    "cli.doctor.path_fix.warn",
                    "错误修改 PATH 可能影响登录与程序查找；避免盲目使用 setx 或直接改注册表。",
                    "Incorrect PATH edits can break sessions. Avoid blind `setx` / registry edits.",
                ),
            )
        );
        #[cfg(windows)]
        {
            println!(
                "{}",
                envr_core::i18n::tr_key(
                    "cli.doctor.path_fix.ps_user",
                    "PowerShell（当前用户，若 PATH 中尚无 shims 则追加）：",
                    "PowerShell (User scope; append shims if missing):",
                )
            );
            println!("{}", powershell_append_user_path_snippet(&shims));
        }
        #[cfg(not(windows))]
        {
            let p = shims.display().to_string();
            println!(
                "{}",
                envr_core::i18n::tr_key(
                    "cli.doctor.path_fix.posix_profile",
                    "在 ~/.profile 或 ~/.bashrc 中加入一行（示例）：",
                    "Append one line to `~/.profile` or `~/.bashrc` (example):",
                )
            );
            println!("export PATH=\"{p}:$PATH\"");
        }

        if fix_path_apply {
            #[cfg(windows)]
            {
                use std::io::IsTerminal;
                let is_tty = io::stdin().is_terminal() && io::stdout().is_terminal();
                if is_tty {
                    let prompt = envr_core::i18n::tr_key(
                        "cli.doctor.path_fix.apply_prompt",
                        "是否立即执行上述 PowerShell 以写入当前用户 PATH？[y/N] ",
                        "Run that PowerShell now to update User PATH? [y/N] ",
                    );
                    print!("{prompt}");
                    let _ = io::stdout().flush();
                    let mut line = String::new();
                    if io::stdin().read_line(&mut line).is_ok() {
                        let yes = matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes");
                        if yes {
                            match run_powershell_user_path_snippet(&shims) {
                                Ok(()) => println!(
                                    "{}",
                                    doctor_style_line(
                                        g,
                                        2,
                                        &envr_core::i18n::tr_key(
                                            "cli.doctor.path_fix.apply_ok",
                                            "已更新用户 PATH；请打开新终端以生效。",
                                            "Updated User PATH; open a new terminal to pick it up.",
                                        ),
                                    )
                                ),
                                Err(e) => println!(
                                    "{}",
                                    doctor_style_line(
                                        g,
                                        1,
                                        &fmt_template(
                                            &envr_core::i18n::tr_key(
                                                "cli.doctor.path_fix.apply_err",
                                                "写入用户 PATH 失败：{detail}",
                                                "Failed to update User PATH: {detail}",
                                            ),
                                            &[("detail", &e)],
                                        ),
                                    )
                                ),
                            }
                        }
                    }
                } else {
                    println!(
                        "{}",
                        envr_core::i18n::tr_key(
                            "cli.doctor.path_fix.apply_need_tty",
                            "非交互终端：跳过自动写入 PATH；请手动复制执行上述命令。",
                            "Not a TTY: skipped automatic PATH update; copy/paste the command above.",
                        )
                    );
                }
            }
            #[cfg(not(windows))]
            {
                println!(
                    "{}",
                    envr_core::i18n::tr_key(
                        "cli.doctor.path_fix.apply_windows_only",
                        "`--fix-path-apply` 仅在 Windows 上可用。",
                        "`--fix-path-apply` is only available on Windows.",
                    )
                );
            }
        }
    }

    use std::io::IsTerminal;
    let (_, session_cmd) = shell_add_path_command(&shims);
    let is_tty = io::stdin().is_terminal() && io::stdout().is_terminal();

    println!();
    if is_tty {
        let prompt = envr_core::i18n::tr_key(
            "cli.doctor.path_session.prompt",
            "是否打印仅作用于当前终端会话的 PATH 命令？[y/N] ",
            "Print a command to prepend shims to PATH for this session only? [y/N] ",
        );
        print!("{prompt}");
        let _ = io::stdout().flush();
        let mut line = String::new();
        if io::stdin().read_line(&mut line).is_ok() {
            let yes = matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes");
            if yes {
                println!(
                    "\n{}",
                    envr_core::i18n::tr_key(
                        "cli.doctor.path_session.copy",
                        "在当前 shell 中执行：",
                        "Run in this shell:",
                    )
                );
                println!("{session_cmd}");
            }
        }
    } else {
        println!(
            "{}",
            envr_core::i18n::tr_key(
                "cli.doctor.path_session.non_tty",
                "当前会话 PATH（复制执行，仅本次终端有效）：",
                "Session PATH (copy & run; applies to this terminal only):",
            )
        );
        println!("{session_cmd}");
    }
}

pub(crate) fn print_doctor_human_sections(
    g: &GlobalArgs,
    report: &DoctorReport,
    fixes_for_text: &[String],
) {
    let none_label = envr_core::i18n::tr_key("cli.common.none", "（无）", "(none)");
    if CliUxPolicy::from_global(g).human_text_decorated() {
        println!(
            "{}",
            doctor_style_line(
                g,
                3,
                &envr_core::i18n::tr_key(
                    "cli.doctor.onboarding_heading",
                    "新仓库检查清单：",
                    "New repo checklist:",
                ),
            )
        );
        for item in onboarding_checklist_lines() {
            println!("  - {}", doctor_style_line(g, 3, &item));
        }
        println!();
    }
    println!(
        "{} {}",
        envr_core::i18n::tr_key(
            "cli.doctor.runtime_root_label",
            "运行时根目录：",
            "runtime root:",
        ),
        report.root.display()
    );
    if let Some(ref e) = report.env_override {
        println!(
            "{} {e}",
            envr_core::i18n::tr_key(
                "cli.doctor.env_override_label",
                "ENVR_RUNTIME_ROOT：",
                "ENVR_RUNTIME_ROOT:",
            )
        );
    }
    println!();
    for (kind, ic, cur) in &report.kinds {
        match cur {
            Some(v) => println!(
                "{}",
                fmt_template(
                    &envr_core::i18n::tr_key(
                        "cli.doctor.line_installed_current",
                        "{kind}：已安装 {count} 个版本，当前 = {current}",
                        "{kind}: {count} installed, current = {current}",
                    ),
                    &[("kind", kind), ("count", &ic.to_string()), ("current", v)],
                )
            ),
            None => println!(
                "{}",
                fmt_template(
                    &envr_core::i18n::tr_key(
                        "cli.doctor.line_installed_none",
                        "{kind}：已安装 {count} 个版本，当前 = {none}",
                        "{kind}: {count} installed, current = {none}",
                    ),
                    &[
                        ("kind", kind),
                        ("count", &ic.to_string()),
                        ("none", &none_label),
                    ],
                )
            ),
        }
    }
    if !report.issues.is_empty() {
        println!(
            "\n{}",
            doctor_style_line(
                g,
                1,
                &envr_core::i18n::tr_key("cli.doctor.issues_heading", "问题：", "Issues:"),
            )
        );
        for i in &report.issues {
            println!("  - {}", doctor_style_line(g, 1, i));
        }
    }
    if !report.warnings.is_empty() && CliUxPolicy::from_global(g).human_text_decorated() {
        println!(
            "\n{}",
            doctor_style_line(
                g,
                2,
                &envr_core::i18n::tr_key("cli.doctor.warnings_heading", "警告：", "Warnings:"),
            )
        );
        for w in &report.warnings {
            println!("  - {}", doctor_style_line(g, 2, w));
        }
    }
    if !report.notes.is_empty() && CliUxPolicy::from_global(g).human_text_decorated() {
        println!(
            "\n{}",
            doctor_style_line(
                g,
                3,
                &envr_core::i18n::tr_key("cli.doctor.notes_heading", "提示：", "Notes:"),
            )
        );
        for n in &report.notes {
            println!("  - {}", doctor_style_line(g, 3, n));
        }
    }
    if !fixes_for_text.is_empty() && CliUxPolicy::from_global(g).human_text_primary() {
        println!(
            "\n{}",
            envr_core::i18n::tr_key(
                "cli.doctor.fixes_heading",
                "已执行的修复：",
                "Fixes applied:"
            )
        );
        for f in fixes_for_text {
            println!("  - {f}");
        }
    }
}

pub(crate) fn path_contains_dir(dir: &Path) -> bool {
    let Ok(path_var) = std::env::var("PATH") else {
        return false;
    };
    let want = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    std::env::split_paths(&path_var).any(|p| std::fs::canonicalize(&p).unwrap_or(p) == want)
}

pub(crate) fn shell_add_path_command(shims: &Path) -> (&'static str, String) {
    let shell = detect_shell_kind();
    let p = shims.display().to_string();
    match shell {
        "powershell" => ("powershell", format!("$env:PATH = \"{p};\" + $env:PATH")),
        "cmd" => ("cmd", format!("set PATH={p};%PATH%")),
        _ => ("posix", format!("export PATH=\"{p}:$PATH\"")),
    }
}

fn detect_shell_kind() -> &'static str {
    if std::env::var("PSModulePath").is_ok() {
        return "powershell";
    }
    if let Ok(comspec) = std::env::var("ComSpec")
        && comspec.to_ascii_lowercase().contains("cmd.exe")
    {
        return "cmd";
    }
    "posix"
}
