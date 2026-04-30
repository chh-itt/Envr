# envr 后续增强路线图（对标 asdf/mise 与差异化突破）

本文档记录 `envr` 在当前 CI 通过、命令面基本成型后的后续增强方向。它不是一次性实现清单，而是一个可持续拆解的设计 backlog：每个主题都可以继续展开为独立 RFC、实现计划、测试矩阵与用户文档。

目标不是把 `envr` 做成第二个 `asdf`，而是在吸收老牌版本管理器成熟经验的基础上，形成更清晰的差异化定位：

> `envr` 是一个跨平台运行时环境管理器，强调 Windows 一等支持、统一质量的内置 provider、可复现 lockfile、离线 bundle、自动化契约与可视化体验。

## 1. 背景与定位

### 1.1 当前已有基础

从现有命令谱系和文档看，`envr` 已具备较完整的运行时管理控制面：

- 核心生命周期：`install`、`use`、`list`、`current`、`uninstall`、`which`、`remote`、`doctor`。
- 项目协作：`init`、`project`、`status`、`check`、`why`、`hook`、`deactivate`。
- 自动化运行：`exec`、`run`、`env`、`template`、`profile`。
- 数据与运维：`shim`、`cache`、`bundle`、`diagnostics`、`debug`、`completion`。
- 多 runtime provider：Node、Python、Java、Go、Rust、Ruby、PHP、.NET、Deno、Bun、Zig、Dart、Flutter、JVM 系语言、Terraform 等。
- JSON envelope、schema、automation matrix、diagnostics、offline/bundle 等面向 CI 和自动化的能力。
- GUI 方向已有代码基础。

这意味着后续增强的重点不应只是继续堆 runtime 数量，而应转向：

1. 降低新用户首次上手成本；
2. 降低 `asdf` / `mise` 等用户迁移成本；
3. 增强版本解析、锁定、校验和离线可复现能力；
4. 强化 Windows/PowerShell 一等体验；
5. 建立高质量 provider 治理体系；
6. 形成 CLI、GUI、自动化系统都能依赖的稳定控制面。

### 1.2 与 asdf/mise 的差异化

`asdf` 的典型模型是：

- 轻核心；
- shell 插件生态；
- Unix-first；
- 社区扩展优先。

`envr` 更适合走另一条路线：

- 官方内置 provider 为主；
- 声明式 provider/descriptor 为辅；
- Windows 与 PowerShell 一等支持；
- lockfile 与离线 bundle 作为可复现能力核心；
- checksum/security 策略产品化；
- 机器可读 JSON 契约稳定；
- GUI/TUI 服务低门槛管理和诊断。

因此，本路线图把 `asdf` 视为功能参考对象，而不是产品路线模板。

## 2. 总体优先级

### 2.1 P0：发布与迁移基础

P0 目标是让陌生用户能安装、迁移并跑通核心路径。

1. GitHub Release based bootstrap installer；
2. `.tool-versions` import/export/read 兼容；
3. 统一版本请求解析：`latest`、`stable`、`lts`、major/minor prefix；
4. PowerShell hook 一等体验；
5. Quickstart、Windows guide、asdf migration guide；
6. 基础 checksum 校验与安装脚本校验。

### 2.2 P1：可复现与企业/CI 差异化

P1 目标是建立 `envr` 超过传统版本管理器的核心能力。

1. `.envr.lock`；
2. `envr sync --locked`；
3. `bundle create --from-lock`；
4. checksum policy：`strict` / `warn` / `permissive`；
5. mirror profile 与 per-runtime override；
6. 更完整的 JSON schema 与自动化契约覆盖；
7. offline/air-gapped workflow 闭环。

### 2.3 P2：生态、GUI 与规模化

P2 目标是提升长期扩展能力与用户体验上限。

1. managed tools / global tool abstraction；
2. provider health/status；
3. 声明式 runtime descriptor；
4. GUI runtime center 增强；
5. TUI；
6. 本地 provider override；
7. task runner 增强；
8. 轻量 env 管理。

## 3. GitHub Release based bootstrap installer

### 3.1 目标

当前阶段不必追求 winget、Homebrew、Chocolatey、MSI、deb/rpm 全覆盖。更现实的短期目标是：

- 每个 release 自动产出可下载 artifact；
- Windows、Linux、macOS 都能通过稳定 bootstrap 脚本安装；
- 安装脚本可重复执行、可卸载、可指定版本；
- README 中的一行安装命令长期有效；
- checksum 校验成为默认路径的一部分。

