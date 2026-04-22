# 发布前回归矩阵与冒烟清单

本文档供 **每次发布前**（或重大变更合并前）按项打勾，保证安装、切换、卸载、shim、GUI 等主路径可复现。结果应可追溯（执行人、日期、构建版本、结论）。

**相关文档**：[缺陷分级 `bug-triage.md`](bug-triage.md)、[Windows 安装 `../release/WINDOWS.md`](../release/WINDOWS.md)、[已知问题 `../release/KNOWN-ISSUES.md`](../release/KNOWN-ISSUES.md)。

---

## 0. 本次执行元数据（发布前填写）

| 字段 | 内容 |
|------|------|
| 构建 / 标签 | 例：`v0.1.0` 或 commit SHA |
| 执行人 | |
| 日期 | |
| 平台 | 例：Windows 11 x64 |
| 结论 | 通过 / 阻塞 / 带已知问题发布（注明 Issue） |

---

## 1. 全局冒烟（与语言无关）

在隔离的 `ENVR_RUNTIME_ROOT`（或干净用户目录）下执行，避免污染本机主环境时可使用临时目录并 **仅在本轮回归** 中设置该变量。

- [ ] **`envr --help`** 可执行，子命令列表完整
- [ ] **`envr doctor`** 成功退出；输出中数据目录、路径无异常报错
- [ ] **`envr --format json doctor`**（若适用）JSON 信封字段齐全，与 `crates/envr-cli/tests/json_envelope.rs` 约定一致
- [ ] **`envr --format json list`** 成功（允许列表为空）
- [ ] **`envr init`** 在当前空目录生成 `.envr.toml`；**`envr check`** 在合理配置下通过或可理解的校验失败
- [ ] **`envr cache clean`**：按发布说明选择 `--all` 或指定 `KIND` 做一次**可接受**的清理验证（注意勿删生产缓存）
- [ ] **`envr shim sync`**（可选加 `--globals`）：在测试根下无报错或错误符合 `KNOWN-ISSUES`

---

## 2. 矩阵：平台 × 语言（核心 CLI）

**说明**：下列「语言」指 `envr` 的 runtime kind（`node` / `python` / `java` 等）。首发以 **Windows x86_64** 为必测；Linux / macOS 行在跨平台发布时必填。

### 2.1 Windows x86_64

#### Node

- [ ] **`envr list node`** 可执行
- [ ] **安装路径**：`envr install node <version>` **或** 使用已预置版本验证 **`envr use node <version>`** + 新开 shell 中 **`node -v`** 与 **`npm -v`**（或等价）符合预期
- [ ] **`envr current node`** 与 **`envr list node`** 一致
- [ ] **`envr which node`**（或文档约定的 shim 名）解析到预期可执行文件
- [ ] **`envr uninstall node <version>`** 在测试版本上可执行（或 `prune` 干跑 `--execute` 前确认计划）
- [ ] **npm 全局命令转发自动刷新**：执行 `npm install -g <pkg-with-bin>` 后，无需手动 `envr shim sync --globals`，直接可运行对应命令（例：`claude --version`）
- [ ] **pnpm/yarn 全局命令转发自动刷新**：执行 `pnpm add -g <pkg-with-bin>`、`yarn global add <pkg-with-bin>` 后，无需手动 sync，命令可直接运行
- [ ] **本地安装提示路径**：执行 `npm install <pkg-with-bin>`（不带 `-g`）后，确认有“本地安装不进全局 PATH”的提示或文档引导（如 `npx`）

#### Python

- [ ] **`envr list python`**
- [ ] **安装/切换**：`install`/`use` **或** 预置版本 + **`python -V` / `pip -V`**（路径经 envr）
- [ ] **`envr current python`**
- [ ] **`envr which python`**（或 `python3`）

#### Java

- [ ] **`envr list java`**
- [ ] **安装/切换** + **`java -version`**（或项目约定命令）
- [ ] **`envr current java`**
- [ ] **`envr which java`**；若需编译链，抽查 **`javac`**

### 2.2 Linux x86_64（跨平台发布时）

- [ ] 与 **§2.1** 同构用例至少各 **1 门语言**（建议 Node）跑通
- [ ] **`envr env`** 对 `posix` shell 输出可 `source` 后 PATH 含运行时 `bin`

### 2.3 macOS（跨平台发布时）

- [ ] 与 **§2.2** 相同最低要求；若有 **Liquid Glass / GUI** 专项，见 **§4**

---

## 3. 矩阵：功能域（抽查）

不要求每次全量，但 **Major / Critical 变更** 涉及下列域时应勾选相关行。

| 功能域 | 建议命令 / 场景 | 完成 |
|--------|-----------------|------|
| 远程索引 | `envr remote node`（可加 `--prefix`） | [ ] |
| 项目解析 | 含 `[runtimes.*]` 的目录下 **`envr resolve node`**、**`envr exec`** / **`envr run`** | [ ] |
| 配置 | **`envr config path`**、**`envr config show`** | [ ] |
| 别名 | **`envr alias list`**（若启用 aliases） | [ ] |
| 导入导出 | **`envr import`** / **`envr export`**（小文件夹场景） | [ ] |
| Profile | **`envr profile`** 子命令与 `ENVR_PROFILE` / `--profile` 一致性行为 | [ ] |
| Prune | **`envr prune`** 干跑；必要时在测试根 **`--execute`** | [ ] |
| Shim 刷新 | **`envr shim sync`** 与 **`envr-shim`** 联合：模拟 `node` argv 解析 | [ ] |

---

## 4. GUI 手测（与 T044 / T045 一致）

无头 CI 不统计 GUI 覆盖率；发布前 **至少** 完成下列手测。

- [ ] **`envr-gui`** 可启动，无立即崩溃
- [ ] **主导航**：进入「环境 / 运行时 / 设置」等与当前产品一致的主要视图
- [ ] **下载相关 UI**：触发一次与下载相关的流程（或确认离线时错误提示合理），含 **悬浮下载面板** 显示/拖拽/收起（T051）
- [ ] **设置页**：主题或语言切换（若已实现）保存或即时生效符合设计
- [ ] 关闭应用后无残留僵尸进程（任务管理器抽查）

---

## 5. 与自动化测试的对照

以下在 CI 中已有覆盖，发布前若 CI 全绿可 **在元数据表中注明「CI 已通过」**，无需重复手工执行；若 CI 跳过或仅测子集，仍建议对 **§1–§2** 做最小手工冒烟。

| 范围 | 参考 |
|------|------|
| CLI 集成 | `crates/envr-cli/tests/`（如 `e2e_flows.rs`、`json_envelope.rs`、`list_current_smoke.rs`） |
| 覆盖率门禁 | `cargo envr-cov`（见仓库根 `.cargo/config.toml`） |

---

## 6. 失败与放行

- 任一项失败：按 [`bug-triage.md`](bug-triage.md) 定级，**Blocker / Critical** 未关闭前默认 **不放行发布**。
- 带 **已知问题** 放行：必须在 **`docs/release/KNOWN-ISSUES.md`** 与发行说明中写明，并关联 Issue。

---

## 修订

矩阵行随产品命令演进更新；新增子命令或语言时在本文件增补勾选项，并在 PR 中说明。
