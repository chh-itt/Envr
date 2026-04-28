# 发布文档

English | [简体中文](README.zh-CN.md)

本目录包含面向 release 的说明、安装指引与打包说明。

## 受众分层

- 终端用户应先看 GitHub Releases、release notes、已知问题与平台安装说明。
- 维护者在生成或复核 release artifact 时，应使用下面的打包说明。

| 文件 | 作用 |
|---|---|
| [`WINDOWS.md`](WINDOWS.md) | Windows 安装、PATH 设置、首次检查与包验证。 |
| [`RELEASE-NOTES.md`](RELEASE-NOTES.md) | 版本化 release notes。 |
| [`KNOWN-ISSUES.md`](KNOWN-ISSUES.md) | 当前限制与已知问题。 |

## 当前发布状态

GitHub Releases 是 `envr` 的主要公开安装渠道。
当前计划采用的低成本公开发布目标集合为：

| 平台 | 架构 | 公开产物 |
|---|---:|---|
| Windows | x86_64 | `.zip`、`.msi`、setup bootstrapper |
| Linux | x86_64 | `.tar.gz` |
| macOS | x86_64 | `.tar.gz` 或 `.zip` |
| macOS | arm64 | `.tar.gz` 或 `.zip` |

源码构建仍适用于贡献者、本地调试和未列入公开发布范围的主机组合；但对普通终端用户来说，优先建议使用 GitHub Release 中已发布的 artifact。

运行时可用性仍然取决于具体 provider。某个平台存在 `envr` 的发布包，并不意味着所有 managed runtime 都能在该平台上安装。详见 [`../runtime/platform-support-matrix.md`](../runtime/platform-support-matrix.md)。

## GitHub release 工作流

匹配 `v*` 的 tag push 会运行 `.github/workflows/release.yml`。
该 release workflow 应当：

1. 检查格式、workspace 编译、clippy、测试、i18n lint 与 cargo-deny
2. 构建 Windows x86_64、Linux x86_64、macOS x86_64、macOS arm64 的 release artifact
3. 为各平台产物生成 SHA256 checksum 文件
4. 上传 workflow artifact
5. 创建一个附带全部归档包 / 安装包及 checksum 文件的 **draft** GitHub Release

之所以保持 draft，是为了让维护者在正式发布前检查 release notes、checksum、签名状态、smoke test 结果与已知问题。

## 从源码进行 Windows 打包

当前 Windows 打包脚本面向 Windows x86_64，供维护者使用。
请在仓库根目录、Rust 1.88+ 且已安装 MSVC toolchain 的环境下运行。

### Zip 包

```powershell
.\scripts\package-windows-release.ps1 -Version 0.1.0
```

输出位于 `dist/`：

- `envr-windows-x86_64-<version>/` — `envr.exe`、`er.exe`、`envr-gui.exe`、`envr-shim.exe`，以及 `SHA256SUMS.txt`。
- `envr-windows-x86_64-<version>.zip` — 上述目录的压缩包。
- `SHA256SUMS-archive.txt` — 压缩包 checksum。

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

- `envr-windows-x64-<version>.msi` — 包含 `envr.exe`、`er.exe`、`envr-gui.exe` 与 `envr-shim.exe` 的 MSI 安装包。
- `SHA256SUMS-msi.txt` — MSI checksum。

如果 WiX 提示 extension 损坏，请重新安装：

```powershell
dotnet tool uninstall --global wix
dotnet tool install --global wix
```

### Setup bootstrapper

无论是否已有 MSI，都可以运行：

```powershell
.\scripts\package-windows-setup.ps1 -Version 0.1.0
```

输出位于 `dist/`：

- `envr-setup-x64-<version>.exe` — setup bootstrapper。
- `SHA256SUMS-setup.txt` — bootstrapper checksum。

离线打包时可指定本地 Visual C++ Redistributable：

```powershell
.\scripts\package-windows-setup.ps1 -Version 0.1.0 -VcRedistPath "D:\offline\vc_redist.x64.exe"
```

### MSI + setup 组合打包

```powershell
.\scripts\package-windows-bundle.ps1 -Version 0.1.0
```

离线示例：

```powershell
.\scripts\package-windows-bundle.ps1 -Version 0.1.0 -VcRedistPath "D:\offline\vc_redist.x64.exe"
```

## Linux 与 macOS 归档包

Linux 与 macOS 的公开产物采用由 GitHub Actions 构建的归档包形式。
预期内容是目标平台可用的 release 二进制，以及对应的 checksum 文件。

首批归档目标建议为：

- `envr-linux-x86_64-<version>.tar.gz`
- `envr-macos-x86_64-<version>.tar.gz`
- `envr-macos-arm64-<version>.tar.gz`

release workflow 使用 `scripts/package-unix-release.sh` 生成这些归档包及其 checksum 文件。
如果某个版本的 Unix-like GUI 打包尚未达到 release validation 要求，则对应归档包应以 CLI 为主，并在 release notes 中明确说明。
