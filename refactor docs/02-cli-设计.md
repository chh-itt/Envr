# envr CLI 设计

## 1. CLI 目标

- 成为 envr 的“第一交付面”：功能完整、可脚本化、可测试。
- 命令语义稳定，输出可机器解析（`--format json`）。
- 所有命令通过 `envr-core` 服务执行，CLI 只负责参数与展示。

## 2. 命令分层

- L1（MVP 必需）
  - `install`, `use`, `list`, `current`, `uninstall`, `which`, `remote`, `doctor`
- L2（高频增强）
  - `config`, `alias`, `prune`, `update`, `resolve`, `shell`
- L3（高级能力）
  - `exec`, `run`, `env`, `import`, `export`, `profile`, `status`

## 3. 命令风格规范

- 可读性：`envr <cmd> <lang> <version>` 优先。
- 幂等性：重复执行不报错（或给出明确“已是目标状态”）。
- 退出码约定：
  - `0` 成功
  - `1` 用户输入或业务失败
  - `2` 外部依赖异常（网络、文件系统权限等）

## 4. 参数与输出

- 全局参数：
  - `--format text|json`
  - `--quiet`
  - `--no-color`
  - `--runtime-root <path>`
- JSON 输出统一结构：
  - `success`, `code`, `message`, `data`, `diagnostics`

## 5. 执行链路

- `clap` 解析 -> `Command DTO` -> `core::CommandHandler` -> `RuntimeProvider` -> 输出编码。
- CLI 不直接访问具体语言模块，只通过 `core` 的服务接口。

## 6. 与旧/二版项目整合建议

- 复用“未完成重构项目”中的命令框架与 i18n help 模板。
- 对齐“旧项目”已验证功能命令，优先补齐缺失 handler，而非新增命令。
- 将当前“stub 命令”按优先级逐个实装，避免一次性铺开。

## 7. 测试策略

- `snapshot` 测试：help 输出、json schema。
- `integration` 测试：命令全链路（安装、切换、卸载）。
- `contract` 测试：同命令 text/json 输出语义一致。

