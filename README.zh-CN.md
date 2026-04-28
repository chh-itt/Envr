# envr

[English](README.md) | 简体中文

`envr` 是一个面向开发者与自动化场景的 Rust 语言运行时管理器。
它用于安装和切换工具链/运行时版本、从 `.envr.toml` 解析项目级 pin，并为 CI 与脚本提供稳定的命令行输出。

项目目前仍处于 1.0 之前阶段。CLI 能力已经较丰富，但在整体稳定化过程中，部分行为与契约仍可能演进。

## 平台状态

- **当前首发目标：** Windows x86_64。
- **CLI：** Rust CLI 工作区的技术目标是支持 Windows、Linux 与 macOS 源码构建，但面向终端用户的稳定多平台二进制分发渠道尚未正式发布。
- **GUI：** `envr-gui` 基于跨平台 Rust GUI / runtime 技术栈构建，并非按“仅 Windows”设计。原则上它应可随着当前 `iced` / `wgpu` / native dialog 栈运行于大多数桌面平台，但目前只有 Windows 打包与发布验证进入正式范围。
- **Linux/macOS：** 源码构建属于预期技术方向，但尚未对外承诺稳定打包发布与平台级支持。
- **runtime provider：** 各运行时的支持按 runtime 与宿主平台分别定义；有些 provider 是跨平台的，有些则目前仅支持 Windows 或仅部分支持。

当前 runtime 矩阵见 [`docs/runtime/platform-support-matrix.md`](docs/runtime/platform-support-matrix.md)，发布打包范围见 [`docs/release/README.md`](docs/release/README.md)。

## envr 可以做什么

- 在统一 runtime root 下安装与管理多种语言运行时。
- 通过 `envr use` 切换全局默认版本，并通过 `.envr.toml` 解析项目级 pin。
- 通过 shim 让工具稳定找到当前选中的运行时。
- 通过 `envr exec`、`envr run`、`envr env` 和 shell hook 在合并后的运行时 / 项目环境中执行命令。
- 通过远程索引缓存与 bundle 支持偏离线工作流。
- 默认输出人类可读文本，并通过 `--format json` 为自动化提供 JSON envelope。

## 支持的运行时

`envr` 当前包含多种 provider，包括 Node.js、Python、Java、Kotlin、Scala、Clojure、Groovy、Terraform、Deno、Bun、Dart、Flutter、Go、Rust、Ruby、Elixir、Erlang、PHP、.NET、Zig、Julia、Janet、C3、Babashka、SBCL、Haxe、Lua、Nim、Crystal、Perl、Unison、R、Racket、Elm、Gleam、PureScript、Odin、V 与 Luau。

具体支持情况会受到操作系统、架构与上游发布产物的影响。当前实现矩阵见 [`docs/runtime/platform-support-matrix.md`](docs/runtime/platform-support-matrix.md)。

## 安装

`envr` 目前还没有面向终端用户正式发布稳定的多平台安装通道。
当前推荐路径仍然是从源码构建；Windows 打包文档主要面向维护者准备发布产物。

### 从源码构建

Windows：

```powershell
cargo build --release -p envr-cli
.\target\release\envr.exe --help
```

类 Unix 系统：

```bash
cargo build --release -p envr-cli
./target/release/envr --help
```

当前工作区使用 Rust 2024 edition，并要求 Rust **1.88 或更高版本**（见 [`Cargo.toml`](Cargo.toml) 中的 `rust-version = "1.88"`）。更旧的本地工具链会在构建前直接失败。

### 发布打包状态

- 面向终端用户的安装包尚未被文档定义为稳定分发渠道。
- Windows zip / MSI / setup 等维护者打包说明见 [`docs/release/README.md`](docs/release/README.md)。
- 当前发布限制与已知问题见 [`docs/release/KNOWN-ISSUES.md`](docs/release/KNOWN-ISSUES.md)。

## 快速开始

```bash
# 查看命令与全局参数
envr --help

# 列出某个 runtime 的远程版本
envr remote node

# 安装一个运行时版本
envr install node 22.0.0

# 设置为全局默认
envr use node 22.0.0

# 查看当前选中版本
envr current node

# 在当前目录创建项目配置
envr init

# 在解析后的运行时/项目环境中执行命令
envr exec node -- node --version
```

