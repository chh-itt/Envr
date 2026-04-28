# 发布文档

English | [简体中文](README.zh-CN.md)

本目录包含面向 release 的说明和打包指引。

## 受众分层

- 终端用户应先看 release notes、已知问题和平台安装说明。
- 维护者在生成 release artifact 时，应使用下面的打包说明。

| 文件 | 作用 |
|---|---|
| [`WINDOWS.md`](WINDOWS.md) | Windows 安装、PATH 设置、首次检查与包验证。 |
| [`RELEASE-NOTES.md`](RELEASE-NOTES.md) | 版本化 release notes。 |
| [`KNOWN-ISSUES.md`](KNOWN-ISSUES.md) | 当前限制与已知问题。 |

## 当前发布状态

`envr` 目前还没有描述为“跨所有支持平台都已稳定”的公开安装通道。
在这种状态变化前：

- 以源码构建作为主要文档化安装方式
- Windows 打包脚本属于维护者工具
- 在把打包产物交给用户前，应先检查 release notes 与已知问题

## GitHub release 工作流

匹配 `v*` 的 tag push 会运行 `.github/workflows/release.yml`。
当前 release job 会：

1. 检查格式、workspace 编译、clippy、测试、i18n lint 与 cargo-deny
2. 构建 Windows x86_64 zip 包
3. 上传 workflow artifacts
4. 创建一个带 zip 与 checksum 文件的 **draft** GitHub Release

之所以保持 draft，是为了让维护者在正式发布前检查 release notes、checksum、签名状态与已知问题。
当 MSI / setup artifact 也成为公开通道的一部分时，请把对应打包脚本和 checksum 文件一起加入 workflow artifact 列表。

## 从源码进行 Windows 打包

当前打包脚本面向 Windows x86_64，供维护者使用。
请在仓库根目录、Rust stable 且已安装 MSVC toolchain 的环境下运行。

### Zip 包

```powershell
.\scripts\package-windows-release.ps1 -Version 0.1.0
```

输出位于 `dist/`：

- `envr-windows-x86_64-<version>/` — `envr.exe`、`er.exe`、`envr-gui.exe`、`envr-shim.exe`，以及 `SHA256SUMS.txt`
- `envr-windows-x86_64-<version>.zip` — 上述目录的压缩包
- `SHA256SUMS-archive.txt` — 压缩包 checksum

### MSI 安装包

先安装 WiX v4 CLI：

```powershell
dotnet tool install --global wix
```

然后运行：

```powershell
.\scripts\package-windows-msi.ps1 -Version 0.1.0
```

输出位于 `dist/`：

- `envr-windows-x64-<version>.msi` — 包含 `envr.exe`、`er.exe`、`envr-gui.exe` 与 `envr-shim.exe` 的 MSI 安装包
- `SHA256SUMS-msi.txt` — MSI checksum

如果 WiX 提示 extension 损坏，请重新安装：

```powershell
dotnet tool uninstall --global wix
dotnet tool install --global wix
```
