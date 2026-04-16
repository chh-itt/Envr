// Included by `help_registry.rs` — keep paths aligned with `Cli` / `Command` clap tree.

static HELP_BY_PATH: &[CmdHelpSpec] = &[
    CmdHelpSpec {
        path: &["install"],
        about: I18n(
            "cli.help.cmd.install",
            "安装运行时版本",
            "Install a runtime version",
        ),
        after_long_help: None,
        args: &[
            ArgHelpSpec {
                id: "runtime",
                help: I18n("cli.help.arg.lang", "语言", "Language"),
                long_help: None,
            },
            ArgHelpSpec {
                id: "runtime_version",
                help: I18n("cli.help.arg.version", "版本", "Version"),
                long_help: None,
            },
        ],
    },
    CmdHelpSpec {
        path: &["use"],
        about: I18n(
            "cli.help.cmd.use",
            "设置全局默认运行时版本（写入 runtime root 下的 current；类似 nvm / fnm）",
            "Set the global default runtime version (updates `current` under the runtime root; like nvm / fnm)",
        ),
        after_long_help: Some(I18n(
            "cli.help.use.after_long",
            "此命令修改全局默认版本，新开终端也会沿用。\n若只想在当前终端临时使用合并后的环境，请用 `envr shell`（交互子 shell）、`envr exec`（单语言子进程），或在 POSIX shell 中执行 `eval \"$(envr env …)\"` 导入环境。",
            "This updates the global default; new terminals pick it up.\nFor a temporary merged environment in the current terminal only, use `envr shell` (interactive subshell), `envr exec` (single-language child), or on POSIX run `eval \"$(envr env …)\"` to import env into the current shell.",
        )),
        args: &[
            ArgHelpSpec {
                id: "runtime",
                help: I18n("cli.help.arg.lang", "语言", "Language"),
                long_help: None,
            },
            ArgHelpSpec {
                id: "runtime_version",
                help: I18n("cli.help.arg.version", "版本", "Version"),
                long_help: None,
            },
        ],
    },
    CmdHelpSpec {
        path: &["list"],
        about: I18n(
            "cli.help.cmd.list",
            "列出已安装的运行时",
            "List installed runtimes",
        ),
        after_long_help: None,
        args: &[
            ArgHelpSpec {
                id: "runtime",
                help: I18n(
                    "cli.help.arg.lang_optional",
                    "语言（可选）",
                    "Language (optional)",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "outdated",
                help: I18n(
                    "cli.help.arg.list_outdated",
                    "对照远程索引标出可升级版本（需网络或缓存）",
                    "Mark upgradeable versions using the remote index (network or cache)",
                ),
                long_help: None,
            },
        ],
    },
    CmdHelpSpec {
        path: &["current"],
        about: I18n(
            "cli.help.cmd.current",
            "显示当前激活的运行时版本",
            "Show the active runtime version",
        ),
        after_long_help: None,
        args: &[ArgHelpSpec {
            id: "runtime",
            help: I18n(
                "cli.help.arg.lang_optional",
                "语言（可选）",
                "Language (optional)",
            ),
            long_help: None,
        }],
    },
    CmdHelpSpec {
        path: &["uninstall"],
        about: I18n(
            "cli.help.cmd.uninstall",
            "卸载运行时版本",
            "Uninstall a runtime version",
        ),
        after_long_help: None,
        args: &[
            ArgHelpSpec {
                id: "runtime",
                help: I18n("cli.help.arg.lang", "语言", "Language"),
                long_help: None,
            },
            ArgHelpSpec {
                id: "runtime_version",
                help: I18n("cli.help.arg.version", "版本", "Version"),
                long_help: None,
            },
        ],
    },
    CmdHelpSpec {
        path: &["which"],
        about: I18n(
            "cli.help.cmd.which",
            "定位 shim 或可执行文件",
            "Locate a shim or executable",
        ),
        after_long_help: None,
        args: &[ArgHelpSpec {
            id: "name",
            help: I18n("cli.help.arg.name", "名称", "Name"),
            long_help: None,
        }],
    },
    CmdHelpSpec {
        path: &["remote"],
        about: I18n(
            "cli.help.cmd.remote",
            "列出可用的远程版本",
            "List available remote versions",
        ),
        after_long_help: None,
        args: &[
            ArgHelpSpec {
                id: "runtime",
                help: I18n(
                    "cli.help.arg.lang_optional",
                    "语言（可选）",
                    "Language (optional)",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "prefix",
                help: I18n(
                    "cli.help.arg.prefix",
                    "仅列出标签以此前缀开头的远程版本",
                    "Limit remote versions to those whose labels start with this prefix",
                ),
                long_help: None,
            },
        ],
    },
    CmdHelpSpec {
        path: &["debug"],
        about: I18n(
            "cli.help.cmd.debug_topic",
            "故障排查辅助（路径、环境快照等）",
            "Troubleshooting helpers (paths, environment snapshot)",
        ),
        after_long_help: None,
        args: &[],
    },
    CmdHelpSpec {
        path: &["debug", "info"],
        about: I18n(
            "cli.help.cmd.debug.info",
            "打印配置路径、ENVR_* 环境变量与运行时根目录摘要",
            "Print config paths, `ENVR_*` vars, and runtime root layout summary",
        ),
        after_long_help: None,
        args: &[],
    },
    CmdHelpSpec {
        path: &["doctor"],
        about: I18n(
            "cli.help.cmd.doctor",
            "运行诊断与环境检查",
            "Run diagnostics and environment checks",
        ),
        after_long_help: None,
        args: &[
            ArgHelpSpec {
                id: "fix",
                help: I18n(
                    "cli.help.arg.doctor_fix",
                    "自动修复可安全处理的问题（空 shims、补全 current、修复指向缺失版本的 current）",
                    "Apply safe automatic fixes (empty shims, missing/broken current with installs)",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "fix_path",
                help: I18n(
                    "cli.help.arg.doctor_fix_path",
                    "额外输出将 shims 永久加入 PATH 的复制命令（需自行审核；不修改注册表）",
                    "Also print copy/paste commands to add shims to PATH permanently (review before running; no registry writes)",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "fix_path_apply",
                help: I18n(
                    "cli.help.arg.doctor_fix_path_apply",
                    "与 `--fix-path` 联用：在 Windows 上可交互执行 PowerShell 将 shims 追加到用户 PATH",
                    "With `--fix-path` on Windows: optionally run the PowerShell snippet to append shims to User PATH",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "json",
                help: I18n(
                    "cli.help.arg.doctor_json",
                    "等同于 `--format json`，便于监控与 IDE 消费",
                    "Same as `--format json` for dashboards and IDE plugins",
                ),
                long_help: None,
            },
        ],
    },
    CmdHelpSpec {
        path: &["deactivate"],
        about: I18n(
            "cli.help.cmd.deactivate",
            "在已加载 hook 的 shell 中恢复进入项目前的环境变量（与 `envr off` 相同）",
            "Restore env vars saved by `envr hook` (same as `envr off`)",
        ),
        after_long_help: None,
        args: &[],
    },
    CmdHelpSpec {
        path: &["diagnostics"],
        about: I18n(
            "cli.help.cmd.diagnostics_topic",
            "导出诊断 zip（doctor JSON、环境摘要、近期日志）",
            "Export a diagnostics zip for bug reports (doctor JSON, env summary, recent logs)",
        ),
        after_long_help: None,
        args: &[],
    },
    CmdHelpSpec {
        path: &["diagnostics", "export"],
        about: I18n(
            "cli.help.cmd.diagnostics.export",
            "写入 doctor.json、system.txt、environment.txt 及近期 *.log 到 zip",
            "Write `doctor.json`, `system.txt`, `environment.txt`, and recent `*.log` files into a zip",
        ),
        after_long_help: None,
        args: &[ArgHelpSpec {
            id: "output",
            help: I18n(
                "cli.help.arg.diag_output",
                "输出 .zip 路径（默认：当前目录下 envr-diagnostics-<unix_secs>.zip）",
                "Output `.zip` path (default: `envr-diagnostics-<unix_secs>.zip` in cwd)",
            ),
            long_help: None,
        }],
    },
    CmdHelpSpec {
        path: &["init"],
        about: I18n(
            "cli.help.cmd.init",
            "在指定目录创建初始 `.envr.toml`",
            "Create a starter `.envr.toml` in the given directory",
        ),
        after_long_help: None,
        args: &[
            ArgHelpSpec {
                id: "path",
                help: I18n(
                    "cli.help.arg.init_path",
                    "将包含 `.envr.toml` 的目录",
                    "Directory that will contain `.envr.toml`",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "force",
                help: I18n(
                    "cli.help.arg.force",
                    "覆盖已存在的 `.envr.toml`",
                    "Overwrite an existing `.envr.toml`",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "full",
                help: I18n(
                    "cli.help.arg.init_full",
                    "写入含 [env] / [profiles] 注释示例的较长模板",
                    "Write a longer template with commented `[env]` / `[profiles]` examples",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "interactive",
                help: I18n(
                    "cli.help.arg.init_interactive",
                    "在终端中问答生成 pin（与 --full 二选一逻辑：交互模式会按需附加注释块）",
                    "Prompt for pins in the terminal (interactive flow; optional commented blocks at the end)",
                ),
                long_help: None,
            },
        ],
    },
    CmdHelpSpec {
        path: &["check"],
        about: I18n(
            "cli.help.cmd.check",
            "校验 `.envr.toml` / pin 是否解析到已安装运行时（与 shim 规则一致）",
            "Verify `.envr.toml` / pins resolve to installed runtimes (same rules as shims)",
        ),
        after_long_help: None,
        args: &[ArgHelpSpec {
            id: "path",
            help: I18n(
                "cli.help.arg.search_path",
                "开始向上搜索配置的目录或文件",
                "Directory or file to start config search from",
            ),
            long_help: None,
        }],
    },
    CmdHelpSpec {
        path: &["status"],
        about: I18n(
            "cli.help.cmd.status",
            "显示项目根（若有）、pin 与当前目录下各运行时的激活版本（与 shim 解析一致）",
            "Show project root (if any), pins, and active runtime versions for this directory (same as shim resolution)",
        ),
        after_long_help: None,
        args: &[
            ArgHelpSpec {
                id: "path",
                help: I18n(
                    "cli.help.arg.search_path",
                    "开始向上搜索 `.envr.toml` 的目录",
                    "Directory to start `.envr.toml` search from",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "profile",
                help: I18n(
                    "cli.help.arg.profile_short",
                    "Profile 名称",
                    "Profile name",
                ),
                long_help: None,
            },
        ],
    },
    CmdHelpSpec {
        path: &["project"],
        about: I18n(
            "cli.help.cmd.project.about",
            "管理 `.envr.toml` 中的运行时 pin（添加、同步安装、校验）",
            "Manage runtime pins in `.envr.toml` (add, sync installs, validate)",
        ),
        after_long_help: None,
        args: &[],
    },
    CmdHelpSpec {
        path: &["project", "add"],
        about: I18n(
            "cli.help.cmd.project.add",
            "添加或更新 pin（例如 `node@20`、`python@3.12`）",
            "Add or update a pin (e.g. `node@20`, `python@3.12`)",
        ),
        after_long_help: None,
        args: &[],
    },
    CmdHelpSpec {
        path: &["project", "sync"],
        about: I18n(
            "cli.help.cmd.project.sync",
            "确保 pin 对应版本已安装（`--install` 时自动安装缺失项）",
            "Ensure pinned versions exist (`--install` to fetch missing)",
        ),
        after_long_help: None,
        args: &[],
    },
    CmdHelpSpec {
        path: &["project", "validate"],
        about: I18n(
            "cli.help.cmd.project.validate",
            "校验 pin 能否解析；可选 `--check-remote` 对照远程索引",
            "Validate pins resolve; optional `--check-remote` against remote indexes",
        ),
        after_long_help: None,
        args: &[],
    },
    CmdHelpSpec {
        path: &["completion"],
        about: I18n(
            "cli.help.cmd.completion",
            "生成 shell 补全脚本（写到 stdout；如 `source <(envr completion bash)`）",
            "Generate shell completion script (stdout; e.g. `source <(envr completion bash)`)",
        ),
        after_long_help: Some(I18n(
            "cli.help.completion.after_long",
            "脚本首行注释指向 `envr help shortcuts`（内置 argv 简写在 clap 之前生效，补全无法单独列出这些 token）。",
            "The script starts with a comment pointing at `envr help shortcuts` (built-in argv rewrites run before clap; completions cannot list those tokens alone).",
        )),
        args: &[ArgHelpSpec {
            id: "shell",
            help: I18n(
                "cli.help.arg.completion_shell",
                "目标 shell：bash、zsh、fish、powershell、elvish",
                "Target shell: bash, zsh, fish, powershell, elvish",
            ),
            long_help: None,
        }],
    },
    CmdHelpSpec {
        path: &["help"],
        about: I18n(
            "cli.help.cmd.help_topic",
            "补充说明（argv 简写等）",
            "Supplemental topics (argv shorthands, etc.)",
        ),
        after_long_help: None,
        args: &[],
    },
    CmdHelpSpec {
        path: &["help", "shortcuts"],
        about: I18n(
            "cli.help.cmd.help.shortcuts",
            "列出 preprocess_cli_args 内置改写；用户别名见 config/aliases.toml",
            "List preprocess_cli_args built-ins; user aliases live in config/aliases.toml",
        ),
        after_long_help: None,
        args: &[],
    },
    CmdHelpSpec {
        path: &["why"],
        about: I18n(
            "cli.help.cmd.why",
            "说明某运行时如何解析到当前安装目录（项目 pin / 全局 current）",
            "Explain how a runtime resolves to its install directory (project pin vs global current)",
        ),
        after_long_help: None,
        args: &[
            ArgHelpSpec {
                id: "runtime",
                help: I18n(
                    "cli.help.arg.lang_key",
                    "语言键：node、python、java 等",
                    "Language key: `node`, `python`, etc.",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "path",
                help: I18n("cli.help.arg.workdir", "工作目录", "Working directory"),
                long_help: None,
            },
            ArgHelpSpec {
                id: "profile",
                help: I18n(
                    "cli.help.arg.profile_short",
                    "Profile 名称",
                    "Profile name",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "spec",
                help: I18n(
                    "cli.help.arg.why_spec",
                    "版本 spec 覆盖（与 `resolve --spec` 相同，优先于项目 pin）",
                    "Version spec override (same as `resolve --spec`; wins over project pin)",
                ),
                long_help: None,
            },
        ],
    },
    CmdHelpSpec {
        path: &["resolve"],
        about: I18n(
            "cli.help.cmd.resolve",
            "打印 shim 使用的运行时主目录（项目 pin 或全局 current）",
            "Print the runtime home directory shims would use (project pin, or global current)",
        ),
        after_long_help: None,
        args: &[
            ArgHelpSpec {
                id: "lang",
                help: I18n(
                    "cli.help.arg.lang_key",
                    "语言键：node、python、java 等",
                    "Language key: `node`, `python`, or `java`",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "spec",
                help: I18n(
                    "cli.help.arg.spec",
                    "版本 spec 覆盖（本次调用忽略项目 pin）",
                    "Version spec override (ignores project pin for this invocation)",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "path",
                help: I18n(
                    "cli.help.arg.workdir",
                    "向上搜索 `.envr.toml` 的工作目录",
                    "Working directory for upward `.envr.toml` search",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "profile",
                help: I18n(
                    "cli.help.arg.profile",
                    "Profile 覆盖（`[profiles.<name>]`），本次调用覆盖 ENVR_PROFILE",
                    "Profile overlay (`[profiles.<name>]`), overrides `ENVR_PROFILE` for this invocation",
                ),
                long_help: None,
            },
        ],
    },
    CmdHelpSpec {
        path: &["exec"],
        about: I18n(
            "cli.help.cmd.exec",
            "在单语言 PATH 与环境下调子进程（项目 pin + ENVR_PROFILE / --profile）",
            "Run a subprocess with PATH and env for one language (project pins + `ENVR_PROFILE` / `--profile`)",
        ),
        after_long_help: Some(I18n(
            "cli.help.exec.after_long",
            "定位：对「已有可执行文件」做单语言 PATH/环境增强（例如 `node` / `python`）。\n不解析 `.envr.toml` 中的 `[scripts]`；项目脚本任务请用 `envr run`。",
            "Runs a native executable with one language on PATH (e.g. `node`, `python`).\nDoes not resolve `[scripts]` from `.envr.toml`; use `envr run` for project script tasks.",
        )),
        args: &[
            ArgHelpSpec {
                id: "lang",
                help: I18n(
                    "cli.help.arg.lang_key",
                    "语言键：node、python、java 等",
                    "Language key: `node`, `python`, or `java`",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "spec",
                help: I18n("cli.help.arg.spec", "版本 spec", "Version spec"),
                long_help: None,
            },
            ArgHelpSpec {
                id: "path",
                help: I18n("cli.help.arg.workdir", "工作目录", "Working directory"),
                long_help: None,
            },
            ArgHelpSpec {
                id: "profile",
                help: I18n(
                    "cli.help.arg.profile_short",
                    "Profile 名称",
                    "Profile name",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "command",
                help: I18n("cli.help.arg.command", "要执行的命令", "Command to run"),
                long_help: None,
            },
            ArgHelpSpec {
                id: "args",
                help: I18n("cli.help.arg.args", "命令参数", "Command arguments"),
                long_help: None,
            },
            ArgHelpSpec {
                id: "install_if_missing",
                help: I18n(
                    "cli.help.arg.install_if_missing",
                    "若 `--spec`/项目 pin 的版本未安装则先安装再执行",
                    "Install the `--spec`/project pin if missing, then run the command",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "dry_run",
                help: I18n(
                    "cli.help.arg.dry_run_exec",
                    "打印将使用的环境变量与 PATH，以及命令行，但不执行",
                    "Print env (including PATH) and the command line without executing",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "dry_run_diff",
                help: I18n(
                    "cli.help.arg.dry_run_diff_exec",
                    "仅打印相对当前进程新增/变更的环境变量与 PATH 条目",
                    "Print only env/PATH changes vs the current process (no execute)",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "verbose",
                help: I18n(
                    "cli.help.arg.verbose_exec",
                    "执行前打印解析到的运行时路径（stderr）",
                    "Print resolved runtime paths to stderr before running",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "env",
                help: I18n(
                    "cli.help.arg.child_env",
                    "子进程环境变量 KEY=VALUE（可重复；在 env 文件之后应用）",
                    "Child env `KEY=VALUE` (repeatable; applied after `--env-file` entries)",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "env_file",
                help: I18n(
                    "cli.help.arg.child_env_file",
                    "从文件加载 KEY=VALUE 行（# 注释；可重复）",
                    "Load `KEY=VALUE` lines from a file (`#` comments; repeatable)",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "output",
                help: I18n(
                    "cli.help.arg.exec_output",
                    "将子进程 stdout/stderr 追加写入该文件（envr 自身信息仍在 stderr）",
                    "Append child stdout/stderr to this file (envr status stays on stderr)",
                ),
                long_help: None,
            },
        ],
    },
    CmdHelpSpec {
        path: &["run"],
        about: I18n(
            "cli.help.cmd.run",
            "合并 node/python/java 的 PATH 并运行子进程（含项目 env）；`COMMAND` 可与 `[scripts]` 同名",
            "Run a subprocess with merged PATH for node, python, and java (plus project `env`); `COMMAND` may match a `[scripts]` name",
        ),
        after_long_help: Some(I18n(
            "cli.help.run.after_long",
            "定位：多语言合并 PATH + 项目 `env` 的「项目任务编排」入口；支持 `[scripts]` 命名任务。\n单语言、只包装一个可执行文件时请用 `envr exec`。",
            "Project-scale runner: merged PATH for several languages plus project `env`; supports `[scripts]` tasks.\nPrefer `envr exec` when you only need one language around a native binary.",
        )),
        args: &[
            ArgHelpSpec {
                id: "path",
                help: I18n("cli.help.arg.workdir", "工作目录", "Working directory"),
                long_help: None,
            },
            ArgHelpSpec {
                id: "profile",
                help: I18n(
                    "cli.help.arg.profile_short",
                    "Profile 名称",
                    "Profile name",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "command",
                help: I18n(
                    "cli.help.arg.run_command",
                    "可执行文件名，或与 `.envr.toml` 中 `[scripts]` 同名的任务",
                    "Executable name, or a `[scripts]` task name from `.envr.toml`",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "args",
                help: I18n(
                    "cli.help.arg.run_args",
                    "传给命令或脚本的额外参数",
                    "Extra arguments for the command or script",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "install_if_missing",
                help: I18n(
                    "cli.help.arg.install_if_missing_run",
                    "对 `.envr.toml` 中已 pin 但缺失的运行时先安装再运行",
                    "Install pinned runtimes from `.envr.toml` when missing, then run",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "dry_run",
                help: I18n(
                    "cli.help.arg.dry_run_run",
                    "打印合并后的环境与命令行，但不执行",
                    "Print merged environment and command line without executing",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "dry_run_diff",
                help: I18n(
                    "cli.help.arg.dry_run_diff_run",
                    "仅打印相对当前进程新增/变更的环境变量与 PATH 条目",
                    "Print only env/PATH changes vs the current process (no execute)",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "verbose",
                help: I18n(
                    "cli.help.arg.verbose_run",
                    "执行前为每个解析到的运行时打印一行说明（stderr）",
                    "Print one line per resolved runtime to stderr before running",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "env",
                help: I18n(
                    "cli.help.arg.child_env",
                    "子进程环境变量 KEY=VALUE（可重复；在 env 文件之后应用）",
                    "Child env `KEY=VALUE` (repeatable; applied after `--env-file` entries)",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "env_file",
                help: I18n(
                    "cli.help.arg.child_env_file",
                    "从文件加载 KEY=VALUE 行（# 注释；可重复）",
                    "Load `KEY=VALUE` lines from a file (`#` comments; repeatable)",
                ),
                long_help: None,
            },
        ],
    },
    CmdHelpSpec {
        path: &["env"],
        about: I18n(
            "cli.help.cmd.env",
            "打印设置 PATH / 运行时 HOME 环境变量 / 项目 env 的 shell 片段（合并运行时）",
            "Print shell snippets setting PATH / runtime-home env / project env (merged runtimes)",
        ),
        after_long_help: None,
        args: &[
            ArgHelpSpec {
                id: "path",
                help: I18n("cli.help.arg.workdir", "工作目录", "Working directory"),
                long_help: None,
            },
            ArgHelpSpec {
                id: "profile",
                help: I18n(
                    "cli.help.arg.profile_short",
                    "Profile 名称",
                    "Profile name",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "shell",
                help: I18n("cli.help.arg.shell", "Shell 类型", "Shell kind"),
                long_help: Some(I18n(
                    "cli.help.arg.shell_long",
                    "posix：POSIX shell；cmd：Windows cmd；powershell：PowerShell。",
                    "`posix`: POSIX shell. `cmd`: Windows cmd. `powershell`: PowerShell.",
                )),
            },
        ],
    },
    CmdHelpSpec {
        path: &["template"],
        about: I18n(
            "cli.help.cmd.template",
            "用合并后的运行时环境渲染模板（`${VAR}` 占位符）",
            "Render a template using the merged runtime env (`${VAR}` placeholders)",
        ),
        after_long_help: None,
        args: &[
            ArgHelpSpec {
                id: "file",
                help: I18n(
                    "cli.help.arg.template_file",
                    "模板文件路径",
                    "Template file path",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "path",
                help: I18n("cli.help.arg.workdir", "工作目录", "Working directory"),
                long_help: None,
            },
            ArgHelpSpec {
                id: "profile",
                help: I18n(
                    "cli.help.arg.profile_short",
                    "Profile 名称",
                    "Profile name",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "env",
                help: I18n(
                    "cli.help.arg.template_env",
                    "仅用于替换的额外 KEY=VALUE（可重复）",
                    "Extra `KEY=VALUE` for substitution only (repeatable)",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "env_file",
                help: I18n(
                    "cli.help.arg.template_env_file",
                    "用于替换的 dotenv 风格文件（可重复）",
                    "Dotenv-style file for substitution (repeatable)",
                ),
                long_help: None,
            },
        ],
    },
    CmdHelpSpec {
        path: &["shell"],
        about: I18n(
            "cli.help.cmd.shell",
            "在合并后的项目环境中启动交互式子 shell（退出即恢复原环境）",
            "Start an interactive subshell with merged project env (leaving restores your shell)",
        ),
        after_long_help: Some(I18n(
            "cli.help.shell.after_long",
            "适合「只在当前终端会话」临时使用 pin 与项目 env；不会改变 `envr use` 写入的全局 current。",
            "Use this for a temporary session with pins and project env; it does not change the global `current` written by `envr use`.",
        )),
        args: &[
            ArgHelpSpec {
                id: "path",
                help: I18n("cli.help.arg.workdir", "工作目录", "Working directory"),
                long_help: None,
            },
            ArgHelpSpec {
                id: "profile",
                help: I18n(
                    "cli.help.arg.profile_short",
                    "Profile 名称",
                    "Profile name",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "shell",
                help: I18n(
                    "cli.help.arg.shell_exe",
                    "要启动的可执行文件（默认 $SHELL / %ComSpec% 或 ENVR_SHELL）",
                    "Shell executable to run (default: `$SHELL` / `%ComSpec%` or `ENVR_SHELL`)",
                ),
                long_help: None,
            },
        ],
    },
    CmdHelpSpec {
        path: &["hook"],
        about: I18n(
            "cli.help.cmd.hook_topic",
            "在包含 `.envr.toml` 的目录树内 `cd` 时自动合并环境（类似 direnv；需 eval 安装脚本）",
            "Auto-merge project env when you `cd` into a tree with `.envr.toml` (direnv-style; install via `eval`)",
        ),
        after_long_help: None,
        args: &[],
    },
    CmdHelpSpec {
        path: &["hook", "bash"],
        about: I18n(
            "cli.help.cmd.hook.bash",
            "输出 bash 钩子脚本（bash 4+；写入 ~/.bashrc：`eval \"$(envr hook bash)\"`）",
            "Emit bash hook script (bash 4+; add to `~/.bashrc`: `eval \"$(envr hook bash)\"`)",
        ),
        after_long_help: None,
        args: &[],
    },
    CmdHelpSpec {
        path: &["hook", "zsh"],
        about: I18n(
            "cli.help.cmd.hook.zsh",
            "输出 zsh 钩子脚本（写入 ~/.zshrc：`eval \"$(envr hook zsh)\"`）",
            "Emit zsh hook script (add to `~/.zshrc`: `eval \"$(envr hook zsh)\"`)",
        ),
        after_long_help: None,
        args: &[],
    },
    CmdHelpSpec {
        path: &["hook", "keys"],
        about: I18n(
            "cli.help.cmd.hook.keys",
            "按行打印钩子会保存/恢复的环境变量名（供钩子内部使用；也可用于调试）",
            "Print env var names the hook saves/restores (one per line; for hook internals / debugging)",
        ),
        after_long_help: None,
        args: &[ArgHelpSpec {
            id: "path",
            help: I18n("cli.help.arg.workdir", "工作目录", "Working directory"),
            long_help: None,
        }],
    },
    CmdHelpSpec {
        path: &["hook", "prompt"],
        about: I18n(
            "cli.help.cmd.hook.prompt",
            "单行运行时摘要，供 PS1 使用（钩子会定义 `_envr_prompt_segment` 调用此子命令）",
            "One-line runtime summary for PS1 (the hook defines `_envr_prompt_segment` calling this)",
        ),
        after_long_help: None,
        args: &[
            ArgHelpSpec {
                id: "path",
                help: I18n("cli.help.arg.workdir", "工作目录", "Working directory"),
                long_help: None,
            },
            ArgHelpSpec {
                id: "profile",
                help: I18n(
                    "cli.help.arg.profile_short",
                    "Profile 名称",
                    "Profile name",
                ),
                long_help: None,
            },
        ],
    },
    CmdHelpSpec {
        path: &["import"],
        about: I18n(
            "cli.help.cmd.import",
            "将 TOML 合并到项目 `.envr.toml`（导入项冲突时覆盖）",
            "Merge a TOML file into the project `.envr.toml` (imported keys win on conflict)",
        ),
        after_long_help: None,
        args: &[
            ArgHelpSpec {
                id: "file",
                help: I18n("cli.help.arg.file", "源文件", "Source file"),
                long_help: None,
            },
            ArgHelpSpec {
                id: "path",
                help: I18n("cli.help.arg.workdir", "工作目录", "Working directory"),
                long_help: None,
            },
        ],
    },
    CmdHelpSpec {
        path: &["export"],
        about: I18n(
            "cli.help.cmd.export",
            "将合并后的项目配置（base + local，无 profile）以 TOML 打印",
            "Print merged on-disk project config (base + local, no profile overlay) as TOML",
        ),
        after_long_help: None,
        args: &[
            ArgHelpSpec {
                id: "path",
                help: I18n("cli.help.arg.workdir", "工作目录", "Working directory"),
                long_help: None,
            },
            ArgHelpSpec {
                id: "output",
                help: I18n(
                    "cli.help.arg.output_file",
                    "输出文件（可选）",
                    "Output file (optional)",
                ),
                long_help: None,
            },
        ],
    },
    CmdHelpSpec {
        path: &["profile"],
        about: I18n(
            "cli.help.cmd.profile_topic",
            "查看 `[profiles.*]`（用 ENVR_PROFILE 或 exec/run 的 `--profile` 激活）",
            "Inspect `[profiles.*]` blocks (use `ENVR_PROFILE` or `exec`/`run` `--profile` to activate)",
        ),
        after_long_help: None,
        args: &[],
    },
    CmdHelpSpec {
        path: &["profile", "list"],
        about: I18n(
            "cli.help.cmd.profile.list",
            "列出合并项目配置中的 profile 名",
            "List profile names defined in merged project config",
        ),
        after_long_help: None,
        args: &[ArgHelpSpec {
            id: "path",
            help: I18n("cli.help.arg.workdir", "工作目录", "Working directory"),
            long_help: None,
        }],
    },
    CmdHelpSpec {
        path: &["profile", "show"],
        about: I18n(
            "cli.help.cmd.profile.show",
            "显示某 profile 的运行时与环境",
            "Show runtimes and env for a named profile",
        ),
        after_long_help: None,
        args: &[
            ArgHelpSpec {
                id: "name",
                help: I18n(
                    "cli.help.arg.profile_name",
                    "Profile 名",
                    "Profile name",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "path",
                help: I18n("cli.help.arg.workdir", "工作目录", "Working directory"),
                long_help: None,
            },
        ],
    },
    CmdHelpSpec {
        path: &["config"],
        about: I18n(
            "cli.help.cmd.config_topic",
            "查看用户设置（settings.toml）",
            "Inspect user settings (`settings.toml`)",
        ),
        after_long_help: None,
        args: &[],
    },
    CmdHelpSpec {
        path: &["config", "schema"],
        about: I18n(
            "cli.help.cmd.config.schema",
            "打印带中文注释的 settings.toml 完整模板（默认值与字段说明）",
            "Print a full commented `settings.toml` template (defaults + Chinese descriptions)",
        ),
        after_long_help: None,
        args: &[],
    },
    CmdHelpSpec {
        path: &["config", "validate"],
        about: I18n(
            "cli.help.cmd.config.validate",
            "校验 settings.toml（语法 + 语义规则）",
            "Validate `settings.toml` (parse + semantic rules)",
        ),
        after_long_help: None,
        args: &[],
    },
    CmdHelpSpec {
        path: &["config", "edit"],
        about: I18n(
            "cli.help.cmd.config.edit",
            "用 $EDITOR / VISUAL 打开 settings.toml，保存后校验",
            "Open `settings.toml` in `$EDITOR` / `VISUAL` and validate after save",
        ),
        after_long_help: None,
        args: &[],
    },
    CmdHelpSpec {
        path: &["config", "path"],
        about: I18n(
            "cli.help.cmd.config.path",
            "打印 settings.toml 的绝对路径",
            "Print absolute path to `settings.toml`",
        ),
        after_long_help: None,
        args: &[],
    },
    CmdHelpSpec {
        path: &["config", "show"],
        about: I18n(
            "cli.help.cmd.config.show",
            "打印合并后的设置（默认值 + 文件）",
            "Print merged settings (defaults + file)",
        ),
        after_long_help: None,
        args: &[],
    },
    CmdHelpSpec {
        path: &["config", "keys"],
        about: I18n(
            "cli.help.cmd.config.keys",
            "列出可写配置键路径",
            "List writable config key paths",
        ),
        after_long_help: None,
        args: &[],
    },
    CmdHelpSpec {
        path: &["config", "get"],
        about: I18n(
            "cli.help.cmd.config.get",
            "按点路径读取配置值（如 mirror.mode）",
            "Read one config value by dotted key (e.g. mirror.mode)",
        ),
        after_long_help: None,
        args: &[ArgHelpSpec {
            id: "key",
            help: I18n("cli.help.arg.config_key", "配置键路径", "Config key path"),
            long_help: None,
        }],
    },
    CmdHelpSpec {
        path: &["config", "set"],
        about: I18n(
            "cli.help.cmd.config.set",
            "按点路径写入配置值（如 mirror.mode manual）",
            "Write one config value by dotted key (e.g. mirror.mode manual)",
        ),
        after_long_help: None,
        args: &[
            ArgHelpSpec {
                id: "key",
                help: I18n("cli.help.arg.config_key", "配置键路径", "Config key path"),
                long_help: None,
            },
            ArgHelpSpec {
                id: "value",
                help: I18n("cli.help.arg.config_value", "配置值", "Config value"),
                long_help: None,
            },
            ArgHelpSpec {
                id: "value_type",
                help: I18n(
                    "cli.help.arg.config_value_type",
                    "值类型（string/bool/int/float/json）",
                    "Value type (string/bool/int/float/json)",
                ),
                long_help: None,
            },
        ],
    },
    CmdHelpSpec {
        path: &["alias"],
        about: I18n(
            "cli.help.cmd.alias_topic",
            "管理 CLI 别名（config/aliases.toml）",
            "Manage CLI aliases (`config/aliases.toml`)",
        ),
        after_long_help: None,
        args: &[],
    },
    CmdHelpSpec {
        path: &["alias", "list"],
        about: I18n("cli.help.cmd.alias.list", "列出别名", "List aliases"),
        after_long_help: None,
        args: &[],
    },
    CmdHelpSpec {
        path: &["alias", "add"],
        about: I18n(
            "cli.help.cmd.alias.add",
            "添加或替换别名（name 在解析前展开为 target；target 可为多词，例如 mydiag → diagnostics export）",
            "Add or replace an alias (`name` expands to `target` before parsing; `target` may be multiple words, e.g. `mydiag` → `diagnostics export`)",
        ),
        after_long_help: None,
        args: &[
            ArgHelpSpec {
                id: "name",
                help: I18n("cli.help.arg.alias_name", "别名", "Alias name"),
                long_help: None,
            },
            ArgHelpSpec {
                id: "target",
                help: I18n("cli.help.arg.alias_target", "目标", "Target"),
                long_help: None,
            },
        ],
    },
    CmdHelpSpec {
        path: &["alias", "remove"],
        about: I18n("cli.help.cmd.alias.remove", "删除别名", "Remove an alias"),
        after_long_help: None,
        args: &[ArgHelpSpec {
            id: "name",
            help: I18n("cli.help.arg.alias_name", "别名", "Alias name"),
            long_help: None,
        }],
    },
    CmdHelpSpec {
        path: &["prune"],
        about: I18n(
            "cli.help.cmd.prune",
            "删除除当前 `current` 外的已安装版本",
            "Remove installed versions except the active `current` selection",
        ),
        after_long_help: None,
        args: &[
            ArgHelpSpec {
                id: "lang",
                help: I18n(
                    "cli.help.arg.lang_prune",
                    "限制为单一语言（node、python、java）；默认全部",
                    "Limit to one language (`node`, `python`, `java`); default: all",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "execute",
                help: I18n(
                    "cli.help.arg.execute",
                    "实际卸载（默认为仅计划）",
                    "Actually uninstall (default is a dry-run plan only)",
                ),
                long_help: None,
            },
        ],
    },
    CmdHelpSpec {
        path: &["update"],
        about: I18n(
            "cli.help.cmd.update",
            "显示 CLI 版本与更新说明",
            "Show CLI version and update notes",
        ),
        after_long_help: None,
        args: &[ArgHelpSpec {
            id: "check",
            help: I18n(
                "cli.help.arg.check_update",
                "预留：检查新版本",
                "Reserved for a future release check",
            ),
            long_help: None,
        }],
    },
    CmdHelpSpec {
        path: &["rust"],
        about: I18n(
            "cli.help.cmd.rust_topic",
            "Rust / rustup 相关（无系统 rustup 时的托管安装等）",
            "Rust / rustup helpers (managed install when no system rustup, etc.)",
        ),
        after_long_help: None,
        args: &[],
    },
    CmdHelpSpec {
        path: &["rust", "install-managed"],
        about: I18n(
            "cli.help.cmd.rust.install_managed",
            "下载并安装 envr 托管 rustup（默认 stable；若已存在系统 rustup 则拒绝）",
            "Download and install envr-managed rustup (stable default; refused if system rustup exists)",
        ),
        after_long_help: None,
        args: &[],
    },
    CmdHelpSpec {
        path: &["shim"],
        about: I18n(
            "cli.help.cmd.shim_topic",
            "管理 `{runtime_root}/shims` 下的 shim",
            "Manage shims under `{runtime_root}/shims`",
        ),
        after_long_help: None,
        args: &[],
    },
    CmdHelpSpec {
        path: &["shim", "sync"],
        about: I18n(
            "cli.help.cmd.shim.sync",
            "刷新核心 shim（可选同步全局包转发）",
            "Refresh core shims (and optionally global package forwards)",
        ),
        after_long_help: None,
        args: &[ArgHelpSpec {
            id: "globals",
            help: I18n(
                "cli.help.arg.globals",
                "同时同步全局包可执行文件（npm global bin、bun global bin）",
                "Also sync global package executables (npm global bin, bun global bin)",
            ),
            long_help: None,
        }],
    },
    CmdHelpSpec {
        path: &["cache"],
        about: I18n(
            "cli.help.cmd.cache_topic",
            "管理 `{runtime_root}/cache` 下的下载缓存",
            "Manage envr download caches under `{runtime_root}/cache`",
        ),
        after_long_help: None,
        args: &[],
    },
    CmdHelpSpec {
        path: &["cache", "clean"],
        about: I18n(
            "cli.help.cmd.cache.clean",
            "删除下载/解压缓存",
            "Remove download/extract caches",
        ),
        after_long_help: None,
        args: &[
            ArgHelpSpec {
                id: "kind",
                help: I18n(
                    "cli.help.arg.cache_kind",
                    "限制为某一缓存类型（如 bun、node）。默认删除全部。",
                    "Limit to one cache kind (e.g. `bun`, `node`). Default: remove all cache.",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "all",
                help: I18n(
                    "cli.help.arg.cache_all",
                    "删除全部缓存的别名（与省略 KIND 相同）",
                    "Alias for removing all cache (same as no KIND).",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "older_than",
                help: I18n(
                    "cli.help.arg.cache_older_than",
                    "仅删除早于该时长的文件（如 30d、24h）；不删整个目录树",
                    "Remove only files older than this duration (e.g. `30d`, `24h`); does not remove the whole tree",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "newer_than",
                help: I18n(
                    "cli.help.arg.cache_newer_than",
                    "与 `--older-than` 联用：只删修改时间在该窗口内的文件（`--newer-than` 须为更长的时间跨度）",
                    "With `--older-than`: only delete files whose mtime falls in the window (`--newer-than` must be a longer span)",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "dry_run",
                help: I18n(
                    "cli.help.arg.cache_dry_run",
                    "只报告将删除的内容，不实际删除",
                    "Report what would be removed without deleting",
                ),
                long_help: None,
            },
        ],
    },
    CmdHelpSpec {
        path: &["cache", "index"],
        about: I18n(
            "cli.help.cmd.cache.index.topic",
            "管理离线远程索引缓存（用于 remote/resolve 离线运行）",
            "Manage offline remote index cache (for offline remote/resolve)",
        ),
        after_long_help: None,
        args: &[],
    },
    CmdHelpSpec {
        path: &["cache", "index", "sync"],
        about: I18n(
            "cli.help.cmd.cache.index.sync",
            "预下载远程索引到索引缓存目录",
            "Download remote indexes into index cache directory",
        ),
        after_long_help: None,
        args: &[
            ArgHelpSpec {
                id: "runtime",
                help: I18n(
                    "cli.help.arg.cache.index.runtime",
                    "限制为某个运行时（默认全部）",
                    "Limit to one runtime (default: all)",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "all",
                help: I18n(
                    "cli.help.arg.cache.index.all",
                    "同步所有运行时（与省略 RUNTIME 相同）",
                    "Sync all runtimes (same as omitting RUNTIME)",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "dir",
                help: I18n(
                    "cli.help.arg.cache.index.dir",
                    "覆盖索引缓存目录（默认：ENVR_INDEX_CACHE_DIR 或 {runtime_root}/cache/indexes）",
                    "Override index cache directory (default: ENVR_INDEX_CACHE_DIR or {runtime_root}/cache/indexes)",
                ),
                long_help: None,
            },
        ],
    },
    CmdHelpSpec {
        path: &["cache", "index", "status"],
        about: I18n(
            "cli.help.cmd.cache.index.status",
            "显示索引缓存状态",
            "Show index cache status",
        ),
        after_long_help: None,
        args: &[ArgHelpSpec {
            id: "dir",
            help: I18n(
                "cli.help.arg.cache.index.dir",
                "覆盖索引缓存目录（默认：ENVR_INDEX_CACHE_DIR 或 {runtime_root}/cache/indexes）",
                "Override index cache directory (default: ENVR_INDEX_CACHE_DIR or {runtime_root}/cache/indexes)",
            ),
            long_help: None,
        }],
    },
    CmdHelpSpec {
        path: &["bundle"],
        about: I18n(
            "cli.help.cmd.bundle_topic",
            "打包/应用可移植离线环境（运行时 + 索引 + 项目配置）",
            "Create/apply a portable offline bundle (runtimes + indexes + project config)",
        ),
        after_long_help: None,
        args: &[],
    },
    CmdHelpSpec {
        path: &["bundle", "create"],
        about: I18n(
            "cli.help.cmd.bundle.create",
            "创建 bundle 压缩包",
            "Create a bundle zip",
        ),
        after_long_help: None,
        args: &[
            ArgHelpSpec {
                id: "output",
                help: I18n(
                    "cli.help.arg.bundle.output",
                    "输出 .zip 路径（默认：当前目录 envr-bundle-<unix_secs>.zip）",
                    "Output `.zip` path (default: envr-bundle-<unix_secs>.zip in cwd)",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "path",
                help: I18n("cli.help.arg.workdir", "工作目录", "Working directory"),
                long_help: None,
            },
            ArgHelpSpec {
                id: "profile",
                help: I18n(
                    "cli.help.arg.profile_name_optional",
                    "Profile（可选）",
                    "Profile (optional)",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "include_indexes",
                help: I18n(
                    "cli.help.arg.bundle.include_indexes",
                    "包含离线远程索引缓存（cache/indexes）",
                    "Include offline remote index cache (cache/indexes)",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "include_shims",
                help: I18n(
                    "cli.help.arg.bundle.include_shims",
                    "包含 shims 目录（{runtime_root}/shims）",
                    "Include shims directory ({runtime_root}/shims)",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "full",
                help: I18n(
                    "cli.help.arg.bundle.full",
                    "全量打包（包含 {runtime_root}/runtimes 下所有版本）",
                    "Full bundle (include all versions under {runtime_root}/runtimes)",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "no_current",
                help: I18n(
                    "cli.help.arg.bundle.no_current",
                    "不包含全局 current（仅打包项目 pin）",
                    "Do not include global current selections (project pins only)",
                ),
                long_help: None,
            },
        ],
    },
    CmdHelpSpec {
        path: &["bundle", "apply"],
        about: I18n(
            "cli.help.cmd.bundle.apply",
            "应用 bundle 到当前机器",
            "Apply a bundle to current machine",
        ),
        after_long_help: None,
        args: &[
            ArgHelpSpec {
                id: "file",
                help: I18n(
                    "cli.help.arg.bundle.file",
                    "bundle zip 文件路径",
                    "Bundle zip file path",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "runtime_root",
                help: I18n(
                    "cli.help.arg.bundle.runtime_root",
                    "覆盖运行时根目录",
                    "Override runtime root directory",
                ),
                long_help: None,
            },
            ArgHelpSpec {
                id: "index_cache_dir",
                help: I18n(
                    "cli.help.arg.bundle.index_cache_dir",
                    "覆盖离线索引缓存目录",
                    "Override offline index cache directory",
                ),
                long_help: None,
            },
        ],
    },
];
