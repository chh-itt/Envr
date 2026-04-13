# i18n 强制规范（T910）

本规范适用于 `envr` 全仓库（GUI + CLI + 文档中面向最终用户的可见文本）。目标是将 i18n 变成**默认约束**而非“可选优化”。

**输入依据**：`refactor docs/03-gui-设计.md`、`refactor docs/02-cli-设计.md`。

---

## 1. 总则（必须遵守）

- 任何**用户可见文本**必须来自 i18n key，不允许新增硬编码展示文本。
- 专业术语可保留原文，但必须进入术语表/白名单统一管理（由 T913 提供文档）。
- 新功能上线时，至少提供 `zh-CN` 与 `en` 两种文案；缺失任一视为不完整交付。
- 评审与 CI 对 i18n 违规执行阻断（T914 会补齐自动化规则）。

---

## 2. 适用范围

以下内容均属于“用户可见文本”：

- GUI：页面标题、导航、按钮、标签、提示、弹窗、空态、错误信息、下载状态。
- CLI：`--help` 文案、参数帮助、命令说明、标准输出提示、错误提示、诊断建议。
- 诊断与日志中的用户输出层：若文本会回显给用户，也必须走 i18n key。

以下内容不纳入翻译，但需统一写法：

- 协议/命令/环境变量/文件名（如 `PATH`、`ENVR_RUNTIME_ROOT`、`cargo`）。
- 语言运行时标识与版本号（如 `node`, `python`, `v20.11.1`）。

---

## 3. Key 设计规范

- 命名采用点分层级：`<domain>.<module>.<scene>.<item>`。
- key 只表达语义，不包含语言信息，不包含 UI 样式信息。
- 禁止同义重复 key；同一语义复用同一 key。
- 建议示例：
  - `gui.settings.title`
  - `gui.download.toast.success`
  - `cli.doctor.issue.root_not_writable`
  - `common.action.retry`

### 3.1 `locales/*.toml` 点号键约束（必须）

`[messages]` 里使用点号路径时，**同一个前缀不能既是字符串值又是更长 key 的父路径**。否则 TOML 解析失败（例如不能同时存在 `foo.bar = "…"` 与 `foo.bar.baz = "…"`）。

- **错误**：`gui.runtime.search_placeholder` + `gui.runtime.search_placeholder.python`
- **正确**：`gui.runtime.search_placeholder.default` + `gui.runtime.search_placeholder.python`
- **错误**：`cli.help.cmd.project` + `cli.help.cmd.project.add`
- **正确**：`cli.help.cmd.project.about` + `cli.help.cmd.project.add`

新增文案前先扫一眼是否会产生“父键字符串 + 子键”冲突；`envr-i18n-lint --write-locales` 会在写入前检测并报错。

---

## 4. 参数与复数规则

- 变量插值必须使用占位符，不允许字符串拼接（避免语序在多语言下出错）。
- 数量相关文本必须支持复数/量词策略（按语言规则实现）。
- 时间、大小、百分比等格式化应由统一函数处理，不在文案中手工拼接单位。

示例（示意）：

- `cli.cache.cleaned = "已清理 {count} 项缓存"`
- `en: "Cleaned {count} cache entries"`

---

## 5. 回退与缺失策略

- 运行时语言优先级：显式设置 > 系统语言 > 默认语言（`en` 或项目约定默认值）。
- 缺失 key 回退：当前语言缺失时回退到默认语言；默认语言仍缺失则显示 key 并记录告警。
- 禁止静默吞掉缺失 key。

---

## 6. 开发流程（DoD）

每个涉及用户文本的改动，提交前必须满足：

1. 新增/修改文本均使用 i18n key（`tr_key("…", zh, en)` 或 `cli_help.rs` 的 `tr(…)`），**中英回退字面量必须与最终 `locales` 语义一致**。
2. `zh-CN.toml` 与 `en-US.toml` key 集合完全一致；可运行 `cargo run -p envr-i18n-lint --locked` 校验（CI 同命令）。
3. 若大量 key 缺漏，可一次性从源码回退字面量合并进 locale：`cargo run -p envr-i18n-lint --locked -- --write-locales`（会按 key 排序重写两个文件；提交前请 `git diff` 审阅）。
4. 本地运行对应模块测试/冒烟，确认无 key 缺失与回退异常。
5. PR 描述包含 i18n 影响范围（GUI/CLI、涉及 key 前缀）。

**扫描范围（lint）**：`crates/envr-gui/src`、`envr-cli/src`、`envr-core/src`、`envr-shim/src`。在其它 crate 使用 `tr_key` 时需同步扩展 lint 扫描路径。

---

## 7. Code Review 检查清单

- 是否存在新增硬编码用户可见文本？
- 新 key 是否符合命名规范且无重复语义？
- 插值、复数、单位格式是否由统一机制处理？
- 双语资源是否齐全、语义一致？
- 缺失 key 的 fallback 行为是否可观测？

任一项失败，评审应要求修改后再合并。

---

## 8. 与后续任务的关系

- T911/T912：按本规范实施 GUI/CLI 全量迁移。
- T913：术语表与白名单作为本规范的配套约束（见 [`glossary.md`](glossary.md)、[`whitelist.md`](whitelist.md)）。
- T914：`envr-i18n-lint` 校验 zh/en 对等、代码引用 key 存在、GUI/CLI 禁止 `i18n::tr(`；`--write-locales` 用于从源码批量补全缺键。
- T915：以本规范作为中英文回归的验收基线。