### 3.2 Windows 安装体验

建议提供：

```powershell
irm https://github.com/<org>/<repo>/releases/latest/download/install.ps1 -OutFile install.ps1
.\install.ps1
```

可选参数：

```powershell
.\install.ps1 -Version "v0.1.0"
.\install.ps1 -InstallDir "$env:USERPROFILE\.envr"
.\install.ps1 -NoModifyPath
.\install.ps1 -Yes
.\install.ps1 -Uninstall
```

需要覆盖的行为：

- 检测 CPU 架构；
- 选择正确 artifact；
- 下载 checksum；
- 校验 checksum；
- 解压到 install dir；
- 更新用户 PATH；
- 检测已有安装并原子替换；
- 支持 uninstall；
- 给出下一步 shell hook 引导。

### 3.3 Linux/macOS 安装体验

建议提供：

```bash
curl -fsSL -o install.sh https://github.com/<org>/<repo>/releases/latest/download/install.sh
sh install.sh
```

可选参数：

```bash
sh install.sh --version v0.1.0
sh install.sh --install-dir "$HOME/.envr"
sh install.sh --no-modify-path
sh install.sh --yes
sh install.sh --uninstall
```

### 3.4 Release artifact 约定

建议统一命名：

```text
envr-<version>-windows-x64.zip
envr-<version>-windows-arm64.zip
envr-<version>-linux-x64.tar.gz
envr-<version>-linux-arm64.tar.gz
envr-<version>-macos-x64.tar.gz
envr-<version>-macos-arm64.tar.gz
checksums.txt
checksums.txt.sig        # 可选
envr-<version>.sbom.json # 可选
```

### 3.5 验收标准

- 全平台安装脚本可在 CI smoke 中跑通；
- 可指定版本安装；
- 重复执行不会破坏已有安装；
- checksum mismatch 会中止；
- Windows PATH 修改失败时有明确下一步；
- `envr --version` 在安装结束后可用；
- README Quickstart 命令保持可复制运行。

### 3.6 后续深挖问题

- 是否需要支持 self-update？
- PATH 修改是否默认开启，还是默认提示用户确认？
- 安装脚本是否应独立版本化？
- 是否需要安装前检测 Defender/SmartScreen 误报风险？
- install script 是否需要 JSON/quiet 模式供自动化使用？

## 4. `.tool-versions` 兼容与 asdf 迁移

### 4.1 目标

降低 `asdf` 用户迁移成本。先做低风险 import/export，再逐步进入解析链路。

典型 `.tool-versions`：

```text
nodejs 22.11.0
python 3.12.7
ruby 3.3.5
golang 1.23.2
java temurin-21.0.4+7
```

### 4.2 Runtime 名称映射

需要维护默认映射表：

| asdf/plugin 名称 | envr runtime |
|---|---|
| `nodejs` | `node` |
| `golang` | `go` |
| `dotnet-core` | `dotnet` |
| `java` | `java` |
| `python` | `python` |
| `ruby` | `ruby` |
| `rust` | `rust` |
| `bun` | `bun` |
| `deno` | `deno` |
| `terraform` | `terraform` |

后续允许用户覆盖：

```toml
[compat.asdf.names]
nodejs = "node"
golang = "go"
dotnet-core = "dotnet"
```

### 4.3 阶段一：import/export

命令建议：

```bash
envr import tool-versions
envr import tool-versions --input .tool-versions --output .envr.toml
envr export tool-versions
envr export tool-versions --output .tool-versions
```

行为：

- 读取 `.tool-versions`；
- 映射 runtime 名称；
- 生成 `.envr.toml`；
- 对未知插件给 warning 和保留注释；
- 不自动覆盖已有 `.envr.toml`，除非 `--force`；
- 支持 dry-run。

### 4.4 阶段二：解析 fallback

解析优先级建议：

1. 显式 CLI 参数；
2. `.envr.toml`；
3. `.tool-versions`；
4. global current；
5. system。

`why` 输出需要解释来源：

```text
node resolved to 22.11.0
source: .tool-versions
reason: no node entry found in .envr.toml
```

### 4.5 阶段三：迁移文档

新增用户文档：

- `docs/cli/migration-asdf.md`；
- 解释命令对照；
- 解释 `.tool-versions` 与 `.envr.toml` 差异；
- 解释 lockfile 带来的可复现优势；
- 解释 Windows 支持差异。

### 4.6 验收标准

