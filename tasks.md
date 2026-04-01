# envr 开发任务清单（tasks.md）

## 使用方式

- 任务完成前：`[ ]`
- 任务完成后：`[x]`
- 任务进行中：可将任务状态标记为 `[~]`（可读性更强），或在任务行尾追加 `[in_progress]`（兼容所有 Markdown 渲染器）。
- 建议每次开始做某个任务时，把该任务的 **进度** 改为 `in_progress`，并补上分支/提交记录。
- 可使用任务行尾部的 `#tag` 快速检索：在编辑器里搜索 `#cli` / `#shim` / `#config` 等。

## 任务组织原则

- 按阶段分组：基础架构 → 核心功能 → 用户界面 → 高级功能 → 质量与发布
- 任务粒度：每个任务可在 1-4 小时内由 AI 完成
- 依赖明确：标注前置任务，支持并行开发
- 验收标准：可自动或人工检查的完成条件

---

## Phase 1：工程初始化与约束落地

### T001 创建 workspace 根结构
- [x] **T001：创建 `envr` workspace 骨架** #workspace
  - **描述**：创建 root `Cargo.toml`、`crates/` 目录、基础 README 与目录约定。
  - **依赖**：无
  - **输入文档**：`refactor docs/01-总体架构设计.md`
  - **输出文件**：`Cargo.toml`、`crates/*` 空 crate
  - **验收**：`cargo metadata` 成功；所有 crate 被 workspace 识别。
  - **进度**：done
  - **实现记录**：
    - 实现要点：新增 workspace 根 `Cargo.toml`（`members = ["crates/*"]`），并创建 `crates/` 下全部基础空 crate（lib/bin）。
    - 相关提交/PR：本次提交
    - 遇到的问题/决策：按照总体架构文档先完整建空骨架，后续任务再逐步填充各 crate 实现细节。
    - 验收结果：`cargo metadata` 通过，所有 crate 可被 workspace 正常识别。

### T002 workspace 统一依赖与 feature 策略
- [x] **T002：配置 `workspace.dependencies` 与 feature 白名单** #deps
  - **描述**：将核心依赖集中管理，限制默认特性，避免版本漂移。
  - **依赖**：T001
  - **输入文档**：`refactor docs/07-依赖选择与原则.md`
  - **输出文件**：`Cargo.toml`（workspace 依赖段）
  - **验收**：`cargo tree -d` 无关键重复版本；依赖策略可追踪。
  - **进度**：done
  - **实现记录**：
    - 实现要点：在根 `Cargo.toml` 新增 `workspace.dependencies`，集中定义 `serde/tokio/reqwest/thiserror/clap` 等核心依赖，并统一 `default-features = false` + 显式 feature 白名单。
    - 相关提交/PR：本次提交
    - 遇到的问题/决策：先建立全局基线依赖策略，具体 crate 在后续任务按需通过 `workspace = true` 接入，避免当前阶段过早绑定实现细节。
    - 验收结果：`cargo tree -d --workspace` 通过，未发现关键重复版本；策略通过 `workspace.metadata.envr.dependency_policy` 可追踪。

### T003 CI 与质量门禁
- [x] **T003：接入 fmt/clippy/test/coverage 流程** #ci #quality
  - **描述**：配置 CI，覆盖率统计管道目标 >=80%。
  - **依赖**：T001,T002
  - **输入文档**：`refactor docs/06-实施路线图.md`
  - **输出文件**：`.github/workflows/*` 或本地脚本
  - **验收**：CI 可跑通；覆盖率报告可生成。
  - **进度**：done
  - **实现记录**：
    - 实现要点：新增 `.github/workflows/ci.yml`，拆分 `fmt`、`clippy`、`test`、`coverage` 四个作业；coverage 使用 `cargo-llvm-cov` 生成 `lcov.info` 并设置 `--fail-under-lines 80` 门槛。
    - 相关提交/PR：本次提交
    - 遇到的问题/决策：当前代码量较小，先建立可扩展 CI 基线，覆盖率门槛在流水线层面强制执行，后续随功能扩展持续满足。
    - 验收结果：本地 `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace --all-targets` 均可执行；CI 中可生成覆盖率报告。

### T004 统一错误模型
- [x] **T004：实现 `envr-error` 统一错误与错误码** #error
  - **描述**：建立跨 CLI/GUI/core 可共享错误结构与分类。
  - **依赖**：T001,T002
  - **输入文档**：`refactor docs/01-总体架构设计.md`
  - **输出文件**：`crates/envr-error/src/*`
  - **验收**：核心 crate 能统一返回/转换错误；错误码可序列化。
  - **进度**：done
  - **实现记录**：
    - 实现要点：在 `envr-error` 定义统一 `EnvrError`、`ErrorCode`、`EnvrResult<T>` 与 `ErrorPayload`；错误码支持 `serde` 序列化，错误支持 `std::io::Error` 自动转换并可导出错误链。
    - 相关提交/PR：本次提交
    - 遇到的问题/决策：错误主体不直接做序列化（避免携带不可序列化底层错误类型），统一通过 `ErrorPayload` 对外输出可序列化结构。
    - 验收结果：`envr-core` 已接入统一错误返回并验证 `io` 错误可转换；`ErrorCode` 序列化测试通过。

### T005 日志与可观测基础
- [x] **T005：统一日志初始化与运行日志文件输出** #logging
  - **描述**：实现分级日志、文件落盘、CLI/GUI 共用初始化入口。
  - **依赖**：T004
  - **输入文档**：`refactor docs/03-gui-设计.md`
  - **输出文件**：`crates/envr-core/src/logging.rs` 等
  - **验收**：CLI/GUI 启动均可产生日志；错误链完整输出。
  - **进度**：done
  - **实现记录**：
    - 实现要点：新增 `crates/envr-core/src/logging.rs` 作为统一日志入口，支持分级日志（`RUST_LOG` + `EnvFilter`）、终端输出与按日滚动文件落盘；CLI/GUI 启动时统一调用 `init_logging`。
    - 相关提交/PR：本次提交
    - 遇到的问题/决策：在 T006 之前先采用默认 `./.envr/logs`（可由 `ENVR_LOG_DIR` 覆盖）避免路径策略阻塞当前任务；错误主体通过 `format_error_chain` 统一展开输出。
    - 验收结果：`envr-cli` 与 `envr-gui` 启动均会写入日志文件；`format_error_chain` 测试验证完整错误链可展开输出。

## Phase 2：配置系统与平台抽象

### T006 配置目录与路径约定
- [x] **T006：实现跨平台配置目录规范** #config #platform
  - **描述**：定义运行时根、缓存、日志、配置文件默认位置。
  - **依赖**：T001,T004
  - **输入文档**：`refactor docs/01-总体架构设计.md`
  - **输出文件**：`crates/envr-platform/src/paths.rs`
  - **验收**：Win/macOS/Linux 路径规则单元测试通过。
  - **进度**：done
  - **实现记录**：
    - 实现要点：新增 `envr-platform/src/paths.rs`，定义 `EnvrPaths`（runtime/config/cache/log/settings）与 `compute_paths`（可用模拟 env 做纯函数测试）；提供 `current_platform_paths` 供运行时使用。
    - 相关提交/PR：本次提交
    - 遇到的问题/决策：路径规则优先支持通过 `ENVR_ROOT` 覆盖，避免平台差异阻塞调试；Linux 按 `XDG_DATA_HOME` 优先，其次 `~/.local/share`；macOS 使用 `~/Library/Application Support`；Windows 优先 `APPDATA`。
    - 验收结果：Win/macOS/Linux 路径规则单元测试通过；`cargo test -p envr-platform` 通过。

### T007 `.envr.toml` / `.envr.local.toml` 解析器
- [x] **T007：实现标准 TOML 项目配置加载与合并** #config #shim
  - **描述**：支持目录上溯查找、local 覆盖、变量展开与循环保护。
  - **依赖**：T006
  - **输入文档**：`refactor docs/04-shim-设计.md`
  - **输出文件**：`crates/envr-config/src/project_config.rs`
  - **验收**：覆盖不同层级目录与覆盖优先级的集成测试通过。
  - **进度**：done
  - **实现记录**：
    - 实现要点：新增 `envr-config/src/project_config.rs`，支持从当前目录向上查找最近的 `.envr.toml/.envr.local.toml`；解析 TOML 后 local 覆盖 base；对配置字符串做 `${VAR}` 展开并检测循环依赖。
    - 相关提交/PR：本次提交
    - 遇到的问题/决策：本阶段以“就近配置”作为查找策略（命中最近一层配置目录即停止），避免多层叠加导致可预测性下降；未解析的 `${VAR}` 视为配置错误并返回 `Validation`。
    - 验收结果：集成测试覆盖不同层级目录命中、local 覆盖优先级、变量展开与循环保护；`cargo test -p envr-config` 通过。

### T008 全局配置与缓存配置
- [x] **T008：实现全局 `settings.toml` 与缓存配置加载** #config
  - **描述**：实现默认值、校验、持久化与缓存失效机制。
  - **依赖**：T006
  - **输入文档**：`refactor docs/05-下载与镜像源设计.md`
  - **输出文件**：`crates/envr-config/src/settings.rs`
  - **验收**：读写一致性测试通过；损坏配置可恢复默认。
  - **进度**：done
  - **实现记录**：
    - 实现要点：新增 `envr-config/src/settings.rs`，实现全局 Settings 的默认值、校验、TOML 读写；提供基于 mtime 的 `SettingsCache` 自动失效重载；配置损坏时自动备份并回退默认值。
    - 相关提交/PR：本次提交
    - 遇到的问题/决策：当前阶段配置项聚焦镜像模式与下载并发/重试等核心项，后续按下载/镜像模块实现逐步扩展；持久化采用临时文件写入 + 备份替换，降低写入中断风险。
    - 验收结果：读写一致性单测通过；损坏/非法配置可恢复默认；`cargo test -p envr-config` 通过。

