# Release documentation

English | [简体中文](README.zh-CN.md)

This directory contains release-facing notes, installation guidance, and packaging instructions.

## Audience split

- End users should start with GitHub Releases, release notes, known issues, and platform-specific install notes.
- Maintainers should use the packaging sections below when producing or reviewing release artifacts.

| File | Purpose |
|---|---|
| [`WINDOWS.md`](WINDOWS.md) | Windows installation, PATH setup, first-run checks, and package verification. |
| [`RELEASE-NOTES.md`](RELEASE-NOTES.md) | Versioned release notes. |
| [`KNOWN-ISSUES.md`](KNOWN-ISSUES.md) | Current limitations and known issues. |

## Current distribution status

GitHub Releases are the primary public installation channel for `envr`.
The intended low-cost release target set is:

| Platform | Architecture | Public artifacts |
|---|---:|---|
| Windows | x86_64 | `.zip`, `.msi`, setup bootstrapper |
| Linux | x86_64 | `.tar.gz` |
| macOS | x86_64 | `.tar.gz` or `.zip` |
| macOS | arm64 | `.tar.gz` or `.zip` |

Source builds remain useful for contributors, local debugging, and unsupported host combinations, but end users should prefer published GitHub Release artifacts when available.

Runtime availability is still provider-specific. A platform-level `envr` artifact does not mean every managed runtime can be installed on that platform. See [`../runtime/platform-support-matrix.md`](../runtime/platform-support-matrix.md).

## GitHub release workflow

Tag pushes matching `v*` run `.github/workflows/release.yml`.
The release workflow should:

1. check formatting, workspace compilation, clippy, tests, i18n lint, and cargo-deny
2. build release artifacts for Windows x86_64, Linux x86_64, macOS x86_64, and macOS arm64
3. generate SHA256 checksum files for each artifact group
4. upload workflow artifacts
5. create a **draft** GitHub Release with all archives/installers and checksum files attached

The release stays draft so maintainers can review notes, checksums, signing status, smoke-test results, and known issues before publishing.

## Windows packaging from source

The Windows packaging scripts target Windows x86_64 and are intended for maintainers.
Run them from the repository root with Rust 1.88+ and the MSVC toolchain installed.

### Zip package

```powershell
.\scripts\package-windows-release.ps1 -Version 0.1.0
```

Outputs under `dist/`:

- `envr-windows-x86_64-<version>/` — `envr.exe`, `er.exe`, `envr-gui.exe`, `envr-shim.exe`, and `SHA256SUMS.txt`.
- `envr-windows-x86_64-<version>.zip` — archive for the same directory.
- `SHA256SUMS-archive.txt` — checksum for the archive.

### MSI installer

Install WiX v4 CLI once:

```powershell
dotnet tool install --global wix
```

Then run:

```powershell
.\scripts\package-windows-msi.ps1 -Version 0.1.0
```

Outputs under `dist/`:

- `envr-windows-x64-<version>.msi` — MSI installer containing `envr.exe`, `er.exe`, `envr-gui.exe`, and `envr-shim.exe`.
- `SHA256SUMS-msi.txt` — checksum for the MSI.

If WiX reports a damaged extension after using another WiX version, reinstall the tool:

```powershell
dotnet tool uninstall --global wix
dotnet tool install --global wix
```

### Setup bootstrapper

With or without an existing MSI, run:

```powershell
.\scripts\package-windows-setup.ps1 -Version 0.1.0
```

Outputs under `dist/`:

- `envr-setup-x64-<version>.exe` — setup bootstrapper.
- `SHA256SUMS-setup.txt` — checksum for the bootstrapper.

Use a local Visual C++ Redistributable when packaging offline:

```powershell
.\scripts\package-windows-setup.ps1 -Version 0.1.0 -VcRedistPath "D:\offline\vc_redist.x64.exe"
```

### MSI + setup bundle

```powershell
.\scripts\package-windows-bundle.ps1 -Version 0.1.0
```

Offline example:

```powershell
.\scripts\package-windows-bundle.ps1 -Version 0.1.0 -VcRedistPath "D:\offline\vc_redist.x64.exe"
```

## Linux and macOS archive packaging

Linux and macOS public artifacts are archive-based packages produced by GitHub Actions.
The intended contents are the release binaries that are valid for the target platform, plus a checksum file.

Initial archive targets:

- `envr-linux-x86_64-<version>.tar.gz`
- `envr-macos-x86_64-<version>.tar.gz`
- `envr-macos-arm64-<version>.tar.gz`

The release workflow uses `scripts/package-unix-release.sh` to build these archive artifacts and their checksum files.
If GUI packaging for Unix-like hosts is not release-validated for a given version, the corresponding archive should be CLI-focused and release notes should say so explicitly.