- 常见 `.tool-versions` 文件可成功 import；
- 未知插件不会导致整体失败；
- 名称映射有测试；
- `why` 能解释 `.tool-versions` 参与解析；
- export 后再 import 结果稳定；
- 文档给出 asdf -> envr 最短迁移路径。

### 4.7 后续深挖问题

- 是否支持 `.tool-versions` 中的 `system`？
- 一个 runtime 多版本写法如何处理？
- asdf 插件特有语义如何保留？
- `.tool-versions` 与 `.envr.lock` 的关系是什么？
- 是否默认读取 `.tool-versions`，还是需要配置开启？

## 5. 统一版本请求解析

### 5.1 目标

把用户请求与实际安装版本分离，支持人类习惯写法，同时为 lockfile 提供精确解析结果。

支持请求类型：

- exact：`22.11.0`；
- major prefix：`22`；
- minor prefix：`22.11`；
- alias：`latest`、`stable`、`lts`、`system`；
- range：`>=1.20 <1.23`；
- compatibility range：`~> 1.9`；
- runtime-specific channel：`temurin-21`、`graalvm-21` 等。

### 5.2 抽象建议

定义统一的 `VersionRequest`：

```text
VersionRequest
- Exact(version)
- Prefix(major, optional_minor)
- Alias(name)
- Range(expr)
- Channel(runtime_specific)
- System
```

Provider 暴露：

```text
ProviderVersionIndex
- available_versions(host)
- aliases(host)
- channels(host)
- prerelease_policy
- sorting_policy
- host_installability(version, host)
```

Resolver 执行：

1. 获取本地或远程 version index；
2. 按 host 过滤可安装版本；
3. 应用 request；
4. 处理 prerelease 策略；
5. 排序选出最佳版本；
6. 返回 resolved exact version 和 reason。

### 5.3 `why` 集成

示例：

```text
requested: lts
resolved: 22.11.0
source: .envr.toml
channel: lts
candidate_count: 42
selected_reason: highest version in active LTS channel
```

JSON 结构应包括：

- requested；
- resolved；
- source；
- candidates_count；
- filtered_reasons；
- selected_reason；
- provider；
- index_age；
- offline/online mode。

### 5.4 Runtime 特殊规则

需要逐步为不同 runtime 定义策略：

- Node：`lts`、`latest`、odd/even release line；
- Python：`3.12` prefix、pre-release 排除；
- Java：vendor + feature version，如 `temurin-21`；
- Go：`1.22` line；
- Terraform：semver range；
- Ruby：patchlevel 与 preview 排除；
- Rust：stable/beta/nightly 与 rustup 托管关系；
- .NET：SDK channel 与 runtime version 差异。

### 5.5 验收标准

- 统一 resolver 有独立测试；
- 主流 runtime 支持 `latest` 和 prefix；
- Node 支持 `lts`；
- 解析失败能给出候选建议；
- `why` 可解释解析过程；
- lockfile 记录 request 和 resolved；
- 离线模式使用缓存 index 时有明确提示。

### 5.6 后续深挖问题

- `latest` 是否允许 prerelease？
- range 语法采用 semver crate 还是自定义兼容层？
- 不符合 semver 的 runtime 如何排序？
- remote index 缓存过期策略是什么？
- GUI 中如何展示 request 与 resolved 的差异？

## 6. `.envr.lock` 与 locked sync

### 6.1 目标

把 `envr` 从“版本声明工具”提升为“可复现运行时环境工具”。

`.envr.toml` 表示用户意图：

```toml
[runtimes]
node = "lts"
python = "3.12"
terraform = "~> 1.9"
```

`.envr.lock` 表示精确结果：

```toml
version = 1

[[runtime]]
name = "node"
request = "lts"
resolved = "22.11.0"
host = "windows-x64"
source = "github-release"
url = "https://example.invalid/node-v22.11.0-win-x64.zip"
archive = "zip"
checksum = "sha256:..."
install_layout = "node-v22.11.0-win-x64"
provider = "envr-runtime-node"
provider_version = "0.1.0"
```

### 6.2 命令设计

建议命令：

```bash
envr lock
envr lock --update node
envr lock --refresh-checksums
envr sync
envr sync --locked
envr install --locked
envr check --locked
```

语义：

- `envr lock`：根据 `.envr.toml` 解析并写入 `.envr.lock`；
- `envr sync`：按 `.envr.toml` 安装缺失版本，可更新 lock；
- `envr sync --locked`：只按 `.envr.lock` 安装，不重新解析；
- `envr check --locked`：验证本机安装与 lock 一致；
- `envr lock --update node`：只更新指定 runtime。