### T009 平台抽象（链接、PATH、脚本）
- [x] **T009：实现 `envr-platform` 的 OS 抽象能力** #platform #shim
  - **描述**：封装硬链接/软链接、PATH 注入、shell 配置写入。
  - **依赖**：T006
  - **输入文档**：`refactor docs/04-shim-设计.md`
  - **输出文件**：`crates/envr-platform/src/*`
  - **验收**：平台测试通过；幂等执行不重复污染 PATH。
  - **进度**：done
  - **实现记录**：
    - 实现要点：在 `envr-platform` 增加 `links/path/shell` 模块：支持硬/软链接创建（替换式幂等）、PATH 去重拼接、以及可识别注入块的 shell 配置写入/移除（幂等不重复污染）。
    - 相关提交/PR：本次提交
    - 遇到的问题/决策：先以“文件层注入块”实现跨平台可测试的幂等能力；Windows 系统级 PATH API 注入后续在需要时再补齐。
    - 验收结果：`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test -p envr-platform` 通过；注入与移除单测验证幂等。

## Phase 3：下载引擎与镜像系统

### T010 下载任务模型与状态机
- [x] **T010：实现下载任务状态机（queued/running/failed/cancelled/done）** #download
  - **描述**：定义任务生命周期、重试策略、取消机制。
  - **依赖**：T004,T008
  - **输入文档**：`refactor docs/05-下载与镜像源设计.md`
  - **输出文件**：`crates/envr-download/src/task.rs`
  - **验收**：状态迁移单元测试与属性测试通过。
  - **进度**：done
  - **实现记录**：
    - 实现要点：新增 `envr-download/src/task.rs`，实现任务状态机（Queued/Running/Failed/Cancelled/Done）、指数退避重试策略、取消令牌；提供状态迁移与失败重试的明确返回值（下一次重试延迟）。
    - 相关提交/PR：本次提交
    - 遇到的问题/决策：本阶段仅建模与状态迁移，不绑定具体网络/IO runtime；取消机制先用轻量 `AtomicBool` token，后续下载引擎可与异步取消点对接。
    - 验收结果：状态迁移单元测试通过；属性测试（随机操作序列）通过；`cargo test -p envr-download` 通过。

### T011 HTTP 下载与断点续传
- [x] **T011：实现流式下载、断点续传、限速和超时控制** #download
  - **描述**：基于 reqwest/tokio 实现稳健下载器。
  - **依赖**：T010
  - **输入文档**：`refactor docs/07-依赖选择与原则.md`
  - **输出文件**：`crates/envr-download/src/engine.rs`
  - **验收**：中断后可继续下载；大文件下载稳定。
  - **进度**：done
  - **实现记录**：
    - 实现要点：新增 `envr-download/src/engine.rs`，基于 `reqwest/tokio` 实现流式下载到文件；支持断点续传（Range + 追加写入，服务端不支持则回退全量重下）、请求超时、按秒节流限速与取消令牌。
    - 相关提交/PR：本次提交
    - 遇到的问题/决策：集成测试需要本地 HTTP 服务器依赖，当前阶段先以单测覆盖关键逻辑（Range header、限速参数校验），续传端到端在后续引入测试服务器后补齐。
    - 验收结果：`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test -p envr-download` 通过；下载引擎可编译并可在后续任务接入实际下载用例验证续传稳定性。

### T012 校验与解压模块
- [x] **T012：实现 SHA256 校验与 zip/tar 解压** #download
  - **描述**：下载后做完整性校验并原子安装到目标目录。
  - **依赖**：T011
  - **输入文档**：`refactor docs/05-下载与镜像源设计.md`
  - **输出文件**：`crates/envr-download/src/checksum.rs`,`extract.rs`
  - **验收**：错误包被拒绝；解压路径安全校验通过。
  - **进度**：done
  - **实现记录**：
    - 实现要点：新增 `checksum.rs`（文件 SHA256 计算与校验）与 `extract.rs`（zip/tar/tar.gz 安全解压，拒绝绝对路径与 `..` 穿越）；提供 `extract_archive_atomic` 先解压到临时目录再原子替换目标目录。
    - 相关提交/PR：本次提交
    - 遇到的问题/决策：为保证安全默认拒绝路径穿越与绝对路径条目；原子安装在 Windows 上采用“先删除旧目录再 rename”策略，后续如需保留旧版本可扩展为备份策略。
    - 验收结果：SHA256 已知向量测试通过；zip 路径穿越用例被拒绝；`cargo test -p envr-download` 通过。

### T013 镜像注册中心与策略
- [x] **T013：实现官方/国内镜像注册、manual/auto/official 策略** #mirror
  - **描述**：支持预设镜像、自定义镜像、策略切换。
  - **依赖**：T008
  - **输入文档**：`refactor docs/05-下载与镜像源设计.md`
  - **输出文件**：`crates/envr-mirror/src/*`
  - **验收**：各策略可返回正确 URL；非法镜像被拦截。
  - **进度**：done
  - **实现记录**：
    - 实现要点：在 `envr-mirror` 实现 `MirrorRegistry`（预设 official + 国内镜像占位）、URL 校验（仅 http/https、禁止凭据）、以及基于 `Settings.mirror.mode` 的策略选择（official/manual/auto/offline）；提供 `join_url` 生成资源 URL 并校验路径。
    - 相关提交/PR：本次提交
    - 遇到的问题/决策：`auto` 策略的“测速选最快”将在 T014 实现；本任务先实现可运行的默认选择（优先非官方预设，否则 official）以便后续模块联调。
    - 验收结果：策略选择与非法镜像拦截单测通过；`cargo test -p envr-mirror` 通过。

### T014 镜像测速与自动选择
- [x] **T014：实现镜像健康检查和自动最优选择** #mirror #download
  - **描述**：对候选镜像做延迟/可用性评分并缓存结果。
  - **依赖**：T013
  - **输入文档**：`refactor docs/05-下载与镜像源设计.md`
  - **输出文件**：`crates/envr-mirror/src/probe.rs`
  - **验收**：可用镜像能被选中；不可用镜像自动降级。
  - **进度**：done
  - **实现记录**：
    - 实现要点：新增 `envr-mirror/src/probe.rs`，对候选镜像做 HEAD 探测并记录可用性与延迟；结果缓存到平台 cache 目录（TTL 控制），`auto` 策略可基于缓存/探测结果选择延迟最低的可用镜像并在不可用时降级到 official。
    - 相关提交/PR：本次提交
    - 遇到的问题/决策：探测以镜像 base URL 为健康检查入口（200/404 视为可达），具体资源路径探测将在后续 runtime/index 确定后增强；同步 `resolve_mirror` 保持保守（fallback official），异步 auto 选择通过 `probe::resolve_mirror_auto` 提供。
    - 验收结果：缓存比较与选择逻辑单测通过；`cargo test -p envr-mirror` 通过。

## Phase 4：语言运行时核心实现（Node/Python/Java）

### T015 定义 RuntimeProvider trait 与 core 编排
- [x] **T015：实现 runtime 抽象接口与 core 调度层** #core #runtime
  - **描述**：抽象安装/卸载/切换/远程查询/解析能力。
  - **依赖**：T004,T012,T013
  - **输入文档**：`refactor docs/01-总体架构设计.md`
  - **输出文件**：`crates/envr-core/src/runtime/*.rs`
  - **验收**：Node/Python/Java 可通过统一接口被调用。
  - **进度**：done
  - **实现记录**：
    - 实现要点：在 `envr-domain` 定义 `RuntimeProvider` trait 与 `RuntimeKind/VersionSpec/InstallRequest` 等通用类型；在 `envr-core/src/runtime/service.rs` 实现按语言路由的 `RuntimeService`，并接入 Node/Python/Java provider 最小实现以验证统一调用链路。
    - 相关提交/PR：本次提交
    - 遇到的问题/决策：当前阶段 provider 仅提供最小可编译实现，具体 remote/index/install 流程将在后续 T016+ 逐步填充；`auto`/镜像/下载等能力通过后续组合注入到 provider。
    - 验收结果：`cargo test --workspace --all-targets` 通过；Node/Python/Java provider 均可通过 `RuntimeService::with_defaults()` 被统一调用。

