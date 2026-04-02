# envr GUI 设计

## 1. GUI 定位与性能目标

- 面向“非 CLI 重度用户”的主入口。
- 不是独立业务系统，而是 `envr-core` 的图形化壳层。
- 所有状态变更必须通过 `core` 服务，杜绝 GUI 自己实现安装逻辑。
- 平台视觉风格：
  - Windows：采用 Fluent Design（亚克力材质、柔和模糊、圆角、轻量阴影与过渡动画）。
  - macOS：采用 Liquid Glass 风格（半透明玻璃效果、系统一致的模糊与动效节奏）。
  - Linux：采用 Material 3 风格（动态色彩、层级系统、现代化组件），保证不同桌面环境下的一致性。
- 性能目标（参考 Win11 + iced 0.14 + gl 后端）：
  - 冷启动时间：≤ 800ms（首次启动到主窗口可交互）。
  - 热启动时间：≤ 400ms（二次启动）。
  - 页面切换/加载时间：≤ 150ms 完成主要内容渲染。
  - 常规操作响应延迟：≤ 50ms（点击到 UI 反馈）。
  - 帧率：常规操作下维持 ≥ 60 FPS，无明显卡顿。
  - 内存占用峰值：常驻 < 40MB（非极端场景），大任务期间的峰值通过监控记录并控制回落。
  - CPU 占用：空闲 < 3%，典型操作 < 15%。

## 2. 视图模块（第一阶段）

- 采用单页应用（SPA）架构，主窗体固定为“左导航 + 右内容区”。
- 左侧导航（固定）：`仪表盘 / 运行时 / 设置 / 关于`。
- 右侧内容区按导航切换，保持单窗口、单状态树，减少上下文切换。

### 2.1 仪表盘（Dashboard）建议内容

- 运行时总览卡片：已安装数量、活跃版本数量、磁盘占用总量。
- 快速健康状态：PATH/shim 状态、下载服务状态、镜像连通性。
- 最近任务：最近安装/卸载/失败重试记录（可点击跳转到运行时页）。
- 推荐操作：常用入口（安装推荐 LTS、清理缓存、打开配置目录）。

### 2.2 运行时页面（核心页面）

- 顶部：运行时横向导航（Node/Python/Go/Java/Rust/PHP/Deno/Bun）。
- 中部：对应运行时设置区域（可折叠，默认折叠）。
  - 该区域的具体配置项优先参考“未完成的重构项目”，保持精练实用。
- 下部：版本列表区。
  - 支持“智能（Smart）/精确（Exact）”两种模式。
  - 智能模式：按版本组展示（如 Node 按 major，Python 按 major.minor）。
  - 精确模式：展示完整版本明细，支持搜索与筛选。
- 版本操作规则（必须一致）：
  - 未安装：仅显示“安装”按钮。
  - 已安装未使用：显示“使用/卸载”按钮。
  - 已使用（Active）：禁用该行可变更按钮（至少禁用卸载与重复使用）。

### 2.3 下载面板（防挤压方案）

- 下载面板采用“悬浮小面板”而不是插入式挤压布局。
- 默认停靠在左下角（避免影响主内容阅读区域）。
- 支持拖拽、隐藏/展开、记忆上次位置与展开状态。
- 面板状态变化不触发布局重排，避免内容跳动（layout jump）。

## 3. 状态管理原则

- UI 状态分三类：
  - `PersistentState`（设置与用户偏好）
  - `RuntimeState`（当前安装、当前版本）
  - `TaskState`（下载任务、执行进度）
- 异步任务统一走消息总线（command -> event -> reducer）。
- 页面渲染稳定性要求：
  - 避免因下载面板显隐导致主内容区域宽高突变。
  - 对高频状态更新做节流（如下载进度刷新频率限制）。
  - 使用骨架屏/占位块替代突然插入，减少视觉闪烁。

## 4. 与 CLI 的一致性约束

- 同一业务动作，GUI 与 CLI 必须触发同一 `core` API。
- 同一错误代码，GUI 与 CLI 用同一错误映射表。
- GUI 显示语义必须可追溯到 CLI/JSON 输出字段。

## 5. i18n 与可观测性

- i18n key 与 CLI 共用命名空间前缀，避免重复词条。
- 关键用户动作埋点：
  - 安装开始/成功/失败
  - 镜像切换
  - shim 启用状态变更
- 日志支持导出，便于用户反馈问题。

## 6. UI 技术建议

