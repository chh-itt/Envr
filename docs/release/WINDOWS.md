# Windows 安装与校验

## 环境要求

- **Windows 10/11** x86_64
- **Visual C++ 可再发行组件**（多数机器已具备；若启动 `envr-gui` 报缺少 `VCRUNTIME` 等，请安装 [Microsoft VC++ Redistributable](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist)）

## 安装步骤（便携 zip）

1. 下载发布页中的 `envr-windows-x86_64-<version>.zip`。
2. 解压到任意目录，例如 `C:\Tools\envr\`。
3. 将该目录加入 **用户或系统 PATH**，以便在终端中直接运行 `envr`。
4. （可选）若使用 **shim**，需将同一目录中的 `envr-shim.exe` 与由 `envr` 管理的启动器一并配置；详见主文档中的 shim 说明。

> 便携 zip 默认包含：`envr.exe`、`er.exe`、`envr-gui.exe`、`envr-shim.exe`。

## 安装步骤（MSI）

1. 下载 `envr-windows-x64-<version>.msi`。
2. 双击安装，默认安装到 `C:\Program Files\envr\`。
3. 安装器会将安装目录追加到 **机器 PATH**；重新打开终端后生效。
4. 安装完成后可执行 `envr --help` 和 `envr-gui` 验证。

## 安装步骤（setup.exe，引导器）

1. 下载 `envr-setup-x64-<version>.exe`。
2. 双击运行，引导器会先检测并安装 VC++ Runtime（如已安装则跳过）。
3. 然后自动安装 `envr-windows-x64-<version>.msi`。
4. 完成后重新打开终端，执行 `envr --help` / `envr-gui` 验证。

## 校验包完整性

在解压目录中应存在 `SHA256SUMS.txt`。在 PowerShell 中可核对单个文件：

```powershell
Get-FileHash -Algorithm SHA256 .\envr.exe
# 与 SHA256SUMS.txt 中对应行比对
```

发布页同时提供压缩包级别的 `SHA256SUMS-archive.txt`（或 Release 正文中的校验和），用于核对 **zip 未损坏、未被替换**。

## 首次运行建议

1. 执行 `envr --help` 确认 CLI 可用。
2. 执行 `envr doctor`（或 `--format json`）检查数据目录与运行时根路径。
3. 再启动 `envr-gui` 做图形界面冒烟（网络、下载、权限等依环境而异）。

## 卸载

- **zip 便携版**：删除解压目录，并从 PATH 中移除对应条目。
- **MSI 安装版**：在“设置 -> 应用 -> 已安装的应用”中卸载 `envr`，或执行 `msiexec /x envr-windows-x64-<version>.msi`。
- **setup 安装版**：卸载主程序同 MSI（setup 仅负责引导，不替代 MSI 的卸载入口）。

用户数据默认位于 `%APPDATA%\envr`（或 `ENVR_ROOT` / 平台路径逻辑所指向的位置），如需彻底清理可一并删除。
