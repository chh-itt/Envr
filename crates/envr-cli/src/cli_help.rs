//! Localized `--help` strings for the clap tree (must match `Cli::command()` structure).

use clap::{Command, CommandFactory};

use crate::cli::Cli;

fn tr(key: &'static str, zh: &'static str, en: &'static str) -> String {
    envr_core::i18n::tr_key(key, zh, en)
}

/// Same as [`Cli::command()`] but with `settings.toml` locale applied to about/help text.
/// Call after [`envr_core::i18n::init_from_settings`].
pub fn localized_command() -> Command {
    let mut cmd = Cli::command();
    patch_root(&mut cmd);
    cmd
}

fn patch_root(cmd: &mut Command) {
    *cmd = cmd
        .clone()
        .about(tr(
            "cli.help.about",
            "语言运行时版本管理器",
            "Language runtime version manager",
        ))
        .mut_arg("output_format", |a| {
            a.help(tr(
                "cli.help.global.format",
                "输出格式（text 或 json）。默认：text。",
                "Output format (`text` or `json`). Default: `text`.",
            ))
            .long_help(tr(
                "cli.help.global.format_long",
                "text：人类可读文本；json：单行 JSON 信封，便于脚本解析。",
                "`text`: human-readable. `json`: one JSON line per envelope for automation.",
            ))
        })
        .mut_arg("quiet", |a| {
            a.help(tr(
                "cli.help.global.quiet",
                "抑制非错误输出。",
                "Suppress non-error output.",
            ))
        })
        .mut_arg("no_color", |a| {
            a.help(tr(
                "cli.help.global.no_color",
                "禁用终端 ANSI 颜色。",
                "Disable ANSI color in terminal output.",
            ))
        })
        .mut_arg("runtime_root", |a| {
            a.help(tr(
                "cli.help.global.runtime_root",
                "覆盖运行时根目录（设置 ENVR_RUNTIME_ROOT）。",
                "Override runtime root directory (sets `ENVR_RUNTIME_ROOT`).",
            ))
        });

    for sc in cmd.get_subcommands_mut() {
        patch_subcommand(sc);
    }
}