### 6.3 Lockfile 字段建议

每个 runtime 记录：

- name；
- request；
- resolved；
- host triple；
- source type；
- source URL；
- checksum；
- archive kind；
- install layout；
- provider id；
- provider version；
- resolved_at；
- index metadata；
- mirror source；
- security level。

对于多平台团队，可考虑两种模式：

1. host-specific lock：每个平台生成自己的 section；
2. multi-host lock：同一个 lock 包含多个 host 的 resolved artifacts。

示例：

```toml
[[runtime.targets]]
host = "windows-x64"
url = "..."
checksum = "sha256:..."

[[runtime.targets]]
host = "linux-x64"
url = "..."
checksum = "sha256:..."
```

### 6.4 与 bundle/offline 的关系

强组合：

```bash
envr lock
envr bundle create --from-lock --output envr-bundle.zip
envr bundle apply envr-bundle.zip
envr sync --locked --offline
envr check --locked
```

bundle 应携带：

- `.envr.lock`；
- runtime archives；
- checksum manifest；
- provider metadata；
- optional tool packages metadata；
- bundle manifest version。

### 6.5 验收标准

- lockfile schema 固化；
- `sync --locked` 不访问远程解析；
- checksum mismatch 会阻止安装；
- lockfile 变更可读、可 review；
- 多平台策略有明确文档；
- bundle from lock 可闭环；
- CI 中可只依赖 lock 复现环境。

### 6.6 后续深挖问题

- lockfile 是否提交到 VCS？默认建议是什么？
- multi-host lock 是否会过大？
- provider version 升级是否导致 lock invalid？
- mirror URL 与 official URL 如何同时记录？
- lockfile 是否需要签名？

## 7. Checksum、signature 与安全策略

### 7.1 目标

建立可解释、可配置、可审计的下载安全模型。

不同上游提供的校验能力不同，不能简单一刀切。因此需要安全等级。

### 7.2 安全等级

建议定义：

| 等级 | 含义 |
|---|---|
| `verified` | 官方 checksum/signature 验证通过 |
| `pinned` | lockfile 中有 checksum，且匹配 |
| `envr-index` | envr 维护的 manifest checksum 匹配 |
| `unverified` | 没有 checksum，只能下载 |
| `blocked` | 当前策略禁止安装 |

### 7.3 策略配置

```toml
[security]
checksum_policy = "strict" # strict / warn / permissive
allow_unverified = false
require_lock_checksum = true
```

命令参数：

```bash
envr install node@22 --checksum-policy strict
envr doctor security
envr lock --refresh-checksums
```

### 7.4 Provider checksum 来源

每个 provider 文档应说明：

- checksum 是否来自 upstream；
- signature 是否可验证；
- 是否使用 envr-maintained manifest；
- 不支持 checksum 的原因；
- strict mode 下是否可安装。

### 7.5 GUI 展示

GUI 可以显示：

- 绿色：verified；
- 黄色：unverified/warn；
- 红色：mismatch/blocked；
- 可点击查看 checksum 来源与策略解释。

### 7.6 验收标准

- download 层统一返回 checksum/security metadata；
- lockfile 记录 checksum 与安全等级；
- strict mode 下 unverified artifact 被阻止；
- mismatch 错误有稳定 error code；
- `doctor security` 能汇总风险；
- 文档解释不同策略适合的场景。

### 7.7 后续深挖问题

- 是否维护官方 checksum index？
- 签名验证是否纳入 P1，还是 P2？
- Windows Authenticode 是否要支持？
- strict mode 对没有 checksum 的上游是否过于严格？
- 企业内网 artifact 如何声明可信？

## 8. PowerShell 与 shell 自动激活

### 8.1 目标

把 Windows/PowerShell 体验做成 `envr` 的明确优势。

### 8.2 PowerShell hook 命令

建议：

```powershell
envr hook init powershell
envr hook init powershell --dry-run
envr hook init powershell --yes
envr hook uninstall powershell
envr hook status
envr hook doctor
```

行为：

- 检测 PowerShell profile；
- 输出将写入内容；
- 默认交互确认；
- 支持 `--yes`；
- 支持卸载；
- 检查 hook 是否生效；
- 失败时给出手动修复命令。

### 8.3 自动激活要求

- `cd` 到项目目录自动加载 `.envr.toml`；
- 离开项目目录恢复旧环境；
- 嵌套项目正确切换；
- 不明显拖慢 prompt；
- 错误不污染终端；
- 支持禁用：`ENVR_AUTO_ACTIVATE=0`；
- 支持 prompt 摘要，但默认不吵。

