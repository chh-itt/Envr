# CLI 文档

English | [简体中文](README.zh-CN.md)

本目录包含 `envr` CLI 的用户文档，以及自动化和维护者参考资料。

## 从这里开始

| 文档 | 受众 | 作用 |
|---|---|---|
| [`commands.md`](commands.md) | 用户 | 按生命周期、项目工作流、自动化与诊断分组的命令图谱。 |
| [`recipes.md`](recipes.md) | 用户 | 常见工作流的任务型示例。 |
| [`config.md`](config.md) | 用户 | `settings.toml`、`envr config`、mirror、路径与偏好设置。 |
| [`scripting.md`](scripting.md) | 用户 / CI | 在脚本与子进程工作流中使用 `envr`。 |
| [`offline.md`](offline.md) | 用户 / CI | 离线 index 与缓存行为。 |
| [`bundle.md`](bundle.md) | 用户 / CI | 可移植离线 bundle。 |
| [`output-contract.md`](output-contract.md) | 集成者 | 文本 / JSON 输出契约与错误 envelope 预期。 |

## 维护者参考

| 文档 | 作用 |
|---|---|
| [`automation-matrix.md`](automation-matrix.md) | 命令输出模式与自动化行为检查表。 |
| [`v1.0-definition.md`](v1.0-definition.md) | 面向 v1.0 目标的产品定义与验收范围。 |
| [`v1.0-metrics.md`](v1.0-metrics.md) | v1.0 准备度的建议指标与可观测信号。 |

## 文档状态

- 第一张表旨在对用户与外部集成者保持足够稳定。
- 维护者参考中可能包含规划中或愿景性的内容；请结合当前 `envr --help` 与测试确认实际行为。