fn patch_subcommand(cmd: &mut Command) {
    let name = cmd.get_name().to_string();
    match name.as_str() {
        "install" => {
            *cmd = cmd
                .clone()
                .about(tr(
                    "cli.help.cmd.install",
                    "安装运行时版本",
                    "Install a runtime version",
                ))
                .mut_arg("lang", |a| a.help(tr("cli.help.arg.lang", "语言", "Language")))
                .mut_arg("runtime_version", |a| {
                    a.help(tr("cli.help.arg.version", "版本", "Version"))
                });
        }
        "use" => {
            *cmd = cmd
                .clone()
                .about(tr(
                    "cli.help.cmd.use",
                    "为当前 shell 选择运行时",
                    "Select a runtime for the current shell",
                ))
                .mut_arg("lang", |a| a.help(tr("cli.help.arg.lang", "语言", "Language")))
                .mut_arg("runtime_version", |a| {
                    a.help(tr("cli.help.arg.version", "版本", "Version"))
                });
        }
        "list" => {
            *cmd = cmd.clone().about(tr(
                "cli.help.cmd.list",
                "列出已安装的运行时",
                "List installed runtimes",
            )).mut_arg("lang", |a| {
                a.help(tr(
                    "cli.help.arg.lang_optional",
                    "语言（可选）",
                    "Language (optional)",
                ))
            });
        }
        "current" => {
            *cmd = cmd.clone().about(tr(
                "cli.help.cmd.current",
                "显示当前激活的运行时版本",
                "Show the active runtime version",
            )).mut_arg("lang", |a| {
                a.help(tr(
                    "cli.help.arg.lang_optional",
                    "语言（可选）",
                    "Language (optional)",
                ))
            });
        }
        "uninstall" => {
            *cmd = cmd
                .clone()
                .about(tr(
                    "cli.help.cmd.uninstall",
                    "卸载运行时版本",
                    "Uninstall a runtime version",
                ))
                .mut_arg("lang", |a| a.help(tr("cli.help.arg.lang", "语言", "Language")))
                .mut_arg("runtime_version", |a| {
                    a.help(tr("cli.help.arg.version", "版本", "Version"))
                });
        }
        "which" => {
            *cmd = cmd.clone().about(tr(
                "cli.help.cmd.which",
                "定位 shim 或可执行文件",
                "Locate a shim or executable",
            )).mut_arg("name", |a| a.help(tr("cli.help.arg.name", "名称", "Name")));
        }
        "remote" => {
            *cmd = cmd
                .clone()
                .about(tr(
                    "cli.help.cmd.remote",
                    "列出可用的远程版本",
                    "List available remote versions",
                ))
                .mut_arg("lang", |a| {
                    a.help(tr(
                        "cli.help.arg.lang_optional",
                        "语言（可选）",
                        "Language (optional)",
                    ))
                })
                .mut_arg("prefix", |a| {
                    a.help(tr(
                        "cli.help.arg.prefix",
                        "仅列出标签以此前缀开头的远程版本",
                        "Limit remote versions to those whose labels start with this prefix",
                    ))
                });
        }
        "doctor" => {
            *cmd = cmd.clone().about(tr(
                "cli.help.cmd.doctor",
                "运行诊断与环境检查",
                "Run diagnostics and environment checks",
            ));
        }
        "diagnostics" => {
            *cmd = cmd.clone().about(tr(
                "cli.help.cmd.diagnostics",
                "导出诊断 zip（doctor JSON、环境摘要、近期日志）",
                "Export a diagnostics zip for bug reports (doctor JSON, env summary, recent logs)",
            ));
            for nested in cmd.get_subcommands_mut() {
                patch_diagnostics_sub(nested);
            }
        }
        "init" => {
            *cmd = cmd
                .clone()
                .about(tr(
                    "cli.help.cmd.init",
                    "在指定目录创建初始 `.envr.toml`",
                    "Create a starter `.envr.toml` in the given directory",
                ))
                .mut_arg("path", |a| {
                    a.help(tr(
                        "cli.help.arg.init_path",
                        "将包含 `.envr.toml` 的目录",
                        "Directory that will contain `.envr.toml`",
                    ))
                })
                .mut_arg("force", |a| {
                    a.help(tr(
                        "cli.help.arg.force",
                        "覆盖已存在的 `.envr.toml`",
                        "Overwrite an existing `.envr.toml`",
                    ))
                });
        }
        "check" => {
            *cmd = cmd.clone().about(tr(
                "cli.help.cmd.check",
                "校验 `.envr.toml` / pin 是否解析到已安装运行时（与 shim 规则一致）",
                "Verify `.envr.toml` / pins resolve to installed runtimes (same rules as shims)",
            )).mut_arg("path", |a| {
                a.help(tr(
                    "cli.help.arg.search_path",
                    "开始向上搜索配置的目录或文件",
                    "Directory or file to start config search from",
                ))
            });
        }
        "resolve" => {
            *cmd = cmd
                .clone()
                .about(tr(
                    "cli.help.cmd.resolve",
                    "打印 shim 使用的运行时主目录（项目 pin 或全局 current）",
                    "Print the runtime home directory shims would use (project pin, or global current)",
                ))
                .mut_arg("lang", |a| {
                    a.help(tr(
                        "cli.help.arg.lang_key",
                        "语言键：node、python、java 等",
                        "Language key: `node`, `python`, or `java`",
                    ))
                })
                .mut_arg("spec", |a| {
                    a.help(tr(
                        "cli.help.arg.spec",
                        "版本 spec 覆盖（本次调用忽略项目 pin）",
                        "Version spec override (ignores project pin for this invocation)",
                    ))
                })
                .mut_arg("path", |a| {
                    a.help(tr(
                        "cli.help.arg.workdir",
                        "向上搜索 `.envr.toml` 的工作目录",
                        "Working directory for upward `.envr.toml` search",
                    ))
                })
                .mut_arg("profile", |a| {
                    a.help(tr(
                        "cli.help.arg.profile",
                        "Profile 覆盖（`[profiles.<name>]`），本次调用覆盖 ENVR_PROFILE",
                        "Profile overlay (`[profiles.<name>]`), overrides `ENVR_PROFILE` for this invocation",
                    ))
                });
        }
        "exec" => {
            *cmd = cmd
                .clone()
                .about(tr(
                    "cli.help.cmd.exec",
                    "在单语言 PATH 与环境下调子进程（项目 pin + ENVR_PROFILE / --profile）",
                    "Run a subprocess with PATH and env for one language (project pins + `ENVR_PROFILE` / `--profile`)",
                ))
                .mut_arg("lang", |a| {
                    a.help(tr(
                        "cli.help.arg.lang_key",
                        "语言键：node、python、java 等",
                        "Language key: `node`, `python`, or `java`",
                    ))
                })
                .mut_arg("spec", |a| {
                    a.help(tr("cli.help.arg.spec", "版本 spec", "Version spec"))
                })
                .mut_arg("path", |a| {
                    a.help(tr(
                        "cli.help.arg.workdir",
                        "工作目录",
                        "Working directory",
                    ))
                })
                .mut_arg("profile", |a| {
                    a.help(tr(
                        "cli.help.arg.profile_short",
                        "Profile 名称",
                        "Profile name",
                    ))
                })
                .mut_arg("command", |a| {
                    a.help(tr("cli.help.arg.command", "要执行的命令", "Command to run"))
                })
                .mut_arg("args", |a| {
                    a.help(tr("cli.help.arg.args", "命令参数", "Command arguments"))
                });
        }
        "run" => {
            *cmd = cmd
                .clone()
                .about(tr(
                    "cli.help.cmd.run",
                    "合并 node/python/java 的 PATH 并运行子进程（含项目 env）",
                    "Run a subprocess with merged PATH for node, python, and java (plus project `env`)",
                ))
                .mut_arg("path", |a| {
                    a.help(tr(
                        "cli.help.arg.workdir",
                        "工作目录",
                        "Working directory",
                    ))
                })
                .mut_arg("profile", |a| {
                    a.help(tr(
                        "cli.help.arg.profile_short",
                        "Profile 名称",
                        "Profile name",
                    ))
                })
                .mut_arg("command", |a| {
                    a.help(tr("cli.help.arg.command", "要执行的命令", "Command to run"))
                })
                .mut_arg("args", |a| {
                    a.help(tr("cli.help.arg.args", "命令参数", "Command arguments"))
                });
        }
        "env" => {
            *cmd = cmd
                .clone()
                .about(tr(
                    "cli.help.cmd.env",
                    "打印设置 PATH / JAVA_HOME / 项目 env 的 shell 片段（合并运行时）",
                    "Print shell snippets setting PATH / JAVA_HOME / project env (merged runtimes)",
                ))
                .mut_arg("path", |a| {
                    a.help(tr(
                        "cli.help.arg.workdir",
                        "工作目录",
                        "Working directory",
                    ))
                })
                .mut_arg("profile", |a| {
                    a.help(tr(
                        "cli.help.arg.profile_short",
                        "Profile 名称",
                        "Profile name",
                    ))
                })
                .mut_arg("shell", |a| {
                    a.help(tr("cli.help.arg.shell", "Shell 类型", "Shell kind"))
                        .long_help(tr(
                            "cli.help.arg.shell_long",
                            "posix：POSIX shell；cmd：Windows cmd；powershell：PowerShell。",
                            "`posix`: POSIX shell. `cmd`: Windows cmd. `powershell`: PowerShell.",
                        ))
                });
        }
        "import" => {
            *cmd = cmd
                .clone()
                .about(tr(
                    "cli.help.cmd.import",
                    "将 TOML 合并到项目 `.envr.toml`（导入项冲突时覆盖）",
                    "Merge a TOML file into the project `.envr.toml` (imported keys win on conflict)",
                ))
                .mut_arg("file", |a| {
                    a.help(tr("cli.help.arg.file", "源文件", "Source file"))
                })
                .mut_arg("path", |a| {
                    a.help(tr(
                        "cli.help.arg.workdir",
                        "工作目录",
                        "Working directory",
                    ))
                });
        }
        "export" => {
            *cmd = cmd
                .clone()
                .about(tr(
                    "cli.help.cmd.export",
                    "将合并后的项目配置（base + local，无 profile）以 TOML 打印",
                    "Print merged on-disk project config (base + local, no profile overlay) as TOML",
                ))
                .mut_arg("path", |a| {
                    a.help(tr(
                        "cli.help.arg.workdir",
                        "工作目录",
                        "Working directory",
                    ))
                })
                .mut_arg("output", |a| {
                    a.help(tr(
                        "cli.help.arg.output_file",
                        "输出文件（可选）",
                        "Output file (optional)",
                    ))
                });
        }
        "profile" => {
            *cmd = cmd.clone().about(tr(
                "cli.help.cmd.profile",
                "查看 `[profiles.*]`（用 ENVR_PROFILE 或 exec/run 的 `--profile` 激活）",
                "Inspect `[profiles.*]` blocks (use `ENVR_PROFILE` or `exec`/`run` `--profile` to activate)",
            ));
            for nested in cmd.get_subcommands_mut() {
                patch_profile_sub(nested);
            }
        }
        "config" => {
            *cmd = cmd.clone().about(tr(
                "cli.help.cmd.config",
                "查看用户设置（settings.toml）",
                "Inspect user settings (`settings.toml`)",
            ));
            for nested in cmd.get_subcommands_mut() {
                patch_config_sub(nested);
            }
        }
        "alias" => {
            *cmd = cmd.clone().about(tr(
                "cli.help.cmd.alias",
                "管理 CLI 别名（config/aliases.toml）",
                "Manage CLI aliases (`config/aliases.toml`)",
            ));
            for nested in cmd.get_subcommands_mut() {
                patch_alias_sub(nested);
            }
        }
        "prune" => {
            *cmd = cmd
                .clone()
                .about(tr(
                    "cli.help.cmd.prune",
                    "删除除当前 `current` 外的已安装版本",
                    "Remove installed versions except the active `current` selection",
                ))
                .mut_arg("lang", |a| {
                    a.help(tr(
                        "cli.help.arg.lang_prune",
                        "限制为单一语言（node、python、java）；默认全部",
                        "Limit to one language (`node`, `python`, `java`); default: all",
                    ))
                })
                .mut_arg("execute", |a| {
                    a.help(tr(
                        "cli.help.arg.execute",
                        "实际卸载（默认为仅计划）",
                        "Actually uninstall (default is a dry-run plan only)",
                    ))
                });
        }
        "update" => {
            *cmd = cmd.clone().about(tr(
                "cli.help.cmd.update",
                "显示 CLI 版本与更新说明",
                "Show CLI version and update notes",
            )).mut_arg("check", |a| {
                a.help(tr(
                    "cli.help.arg.check_update",
                    "预留：检查新版本",
                    "Reserved for a future release check",
                ))
            });
        }
        "shim" => {
            *cmd = cmd.clone().about(tr(
                "cli.help.cmd.shim",
                "管理 `{runtime_root}/shims` 下的 shim",
                "Manage shims under `{runtime_root}/shims`",
            ));
            for nested in cmd.get_subcommands_mut() {
                patch_shim_sub(nested);
            }
        }
        "cache" => {
            *cmd = cmd.clone().about(tr(
                "cli.help.cmd.cache",
                "管理 `{runtime_root}/cache` 下的下载缓存",
                "Manage envr download caches under `{runtime_root}/cache`",
            ));
            for nested in cmd.get_subcommands_mut() {
                patch_cache_sub(nested);
            }
        }
        _ => {}
    }
}

