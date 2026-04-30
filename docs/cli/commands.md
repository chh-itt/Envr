# envr 命令谱系（CLI command spectrum）

本文档把 **产品命令面** 与 `refactor docs/02-cli-设计.md` §2 的分层（L1 / L2 / L3）对齐，并列出实现中多出来的 **运维与数据面** 命令。`envr --help` 末尾的「命令分组」按 **主题** 排列；「命令层级」按 **设计优先级** 排列。

## L1 — 核心生命周期（MVP）

| 命令 | 常见别名 | 说明 |
|------|-----------|------|
| `install` | `i` | 安装指定运行时版本 |
| `use` | `sw` | 设置全局默认版本（`current`） |
| `list` | `ls` | 已安装列表；可选 `--outdated` |
| `current` | `cur` | 当前激活版本 |
| `uninstall` | `u` | 卸载某一版本 |
| `which` | — | 解析 shim / 可执行路径 |
| `remote` | — | 远程可用版本列表 |
| `doctor` | `doc` | 环境与诊断 |

## L2 — 高频增强

| 命令 | 常见别名 | 说明 |
|------|-----------|------|
| `config` | `cfg` | `settings.toml` 路径、展示、校验、`get`/`set` 等 |
| `alias` | — | `config/aliases.toml` 用户 argv 别名 |
| `prune` | — | 清理非 `current` 的已安装版本（默认 dry-run） |
| `update` | — | 版本与更新说明（自更新占位） |
| `resolve` | — | 打印 shim 将使用的运行时根目录 |
| `shell` | — | 在合并后的环境中启动交互子 shell |

以下为设计文档未单列、但属于 **项目 / Shell 协作** 的 L2 能力：

| 命令 | 说明 |
|------|------|
| `why` | 解释某语言版本如何从 pin / 全局 `current` 解析 |
| `init` | 生成 `.envr.toml` |
| `check` | 校验 pin 能否解析到已安装目录；支持 `--github-annotations` 输出 CI 注释 |
| `status` | `st`：项目根、pin、当前目录下各运行时激活版本 |
| `project` | `add` / `sync` / `validate` 管理 pin |
| `hook` | `bash` / `zsh` / `prompt` 等 shell 集成 |
| `deactivate` | `off`：与 `hook` 配对，恢复环境 |
| `rust` | `install-managed` 等 rustup 托管辅助 |

## L3 — 高级自动化

| 命令 | 说明 |
|------|------|
| `exec` | 单语言子进程 + 可选 `--install-if-missing` |
| `run` | 合并多语言 PATH / `env`；支持 `[scripts]` 别名 |
| `env` | 打印 `export` / `set` / PowerShell 形式环境片段；支持 `--diff` |
| `template` | 按合并环境替换模板中的 `${VAR}` |
| `import` / `export` | 项目 TOML 合并与导出；支持 `--config-format tool-versions` 迁移 `.tool-versions` |
| `profile` | 查看 `[profiles.*]` |

## 平台、数据与工具（实现扩展）

| 命令 | 常见别名 | 说明 |
|------|-----------|------|
| `shim` | `sh` | 刷新核心 shim；`sync --globals` 同步全局包转发 |
| `cache` | `c` | 清理下载缓存；`cache index sync|status` 离线索引 |
| `bundle` | — | 便携离线包 `create` / `apply` |
| `debug` | — | `info` 等排障快照 |
| `diagnostics` | — | 导出诊断 zip，包含 doctor / system / environment / provider-state 快照 |
| `completion` | — | 生成 shell 补全脚本 |
| `help` | — | `shortcuts`：内置 argv 简写（先于 clap 生效） |

内置 argv 简写与用户别名规则：`envr help shortcuts`；补全脚本头注释也会指向该主题。

## 相关文档

- [v1.0 终态定义（草案）](./v1.0-definition.md)
- [输出契约（JSON / text）](./output-contract.md)
- [自动化输出矩阵（维护者清单）](./automation-matrix.md)
- [v1.0 指标采集建议](./v1.0-metrics.md)
- [任务导向菜谱](./recipes.md)
- [脚本化与子进程](./scripting.md)
- [settings 与 `config`](./config.md)
- [离线能力与索引](./offline.md)
- [便携包 bundle](./bundle.md)
- 设计原文：`refactor docs/02-cli-设计.md`

## 后续可能方向（占位）

- **解析阶段的 JSON 化**：当前参数/子命令解析失败由 clap 输出到 stderr，不保证信封；仅当集成方有强需求时再评估自定义解析与统一 `write_envelope`（成本与边界情况较多）。
- **主题 / 命名空间式导航**：子命令继续膨胀时，可考虑二级命令组或单独的「主题」帮助入口，减轻扁平 `--help` 列表的认知负担。
