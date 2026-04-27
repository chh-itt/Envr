# Release documentation

This directory contains release-facing notes and packaging instructions.

| File | Purpose |
|---|---|
| [`WINDOWS.md`](WINDOWS.md) | Windows installation, PATH setup, first-run checks, and package verification. |
| [`RELEASE-NOTES.md`](RELEASE-NOTES.md) | Versioned release notes. |
| [`KNOWN-ISSUES.md`](KNOWN-ISSUES.md) | Current limitations and known issues. |

## Windows packaging from source

The current packaging scripts target Windows x86_64 and are intended for maintainers.
Run them from the repository root with Rust stable and the MSVC toolchain installed.

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