fn patch_diagnostics_sub(cmd: &mut Command) {
    if cmd.get_name() == "export" {
        *cmd = cmd
            .clone()
            .about(tr(
                "cli.help.cmd.diagnostics.export",
                "写入 doctor.json、system.txt、environment.txt 及近期 *.log 到 zip",
                "Write `doctor.json`, `system.txt`, `environment.txt`, and recent `*.log` files into a zip",
            ))
            .mut_arg("output", |a| {
                a.help(tr(
                    "cli.help.arg.diag_output",
                    "输出 .zip 路径（默认：当前目录下 envr-diagnostics-<unix_secs>.zip）",
                    "Output `.zip` path (default: `envr-diagnostics-<unix_secs>.zip` in cwd)",
                ))
            });
    }
}

fn patch_profile_sub(cmd: &mut Command) {
    match cmd.get_name() {
        "list" => {
            *cmd = cmd.clone().about(tr(
                "cli.help.cmd.profile.list",
                "列出合并项目配置中的 profile 名",
                "List profile names defined in merged project config",
            )).mut_arg("path", |a| {
                a.help(tr(
                    "cli.help.arg.workdir",
                    "工作目录",
                    "Working directory",
                ))
            });
        }
        "show" => {
            *cmd = cmd
                .clone()
                .about(tr(
                    "cli.help.cmd.profile.show",
                    "显示某 profile 的运行时与环境",
                    "Show runtimes and env for a named profile",
                ))
                .mut_arg("name", |a| {
                    a.help(tr("cli.help.arg.profile_name", "Profile 名", "Profile name"))
                })
                .mut_arg("path", |a| {
                    a.help(tr(
                        "cli.help.arg.workdir",
                        "工作目录",
                        "Working directory",
                    ))
                });
        }
        _ => {}
    }
}