可使用 `envr help shortcuts` 查看内置 argv 快捷词和命令别名。

## 核心命令

| 领域 | 命令 |
|---|---|
| Runtime 生命周期 | `install`, `use`, `list`, `current`, `uninstall`, `which`, `remote`, `doctor` |
| 项目环境 | `init`, `check`, `status`, `project`, `why`, `resolve`, `exec`, `run`, `env`, `template`, `shell`, `hook`, `deactivate` |
| 配置 | `config`, `alias`, `profile`, `import`, `export` |
| 数据与离线工作流 | `shim`, `cache`, `bundle`, `prune` |
| 诊断与工具 | `debug`, `diagnostics`, `completion`, `help`, `update` |

完整命令图谱与命令层级见 [`docs/cli/commands.md`](docs/cli/commands.md)。

## 自动化与 JSON 输出

大多数自动化导向命令支持全局 `--format json`：

```bash
envr --format json current node
```

JSON 输出被设计为稳定的 envelope，其中 `data` 部分按命令变化。可参考：

- [`docs/cli/output-contract.md`](docs/cli/output-contract.md)
- [`docs/cli/scripting.md`](docs/cli/scripting.md)
- [`docs/schemas/README.md`](docs/schemas/README.md)

## 配置、路径与缓存

- 用户设置通过 `envr config` 管理。
- 项目 pin 保存在 `.envr.toml`。
- 运行时安装、shim、缓存条目与远程索引保存在 runtime root 下。

相关文档：

- [`docs/cli/config.md`](docs/cli/config.md)
- [`docs/paths-and-caches.md`](docs/paths-and-caches.md)
- [`docs/cli/offline.md`](docs/cli/offline.md)
- [`docs/cli/bundle.md`](docs/cli/bundle.md)

## 文档导航

完整文档索引见 [`docs/README.md`](docs/README.md)。
中文入口见 [`docs/README.zh-CN.md`](docs/README.zh-CN.md)。

快捷入口：

- CLI 用法与配方：[`docs/cli/README.md`](docs/cli/README.md)
- 运行时支持与逐项说明：[`docs/runtime/README.md`](docs/runtime/README.md)
- 发布说明与已知问题：[`docs/release/README.md`](docs/release/README.md)
- 支持策略与提问方式：[`SUPPORT.zh-CN.md`](SUPPORT.zh-CN.md)
- 安全策略与漏洞报告：[`SECURITY.zh-CN.md`](SECURITY.zh-CN.md)
- 架构说明与 ADR：[`docs/architecture/README.md`](docs/architecture/README.md)
- QA 与回归资料：[`docs/qa/README.md`](docs/qa/README.md)
- 支持排障数据采集：[`docs/qa/diagnostics.md`](docs/qa/diagnostics.md)
- 历史重构资料：[`refactor docs/`](refactor%20docs/)

## 开发

常用检查：

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace
```

涉及 CLI 外部行为的改动还应遵循 [`CONTRIBUTING.md`](CONTRIBUTING.md)，尤其是 JSON contract 与治理检查项。

## 社区与项目策略

- 贡献流程与维护者检查项见 [`CONTRIBUTING.md`](CONTRIBUTING.md)。
- 社区行为预期见 [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md)。
- 一般问题、bug、功能建议与支持预期见 [`SUPPORT.md`](SUPPORT.md)。
- issue 入口行为见 [`.github/ISSUE_TEMPLATE/config.yml`](.github/ISSUE_TEMPLATE/config.yml)。
- 如怀疑存在安全漏洞，请不要公开提 issue，而应遵循 [`SECURITY.md`](SECURITY.md)。

## 项目状态

`envr` 正在持续演进，目标是形成稳定的 CLI 与 runtime-provider 架构。`docs/architecture/`、`refactor docs/` 与一些历史任务资料更多属于设计历史或实现规划，而非面向终端用户的使用文档。

## 许可证

本工作区采用 **Apache License 2.0** 或 **MIT** 双许可证，使用者可任选其一。详情见 [LICENSE](LICENSE)。