- 可继续使用 `iced`（已有积累），但减少自定义复杂组件数量。
- 组件层 `envr-ui` 只放纯表现组件，不放业务逻辑。

## 7. 设计 Token 落地（单一真相，`envr_ui::theme`）

**代码位置**：`crates/envr-ui/src/theme/tokens.rs`（色板锚点、`ThemeTokens`）、`presets.rs`（三风味 × 明暗）、`shell` 子模块窗口常量。

### 7.1 色板与语义（sRGB 锚点）

| 角色 | 浅色示例 | 说明 |
|------|----------|------|
| 页面背景 | `#F9F9F9` | `SURFACE_PAGE_LIGHT` |
| 卡片表面 | `#FFFFFF` | `SURFACE_CARD_LIGHT` |
| 主文字 | `#1E1E1E` | `TEXT_PRIMARY_LIGHT` |
| 次要文字 | `#595959`（相对卡片 ≥4.5:1） | `TEXT_MUTED_LIGHT` |
| Fluent 品牌主色 | `#0078D4` | `BRAND_PRIMARY_FLUENT` |
| Liquid 品牌主色 | `#0A84FF` | `BRAND_PRIMARY_LIQUID` |
| 语义 成功 / 警告 / 危险 | `#2E7D32` / `#FBC02D` / `#D32F2F` | `SEMANTIC_*` |

暗色方案见同模块 `*_DARK`、`BRAND_*_DARK`。用户自定义强调色通过设置合并进 `tokens_for_appearance(..., accent)`。

### 7.2 间距（8pt 网格）

静态表 `SPACING_8PT`：`xs=4, sm=8, md=12, lg=16, xl=24, xxl=32`（逻辑 px）。主内容区外边距基准为 **`ThemeTokens::content_spacing()`**（=`md`）。

### 7.3 圆角与控件高度（按风味预设）

`ThemeTokens` 内：`radius_sm/md/lg`、`control_height_primary`（约 36）、`control_height_secondary`（约 32）。卡片圆角 **`card_corner_radius()` = 12**；下载浮层与卡片一致 **`download_panel_corner_radius()`**。

### 7.4 列表与虚拟化

- 行高 **`list_row_height()` = 44**（与 `min_interactive_size` 默认一致，满足点击区域基线）。
- 骨架行数 **`list_skeleton_rows()` = 5**。
- 虚拟化阈值 **`list_virtualize_min_rows` = 28**（安装列表等）。

### 7.5 动效（`MotionTokens`）

默认 **`standard_ms = 200`**、**`emphasized_ms = 300`**，**easing** `cubic-bezier(0.2, 0, 0, 1)`（`easing_standard` 数组）。系统/环境「减少动效」时 GUI 可将时长置 0（见 `envr_platform::a11y`）。

### 7.6 排版

`typography()` 字号由基准 × **`content_text_scale`**（默认 1.0，GUI 可通过环境变量 **`ENVR_UI_SCALE`** 限制在约 0.85～1.35）得出 page_title / section / body 等 ramp。

### 7.7 Shell 窗口（`shell` 模块）

| 常量 | 值 |
|------|-----|
| `WINDOW_DEFAULT_W×H` | 1200 × 720 |
| `WINDOW_MIN_W×H` | 960 × 600 |
| `CONTENT_MAX_WIDTH` | 960 |
| 侧栏宽度（方法） | `sidebar_width()` = **240** |

### 7.8 下载浮层与持久化

- 卡片右宽常量（与持久化公式一致）：**320px**（`envr-gui` `DOWNLOAD_PANEL_SHELL_W`）。
- `settings.toml`：`gui.downloads_panel` 含 `visible` / `expanded` / `x` / `y` 及归一化 **`x_frac` / `y_frac`**（相对客户区减内边距与面板宽），便于 DPI 与窗口尺寸变化。

### 7.9 无障碍相关环境变量

| 变量 | 作用 |
|------|------|
| `ENVR_UI_SCALE` | 界面正文字号缩放 |
| `ENVR_REDUCE_MOTION` / `ACCESSIBILITY_REDUCED_MOTION` | 强制减少动效 |
| Windows | `SPI_GETUIEFFECTS` 关闭时视为减少动效（`envr_platform::a11y`） |

### 7.10 空态 / 错误态呈现（GUI-070）

- `envr-gui`：`view/empty_state.rs` 提供几何弱插图 + 图标 + 分级文案的 **`illustrative_block`** / **`illustrative_block_compact`**；仪表盘、运行时列表空、下载面板空态与全局错误条统一使用该模式，避免单行灰字。

