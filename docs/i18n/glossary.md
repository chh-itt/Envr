# i18n 术语表（T913）

本文与 [`style-guide.md`](style-guide.md)、[`whitelist.md`](whitelist.md) 配套：规定 **zh-CN / en-US** 下用户可见文案的**首选译法、大小写与禁用同义词**。新增词条时请先更新本表，再改 `locales/*`。

---

## 1. 产品域（envr 核心概念）

| 术语（概念） | en-US（展示） | zh-CN（推荐） | 说明 |
|-------------|---------------|---------------|------|
| Runtime (language runtime) | runtime **/** runtimes | **运行时** | 指 Node/Python 等语言运行时整体，非单独一次 “run”。CLI 子命令名、语言键见白名单。 |
| Runtime root | runtime root | **运行时根目录** | 与 `ENVR_RUNTIME_ROOT`、`settings.toml` 中 paths 一致；禁止混用「根路径」「数据目录」指同一概念。 |
| Current selection | current | **当前（版本）** **/** **current** | 与 `current` 符号链接、CLI `current` 子命令对应；短标签可用「当前」。 |
| Version spec | version spec | **版本 spec** | 解析/匹配用的版本描述，非仅 `x.y.z`；中英文均可保留 **spec**。 |
| Pin / pinned | pin（如 project pin） | **项目 pin** **/** **pin** | 指 `.envr.toml` 中钉死的版本；说明句首可用「项目固定版本」作补充，UI 短文本优先 **pin**。 |
| Profile | profile | **profile**（专名）；说明可写 **配置方案** | `[profiles.*]` 块名；禁止与「用户资料」混淆。 |
| Shim | shim **/** shims | **Shim** **/** **Shims**（与 UI 一致） | 详见白名单；说明性长句可写「可执行转发/垫片」，但按钮/表头与现有 `locales` 一致。 |
| Mirror strategy | mirror (mode/strategy) | **镜像（策略）** | 模式取值 `official`/`auto`/`manual`/`offline` 为**保留英文**的产品枚举，见白名单。 |
| Diagnostics bundle | diagnostics (export/bundle) | **诊断包** **/** **诊断导出** | ZIP 内含 doctor JSON、环境摘要、日志等。 |
| Doctor | doctor | **doctor**（命令名）；文案可写 **诊断** | CLI 子命令名小写 `doctor`；中文说明可用「环境检查/诊断」。 |

---

## 2. GUI 专有条目

| 术语 | en-US | zh-CN | 说明 |
|------|-------|--------|------|
| Dashboard | Dashboard | **仪表盘** | 与 `gui.route.dashboard` 一致。 |
| Flavor (UI theme family) | Fluent / Liquid Glass / Material 3 | Fluent / Liquid Glass / Material 3（可加系统括号） | 产品名词，括号内标注平台时可本地化，名称本体保留英文。 |
| Smart / Exact mode | Smart / Exact | **智能（Smart）** **/** **精确（Exact）** | 安装/解析模式；括号内保留英文模式名便于对照文档。 |
| Health check | health check | **健康检查** | Dashboard 卡片标题与状态行统一用此译法。 |
| Downloads panel | downloads | **下载** | 浮动面板标题与入口按钮一致。 |

---

## 3. CLI / 配置域

| 术语 | en-US | zh-CN | 说明 |
|------|-------|--------|------|
| Settings file | `settings.toml` | `settings.toml` | 文件名不翻译；可说「设置文件」。 |
| Project config | `.envr.toml` | `.envr.toml` | 不翻译；可说「项目配置」。 |
| JSON envelope | JSON envelope / `success` `message` `data` | **JSON 信封**（说明用）；字段名保留英文 | 机器字段名（`list_installed` 等）不译，见白名单。 |
| Dry run | dry run | **试运行** **/** **dry run** | `prune` 等场景；优先与 `locales` 已有「试运行」一致。 |
| Cache kind | cache kind | **缓存类型** | 与 `cache clean` 的 KIND 参数说明一致。 |

---

## 4. 大小写与标点（统一点）

- **sentence case** 用于完整英文句子；按钮、短标签可与 `locales/en-US.toml` 保持标题式大小写一致。
- **zh-CN**：使用全角括号 `（）` 与中文标点；省略号用 `…`（与现有 GUI 字符串一致）。
- **代码/标识符**（语言键、`current`、镜像枚举值）：**不**因语言切换改变大小写。

---

## 5. 维护流程

1. 新功能涉及新概念：先在本表增加一行（en + zh + 说明），再添加 `locales` key。
2. 若术语应**永不翻译**：写入 [`whitelist.md`](whitelist.md)，本表可「仅概念说明」或省略 zh 列。
3. 与本用冲突时：**以 `locales/zh-CN.toml` / `en-US.toml` 与本文双重核对**后批量修正，并在本表更新「说明」记录决策日期或 PR。

---

## 6. 修订记录

| 日期 | 变更摘要 |
|------|----------|
| 2026-04-02 | 初版：对齐 T911/T912 已落地词条与 style-guide。 |