### 8.4 Bash/Zsh/Fish

PowerShell 完成后，按同样模型推广到：

```bash
envr hook init bash
envr hook init zsh
envr hook init fish
```

### 8.5 验收标准

- PowerShell hook 有 smoke test；
- init/uninstall 可重复执行；
- PATH 恢复正确；
- 嵌套项目切换正确；
- `hook doctor` 能发现常见配置问题；
- 文档覆盖 VSCode terminal、Windows Terminal、普通 PowerShell。

### 8.6 后续深挖问题

- 是否默认自动写 profile，还是只输出片段？
- prompt 信息如何避免干扰？
- hook 是否需要缓存解析结果？
- Windows 下 PATH 长度问题如何处理？
- GUI 是否提供 hook 安装向导？

## 9. Mirror profile 与中国大陆优化源

### 9.1 目标

改善国内网络环境下的下载成功率，同时保持默认官方源和可审计行为。

### 9.2 配置模型

默认：

```toml
[mirror]
profile = "official"
fallback = "none"
```

中国大陆优化：

```toml
[mirror]
profile = "china"
fallback = "official"
```

Per-runtime override：

```toml
[mirror.node]
base_url = "https://npmmirror.com/mirrors/node"

[mirror.python]
base_url = "https://internal.example.com/python"
```

### 9.3 命令设计

```bash
envr mirror list
envr mirror test
envr mirror test node
envr config set mirror.profile china
envr config set mirror.fallback official
```

### 9.4 行为要求

- 默认永远使用 official；
- china profile 是推荐配置，不承诺永久可用；
- mirror 下载后仍进行 checksum 校验；
- fallback 行为必须可见；
- lockfile 同时记录 requested source 与 actual source；
- 企业用户可配置 internal mirror。

### 9.5 验收标准

- mirror resolver 有测试；
- fallback 有明确日志和 JSON 字段；
- checksum 仍基于可信 manifest；
- `mirror test` 能给出延迟和可用性；
- 文档说明安全边界。

### 9.6 后续深挖问题

- 是否内置 china profile？
- 如何维护镜像 URL 变化？
- mirror 与 lockfile URL 如何交互？
- 企业镜像是否需要认证？
- GUI 是否提供镜像测速？

## 10. Managed tools / 全局工具管理

### 10.1 目标

统一管理 runtime 下产生的常用 CLI 工具，而不是替代 npm/pip/cargo 等包管理器。

定位：

> managed tools 是 `envr` 管理的、可通过 shim 暴露的开发工具可执行文件。

### 10.2 用户场景

- Node：`prettier`、`eslint`、`typescript`；
- Python：`ruff`、`black`、`mypy`；
- Rust：`ripgrep`、`cargo-nextest`；
- Go：`gopls`、`staticcheck`；
- Ruby：`bundler`；
- .NET：`dotnet-ef`；
- Deno/Bun：installed executables。

### 10.3 命令设计

```bash
envr tool install node prettier
envr tool install python ruff@0.8
envr tool install rust ripgrep@14
envr tool list
envr tool uninstall python ruff
envr tool sync
envr tool which ruff
```

项目声明：

```toml
[tools.node]
prettier = "latest"
eslint = "9"

[tools.python]
ruff = "0.8"
black = "24"

[tools.rust]
ripgrep = "14"
```

### 10.4 抽象建议

```text
ToolProvider
- install_tool(runtime_home, name, version)
- uninstall_tool(runtime_home, name)
- list_tools(runtime_home)
- tool_bin_dirs(runtime_home)
- sync_shims(runtime_home)
```

### 10.5 与 shim 的关系

- tool 安装后自动刷新 shim；
- 支持 project-local tool 解析；
- 支持 global tool 解析；
- `envr which` 能解释工具来自哪个 runtime/version。

### 10.6 验收标准

- 先支持 Node 与 Python；
- tool shim 可稳定生成；
- `tool list` 可区分 runtime 与版本；
- project tools 可通过 `envr sync` 安装；
- `why/which` 能解释工具解析来源；
- GUI 可显示已安装工具。

### 10.7 后续深挖问题

- Python 应使用 pipx、uv tool 还是 pip？
- Node 支持 npm、pnpm、yarn、bun 哪些路径？
- tool 是否进入 `.envr.lock`？
- tool 的 checksum 如何处理？
- project-local 与 global tool 冲突时谁优先？

## 11. Provider 质量治理

### 11.1 目标

