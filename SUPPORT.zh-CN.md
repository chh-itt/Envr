# 支持

感谢使用 `envr`。

本文说明在哪里提问、在哪里报告 bug，以及如何报告安全问题。

## 在开 issue 之前

请先查看：

- 根目录 [`README.zh-CN.md`](README.zh-CN.md)
- 文档索引 [`docs/README.zh-CN.md`](docs/README.zh-CN.md)
- CLI 命令文档 [`docs/cli/`](docs/cli/)
- runtime 支持说明 [`docs/runtime/`](docs/runtime/)
- 已知问题 [`docs/release/KNOWN-ISSUES.md`](docs/release/KNOWN-ISSUES.md)

如果你报告的是 CLI 契约或自动化回归，还请同时查看 [`CONTRIBUTING.md`](CONTRIBUTING.md) 和 [`docs/cli/output-contract.md`](docs/cli/output-contract.md)。

## 获取帮助

可在 issue tracker 中提交以下内容：

- 安装问题
- runtime 安装失败
- 平台支持问题
- 文档不清楚
- 看起来有问题、但尚未确认是 bug 的行为
- 功能请求与易用性反馈

提问时请尽量包含：

- 你的操作系统与架构
- `envr` 版本或 commit
- 你是如何安装 `envr` 的
- 相关 runtime 的类型与版本
- 你执行的完整命令
- 你期望得到的结果
- 实际得到的结果
- 是否使用了自定义 runtime root、mirror、离线 / 缓存流程，或项目本地 `.envr.toml`

如果是环境、shim、PATH、mirror、离线或缓存问题，也请提供 `envr doctor` 输出；必要时再附上经过脱敏的 `envr diagnostics export` 摘要。参见 [`docs/qa/diagnostics.md`](docs/qa/diagnostics.md)。

## 报告 bug

非安全问题请正常提交 GitHub issue。

通常有帮助的信息包括：

- 复现步骤
- 预期行为
- 实际行为
- 相关日志或诊断信息
- 是否能在干净的 runtime root 或全新项目目录中复现

如果 bug 涉及机器可读输出，请说明你是否使用了：

- `--format json`
- `--porcelain`
- `--quiet`

## 报告安全问题

请不要在公开 issue 中报告安全漏洞。

例如：

- 恶意归档解压行为
- 校验和或完整性校验绕过
- 不安全的 mirror 或远端元数据信任行为
- 可能导致非预期执行的 shim 或 `PATH` 行为
- 跨越预期信任边界的本地配置处理
- secrets 或环境数据的意外暴露

请按照 [`SECURITY.md`](SECURITY.md) 中的私下报告流程提交。

## 范围预期

`envr` 是 runtime 管理器，因此支持请求通常会落在以下几类中：

- `envr` 自身 bug
- 不受支持的主机 / 平台组合
- 上游 runtime 打包变化
- mirror / index / 网络问题
- 项目配置问题
- 本地环境或 shell 集成问题

如果根因属于上游或超出支持范围，维护者可能会重新分类或转交。

## 响应预期

支持工作为 best effort。

- 问题与 bug 报告不一定会立即得到回复。
- 安全报告请遵循 [`SECURITY.md`](SECURITY.md) 中的时间预期。
- pre-1.0 阶段的行为仍可能变化，尤其是在高级自动化与较新的 runtime provider 上。

## 文档反馈

如果主要问题是文档令人困惑、不完整或已过时，欢迎直接开 issue。
文档问题也是有效的支持问题。
