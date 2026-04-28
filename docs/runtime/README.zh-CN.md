# Runtime 文档

English | [简体中文](README.zh-CN.md)

本目录包含 `envr` 的 runtime 专题文档。

这里主要有两类文档：

1. 稳定的 runtime 说明，通常命名为 `<runtime>.md`
2. 面向维护者的规划说明，通常命名为 `<runtime>-integration-plan.md`

## 如何使用本目录

### 如果你是用户

建议先看：

- [`platform-support-matrix.md`](platform-support-matrix.md) — 当前按 OS / host 划分的支持情况
- 你关心的 runtime 文件，例如 [`zig.md`](zig.md)、[`deno.md`](deno.md) 或 [`flutter.md`](flutter.md)

稳定 runtime 文档通常会覆盖：

- `envr` 安装什么
- 主机 / 平台要求
- 常用命令
- 项目 pin 示例
- 安装布局与环境变量
- 缓存行为

### 如果你是贡献者

请配合对应的 `*-integration-plan.md` 文件使用。
这些规划文件记录实现细节、验收清单、上游 artifact 规则与 rollout 说明。
它们面向维护者，可能描述尚未完成或刻意延后的工作。

## 命名约定

| 模式 | 含义 |
|---|---|
| `<runtime>.md` | 面向用户的 runtime 行为 / 参考文档 |
| `<runtime>-integration-plan.md` | 面向维护者的集成规划或实现清单 |
| `platform-support-matrix.md` | 跨 runtime 支持总览 |

## 维护规则

变更 runtime 行为时：

1. 如果外部行为变化，更新面向用户的 `<runtime>.md`
2. 如果 host 支持变化，更新 `platform-support-matrix.md`
3. 如果对应的 `*-integration-plan.md` 仍然有效，也要同步更新或关闭
