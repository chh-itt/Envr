# i18n 专业术语白名单（T913）

下列条目在 **zh-CN**（及一般非英文 UI）中**允许保留原文或固定拉丁形式**，不视为 i18n 违规。新增条目须注明**范围**与**原因**，并同步 [`glossary.md`](glossary.md) 中相关说明。

**原则**（与 [`style-guide.md`](style-guide.md) §2 一致）：用户能理解、稳定性（脚本/文档/搜索）、或业界习惯。

---

## 1. 协议 · 环境变量 · 路径形态

| 条目 | 保留形式 | 范围 | 原因 |
|------|----------|------|------|
| `PATH` | `PATH` | CLI/GUI 说明 | 系统环境与排障通用符号 |
| `JAVA_HOME` | `JAVA_HOME` | CLI `env`、文档 | 标准 JVM 变量名 |
| `GOPROXY` | `GOPROXY` | 设置/运行时设置 | Go 生态固定名 |
| `ENVR_*` | `ENVR_RUNTIME_ROOT`、`ENVR_PROFILE`、`ENVR_OUTPUT_FORMAT` 等 | CLI/GUI/诊断 | 产品前缀 + 稳定契约 |
| `NO_COLOR` | `NO_COLOR` | CLI `--no-color` 说明 | 通用约定 |
| `cargo` / `npm` / `pip` 等 | 工具原名 | 错误提示、示例 | 命令名 |

---

## 2. 文件名与配置键

| 条目 | 保留形式 | 范围 |
|------|----------|------|
| `settings.toml` | 字面文件名 | 所有用户可见提及 |
| `.envr.toml`、`.envr.local.toml` | 字面文件名 | 同上 |
| `config/aliases.toml` | 路径片段说明 | CLI alias 帮助 |
| TOML 表键 / JSON 字段 | 如 `download.max_concurrent_downloads`、`success`、`runtimes` | 校验错误、JSON 信封、日志中的「用户可见」技术层 |
| `doctor.json`、`system.txt`、`environment.txt` | 诊断包内文件名 | CLI diagnostics 帮助 |

---

## 3. 运行时语言键与工具名

| 条目 | 保留形式 | 说明 |
|------|----------|------|
| `node`, `python`, `java`, `go`, `rust`, `php`, `deno`, `bun` | 小写标识符 | CLI 参数、`kind`、列表列名 |
| `npm`, `npx`, `pip`, `javac`, `bunx` 等 | 小写 | `which` 错误提示示例 |

版本号、`v20.x`、构建号等 **永不翻译**。

---

## 4. 镜像与网络策略枚举

| 条目 | 保留形式 | 说明 |
|------|----------|------|
| `official`, `auto`, `manual`, `offline` | 小写英文 | `settings`/镜像策略 UI 与存储值一致 |
| 镜像 ID（如 `cn-1`） | 按配置原样 | 与 envr-mirror 预设一致 |

---

## 5. CLI 子命令与机器 message token

| 条目 | 保留形式 | 说明 |
|------|----------|------|
| `envr` | 产品名 | 帮助模板、`envr:` 前缀 |
| 子命令名 | `install`, `doctor`, `diagnostics export`, … | `--help` 树与脚本解析 |
| JSON `message` 成功/流程 token | `list_installed`, `doctor_ok`, `project_config_ok`, `child_completed`, … | **契约**：自动化依赖固定英文 token；自然语言在 `data`/stderr 等层 |

---

## 6. UI 与品牌名词

| 条目 | 保留形式 | 说明 |
|------|----------|------|
| `Fluent`, `Liquid Glass`, `Material 3` | 英文 | 设计体系专名；可在括号内加平台 |
| `iced` | 英文 | 字体/技术说明（若对用户可见） |
| **Shims**（表头） | `Shims` | 与当前 `gui.label.shims` 一致；概念见 glossary |

---

## 7. 刻意不译的 snippet

| 条目 | 保留形式 | 说明 |
|------|----------|------|
| Shell 片段中的 `export …=` | 原样 | `env` 子命令输出 |
| `--profile`, `--format`, `--execute` 等 | 原样 | help 与示例 |
| `CLI`、`GUI` | 大写缩写 | 技术读者向说明句 |

---

## 8. 审计与变更

- **Code Review**：若 PR 在 zh-CN 字符串中引入**新**英文专名，须在本表登记或归入 glossary 译法，避免「随手英文」。
- **T914**：可将「非 ASCII 文案中含未在白名单的连续拉丁词」等作为启发式（具体规则以后续脚本为准）。

---

## 9. 修订记录

| 日期 | 变更摘要 |
|------|----------|
| 2026-04-02 | 初版：对齐 T911/T912 与 style-guide §2。 |
