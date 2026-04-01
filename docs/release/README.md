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

- `envr-windows-x86_64-<version>/` — `envr.exe`、`envr-gui.exe`、`envr-shim.exe` 及 `SHA256SUMS.txt`
- `envr-windows-x86_64-<version>.zip` — 同上目录的压缩包
- `SHA256SUMS-archive.txt` — 压缩包本身的 SHA256

CI 在推送 `v*` 标签时也会构建并上传同名 artifact（见 `.github/workflows/release.yml`）。
