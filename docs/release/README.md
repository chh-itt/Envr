# envr 发布说明（Release）

本目录收录 **Windows 首发**相关的安装说明、发行说明模板与已知问题，供发布流程与 GitHub Release 页面复用。

| 文件 | 说明 |
|------|------|
| [WINDOWS.md](WINDOWS.md) | Windows 下解压、PATH、首启与校验包用法 |
| [RELEASE-NOTES.md](RELEASE-NOTES.md) | 按版本维护的发行说明（含 0.1.0 首发条目） |
| [KNOWN-ISSUES.md](KNOWN-ISSUES.md) | 已知问题与限制（发布前更新） |

## 本地打 Windows 包（x86_64）

在仓库根目录执行（需已安装 Rust stable 与 MSVC 工具链）：

```powershell
.\scripts\package-windows-release.ps1 -Version 0.1.0
```

产物默认在 `dist/`：

- `envr-windows-x86_64-<version>/` — `envr.exe`、`er.exe`、`envr-gui.exe`、`envr-shim.exe` 及 `SHA256SUMS.txt`
- `envr-windows-x86_64-<version>.zip` — 同上目录的压缩包
- `SHA256SUMS-archive.txt` — 压缩包本身的 SHA256

## 本地打 MSI 安装包（手动流程）

需先安装 **WiX v4 CLI**（仅需一次）：

```powershell
dotnet tool install --global wix
```

若安装过 WiX 7 且出现 `wixext ... damaged`，可先重装：

```powershell
dotnet tool uninstall --global wix
dotnet tool install --global wix
```

然后在仓库根目录执行：

```powershell
.\scripts\package-windows-msi.ps1 -Version 0.1.0
```

产物默认在 `dist/`：

- `envr-windows-x64-<version>.msi` — MSI 安装包（包含 `envr.exe`、`er.exe`、`envr-gui.exe`、`envr-shim.exe`，并写入机器 PATH）
- `SHA256SUMS-msi.txt` — MSI 的 SHA256

## 本地打 setup.exe（引导安装器）

在已有 MSI 的基础上执行（若 MSI 不存在，脚本会先调用 MSI 脚本）：

```powershell
.\scripts\package-windows-setup.ps1 -Version 0.1.0
```

产物默认在 `dist/`：

- `envr-setup-x64-<version>.exe` — setup 引导器（先处理 VC++ Runtime，再安装 MSI）
- `SHA256SUMS-setup.txt` — setup 的 SHA256

可选参数：

- `-VcRedistPath <path>`：使用本地 `vc_redist.x64.exe`，避免每次联网下载

## 一次命令打 MSI + setup.exe

```powershell
.\scripts\package-windows-bundle.ps1 -Version 0.1.0
```

常见离线场景：

```powershell
.\scripts\package-windows-bundle.ps1 -Version 0.1.0 -VcRedistPath "D:\offline\vc_redist.x64.exe"
```
