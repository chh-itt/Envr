# Windows 安装与校验

## 环境要求

- **Windows 10/11** x86_64
- **Visual C++ 可再发行组件**（多数机器已具备；若启动 `envr-gui` 报缺少 `VCRUNTIME` 等，请安装 [Microsoft VC++ Redistributable](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist)）

## 安装步骤（便携 zip）

1. 下载发布页中的 `envr-windows-x86_64-<version>.zip`。
2. 解压到任意目录，例如 `C:\Tools\envr\`。
3. 将该目录加入 **用户或系统 PATH**，以便在终端中直接运行 `envr`。
4. （可选）若使用 **shim**，需将同一目录中的 `envr-shim.exe` 与由 `envr` 管理的启动器一并配置；详见主文档中的 shim 说明。

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

删除解压目录，并从 PATH 中移除对应条目。用户数据默认位于 `%APPDATA%\envr`（或 `ENVR_ROOT` / 平台路径逻辑所指向的位置），如需彻底清理可一并删除。