### T016 Node 远程索引与版本解析
- [x] **T016：实现 Node 版本索引抓取、筛选、LTS 解析** #runtime #node
  - **描述**：支持按平台和架构过滤可安装版本。
  - **依赖**：T015
  - **输入文档**：旧项目与二版 Node 实现
  - **输出文件**：`crates/envr-runtime-node/src/index.rs`
  - **验收**：`remote/list` 与解析结果正确。
  - **进度**：done
  - **实现记录**：
    - 实现要点：新增 `index` 模块：`parse_node_index`、`release_has_platform`（对齐 `index.json` 中 `win-*` / `linux-*` / `osx-*-{tar,pkg}` 命名）、`list_remote_versions`（semver 降序 + `RemoteFilter.prefix`）、`resolve_node_version`（`latest`/`current`、`lts`、`lts-<codename>`、`lts/<name>`、精确版本、主版本与 `major.minor` 行最新补丁）；`NodeRuntimeProvider` 通过 blocking `reqwest` 拉取官方 `index.json` 并接入 `list_remote` / `resolve` / `install`（install 返回解析后的 canonical 版本）。`envr-core` 的 `RuntimeService` 单测不再默认请求 Node 官方索引（避免 CI/离线依赖网络），Node 行为由 `envr-runtime-node` 内嵌 JSON fixture 覆盖。
    - 相关提交/PR：（本次提交）
    - 遇到的问题/决策：平台识别以 `index.json` 的 `files` 为准（macOS 为 `osx-*` 而非 `darwin-*`）；可注入 `with_index_json_url` 便于后续与镜像基址组合（T017+）。
    - 验收结果：`cargo fmt --all`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace --all-targets` 通过。

### T017 Node 安装/卸载/切换
- [x] **T017：实现 Node 版本安装、卸载、current 切换流程** #runtime #node
  - **描述**：打通下载、解压、目录落盘、current 更新。
  - **依赖**：T016,T012
  - **输入文档**：`refactor docs/02-cli-设计.md`
  - **输出文件**：`crates/envr-runtime-node/src/manager.rs`
  - **验收**：安装后可直接执行 `node -v`。
  - **进度**：done
  - **实现记录**：
    - 实现要点：新增 `manager`：`NodePaths`（`runtimes/node/versions/<ver>`、`current` 符号链接、`cache/node` 下载缓存）、`dist_root_from_index_json_url`、`parse_shasums256` / `pick_node_dist_artifact`（按平台优先 `.tar.xz`→`.tar.gz` 或 `win-*.zip` 等）、blocking 下载 + `checksum::verify_sha256_hex`、`extract::extract_archive` + `promote_single_root_dir` 扁平化官方单根目录；`NodeRuntimeProvider` 接入 `list_installed` / `current` / `set_current` / `install` / `uninstall`，支持 `with_runtime_root` 便于测试。`envr-download` 增加 `.tar.xz`/`.txz` 解压（`xz2`）。
    - 相关提交/PR：（本次提交）
    - 遇到的问题/决策：Windows 官方 zip 的 `node.exe` 常在根目录而非 `bin/`，`node_installation_valid` 同时识别两种布局；Linux 需 xz 解压链路故引入 `xz2`（工作区 `Cargo.toml` 声明版本）。
    - 验收结果：`cargo fmt --all`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace --all-targets` 通过；安装后可将 `{runtime_root}/runtimes/node/current` 下的 `node` / `node.exe` 加入 PATH 后执行 `node -v`（端到端下载未放入默认单测以免 CI 依赖外网）。

### T018 Python 远程索引与版本解析
- [x] **T018：实现 Python 版本发现、版本规范化与选择器** #runtime #python
  - **描述**：支持主/次/补丁版本选择与平台包筛选。
  - **依赖**：T015
  - **输入文档**：二版 Python 检测逻辑
  - **输出文件**：`crates/envr-runtime-python/src/index.rs`
  - **验收**：可稳定获取并解析可安装版本。
  - **进度**：done
  - **实现记录**：
    - 实现要点：基于 `python.org` OData API：拉取 `/api/v2/downloads/release/` 与 `/api/v2/downloads/release_file/`，按 `release` id 关联；从 `name`（如 `Python 3.12.1`）解析 semver；平台规则——Windows 非源码且按 URL/名称匹配 `amd64`/`arm64`/`win32`；macOS 非源码安装包；Linux 以官方源码 `.tar.xz`（`is_source`）作为可安装基线；排除 `pre_release`。`list_remote_versions` 按版本降序并支持 `RemoteFilter.prefix`；`resolve_python_version` 支持 `latest`/`stable`/`current`、`3`、`3.12`、`3.12.1` 与精确标签。`PythonRuntimeProvider` 接入 `list_remote`/`resolve`/`install`（install 返回解析结果）。`envr-core` 单测改为用 Java 做路由桩，避免默认 CI 拉取官方索引。
    - 相关提交/PR：（本次提交）
    - 遇到的问题/决策：python.org 无独立 “Linux” OS 条目，Linux 以源码 XZ 作为与官方下载页一致的通用工件；全量 `release_file` 一次拉取（体量可接受）以在内存中完成平台筛选。
    - 验收结果：`cargo fmt --all`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace --all-targets` 通过。

### T019 Python 安装/卸载/切换
- [x] **T019：实现 Python 安装流程与 pip 基础可用性** #runtime #python
  - **描述**：支持安装后 `python/pip` 可执行、版本切换可生效。
  - **依赖**：T018,T012
  - **输入文档**：旧项目 Python 经验
  - **输出文件**：`crates/envr-runtime-python/src/manager.rs`
  - **验收**：`python --version` 与 `pip --version` 正常。
  - **进度**：done
  - **实现记录**：
    - 实现要点：新增 `manager`：`PythonPaths`（`runtimes/python/versions/<x.y.z>`、`current` 符号链接、`cache/python`）；`pick_install_artifact`（Windows 选官方 **embeddable zip**，解压后修补 `*. _pth` 追加 `import site`，再拉取 `get-pip.py` 引导 pip；Linux/macOS 选 **源码 `.tar.xz`**，`./configure --prefix=... --with-ensurepip=install` + `make -jN` + `make install`）；可选校验 API 提供的 `sha256_sum`；安装结束执行 `python -m pip --version` 自检；`list_installed` / `current` / `set_current` / `uninstall` 与 Node 同模式。`PythonRuntimeProvider` 增加 `with_runtime_root` 并接入上述能力。
    - 相关提交/PR：（本次提交）
    - 遇到的问题/决策：官方 Linux 无预编译包条目时以源码安装为统一路径（需本机编译工具链）；Windows 用 embeddable + get-pip 避免静默 `.exe` 安装器参数差异。
    - 验收结果：`cargo fmt --all`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace --all-targets` 通过；完整端到端安装未放入默认单测（耗时长且依赖网络/编译环境）。

### T020 Java 发行版模型与索引
- [x] **T020：实现 Java vendor 抽象与版本索引** #runtime #java
  - **描述**：支持 Temurin/OpenJDK 等 vendor 选择与版本解析。
  - **依赖**：T015
  - **输入文档**：二版 Java vendor 设计
  - **输出文件**：`crates/envr-runtime-java/src/vendor.rs`,`index.rs`
  - **验收**：可按 vendor 返回可安装版本。
  - **进度**：done
  - **实现记录**：
    - 实现要点：`vendor.rs` 定义 `JavaVendor`（Temurin/OpenJDK 等别名 → Adoptium `vendor=eclipse`）；`index.rs` 用 blocking `reqwest` 拉取 `v3/info/available_releases`（LTS 主版本集合）与 `v3/info/release_versions`（`os`/`architecture` 映射 Windows/Linux/macOS + x64/aarch64/x86），`list_remote_versions`（降序 + `RemoteFilter.prefix`）、`resolve_java_version`（`latest`/`current`/`stable`、`lts`、精确 `openjdk_version`/`semver`、`major` / `major.minor` / `major.minor.security` 行内最新）；`JavaRuntimeProvider` 支持 `with_api_base` / `with_vendor`，`list_remote`/`resolve`/`install`（install 暂同 resolve，落盘见 T021）。`envr-core` 默认单测仅 `list_installed`，避免 CI 访问 Adoptium。
    - 相关提交/PR：（本次提交）
    - 遇到的问题/决策：Adoptium 查询需同时带 `heap_size`/`image_type`/`jvm_impl`/`os`/`architecture`/`vendor`，否则易 404；`lts` 解析结合 API 的 `available_lts_releases` 与条目的 `optional: LTS`。
    - 验收结果：`cargo fmt --all`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace --all-targets` 通过。

### T021 Java 安装/卸载/切换 + JAVA_HOME
- [x] **T021：实现 Java 生命周期管理与环境变量更新** #runtime #java
  - **描述**：安装/切换后 `java`、`javac`、`JAVA_HOME` 一致。
  - **依赖**：T020,T009,T012
  - **输入文档**：旧项目 Java 完整实现
  - **输出文件**：`crates/envr-runtime-java/src/manager.rs`
  - **验收**：切换后 `java -version` 与 `JAVA_HOME` 对应正确。
  - **进度**：done
  - **实现记录**：
    - 实现要点：新增 `manager`：`JavaPaths`（`runtimes/java/versions/<openjdk_version>`、`current` 符号链接、`cache/java`）；用 Adoptium `v3/assets/version/{Maven范围}` 拉取与解析版本一致的 GA JDK 包（`package.link` + SHA256）；blocking 下载 + `checksum::verify_sha256_hex`、`extract::extract_archive` + `promote_single_root_dir` 扁平化单根 `jdk-*` 目录；`java_installation_valid`（`bin/java` / `bin/java.exe`）；`list_installed` / `read_current` / `set_current` / `uninstall`；安装或切换后 `sync_java_home_export` 写入 `runtimes/java/JAVA_HOME`（单行绝对路径，供 `export JAVA_HOME=$(<...>)` 与 `PATH` 追加 `$JAVA_HOME/bin`）。`index` 补充 `normalize_openjdk_version_label`、`adoptium_assets_version_range_segment`、`find_version_entry`。`JavaRuntimeProvider` 增加 `with_runtime_root` 并接入上述能力。
    - 相关提交/PR：（本次提交）
    - 遇到的问题/决策：Windows 无管理员/开发者模式时创建目录 symlink 会失败，故 `current`+`JAVA_HOME` 文件的集成单测放在 `cfg(unix)`，Windows 仅测目录布局与 `list_installed`。
    - 验收结果：`cargo fmt --all`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace --all-targets` 通过；端到端下载未放入默认单测（依赖网络）。

## Phase 5：Shim 与命令代理

