# 贡献指南

English | [简体中文](CONTRIBUTING.zh-CN.md)

## CLI 自动化（envr）

如果你修改或新增了可能被脚本依赖的用户可见 CLI 行为：

### 新增 CLI 命令时的一次性检查清单

只要你新增的命令 / 子命令可能被自动化使用，就按下面这份清单一次性完成：

1. **命令身份（spec / trace / dispatch 提示）** — 在 [`crates/envr-cli/src/cli/command_spec.rs`](crates/envr-cli/src/cli/command_spec.rs) 中增加一行，然后保持 [`crates/envr-cli/src/cli/command/mod.rs`](crates/envr-cli/src/cli/command/mod.rs) 里的 command key 映射一致。
2. **派发 + 帮助一致性** — 在 `crates/envr-cli/src/commands/dispatch_*.rs` 下接好路由，并在 [`crates/envr-cli/src/cli/help_registry/table.inc`](crates/envr-cli/src/cli/help_registry/table.inc) 中补充 / 对齐帮助条目。
3. **JSON 契约表面** — 添加 / 调整 `schemas/cli/data/<message>.json`，然后重新生成 / 校验 `schemas/cli/index.json`（`python scripts/generate_cli_schema_index.py --check`）。
4. **测试** — 在 [`crates/envr-cli/tests/json_schema_contract.rs`](crates/envr-cli/tests/json_schema_contract.rs) 及相关 envelope / automation 测试中增加覆盖。
5. **治理索引 + 豁免** — 更新 `schemas/cli/governance-index.json`，临时缺口只使用豁免项，并写清 `reason/owner/due/exit_criteria`。
6. **运行一次性治理门禁** — 在推送前运行 `python scripts/check_cli_governance_all.py`（或 `--quick`）。
7. **破坏性契约变更** — 如果 schema / index 变化会破坏兼容性，则运行 `python scripts/check_cli_contract_gate.py`，并在 [`docs/cli/output-contract.md`](docs/cli/output-contract.md) 中添加 `Migration note`。

### 额外的 CLI 治理工具

- `python scripts/generate_cli_contract_migration_note.py`：根据 schema / index diff 草拟 `Migration note`。
- `python scripts/generate_cli_contract_report.py`：生成机器可读的契约 diff 报告。
- `python -m unittest scripts/test_cli_contract_gate.py`：验证 contract gate 辅助逻辑。
- 规范性契约文档：[`docs/cli/output-contract.md`](docs/cli/output-contract.md)。

**输出格式：** 使用 [`GlobalArgs::effective_output_format`](crates/envr-cli/src/cli/global.rs)。任何等价于 `--format json` 的子命令 flag 都必须扩展 [`Command::legacy_json_shorthand`](crates/envr-cli/src/cli/command/mod.rs)，在 handler 中使用 [`GlobalArgs::cloned_with_legacy_json`](crates/envr-cli/src/cli/global.rs)，并在 [`mod.rs`](crates/envr-cli/src/cli/mod.rs) 中 `legacy_json_shorthand_centralizes_subcommand_json_flags` 附近添加单测（[`Cli::resolved_output_format`](crates/envr-cli/src/cli/mod.rs) / [`apply_global`](crates/envr-cli/src/cli/mod.rs) 会自动遵循 `legacy_json_shorthand`）。

**派发边界：** [`commands::dispatch`](crates/envr-cli/src/commands/mod.rs) 返回 `(CommandOutcome, GlobalArgs)`，这样 [`cli::run`](crates/envr-cli/src/cli/mod.rs) 可以只调用一次 [`CommandOutcome::finish`](crates/envr-cli/src/command_outcome.rs) 而无需克隆 globals。纯 `EnvrResult<i32>` 的 handler 应导出 `pub(crate) fn run_inner`，并在 dispatch 中使用 [`CommandOutcome::from_result`](crates/envr-cli/src/command_outcome.rs)（见 `which`、`resolve_cmd`、`check`、`config_cmd`、`deactivate_cmd`）。返回裸 `i32` 的 handler（例如 completion emit）使用 [`CommandOutcome::from_exit_code`](crates/envr-cli/src/command_outcome.rs)。Runtime 命令通过 [`with_runtime_service`](crates/envr-cli/src/commands/common.rs) 进入，最后也是 `from_result`（连接错误 → `CommandOutcome::Err`）。不要在 `command_outcome.rs` 之外手工构造 [`CommandOutcome::Done`](crates/envr-cli/src/command_outcome.rs)。

提交 CLI 相关改动前请运行 `cargo test -p envr-cli`。

## Runtime provider 拆分策略（CQRS 迁移）

- runtime 读路径逻辑应通过 `RuntimeIndex`；写路径逻辑应通过 `RuntimeInstaller`。
- 迁移期间 `RuntimeProvider` 仍作为兼容层；不要把新的读路径耦合到写 API。
- 对 CLI 读型命令（`current`、`list`、`remote`、`bundle create`），优先使用 `RuntimeService::index_port`，不要直接调用 `RuntimeService` 的读方法。
- 提交 runtime 架构改动前请运行 `python scripts/check_runtime_trait_split.py`。
- 兼容性退场条件：
  - 所有 `envr-runtime-*` provider 都显式暴露 split ports（`index_port` / `installer_port`），并通过 split tests。
  - CLI / GUI 读路径不再通过 `RuntimeService` 调用旧读方法。
  - CI 连续一个完整 release cycle 保持 `check_runtime_trait_split.py` 绿色。