用高质量官方 provider 替代随机插件生态的不确定性。

每个 provider 应明确：

- 支持平台矩阵；
- remote index 策略；
- checksum 策略；
- install layout；
- smoke test；
- uninstall test；
- upgrade test；
- known issues；
- release lag；
- security level。

### 11.2 命令设计

```bash
envr provider status
envr provider status --format json
envr provider doctor node
envr provider matrix
envr provider health
```

输出示例：

```text
node       windows yes   linux yes   macos yes   checksum verified
python     windows yes   linux yes   macos yes   checksum partial
racket     windows yes   linux no    macos no    checksum unknown
```

### 11.3 Provider health 指标

- upstream reachable；
- remote index fresh；
- latest resolvable；
- current host asset available；
- checksum available；
- install layout valid；
- shim target valid；
- docs/platform matrix in sync。

### 11.4 验收标准

- provider status 覆盖所有内置 runtime；
- JSON 输出稳定；
- platform support matrix 可由数据生成或至少自动校验；
- provider doctor 能发现常见问题；
- 新 provider PR 必须补 health metadata。

### 11.5 后续深挖问题

- provider health 是否应作为 CI gate？
- release lag 如何定义？
- provider metadata 放代码还是文档？
- GUI 是否展示 provider health？
- 是否需要 provider badge？

## 12. 声明式 runtime descriptor

### 12.1 目标

降低新增简单 GitHub Release 型 runtime 的成本，同时保持官方审核和质量一致。

这不是传统 shell plugin 机制，而是受限、可验证、可测试的声明式 provider。

### 12.2 Descriptor 示例

```toml
[id]
name = "foo"
display_name = "Foo"

[source]
type = "github-release"
repo = "owner/foo"

[assets.windows.x64]
pattern = "foo-{version}-windows-x64.zip"

[assets.linux.x64]
pattern = "foo-{version}-linux-x64.tar.gz"

[assets.macos.arm64]
pattern = "foo-{version}-macos-arm64.tar.gz"

[layout]
bin = ["foo"]

[security]
checksum = "github-release-asset"
```

### 12.3 支持范围

第一阶段仅支持：

- GitHub Release；
- 静态 asset pattern；
- zip/tar.gz 解压；
- 固定 bin 路径；
- 简单 checksum；
- host mapping。

不支持：

- 任意 shell 脚本；
- 安装后复杂编译；
- 系统级依赖修改；
- 交互安装器；
- 复杂 registry API。

### 12.4 验收标准

- descriptor schema 固化；
- descriptor lint；
- descriptor -> provider metadata；
- descriptor runtime 可参与 `remote/install/which/uninstall`；
- CI 对 descriptor runtime 做 smoke；
- 文档说明适用边界。

### 12.5 后续深挖问题

- descriptor 是否允许用户本地加载？
- 本地 descriptor 是否默认禁用？
- 如何防止恶意 URL？
- descriptor 是否支持 mirror？
- descriptor provider 与 Rust provider 如何共存？

## 13. GUI 与 TUI 增强

### 13.1 GUI 定位

GUI 不应复制 CLI 的所有能力，而应服务最适合图形界面的场景：

- 当前项目状态；
- runtime 安装状态；
- 一键安装/卸载/切换；
- 下载进度与失败重试；
- doctor 诊断结果；
- settings 简化配置；
- mirror 配置；
- hook 安装向导；
- bundle create/apply 向导。

### 13.2 首页建议

首页只展示：

1. 当前项目；
2. 当前激活 runtime；
3. 缺失 runtime；
4. 一键修复；
5. 下载队列；
6. 诊断入口。

高级能力放二级页面。

### 13.3 GUI 与 CLI 关系

GUI 应复用 CLI/core service，而不是复制逻辑：

- install/use/check/doctor 走同一服务层；
- JSON/structured outcome 供 GUI 渲染；
- 错误 code 与 next_steps 统一；
- 下载状态来自统一 control plane。

### 13.4 TUI

TUI 可作为 P2/P3：

- runtime list；
- install queue；
- doctor results；
- project status；
- mirror selector。

不建议早于 lockfile、hook、installer 投入大量时间。

### 13.5 验收标准

- GUI 首屏不超过 5 个主操作；
- GUI 能完成缺失 runtime 修复；
- GUI 能展示下载失败原因和重试；
- GUI 能引导安装 PowerShell hook；
- GUI 不引入独立解析逻辑。

### 13.6 后续深挖问题