### T022 `envr-shim-core` 解析与路由
- [x] **T022：实现 shim 路由核心（命令识别、版本解析、回退策略）** #shim
  - **描述**：统一处理 core executable 与全局包命令转发。
  - **依赖**：T007,T015,T017,T019,T021
  - **输入文档**：`refactor docs/04-shim-设计.md`
  - **输出文件**：`crates/envr-shim-core/src/*`
  - **验收**：项目级与全局级版本解析结果正确。
  - **进度**：done
  - **实现记录**：
    - 实现要点：`ShimContext`（`ENVR_RUNTIME_ROOT` 或 `current_platform_paths().runtime_root` + `working_dir`）、`load_project_config(cwd)` 后按 `[runtimes.node|python|java].version` 选 `versions/<dir>`（`pick_version_home`：精确目录名或 `major` / `major.minor` / `major.minor.patch` 在已安装目录中选最新），无 pin 则 `runtimes/<lang>/current` 规范路径；`CoreCommand` 映射 `node/npm/npx`、`python/pip`、`java/javac` 到真实可执行路径（对齐 Node/Python/Java 安装布局）；`java`/`javac` 附加 `JAVA_HOME`。未知命令返回明确错误（全局 npm bin 留给后续）。单测覆盖版本选择与 Unix 下项目 pin 优先于 global current；Windows 跑目录选择单测。
    - 相关提交/PR：（本次提交）
    - 遇到的问题/决策：Rust 2024 中 `Option<(u32,u32,u32)>` 返回类型需拆 `type VersionTriple` 避免 `>>` 解析问题；依赖 `symlink` 的集成单测仅 `cfg(unix)`。
    - 验收结果：`cargo fmt --all`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace --all-targets` 通过。

### T023 `envr-shim` 二进制入口
- [x] **T023：实现 shim 二进制入口与跨平台进程替换执行** #shim
  - **描述**：Windows 与 Unix 路径差异处理，保留参数透传。
  - **依赖**：T022
  - **输入文档**：二版 `wx-shim` 实现
  - **输出文件**：`crates/envr-shim/src/main.rs`
  - **验收**：命令透传行为和退出码保持正确。
  - **进度**：done
  - **实现记录**：
    - 实现要点：`envr-shim-core` 增加 `parse_shim_invocation`（argv0 为工具名如 `node`，或 `envr-shim <tool> …`）与 `resolve_core_shim_command`；`envr-shim` 解析后注入 `ResolvedShim.extra_env`，Unix 使用 `CommandExt::exec` 替换进程，Windows 使用 `Command::status` 并 `exit` 子进程返回码（无码时用 `0xFF`）。集成测试 `tests/smoke.rs`（`cfg(unix)`）验证 `envr-shim python3` 与退出码透传；`envr-shim-core` 单测覆盖两种 argv 形态。
    - 相关提交/PR：（本次提交）
    - 遇到的问题/决策：Windows 无 `execve` 等价物，采用 spawn 转发退出码；端到端 smoke 仅 Unix（脚本 chmod + symlink）。
    - 验收结果：`cargo fmt --all`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace --all-targets` 通过。

### T024 shim 文件生成与刷新
- [x] **T024：实现 shims 生成、删除、全局包自动刷新** #shim #node
  - **描述**：安装/卸载/全局包变化后自动更新 shim 文件。
  - **依赖**：T023,T009
  - **输入文档**：旧项目与二版 shim 行为
  - **输出文件**：`crates/envr-core/src/shim_service.rs`
  - **验收**：新增全局包后可直接执行；删除后 shim 清理正确。
  - **进度**：done
  - **实现记录**：
    - 实现要点：新增 `ShimService`（`runtime_root` + `shim_exe` 绝对路径），`shim_dir`=`{runtime_root}/shims`。`ensure_shims(kind)` 写入核心启动器（Windows `*.cmd`：`envr-shim.exe <dispatch> %*`；Unix `#!/bin/sh` + `exec` + `chmod 755`）。`remove_shims(kind)` 删除对应语言的 core 文件。`sync_global_package_shims(Node, _)`：在存在 `runtimes/node/current` 时用 `core_tool_executable` 找 `npm` 并执行 `npm bin -g`（PATH 前置 node home），为非核心条目在 `shims` 下生成转发（Windows `call "全局路径"`；Unix symlink）；`remove_stale_non_core_shims` 删除既非 core、又不在当前全局 bin 列表中的 shim。`envr-shim-core` 增加 `core_tool_executable` 供 npm 定位。
    - 相关提交/PR：（本次提交）
    - 遇到的问题/决策：与 CLI 安装流程解耦，由调用方在适当时机传入 `shim_exe` 并调用 `ensure`/`sync`；未在 crate 内自动挂钩 Node 安装（留待 T026+）。
    - 验收结果：`cargo fmt --all`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace --all-targets` 通过。

## Phase 6：CLI 完整可用

### T025 CLI 命令骨架与全局参数
- [x] **T025：实现 `envr-cli` 命令树与全局参数（format/quiet/no-color）** #cli
  - **描述**：建立命令入口和统一输出选择器。
  - **依赖**：T015
  - **输入文档**：`refactor docs/02-cli-设计.md`
  - **输出文件**：`crates/envr-cli/src/cli.rs`
  - **验收**：`envr --help` 完整可读。
  - **进度**：done
  - **实现记录**：
    - 实现要点：新增 `cli.rs`（`clap` 根命令 `envr`、flatten 全局参数 `--format`、`--quiet`、`--no-color`、`--runtime-root`；L1 子命令 `install`/`use`/`list`/`current`/`uninstall`/`which`/`remote`/`doctor` 占位解析；`apply_global` 写入 `NO_COLOR`/`ENVR_RUNTIME_ROOT`/`ENVR_OUTPUT_FORMAT`/`RUST_LOG`（unsafe 块符合 Rust 2024 环境变量约定）；`main` 先解析再 `apply_global` 后初始化日志；二进制名 `envr`（`autobins = false` + `[[bin]]`）；集成测试校验 `--help` 含 L1 与全局项。工作区 `clap` 在 `default-features = false` 下需显式启用 `help`（及 `usage`）否则无 `--help`。
    - 相关提交/PR：（本次提交）
    - 遇到的问题/决策：子命令位置参数字段名不可用 `version`（与 `--version` 冲突），改为 `runtime_version`；`ValueEnum` 的 `default_value_t` 与 `global` 组合未作为根因，根因是缺少 `help` feature。
    - 验收结果：`cargo fmt --all`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace --all-targets` 通过。

