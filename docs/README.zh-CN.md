# 文档导航

[English](README.md) | 简体中文

本目录包含 `envr` 的公开文档与维护者文档。
如果你第一次接触该项目，建议先阅读根目录 [`README.zh-CN.md`](../README.zh-CN.md)；如需英文默认入口，可看 [`../README.md`](../README.md)。

## 信息架构

文档按受众与稳定性组织：

| 层级 | 受众 | 稳定性 | 位置 |
|---|---|---|---|
| 产品文档 | 用户、运维、CI 作者 | 应尽量与当前行为一致 | [`cli/`](cli/)、[`runtime/*.md`](runtime/)、[`paths-and-caches.md`](paths-and-caches.md)、[`release/`](release/) |
| 集成契约 | 工具作者、维护者 | 对测试与脚本相对稳定 | [`cli/output-contract.md`](cli/output-contract.md)、[`schemas/`](schemas/) |
| 贡献者文档 | 贡献者与维护者 | 当前流程指引 | [`../CONTRIBUTING.md`](../CONTRIBUTING.md)、[`qa/`](qa/)、[`i18n/`](i18n/)、[`perf/`](perf/) |
| 设计历史与规划 | 维护者 | 可能是历史、草稿或部分已实现 | [`architecture/`](architecture/)、[`runtime/*-integration-plan.md`](runtime/)、[`../refactor docs/`](../refactor%20docs/) |

## 推荐阅读路径

### 终端用户

- 命令总览：[`cli/README.md`](cli/README.md)、[`cli/commands.md`](cli/commands.md)
- 常见工作流：[`cli/recipes.md`](cli/recipes.md)
- 配置：[`cli/config.md`](cli/config.md)
- 离线使用与 bundle：[`cli/offline.md`](cli/offline.md)、[`cli/bundle.md`](cli/bundle.md)
- 路径、缓存与 runtime root 布局：[`paths-and-caches.md`](paths-and-caches.md)
- 平台 / runtime 支持范围：[`runtime/README.md`](runtime/README.md)、[`runtime/platform-support-matrix.md`](runtime/platform-support-matrix.md)
- 发布说明与已知问题：[`release/README.md`](release/README.md)
- 支持与 issue 报告：[`../SUPPORT.md`](../SUPPORT.md)
- 安全策略与漏洞报告：[`../SECURITY.md`](../SECURITY.md)

### 脚本与 CI 作者

- CLI 输出契约：[`cli/output-contract.md`](cli/output-contract.md)
- JSON schema：[`schemas/README.md`](schemas/README.md)
- 脚本工作流：[`cli/scripting.md`](cli/scripting.md)
- 离线与 bundle 工作流：[`cli/offline.md`](cli/offline.md)、[`cli/bundle.md`](cli/bundle.md)

### 贡献者

- 贡献流程：[`../CONTRIBUTING.md`](../CONTRIBUTING.md)
- 架构索引：[`architecture/README.md`](architecture/README.md)
- Runtime 集成资料：[`runtime/README.md`](runtime/README.md)、[`architecture/new-runtime-playbook.md`](architecture/new-runtime-playbook.md)
- QA 文档：[`qa/README.md`](qa/README.md)
- i18n 文档：[`i18n/README.md`](i18n/README.md)
- 性能资料：[`perf/README.md`](perf/README.md)

## 目录概览

- [`cli/`](cli/) — CLI 行为、命令参考、输出契约、配置、脚本、offline / bundle 说明。
- [`runtime/`](runtime/) — 各 runtime 用户说明、平台矩阵与集成规划。
- [`release/`](release/) — release notes、平台发布说明、已知问题与打包说明。
- [`architecture/`](architecture/) — 设计说明、ADR、迁移计划与 blueprint。
- [`perf/`](perf/) — 性能资料与调查记录。
- [`qa/`](qa/) — 回归、缺陷分诊与诊断复现资料。
- [`i18n/`](i18n/) — 术语表、风格指南与白名单。
- [`schemas/`](schemas/) — schema 说明与生成的契约引用。

## 文档成熟度规则

- 面向用户的文档应描述**当前行为**，而不是未来计划。
- 以 `*-integration-plan.md`、`*-blueprint.md`、`*-draft.md` 结尾的文件，以及 `architecture/` 下的大多数内容，主要面向维护者。
- 如果规划文档与用户文档冲突，应先结合实现与测试判断，再以当前用户文档为准。
- 如果实现改变了外部行为，应在同一个改动中同步更新产品文档。

如果不确定从哪里开始，请先引导用户阅读根目录 [`README.zh-CN.md`](../README.zh-CN.md)，并将草稿 / 历史资料隐藏在贡献者入口之后。
