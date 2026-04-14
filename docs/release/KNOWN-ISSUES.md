# 已知问题（Known issues）

发布前请根据当前里程碑更新本列表；关闭的问题移至发行说明或从列表删除。

## 通用

- **MSI 依赖 WiX 构建环境**：仓库提供了手动 MSI 脚本，发布机需预装 WiX v4 CLI。
- **setup 依赖联网或本地 redist 文件**：默认会下载 `vc_redist.x64.exe`；离线发布机需通过 `-VcRedistPath` 提供本地文件。
- **VC++ 运行库前置**：若目标机器缺少 `VCRUNTIME140.dll` 等，`envr-gui` 仍会启动失败；当前需先安装 Microsoft VC++ Redistributable。
- **网络与镜像**：安装/列表依赖网络与镜像可达性；受限网络环境需配置镜像或离线准备运行时包（能力随版本演进）。

## GUI

- **无头 / 远程桌面**：部分 GPU 驱动或会话下 `envr-gui` 可能无法创建 wgpu 上下文；可回退使用 CLI。
- **首次启动耗时**：冷启动可能较慢，属 Iced/图形栈加载正常现象。

## CLI / 运行时

- **个别语言 provider**：部分 `envr-runtime-*` 在无网络或异常索引时可能报错；以 `envr doctor` 与具体子命令错误信息为准。

## 测试与 CI

- **全 workspace 合并覆盖率**：无头 CI 下合并含 GUI 的 llvm-cov 比例偏低；质量门禁以 `cargo envr-cov`（见 `.cargo/config.toml`）为准。