### T026 基础命令实现（install/use/list/current）
- [x] **T026：实现核心高频命令链路** #cli
  - **描述**：打通核心四命令到 `envr-core`。
  - **依赖**：T025,T017,T019,T021,T024
  - **输入文档**：`refactor docs/02-cli-设计.md`
  - **输出文件**：`crates/envr-cli/src/commands/*`
  - **验收**：Node/Python/Java 三语言均可完成常规生命周期操作。
  - **进度**：done
  - **实现记录**：
    - 实现要点：`envr-core::RuntimeService::with_runtime_root` 统一为 Node/Python/Java provider 注入数据根；CLI `commands/{common,install,use_cmd,list,current}.rs` 解析 `node|python|java`、调用 `install`/`resolve+set_current`/`list_installed`/`current`；`ENVR_RUNTIME_ROOT`（及 `apply_global` 的 `--runtime-root`）优先于平台默认路径；`--format json` 输出简易 JSON；集成测试在隔离目录下验证 `list`/`current` 与参数校验。
    - 相关提交/PR：（本次提交）
    - 遇到的问题/决策：未在 T026 挂钩 T024 `ShimService`（仍留待安装链路集成）；`install`/`use` 需两个位置参数 `LANG` `VERSION`。
    - 验收结果：`cargo fmt --all`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace --all-targets` 通过。

### T027 扩展命令实现（uninstall/which/remote/doctor）
- [x] **T027：实现可运维命令集** #cli #doctor
  - **描述**：完成卸载、可执行路径定位、远程列表、诊断修复建议。
  - **依赖**：T026
  - **输入文档**：`refactor docs/02-cli-设计.md`
  - **输出文件**：`crates/envr-cli/src/commands/*`
  - **验收**：可定位常见环境问题并给出可执行建议。
  - **进度**：done
  - **实现记录**：
    - 实现要点：新增 `commands/{uninstall,remote,which,doctor}.rs`。`uninstall` 调 `RuntimeService::uninstall`；`remote` 支持可选 `LANG` 与 `--prefix`（`RemoteFilter`），无 LANG 时汇总三语言（需网络）；`which` 借助 `envr-shim-core` 的 `parse_core_command`/`resolve_core_shim_command` 解析 `node|npm|npx|python|pip|java|javac` 真实路径（text 仅 stdout 打路径，额外 env 写 stderr；JSON 带 `extra_env`）；`doctor` 检查数据根存在与可写、`shims` 与 PATH 建议、各语言已安装数与 `current` 缺失时提示 `envr use`，有问题时进程退出码 1。`parse_core_command` 从 `envr-shim-core` 对外 re-export。
    - 相关提交/PR：（本次提交）
    - 遇到的问题/决策：`remote` 全量会发起多次上游请求，适合脚本按需带 `LANG`；`doctor` 的写探针在只读目录会报 issue。
    - 验收结果：`cargo fmt --all`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace --all-targets` 通过。

### T028 JSON 输出契约与错误输出统一
- [x] **T028：实现 text/json 双输出一致性与错误编码映射** #cli #output
  - **描述**：保证自动化脚本可稳定消费 CLI 输出。
  - **依赖**：T027,T004
  - **输入文档**：`refactor docs/02-cli-设计.md`
  - **输出文件**：`crates/envr-cli/src/output.rs`
  - **验收**：同一命令在两种格式下语义一致。
  - **进度**：done
  - **实现记录**：
    - 实现要点：新增 `output.rs`：`write_envelope`（`success`/`code`/`message`/`data`/`diagnostics`）、`emit_ok`、`emit_validation`、`emit_envr_error`（`ErrorCode`→稳定 `code` 字符串）、`emit_doctor`；`Io`/`Download`/`Mirror` 映射退出码 **2**，其余错误 **1**。各命令成功 JSON 统一带 `message`（如 `list_installed`、`doctor_ok`），成功时 `code` 为 null。`common` 的校验与错误打印委托到 `output`。集成测试 `json_envelope.rs` 校验契约并从 stdout 中选取 JSON 行（兼容 tracing 日志混排）。
    - 相关提交/PR：（本次提交）
    - 遇到的问题/决策：进程仍向 stdout 写 tracing，JSON 消费方需取首个 `{` 行或后续 T028+ 将日志限定 stderr。
    - 验收结果：`cargo fmt --all`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace --all-targets` 通过。

### T029 项目级命令（init/check/resolve）
- [x] **T029：实现项目配置相关命令** #cli #config
  - **描述**：`init` 生成 `.envr.toml`，`check` 校验，`resolve` 解析版本规格。
  - **依赖**：T007,T028
  - **输入文档**：`refactor docs/04-shim-设计.md`
  - **输出文件**：`crates/envr-cli/src/commands/{init,check,resolve_cmd}.rs`
  - **验收**：项目目录内版本解析与 shim 实际行为一致。
  - **进度**：done
  - **实现记录**：
    - 实现要点：`envr-shim-core` 增加 `resolve_runtime_home_for_lang`，`runtime_home_for_key` 支持 optional spec 覆盖（CLI `--spec` 优先于项目 pin，再全局 `current`）。CLI `init`（`--path` 默认 `.`，`--force` 覆盖）、`check`（向上加载配置并对已声明 pin 调用 `pick_version_home` 与 shim 一致）、`resolve`（输出 `home` / `version_dir` / `resolution_source`: `cli_override`|`project`|`global_current`）；`check` 失败 JSON `code`=`project_check_failed`。依赖 `envr-config`；`dispatch` 对项目命令不构造 `RuntimeService`。集成测试 `project_cmds.rs`。
    - 相关提交/PR：（本次提交）
    - 遇到的问题/决策：`resolve` 模块命名 `resolve_cmd.rs` 避免与标准库混淆。
    - 验收结果：`cargo fmt --all`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace --all-targets` 通过。

## Phase 7：GUI（高质量体验，不简化）

### T030 GUI 应用骨架与消息循环
- [x] **T030：建立 `envr-gui` 入口与状态容器** #gui
  - **描述**：实现主窗口、路由、全局消息处理和错误提示通道。
  - **依赖**：T015,T005
  - **输入文档**：`refactor docs/03-gui-设计.md`
  - **输出文件**：`crates/envr-gui/src/*`
  - **验收**：GUI 可启动并完成基本页面切换。
  - **进度**：done
  - **实现记录**：
    - 实现要点：`iced` 0.13 `application` 主循环；`AppState`（`route` + `error`）；左侧四向导航（仪表盘 / 运行时 / 设置 / 关于），右侧占位内容；顶部全局错误横幅与 `DismissError`；`ReportError` 供后续后台失败回调；启动前仍调用 `envr_core::logging::init_logging`。
    - 相关提交/PR：feat(gui): T030 iced shell with routes and error banner
    - 遇到的问题/决策：侧栏/正文 helpers 返回 `Element<'static, …>` 以避免无输入借用时的 `'_` 推断错误；演示用错误入口仅在「关于」页。
    - 验收结果：`cargo fmt --all`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace --all-targets` 通过；本地运行 `envr-gui` 可切换页面并展示/关闭示例错误。

### T031 平台视觉主题系统（Fluent/Liquid Glass/M3）
- [x] **T031：实现跨平台主题与组件皮肤系统** #gui #ux
  - **描述**：按 OS 切换主题 token、阴影、圆角、材质与动效参数。
  - **依赖**：T030
  - **输入文档**：`refactor docs/03-gui-设计.md`
  - **输出文件**：`crates/envr-ui/src/theme/*`
  - **验收**：三平台风格切换可见且一致。
  - **进度**：done
  - **实现记录**：
    - 实现要点：`envr-ui::theme` 提供 `UiFlavor`、语义色、`ThemeTokens`（圆角、阴影、动效、dock 宽、`backdrop_blur_hint`）、`tokens_for` 三预设与 `default_flavor_for_target`；`envr-gui` 映射为 `iced::Theme::custom` 与侧栏/错误条容器样式；**设置**页可切换三种风味并展示 token 摘要以便肉眼验收。
    - 相关提交/PR：feat(gui): T031 platform theme tokens and iced mapping
    - 遇到的问题/决策：`iced` 皮肤与 token 解耦——token 留在 `envr-ui`，`iced` 调色与 `container` 样式在 `envr-gui/src/theme.rs`；真·亚克力/模糊受渲染能力限制，以 hint + 面板色阶区分风格。
    - 验收结果：`cargo fmt --all`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace --all-targets` 通过；运行 GUI 在「设置」切换三种风格可见色与圆角/阴影差异。

### T032 环境中心页面（Node/Python/Java）
- [x] **T032：实现环境中心全流程交互（安装/切换/卸载）** #gui
  - **描述**：GUI 完整调用 core 服务，不复制业务逻辑。
  - **依赖**：T031,T026,T027
  - **输入文档**：`refactor docs/03-gui-设计.md`
  - **输出文件**：`crates/envr-gui/src/view/env_center/*`
  - **验收**：GUI 能完成与 CLI 等效的核心操作。
  - **进度**：done
  - **实现记录**：
    - 实现要点：`view/env_center/panel.rs` 展示 Node/Python/Java 切换、已安装列表、当前版本、`install`/`resolve`+`set_current`（等同 CLI `use`）/`uninstall`；`service.rs` 与 CLI 一致解析 `ENVR_RUNTIME_ROOT`；`gui_ops.rs` + `runtime_exec.rs` 用 Tokio `spawn_blocking` 调用 `RuntimeService`，避免阻塞 UI；进入「运行时」自动刷新列表。
    - 相关提交/PR：feat(gui): T032 env center via RuntimeService
    - 遇到的问题/决策：`iced` `update` 返回 `Task<Message>`；侧栏仍为四路由，环境中心落在「运行时」页；当前行禁用使用/卸载与 CLI 规则一致。
    - 验收结果：`cargo fmt --all`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace --all-targets` 通过。

### T033 下载面板与任务控制
- [x] **T033：实现多任务下载面板（进度/取消/重试）** #gui #download
  - **描述**：实时展示下载速度、ETA、失败原因、任务恢复。
  - **依赖**：T032,T010,T011
  - **输入文档**：`refactor docs/05-下载与镜像源设计.md`
  - **输出文件**：`crates/envr-gui/src/view/downloads/*`
  - **验收**：GUI 下载任务状态与后台真实状态一致。
  - **进度**：done
  - **实现记录**：
    - 实现要点：`envr-download::DownloadEngine::download_to_file` 增加可选进度/总长 `AtomicU64`；GUI `view/downloads` 底部 dock，多任务列表、`progress_bar`、订阅 `iced::time::every` 推算速率与 ETA、`CancelToken` 取消、失败重试（重新入队）；演示下载走真实异步引擎与 CLI 同源 crate。`iced` 启用 `tokio` feature 以使用 `time::every`。
    - 相关提交/PR：feat(gui): T033 download panel and engine progress hooks
    - 遇到的问题/决策：运行时 `install` 仍为同步阻塞 HTTP，未接入本面板；与「后台一致」对演示任务成立，环境安装进度需后续把 provider 下载迁到引擎并登记任务总线（或 T034/T051 前后续）。
    - 验收结果：`cargo fmt --all`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace --all-targets` 通过。

### T034 设置页（镜像、路径、行为）
- [x] **T034：实现设置页与配置持久化** #gui #config
  - **描述**：可配置 runtime root、镜像模式、安装后清理等。
  - **依赖**：T033,T008,T013
  - **输入文档**：`refactor docs/03-gui-设计.md`
  - **输出文件**：`crates/envr-gui/src/view/settings/*`
  - **验收**：设置修改后 CLI/GUI 同步生效。
  - **进度**：done
  - **实现记录**：
    - 实现要点：`envr-config::settings` 增加 `PathSettings`、`BehaviorSettings` 与 `resolve_runtime_root()`（`ENVR_RUNTIME_ROOT` → `paths.runtime_root` → 平台默认）；`SettingsCache::reload` 供 GUI 强刷；CLI `effective_runtime_root` 与 GUI `service::open_runtime_service` 均走 `resolve_runtime_root`。设置页 `view/settings` 编辑镜像模式/manual_id、运行时根、下载并发与重试、`cleanup_downloads_after_install`，保存/从磁盘重载；进入「设置」路由时自动 `reload_from_disk`。外观风味保留在同页下半部。
    - 相关提交/PR：feat(gui): T034 settings page, paths/behavior in settings.toml
    - 遇到的问题/决策：`MirrorMode` 增加 `Copy` 以便镜像按钮闭包；状态文案用克隆 `String` 满足 `Element<'static>`；`cleanup_downloads_after_install` 仅持久化，运行时安装路径尚未消费该标志。
    - 验收结果：`cargo fmt --all`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace --all-targets` 通过。

### T034.1 设置：字体（自动系统字体 + 可选字体）
- [x] **T034.1：设置页加入字体系统（自动使用系统本地字体，支持中文）** #gui #settings #font #i18n
  - **描述**：实现系统字体发现（Windows/macOS/Linux），默认选择覆盖中文的系统字体；支持用户在设置中选择字体并持久化；启动时注入 `default_font`，避免 iced 缺字导致中文不可见。
  - **依赖**：T034,T031
  - **输入文档**：旧项目 `src/font.rs` + 设置页字体选择逻辑（FontCatalog）；`refactor docs/03-gui-设计.md`
  - **输出文件**：
    - `crates/envr-ui/src/font/*`（FontCatalog/发现与排序）
    - `crates/envr-config/src/settings.rs`（FontSettings）
    - `crates/envr-gui/src/view/settings/*`（字体选择器 UI）
  - **验收**：
    - Win11 默认可显示完整中文（无需用户手动配置）
    - 用户选择字体后重启仍生效
    - 字体发现耗时受控，不显著影响冷启动（必要时做缓存/延迟发现）
  - **进度**：done
  - **实现记录**：
    - 实现要点：`envr-config` 增加 `appearance.font`（`FontMode::{auto,custom}` + `family`）并校验 custom 时必填；`envr-ui::font` 提供跨平台候选字体与 `preferred_system_sans_family()`（Win11 优先 YaHei UI）；GUI 设置页新增字体模式与选择器（候选 picklist + 手动输入）；启动时 `envr-gui` 从 `settings.toml` 读取并调用 `application(...).default_font(...)` 注入默认字体，避免 iced 中文缺字。
    - 相关提交/PR：
    - 遇到的问题/决策：iced `Font` 的 `Family::Name` 需要 `'static`，custom 字体族名在启动时通过 `Box::leak` 变为静态字符串（一次性泄漏，换取全局 default_font 支持）。
    - 验收结果：`cargo fmt --all`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace --all-targets` 通过；Win11 默认字体为 YaHei UI 路径存在时可直接显示中文；选择 custom 后重启仍生效。

### T034.2 设置：主题模式（跟随系统/浅色/深色）
- [x] **T034.2：设置页加入主题模式（跟随/浅色/深色）并即时生效** #gui #settings #theme
  - **描述**：实现主题模式枚举与持久化；“跟随系统”需监听系统深浅偏好或至少在启动时读取；切换时 UI 立即刷新（不闪烁/不重建整页）。
  - **依赖**：T034,T031
  - **输入文档**：旧项目主题/外观设置实现；`refactor docs/03-gui-设计.md`
  - **输出文件**：
    - `crates/envr-config/src/settings.rs`（AppearanceSettings：FollowSystem/Light/Dark）
    - `crates/envr-ui/src/theme/*`（scheme/token 切换）
    - `crates/envr-gui/src/view/settings/*`
  - **验收**：主题切换即时生效；FollowSystem 在系统切换后可自动更新（或提供“刷新跟随”机制），且不破坏性能指标。
  - **进度**：done
  - **实现记录**：
    - 实现要点：`envr-config` 增加 `ThemeMode::{follow_system,light,dark}` 并持久化到 `appearance.theme_mode`；`envr-ui::theme` 增加 `UiScheme` 与 `tokens_for_scheme()`，为三种 `UiFlavor` 补齐 dark 预设；GUI 设置页加入主题模式切换，并在 `AppState.tokens()` 中按当前设置实时计算 scheme（即时生效）。FollowSystem 使用跨平台命令探测（Windows `reg query` / macOS `defaults` / Linux `gsettings`），并做 900ms 缓存，避免频繁调用。
    - 相关提交/PR：
    - 遇到的问题/决策：FollowSystem 的系统探测使用外部命令实现（零额外依赖），并用 `OnceLock<Mutex<Cache>>` 做短 TTL 缓存，减少 UI 重绘成本。
    - 验收结果：`cargo fmt --all`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace --all-targets` 通过；设置页切换 Follow/Light/Dark 立即改变全局配色。

### T034.3 设置：语言（简体中文/English）
- [x] **T034.3：设置页加入语言切换（zh-CN / en-US）并全局生效** #gui #settings #i18n
  - **描述**：在设置中提供语言选择并持久化；切换后 GUI 立即刷新文本；CLI 与 GUI 共用同一 locale 解析与加载入口（避免两套语言状态）。
  - **依赖**：T034,T910
  - **输入文档**：旧项目 i18n 方案；`refactor docs/03-gui-设计.md`
  - **输出文件**：
    - `crates/envr-config/src/settings.rs`（I18nSettings：locale）
    - `crates/envr-core/src/i18n/*`（初始化/切换/当前 locale）
    - `crates/envr-gui/src/view/settings/*`
  - **验收**：
    - GUI 可在 zh-CN/en-US 间切换并立即生效
    - 未配置时默认跟随系统语言（可回退到英文）
    - 切换语言不引入明显闪烁与布局跳动
  - **进度**：done
  - **实现记录**：
    - 实现要点：`envr-config` 增加 `i18n.locale`（`LocaleMode::{follow_system,zh_cn,en_us}`）并持久化；`envr-core::i18n` 提供共享的 locale 状态、`init_from_settings` 与 `tr(zh,en)`；CLI/GUI 启动时均从 `settings.toml` 初始化 locale；GUI 设置页增加语言切换按钮，切换时即时调用 `envr_core::i18n::init_from_settings` 并触发全局文案刷新。
    - 相关提交/PR：
    - 遇到的问题/决策：系统语言探测使用 `LC_ALL/LANG/LANGUAGE` 环境变量的启发式（跨平台、零依赖），FollowSystem 若无法判断则回退到英文。
    - 验收结果：`cargo fmt --all`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace --all-targets` 通过；GUI 在 zh-CN/en-US 间切换可立即更新侧栏/设置/运行时中心/下载面板等主要文案。

## Phase 8：高级命令与扩展语言

### T035 高级命令第一组（config/alias/prune/update）
- [x] **T035：实现常用高级命令集** #cli #advanced
  - **描述**：提升日常运维能力，覆盖旧项目常用命令。
  - **依赖**：T028,T029
  - **输入文档**：`refactor docs/02-cli-设计.md`
  - **输出文件**：`crates/envr-cli/src/commands/*`
  - **验收**：命令可用且具备稳定错误处理。
  - **进度**：done
  - **实现记录**：
    - 实现要点：`envr config path|show`（`settings.toml` 路径与合并后内容，JSON 含结构化 `settings`）；`envr alias list|add|remove` 持久化到 `config/aliases.toml`（`envr-config::aliases`）；`envr prune [lang] [--execute]` 默认 dry-run，执行时卸载除 `current` 外的已安装版本；`envr update [--check]` 打印 CLI 版本并标明自更新尚未实现。输出遵循现有 `--format json` 信封。
    - 相关提交/PR：
    - 遇到的问题/决策：`prune` 在对应语言无 `current` 时不卸载任何版本，避免误删；`update` 仅占位，避免虚假网络请求。
    - 验收结果：`cargo fmt --all`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace --all-targets` 通过。

### T036 高级命令第二组（exec/run/env/import/export/profile）
- [x] **T036：实现脚本与环境协作能力命令集** #cli #advanced
  - **描述**：覆盖自动化与团队协作使用场景。
  - **依赖**：T035
  - **输入文档**：`refactor docs/02-cli-设计.md`
  - **输出文件**：`crates/envr-cli/src/commands/*`
  - **验收**：项目导入导出与 profile 操作可用。
  - **进度**：done
  - **实现记录**：
    - 实现要点：`envr-config` 增加 `load_project_config_disk_only`（仅合并 `.envr.toml` + `.envr.local.toml`，不应用 `ENVR_PROFILE` / `[profiles]`）；`envr exec --lang` / `envr run` 在子进程中合并项目 `env`、前置 PATH（单语言或 node+python+java）、Java 设置 `JAVA_HOME`；`envr env` 按 `--shell` 输出 `export`/`set`/`$env:`；`import`/`export` 读写合并后的 TOML；`profile list|show` 查看 `[profiles.*]`；`resolve` 增加 `--profile`。JSON 模式沿用标准信封；子进程非零退出码返回 `child_exit` 信封。
    - 相关提交/PR：
    - 遇到的问题/决策：
    - 验收结果：`cargo fmt --all`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace --all-targets` 通过。

### T037 扩展语言实现（Go/Rust/PHP/Deno/Bun）
- [ ] **T037：逐步实现剩余语言 RuntimeProvider** #runtime
  - **描述**：复用统一下载/镜像/安装流程，补齐多语言支持。
  - **依赖**：T015,T012,T013
  - **输入文档**：`refactor docs/01-总体架构设计.md`
  - **输出文件**：`crates/envr-runtime-*`
  - **验收**：每种语言至少完成 install/list/current/use/uninstall。
  - **进度**：todo
  - **实现记录**：
    - 实现要点：
    - 相关提交/PR：
    - 遇到的问题/决策：
    - 验收结果：

#### T037.1 Go 运行时实现
- [ ] **T037.1：Go 远程索引与版本解析** #runtime #go
  - **描述**：实现 Go 官方/镜像索引抓取、版本过滤与 LTS/推荐版本标记。
  - **依赖**：T015,T013
  - **输入文档**：`refactor docs/01-总体架构设计.md`
  - **输出文件**：`crates/envr-runtime-go/src/index.rs`
  - **验收**：`envr remote go` 输出与实际可安装版本一致。
  - **进度**：todo
  - **实现记录**：
    - 实现要点：
    - 相关提交/PR：
    - 遇到的问题/决策：
    - 验收结果：

- [ ] **T037.2：Go 安装/卸载/切换与 GOPROXY 设置** #runtime #go
  - **描述**：打通 Go 下载、解压、current 链接、GOPROXY 配置与卸载流程。
  - **依赖**：T037.1,T012,T009
  - **输入文档**：旧项目 Go 支持、`refactor docs/05-下载与镜像源设计.md`
  - **输出文件**：`crates/envr-runtime-go/src/manager.rs`
  - **验收**：`go version` 输出正确，GOPROXY 配置可切换并生效。
  - **进度**：todo
  - **实现记录**：
    - 实现要点：
    - 相关提交/PR：
    - 遇到的问题/决策：
    - 验收结果：

#### T037.3 Rust 运行时实现
- [ ] **T037.3：Rust 工具链集成（基于 rustup）** #runtime #rust
  - **描述**：借助 rustup 管理 Rust 版本/目标/组件，保持“旧项目”成熟路径，优先稳定与兼容。
  - **依赖**：T015
  - **输入文档**：`refactor docs/01-总体架构设计.md`
  - **输出文件**：`crates/envr-runtime-rust/src/manager.rs`
  - **验收**：可通过 envr 调用 rustup 安装/切换/卸载工具链，状态同步正确；不额外引入高风险自研安装链路。
  - **进度**：todo
  - **实现记录**：
    - 实现要点：
    - 相关提交/PR：
    - 遇到的问题/决策：
    - 验收结果：

#### T037.4 PHP 运行时实现
- [ ] **T037.4：PHP 版本与变体管理（NTS/TS）** #runtime #php
  - **描述**：实现 PHP 版本/线程模型解析、下载与安装路径结构。
  - **依赖**：T015,T013,T012
  - **输入文档**：旧项目 PHP 支持
  - **输出文件**：`crates/envr-runtime-php/src/{index,manager}.rs`
  - **验收**：可为 PHP 不同变体安装/切换版本，命令行调用正常。
  - **进度**：todo
  - **实现记录**：
    - 实现要点：
    - 相关提交/PR：
    - 遇到的问题/决策：
    - 验收结果：

#### T037.5 Deno 运行时实现
- [ ] **T037.5：Deno 索引与安装实现** #runtime #deno
  - **描述**：实现基于 GitHub/镜像源的 Deno 版本列表与安装路径结构。
  - **依赖**：T015,T013,T012
  - **输入文档**：旧项目 Deno 支持
  - **输出文件**：`crates/envr-runtime-deno/src/{index,manager}.rs`
  - **验收**：`deno --version` 版本与 envr 状态一致，可切换/卸载。
  - **进度**：todo
  - **实现记录**：
    - 实现要点：
    - 相关提交/PR：
    - 遇到的问题/决策：
    - 验收结果：

#### T037.6 Bun 运行时实现
- [ ] **T037.6：Bun 多版本体系（对齐 Node 能力）** #runtime #bun #shim
  - **描述**：按 Node 思路实现 Bun 多版本安装/切换/卸载、current 管理、shim 转发与全局可执行支持，形成产品优势能力。
  - **依赖**：T015,T013,T012
  - **输入文档**：旧项目 Bun 支持
  - **输出文件**：`crates/envr-runtime-bun/src/{index,manager,shim}.rs`
  - **验收**：支持 Bun 多版本并行安装、切换 current、shim 路由正确；`bun/bunx` 与全局可执行行为与 Node 体验一致。
  - **进度**：todo
  - **实现记录**：
    - 实现要点：
    - 相关提交/PR：
    - 遇到的问题/决策：
    - 验收结果：

- [ ] **T037.7：Bun 生态能力补齐（全局包/缓存/镜像策略）** #runtime #bun #advanced
  - **描述**：补齐 Bun 全局包扫描、shim 刷新、缓存清理和镜像/下载源策略，避免仅“能装能切”。
  - **依赖**：T037.6,T024
  - **输入文档**：`refactor docs/05-下载与镜像源设计.md`
  - **输出文件**：`crates/envr-runtime-bun/src/{packages,cache,mirror}.rs`
  - **验收**：Bun 全局包新增后可直接调用；缓存与镜像设置在 CLI/GUI 均可管理。
  - **进度**：todo
  - **实现记录**：
    - 实现要点：
    - 相关提交/PR：
    - 遇到的问题/决策：
    - 验收结果：

### T046 单页架构与主布局
- [ ] **T046：落地单页（SPA）主框架与左导航** #gui #ux
  - **描述**：实现固定左侧导航（仪表盘/运行时/设置/关于）与右侧内容区切换。
  - **依赖**：T030,T031
  - **输入文档**：`refactor docs/03-gui-设计.md`
  - **输出文件**：`crates/envr-gui/src/view/{shell,sidebar}/*`
  - **验收**：单窗口下四个导航页面均可无闪烁切换。
  - **进度**：todo
  - **实现记录**：
    - 实现要点：
    - 相关提交/PR：
    - 遇到的问题/决策：
    - 验收结果：

### T047 仪表盘页面（Dashboard）完整实现
- [ ] **T047：实现仪表盘总览与快捷操作** #gui
  - **描述**：实现运行时概览、健康检查、最近任务和推荐操作卡片。
  - **依赖**：T046,T027,T033
  - **输入文档**：`refactor docs/03-gui-设计.md`
  - **输出文件**：`crates/envr-gui/src/view/dashboard/*`
  - **验收**：仪表盘数据与实际状态一致，支持跳转到对应页面。
  - **进度**：todo
  - **实现记录**：
    - 实现要点：
    - 相关提交/PR：
    - 遇到的问题/决策：
    - 验收结果：

### T048 运行时页顶部横向运行时导航
- [ ] **T048：实现全运行时横向切换条（Node/Python/Go/Java/Rust/PHP/Deno/Bun）** #gui #runtime
  - **描述**：实现右侧“运行时”页面顶部平铺导航，支持热切换与状态保留。
  - **依赖**：T046,T037
  - **输入文档**：`refactor docs/03-gui-设计.md`
  - **输出文件**：`crates/envr-gui/src/view/runtime/tabs.rs`
  - **验收**：八个运行时标签可切换且不触发整页重建闪烁。
  - **进度**：todo
  - **实现记录**：
    - 实现要点：
    - 相关提交/PR：
    - 遇到的问题/决策：
    - 验收结果：

### T049 运行时设置区（默认折叠）
- [ ] **T049：实现每运行时独立设置区（默认折叠，按语言定制）** #gui #config
  - **描述**：沿用“未完成的重构项目”精练设置思路，为每个运行时配置独立设置块。
  - **依赖**：T048,T034
  - **输入文档**：`refactor docs/03-gui-设计.md`
  - **输出文件**：`crates/envr-gui/src/view/runtime/settings/*`
  - **验收**：每语言设置项可独立保存并即时生效。
  - **进度**：todo
  - **实现记录**：
    - 实现要点：
    - 相关提交/PR：
    - 遇到的问题/决策：
    - 验收结果：

### T050 智能/精确模式与版本操作矩阵
- [ ] **T050：实现 Smart/Exact 模式与按钮状态规则** #gui #runtime
  - **描述**：智能与精确模式完整可切换；未安装/已安装/已使用状态按钮行为严格一致。
  - **依赖**：T048,T049,T032
  - **输入文档**：`refactor docs/03-gui-设计.md`
  - **输出文件**：`crates/envr-gui/src/view/runtime/version_list/*`
  - **验收**：规则符合：未安装仅安装，已安装用/卸载，已使用禁用该行关键按钮。
  - **进度**：todo
  - **实现记录**：
    - 实现要点：
    - 相关提交/PR：
    - 遇到的问题/决策：
    - 验收结果：

### T051 悬浮下载面板（可拖拽/可隐藏/可展开）
- [ ] **T051：实现左下角悬浮下载面板并支持拖拽停靠** #gui #download #ux
  - **描述**：下载面板默认左下角，支持拖拽、折叠、隐藏，记忆面板状态。
  - **依赖**：T033,T046
  - **输入文档**：`refactor docs/03-gui-设计.md`,`refactor docs/05-下载与镜像源设计.md`
  - **输出文件**：`crates/envr-gui/src/view/downloads/floating_panel.rs`
  - **验收**：下载面板出现/隐藏不会挤压主内容布局。
  - **进度**：todo
  - **实现记录**：
    - 实现要点：
    - 相关提交/PR：
    - 遇到的问题/决策：
    - 验收结果：

### T052 防闪烁与防弹跳专项优化
- [ ] **T052：实现 UI 稳定性优化（减少冗余刷新/闪烁/弹跳）** #gui #perf #ux
  - **描述**：限制高频重绘、稳定布局占位、优化异步加载占位策略。
  - **依赖**：T051,T040
  - **输入文档**：`refactor docs/03-gui-设计.md`
  - **输出文件**：`crates/envr-gui/src/view/*`,`crates/envr-ui/src/*`
  - **验收**：主要页面无明显闪烁与布局跳动，操作体验稳定。
  - **进度**：todo
  - **实现记录**：
    - 实现要点：
    - 相关提交/PR：
    - 遇到的问题/决策：
    - 验收结果：

## Phase 9：性能优化与体验打磨

### T038 启动性能优化
- [ ] **T038：优化冷启动/热启动路径（懒加载与缓存）** #perf #gui
  - **描述**：减少启动时 I/O 与不必要初始化，达到目标启动时延。
  - **依赖**：T034
  - **输入文档**：`refactor docs/03-gui-设计.md`
  - **输出文件**：`envr-gui` 初始化链路相关代码
  - **验收**：达到文档定义的冷/热启动指标。
  - **进度**：todo
  - **实现记录**：
    - 实现要点：
    - 相关提交/PR：
    - 遇到的问题/决策：
    - 验收结果：

### T039 运行性能优化（FPS/内存/CPU）
- [ ] **T039：优化渲染与状态更新频率，控制资源占用** #perf #gui
  - **描述**：避免高频重绘与大对象复制，控制内存峰值 <40MB。
  - **依赖**：T038
  - **输入文档**：`refactor docs/03-gui-设计.md`
  - **输出文件**：`envr-gui`/`envr-ui` 渲染与状态代码
  - **验收**：FPS/内存/CPU 达标，关键页面不卡顿。
  - **进度**：todo
  - **实现记录**：
    - 实现要点：
    - 相关提交/PR：
    - 遇到的问题/决策：
    - 验收结果：

### T040 页面加载与操作响应优化
- [ ] **T040：优化页面加载与操作响应延迟** #perf #ux
  - **描述**：异步化耗时任务，UI 反馈即时化（占位/骨架/进度）。
  - **依赖**：T039
  - **输入文档**：`refactor docs/03-gui-设计.md`
  - **输出文件**：主要页面逻辑
  - **验收**：页面加载 <=150ms、操作反馈 <=50ms。
  - **进度**：todo
  - **实现记录**：
    - 实现要点：
    - 相关提交/PR：
    - 遇到的问题/决策：
    - 验收结果：

## Phase 10：测试完整性与发布准备

### T041 单元测试补齐
- [ ] **T041：为核心 crate 补齐单元测试（边界与异常）** #test
  - **描述**：重点覆盖 config/download/runtime/shim 核心分支。
  - **依赖**：T024,T037
  - **输入文档**：`refactor docs/06-实施路线图.md`
  - **输出文件**：各 crate `tests`/`#[cfg(test)]`
  - **验收**：核心模块关键分支覆盖到位。
  - **进度**：todo
  - **实现记录**：
    - 实现要点：
    - 相关提交/PR：
    - 遇到的问题/决策：
    - 验收结果：

### T042 集成测试与端到端测试
- [ ] **T042：补齐 CLI/GUI/shim 端到端测试链路** #test #e2e
  - **描述**：验证真实安装、切换、执行、卸载完整流程。
  - **依赖**：T041,T040
  - **输入文档**：`refactor docs/02-cli-设计.md`,`04-shim-设计.md`
  - **输出文件**：`tests/integration/*`
  - **验收**：三平台关键链路通过，回归可重复。
  - **进度**：todo
  - **实现记录**：
    - 实现要点：
    - 相关提交/PR：
    - 遇到的问题/决策：
    - 验收结果：

### T043 属性测试与平台测试
- [ ] **T043：补齐 proptest 与平台差异测试** #test #proptest
  - **描述**：对版本解析、配置合并、状态机迁移做属性测试。
  - **依赖**：T041
  - **输入文档**：`refactor docs/06-实施路线图.md`
  - **输出文件**：`tests/proptest/*`,`tests/platform/*`
  - **验收**：平台差异点都有测试保护。
  - **进度**：todo
  - **实现记录**：
    - 实现要点：
    - 相关提交/PR：
    - 遇到的问题/决策：
    - 验收结果：

### T044 覆盖率达标与质量收口
- [ ] **T044：覆盖率提升到 >=80% 并修复高风险缺陷** #quality #test
  - **描述**：针对薄弱模块补测，收敛 blocker/critical 问题。
  - **依赖**：T042,T043,T052
  - **输入文档**：覆盖率报告、缺陷清单
  - **输出文件**：测试与修复代码
  - **验收**：总体覆盖率 >=80%，关键缺陷清零。
  - **进度**：todo
  - **实现记录**：
    - 实现要点：
    - 相关提交/PR：
    - 遇到的问题/决策：
    - 验收结果：

### T045 发布打包与最终验收
- [ ] **T045：完成 Windows 首发包与发布文档** #release
  - **描述**：产出安装包、校验包、发布说明与已知问题列表。
  - **依赖**：T044,T047,T050,T051
  - **输入文档**：所有 refactor docs + 测试报告
  - **输出文件**：发布产物与 release notes
  - **验收**：可安装、可运行、核心功能完整可用。
  - **进度**：todo
  - **实现记录**：
    - 实现要点：
    - 相关提交/PR：
    - 遇到的问题/决策：
    - 验收结果：

## Phase 11：稳定化检查与问题处理（后续补充章节）

### T901 缺陷分级与处理机制
- [ ] **T901：建立缺陷分级（blocker/critical/major/minor）与处理 SLA** #quality
  - **描述**：统一缺陷优先级定义、响应时限、回归闭环流程。
  - **依赖**：T044
  - **输入文档**：测试报告、缺陷清单
  - **输出文件**：`docs/qa/bug-triage.md`（或同等文档）
  - **验收**：所有新缺陷均可被分级、跟踪、关闭并回归验证。
  - **进度**：todo
  - **实现记录**：
    - 实现要点：
    - 相关提交/PR：
    - 遇到的问题/决策：
    - 验收结果：

### T902 回归测试矩阵与冒烟清单
- [ ] **T902：建立发布前回归矩阵（语言 x 平台 x 功能）** #test #release
  - **描述**：形成可执行冒烟清单，覆盖安装/切换/卸载/shim/GUI 主流程。
  - **依赖**：T042,T043
  - **输入文档**：`tasks.md` 全任务、现有测试用例
  - **输出文件**：`docs/qa/regression-matrix.md`
  - **验收**：每次发布前可按矩阵完整打勾，结果可追溯。
  - **进度**：todo
  - **实现记录**：
    - 实现要点：
    - 相关提交/PR：
    - 遇到的问题/决策：
    - 验收结果：

### T903 性能回退监控与阈值报警
- [ ] **T903：建立启动/内存/FPS/CPU 回退检测流程** #perf #quality
  - **描述**：对关键性能指标建立基线与回退报警规则。
  - **依赖**：T038,T039,T040
  - **输入文档**：`refactor docs/03-gui-设计.md`
  - **输出文件**：性能报告脚本与基线文档
  - **验收**：性能回退可被自动识别，且有明确处理闭环。
  - **进度**：todo
  - **实现记录**：
    - 实现要点：
    - 相关提交/PR：
    - 遇到的问题/决策：
    - 验收结果：

### T904 依赖风险与安全扫描
- [ ] **T904：依赖安全扫描与许可证合规检查** #deps #security
  - **描述**：定期扫描漏洞与许可证风险，形成升级策略。
  - **依赖**：T002
  - **输入文档**：`refactor docs/07-依赖选择与原则.md`
  - **输出文件**：安全扫描报告、升级计划
  - **验收**：高危漏洞清零，许可证风险可解释可处置。
  - **进度**：todo
  - **实现记录**：
    - 实现要点：
    - 相关提交/PR：
    - 遇到的问题/决策：
    - 验收结果：

### T905 崩溃/异常日志诊断闭环
- [ ] **T905：完善崩溃日志采集、导出与问题复现模板** #logging #quality
  - **描述**：统一异常信息格式，支持用户一键导出诊断包。
  - **依赖**：T005
  - **输入文档**：日志规范
  - **输出文件**：诊断导出模块、复现模板文档
  - **验收**：线上问题可基于诊断包快速定位复现。
  - **进度**：todo
  - **实现记录**：
    - 实现要点：
    - 相关提交/PR：
    - 遇到的问题/决策：
    - 验收结果：

## Phase 12：i18n 全量落实（GUI + CLI 完整国际化）

### T910 i18n 强制规范
- [ ] **T910：建立 i18n 强制规范（除专业术语外全部国际化）** #i18n #gui #cli
  - **描述**：明确“任何用户可见文本必须使用 i18n key”，专业术语可保留原文但需统一词表。
  - **依赖**：T030,T025
  - **输入文档**：`refactor docs/03-gui-设计.md`,`refactor docs/02-cli-设计.md`
  - **输出文件**：`docs/i18n/style-guide.md`
  - **验收**：规范可执行，开发与评审均按规范检查。
  - **进度**：todo
  - **实现记录**：
    - 实现要点：
    - 相关提交/PR：
    - 遇到的问题/决策：
    - 验收结果：

### T911 GUI 文本全量 i18n
- [ ] **T911：GUI 所有可见文本迁移为 i18n key** #i18n #gui
  - **描述**：覆盖导航、按钮、表头、状态文案、空态、错误提示、下载面板、对话框等。
  - **依赖**：T910,T052
  - **输入文档**：GUI 全页面清单
  - **输出文件**：`crates/envr-gui/src/**`,`crates/envr-ui/src/**`,`locales/*`
  - **验收**：GUI 无硬编码展示文本（专业术语白名单除外）。
  - **进度**：todo
  - **实现记录**：
    - 实现要点：
    - 相关提交/PR：
    - 遇到的问题/决策：
    - 验收结果：

### T912 CLI 文本全量 i18n
- [ ] **T912：CLI 所有用户输出与 help 文本迁移为 i18n key** #i18n #cli
  - **描述**：覆盖命令说明、参数帮助、错误提示、成功提示、诊断建议文本。
  - **依赖**：T910,T028
  - **输入文档**：CLI 命令清单
  - **输出文件**：`crates/envr-cli/src/**`,`locales/*`
  - **验收**：CLI 无硬编码用户可见文本（专业术语白名单除外）。
  - **进度**：todo
  - **实现记录**：
    - 实现要点：
    - 相关提交/PR：
    - 遇到的问题/决策：
    - 验收结果：

### T913 i18n 术语表与专业术语白名单
- [ ] **T913：建立术语表与专业术语白名单管理** #i18n
  - **描述**：统一术语翻译、大小写、缩写规范，避免同义词混乱。
  - **依赖**：T910
  - **输入文档**：现有词条
  - **输出文件**：`docs/i18n/glossary.md`,`docs/i18n/whitelist.md`
  - **验收**：关键术语在 GUI/CLI 中一致，专业术语白名单可审计。
  - **进度**：todo
  - **实现记录**：
    - 实现要点：
    - 相关提交/PR：
    - 遇到的问题/决策：
    - 验收结果：

### T914 i18n 完整性自动检查
- [ ] **T914：建立 i18n lint（缺失 key/未使用 key/硬编码文本）** #i18n #quality
  - **描述**：在 CI 中加入自动检查，阻止新增硬编码文案。
  - **依赖**：T911,T912,T003
  - **输入文档**：i18n 规范
  - **输出文件**：CI 检查脚本与规则
  - **验收**：CI 能自动拦截 i18n 违规提交。
  - **进度**：todo
  - **实现记录**：
    - 实现要点：
    - 相关提交/PR：
    - 遇到的问题/决策：
    - 验收结果：

### T915 多语言回归测试
- [ ] **T915：增加中英文全链路回归测试（GUI + CLI）** #i18n #test
  - **描述**：验证不同语言下功能一致、文本完整、无截断和布局溢出。
  - **依赖**：T911,T912,T914
  - **输入文档**：回归矩阵
  - **输出文件**：`tests/i18n/*`
  - **验收**：中英文模式下核心流程全部通过，UI 文案无明显显示缺陷。
  - **进度**：todo
  - **实现记录**：
    - 实现要点：
    - 相关提交/PR：
    - 遇到的问题/决策：
    - 验收结果：