- GUI 是否跟随 CLI release 一起发布？
- GUI 是否支持便携版？
- GUI 设置写入用户配置还是项目配置？
- GUI 如何展示 lockfile diff？
- TUI 是否有真实用户需求？

## 14. Task runner / scripts 增强

### 14.1 目标

`envr run` 的核心价值是：在正确 runtime 环境中运行项目任务。它不应过早发展成复杂构建系统。

### 14.2 第一阶段

```toml
[scripts]
test = "cargo test --workspace"
lint = "cargo clippy --workspace --all-targets"
dev = "npm run dev"
```

命令：

```bash
envr run
envr run test
envr run lint
```

### 14.3 第二阶段

```toml
[scripts.build]
cmd = "cargo build --release"
description = "Build release binary"
depends = ["lint", "test"]
env = { RUST_LOG = "info" }
cwd = "."
```

可支持：

- description；
- depends；
- env；
- cwd；
- shell mode；
- JSON list；
- completion。

### 14.4 边界

暂不做：

- 复杂 DAG 调度；
- 缓存系统；
- watch mode；
- 远程执行；
- 替代 Make/Ninja/Just。

### 14.5 验收标准

- `envr run --list` 输出脚本列表；
- script 运行继承解析后的 runtime PATH；
- 错误 code 稳定；
- JSON 输出包含 command、exit_code、duration；
- 文档说明与 npm scripts/just/make 的关系。

### 14.6 后续深挖问题

- 是否需要 `envr task` 别名或子命令？
- depends 是否并行？
- script 是否进入 lockfile？
- Windows shell quoting 如何保证？
- GUI 是否展示 scripts？

## 15. 轻量 env 管理与 direnv 边界

### 15.1 目标

支持项目运行所需的非 secret 环境变量，但不急于进入完整 secret/direnv 领域。

### 15.2 建议范围

可以支持：

```toml
[env]
FOO = "bar"
RUST_LOG = "info"
```

能力：

- `envr env` 输出 shell 片段；
- `envr run` 自动注入；
- `envr check` 检查必需变量；
- `envr env diff` 展示变化。

### 15.3 暂缓范围

暂缓：

- `.envrc` 任意 shell 执行；
- secret 加密；
- secret store；
- cloud secret provider；
- 自动加载不可信脚本。

### 15.4 验收标准

- 非 secret env 注入稳定；
- shell quoting 正确；
- 不把 secret 打到日志；
- 文档明确不建议存储敏感信息；
- `diagnostics` 自动脱敏。

### 15.5 后续深挖问题

- 是否读取 `.env` / `.env.local`？
- 是否需要 trust/allowlist？
- diagnostics 脱敏规则如何配置？
- GUI 是否允许编辑 env？
- 与 hook 自动激活如何交互？

## 16. C/C++ 与 native toolchain 边界

### 16.1 判断

C/C++ 不应被当作普通 runtime 直接纳入短期目标。它更像工具链集合：

- compiler；
- linker；
- libc/runtime；
- debugger；
- build system；
- package manager；
- sysroot；
- shell environment。

Windows 下还涉及：

- MSVC；
- MinGW；
- MSYS2；
- LLVM/Clang；
- Windows SDK；
- CMake；
- Ninja；
- vcpkg；
- Conan。

### 16.2 短期建议

只管理周边工具，不承诺完整 C/C++ toolchain abstraction：

- `cmake`；
- `ninja`；
- `llvm`；
- `zig`；
- `vcpkg` 可探索；
- `msys2` 可先 discovery/register，不做完整托管。

### 16.3 未来 native profile

未来可设计：

```toml
[native]
compiler = "msvc"
cmake = "3.30"
ninja = "1.12"
sdk = "10.0.22621"
```

但这应作为独立 RFC，不进入当前短期主线。

### 16.4 后续深挖问题

- 是否支持 Visual Studio Build Tools discovery？
- 是否支持 `vcvarsall` 环境捕获？
- MSYS2 是否作为 runtime 还是外部环境？
- vcpkg/conan 是否属于 managed tools？
- native profile 是否会显著增加维护成本？

## 17. 自动化契约与可观测性

### 17.1 目标

继续强化 `envr` 相对传统版本管理器的自动化优势。

### 17.2 增强方向

- 所有核心命令稳定 JSON schema；
- machine-readable error code；
- `next_steps` 结构化；
- `operation_id` / `trace_id`；
- `--github-annotations`；
- `doctor --fix --dry-run --format json`；
- diagnostics zip 中包含脱敏配置、日志、环境快照、provider 状态。

### 17.3 CI 集成

示例：