fn patch_config_sub(cmd: &mut Command) {
    match cmd.get_name() {
        "path" => {
            *cmd = cmd.clone().about(tr(
                "cli.help.cmd.config.path",
                "打印 settings.toml 的绝对路径",
                "Print absolute path to `settings.toml`",
            ));
        }
        "show" => {
            *cmd = cmd.clone().about(tr(
                "cli.help.cmd.config.show",
                "打印合并后的设置（默认值 + 文件）",
                "Print merged settings (defaults + file)",
            ));
        }
        _ => {}
    }
}

fn patch_shim_sub(cmd: &mut Command) {
    if cmd.get_name() == "sync" {
        *cmd = cmd
            .clone()
            .about(tr(
                "cli.help.cmd.shim.sync",
                "刷新核心 shim（可选同步全局包转发）",
                "Refresh core shims (and optionally global package forwards)",
            ))
            .mut_arg("globals", |a| {
                a.help(tr(
                    "cli.help.arg.globals",
                    "同时同步全局包可执行文件（npm global bin、bun global bin）",
                    "Also sync global package executables (npm global bin, bun global bin)",
                ))
            });
    }
}

fn patch_cache_sub(cmd: &mut Command) {
    if cmd.get_name() == "clean" {
        *cmd = cmd
            .clone()
            .about(tr(
                "cli.help.cmd.cache.clean",
                "删除下载/解压缓存",
                "Remove download/extract caches",
            ))
            .mut_arg("kind", |a| {
                a.help(tr(
                    "cli.help.arg.cache_kind",
                    "限制为某一缓存类型（如 bun、node）。默认删除全部。",
                    "Limit to one cache kind (e.g. `bun`, `node`). Default: remove all cache.",
                ))
            })
            .mut_arg("all", |a| {
                a.help(tr(
                    "cli.help.arg.cache_all",
                    "删除全部缓存的别名（与省略 KIND 相同）",
                    "Alias for removing all cache (same as no KIND).",
                ))
            });
    }
}

