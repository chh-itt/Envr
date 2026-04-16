# 数据目录、设置持久化与缓存（envr）

下文中的「数据根」指平台默认的 envr 数据目录；**运行时根**（runtime root）可与数据根不同（见下文优先级）。

## 1. 平台数据根（`EnvrPaths::runtime_root` 的默认基准）

由 `crates/envr-platform/src/paths.rs` 的 `base_dir_for` 决定：

| 平台 | 默认路径 |
|------|----------|
| Windows | `%APPDATA%\envr`，若无则 `%LOCALAPPDATA%\envr`，再否则 `%USERPROFILE%\.envr` |
| macOS | `~/Library/Application Support/envr` |
| Linux | `$XDG_DATA_HOME/envr`，否则 `~/.local/share/envr` |

若设置环境变量 **`ENVR_ROOT`**（非空），则**整棵数据树**以该路径为根，覆盖上表。

在数据根下，`EnvrPaths::new` 固定子目录：

| 子目录 | 用途 |
|--------|------|
| `config/` | 配置目录 |
| `cache/` | 通用缓存根（与「运行时」下载缓存不同，见下） |
| `logs/` | 日志 |

## 2. GUI / CLI 共用设置文件

| 文件 | 内容 |
|------|------|
| `{数据根}/config/settings.toml` | 主设置：镜像、下载并发、外观、语言、`paths.runtime_root`、GUI 下载面板位置等（`envr_config::Settings`） |

保存路径由 `envr_config::settings::settings_path_from_platform` 给出，与 `EnvrPaths::settings_file` 一致。

## 3. 别名

| 文件 | 内容 |
|------|------|
| `{数据根}/config/aliases.toml` | 用户命令别名（`envr_config::aliases`） |

## 4. 运行时根（安装的运行时、shim、Node 索引缓存）

**有效运行时根**解析顺序（`envr_config::settings::resolve_runtime_root`）：

1. 环境变量 **`ENVR_RUNTIME_ROOT`**（非空）
2. `settings.toml` 中 **`paths.runtime_root`**（非空）
3. 平台默认的 **`EnvrPaths::runtime_root`**（即第 1 节数据根）

安装的运行时、当前版本链接、shim 等均在此根下（例如 `runtimes/node/...`、`shims/...`），与 CLI 一致。

## 5. Node.js 远程索引与 GUI 列表缓存

目录：**`{运行时根}/cache/node/`**（`NodePaths::cache_dir`）。

| 文件（示例） | 说明 |
|--------------|------|
| `index_body_{16位十六进制}.json` | 上游 `index.json` 正文缓存；十六进制为 `index_json_url` 的 FNV-1a 指纹 |
| `remote_majors_{os}_{arch}.json` | 远程 major 键列表（字符串数组），TTL 由 `ENVR_NODE_REMOTE_CACHE_TTL_SECS` 控制 |
| `remote_latest_per_major_{os}_{arch}.json` | 每 major 一条「当前平台最新补丁」版本号（字符串数组），同一 TTL |
| `{version}/...` | 该版本安装包下载临时/缓存文件（随安装流程写入） |

TTL 环境变量：**`ENVR_NODE_REMOTE_CACHE_TTL_SECS`**（默认 86400；`0` 表示不读磁盘缓存、每次拉网）。

## 6. 项目级配置（非全局）

项目目录中可存在 **`envr.toml`** / **`envr.local.toml`** 等（见 `envr_config::project_config`），用于项目内运行时版本等；路径随项目，不在全局数据根下。

## 7. GUI 仅内存或随 settings 持久化

| 状态 | 持久化位置 |
|------|------------|
| 下载浮动面板可见性、展开、位置 | `settings.toml` → `[gui.downloads_panel]` |
| 设置页草稿 | 保存时写入 `settings.toml` |
| 主窗口布局（若实现） | 以 `settings.toml` 或专用字段为准（当前以代码为准） |

---

**速记**：全局配置在 **`{数据根}/config/settings.toml`**；**运行时与 Node 索引缓存**在 **`{运行时根}/`**，其中 Node 的索引与列表缓存在 **`{运行时根}/cache/node/`**。

## 8. 缓存读写契约（runtime remote 列表类缓存）

本节约束的是一类“可重建”的磁盘缓存（例如 `remote_latest_per_major_*.json` 等字符串数组），目标是：**稳定**、**可恢复**、并且避免坏缓存长期污染行为。

- **写入（durability + atomic-ish）**：使用 `envr_platform::fs_atomic::write_atomic`（或带备份的 `write_atomic_with_backup`），遵循「写临时文件 → fsync → rename → 尽力 fsync 父目录」模式，避免崩溃/断电导致的半写入文件。
- **读取（TTL + 解析 + 校验 + 自愈）**：统一通过 `envr_platform::cache_recovery::read_json_string_list`：
  - **TTL 未命中**：直接视为无缓存（返回 `None`），走网络刷新。
  - **TTL 命中但读取/反序列化失败**：视为坏缓存，**删除该文件**，走网络刷新。
  - **TTL 命中但数据不合格**：由调用方提供 `validate`（例如“非空”“至少 N 个元素”），不合格同样 **删除该文件**，走网络刷新。
- **TTL=0 约定**：所有 `*_REMOTE_CACHE_TTL_SECS=0` 表示 **不读磁盘缓存**（每次都拉网刷新）。

这套契约的核心收益是：坏缓存不会“等到 TTL 才修复”，而是读到异常/不合格就立刻清理，从而让下一次读取自然触发重建。