```bash
envr check --locked --format json
envr check --github-annotations
envr diagnostics export --output envr-diagnostics.zip
```

### 17.4 验收标准

- P0 命令 JSON schema 固化；
- stdout 不被日志污染；
- parse error 之外主路径都能输出 envelope；
- GitHub annotations 输出可被 CI 消费；
- diagnostics 默认脱敏。

### 17.5 后续深挖问题

- clap parse error 是否需要统一 envelope？
- schema version 如何演进？
- diagnostics 中哪些字段必须脱敏？
- 是否支持 SARIF？
- metrics 是否本地生成而不上传？

## 18. 文档体系增强

### 18.1 用户文档

建议补齐：

- Quickstart；
- Windows guide；
- PowerShell hook guide；
- asdf migration；
- lockfile guide；
- offline/bundle guide；
- mirror guide；
- checksum/security guide；
- troubleshooting；
- GUI guide。

### 18.2 贡献者文档

建议补齐：

- provider authoring guide；
- provider quality checklist；
- descriptor schema guide；
- release checklist；
- installer maintenance guide；
- JSON contract evolution guide。

### 18.3 文档原则

- 用户文档优先任务导向；
- 架构文档记录权衡；
- runtime 文档说明平台差异；
- 每个高级功能都提供最短路径与深挖链接；
- 中文/英文保持核心路径同步。

## 19. 建议实施节奏

### 19.1 Milestone A：早期公开可用

- bootstrap installer；
- `.tool-versions` import/export；
- PowerShell hook init/status/doctor；
- `latest`/prefix resolver；
- Quickstart/Windows/asdf migration 文档。

### 19.2 Milestone B：可复现环境

- `.envr.lock` schema；
- `envr lock`；
- `sync --locked`；
- checksum policy；
- `check --locked`；
- lockfile guide。

### 19.3 Milestone C：离线与企业闭环

- `bundle create --from-lock`；
- `sync --locked --offline`；
- mirror profile；
- `mirror test`；
- diagnostics/security doctor；
- air-gapped guide。

### 19.4 Milestone D：生态扩展

- managed tools；
- provider status/health；
- descriptor provider；
- GUI runtime center；
- provider quality docs。

## 20. 非目标与风险控制

### 20.1 短期非目标

- 完整 `asdf` shell plugin 兼容；
- 完整 C/C++ toolchain abstraction；
- secret manager；
- 复杂 task DAG/cache；
- 包管理器替代品；
- 所有平台包管理渠道一次性覆盖。

### 20.2 风险

| 风险 | 缓解 |
|---|---|
| 功能膨胀 | 按 P0/P1/P2 分阶段，新增 top-level 命令走准入 |
| provider 维护压力 | descriptor + provider health + 文档矩阵 |
| Windows PATH/hook 复杂 | 先做 PowerShell 深度打磨，再扩展其他 shell |
| checksum 来源不一致 | 安全等级 + policy + provider 文档 |
| lockfile 设计过重 | 先 host-specific，再评估 multi-host |
| GUI 复杂化 | GUI 只承载最适合图形界面的核心任务 |
| mirror 可信问题 | 默认 official，mirror 下载仍校验 checksum |

## 21. 单点深挖模板

每个主题进入实现前，建议补一份独立设计文档，回答以下问题：

1. 用户任务是什么？
2. 与现有命令/配置如何衔接？
3. 是否需要新增命令？能否复用现有命令？
4. 人类输出长什么样？
5. JSON 输出长什么样？
6. 错误 code 和 next_steps 如何设计？
7. Windows/Linux/macOS 差异是什么？
8. 离线模式行为是什么？
9. checksum/security 行为是什么？
10. 是否进入 lockfile？
11. 是否影响 GUI？
12. 是否需要 i18n？
13. 测试矩阵是什么？
14. 文档需要更新哪些？
15. Definition of Done 是什么？

## 22. 总结

`envr` 后续不应简单追求“更多 runtime”或“复制 asdf 插件生态”。更值得投入的是：

1. 可安装；
2. 可迁移；
3. 可解释；
4. 可复现；
5. 可校验；
6. 可离线；
7. 可自动化；
8. Windows 体验足够好；
9. provider 质量足够稳定；
10. GUI/TUI 降低非专家用户门槛。

如果上述路线逐步完成，`envr` 将从一个多 runtime manager 成长为一个可靠的项目运行时环境控制平面。它与 `asdf` 的竞争点不在插件数量，而在一致性、可复现性、Windows 友好、企业离线和自动化契约。