fn patch_alias_sub(cmd: &mut Command) {
    match cmd.get_name() {
        "list" => {
            *cmd = cmd.clone().about(tr(
                "cli.help.cmd.alias.list",
                "列出别名",
                "List aliases",
            ));
        }
        "add" => {
            *cmd = cmd
                .clone()
                .about(tr(
                    "cli.help.cmd.alias.add",
                    "添加或替换别名（name 展开为 target，例如 n → node）",
                    "Add or replace an alias (`name` expands to `target`, e.g. `n` → `node`)",
                ))
                .mut_arg("name", |a| {
                    a.help(tr("cli.help.arg.alias_name", "别名", "Alias name"))
                })
                .mut_arg("target", |a| {
                    a.help(tr("cli.help.arg.alias_target", "目标", "Target"))
                });
        }
        "remove" => {
            *cmd = cmd.clone().about(tr(
                "cli.help.cmd.alias.remove",
                "删除别名",
                "Remove an alias",
            )).mut_arg("name", |a| {
                a.help(tr("cli.help.arg.alias_name", "别名", "Alias name"))
            });
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn localized_command_matches_cli_structure() {
        let a = Cli::command();
        let b = localized_command();
        let na: Vec<_> = a.get_subcommands().map(|c| c.get_name().to_string()).collect();
        let nb: Vec<_> = b.get_subcommands().map(|c| c.get_name().to_string()).collect();
        assert_eq!(na, nb);
    }
}
