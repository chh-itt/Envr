# Babashka

## Summary

Babashka runtime integration provides the `bb` executable from upstream GitHub Releases (`babashka/babashka`) as an envr-managed standalone runtime.

## Versions

- Remote source: GitHub Releases (`babashka/babashka`)
- Labels: semver without leading `v` (for example `1.12.218`)
- Host assets:
  - Windows x64: `babashka-<ver>-windows-amd64.zip`
  - Linux x64: `babashka-<ver>-linux-amd64-static.tar.gz` (fallback: non-static tarball)
  - macOS x64/arm64: `babashka-<ver>-macos-<arch>.tar.gz`

## Commands

```powershell
.\envr remote babashka
.\envr install babashka 1.12
.\envr use babashka 1.12
.\envr which --lang babashka
.\envr exec --lang babashka -- bb --version
```

## Environment

- Runtime home env: `BABASHKA_HOME`
- PATH entries: `BABASHKA_HOME\bin`, then `BABASHKA_HOME`

## Shims

- Core shim: `bb`

When path proxy is enabled, `bb` resolves to envr-managed Babashka. When disabled, it passes through to the next `bb` in system PATH.

## Settings

```toml
[runtime.babashka]
path_proxy_enabled = true
```

