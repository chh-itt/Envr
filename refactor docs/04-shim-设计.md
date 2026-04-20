# envr Shim 设计

## 1. Shim 目标

- 提供统一入口，完成“命令代理 + 版本解析 + 项目级切换”。
- 对用户透明：调用 `node/python/java/...` 时自动路由到正确版本。
- 轻量可靠：shim 二进制尽可能少依赖、快速启动。

## 2. 行为优先级

1. 从当前目录向上查找 `.envr.toml` / `.envr.local.toml`（原 `.wxenv` / `.wxenv.local`）。
2. 如存在对应语言版本配置，优先走项目级版本。
3. 否则走全局 `current`。
4. 若目标不存在，给出明确安装建议与退出码。

## 3. 关键能力

- 支持核心可执行文件映射（node/npm/npx、python/pip、java/javac 等）。
- 支持全局包 shim 自动刷新（重点 Node 生态）。
- 支持环境变量扩展（`${VAR}`）并做循环保护。
- 支持 `ENVR_RUNTIME_ROOT` 指定运行时根目录。
- `.envr.toml` / `.envr.local.toml` 使用标准 TOML 语法，借助轻量级 TOML 解析库，不再手写解析逻辑，提升安全性与可维护性。

## 4. 跨平台策略

- Windows
  - `*.cmd` + `envr-shim.exe` 组合。
  - PATH 修改优先用系统 API（避免纯文本拼接污染）。
- macOS/Linux
  - shell hook + shim 可执行文件。
  - PATH 注入块可识别、可幂等移除。

## 5. 风险与改进建议

- 风险：shim 内部逻辑过多导致维护困难。
- 对策：
  - `envr-shim` 只保留进程入口与少量 glue code。
  - 解析、匹配、路径策略沉入 `envr-shim-core`。
  - 增加黑盒测试：不同目录层级与配置组合。

