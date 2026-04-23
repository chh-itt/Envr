# Unison runtime

`envr` supports managing Unison via `RuntimeKind::Unison` (`key = "unison"`), with `ucm` as the core shim command.

## Install source

- Upstream: GitHub releases from `unisonweb/unison`
- Typical upstream tag: `release/1.2.0`
- Host asset examples:
  - Windows: `ucm-windows-x64.zip`
  - macOS: `ucm-macos-x64.tar.gz`, `ucm-macos-arm64.tar.gz`
  - Linux: `ucm-linux-x64.tar.gz`, `ucm-linux-arm64.tar.gz`

`envr` normalizes `release/<semver>` tags to install labels like `1.2.0`.

## Layout

- Versions: `runtimes/unison/versions/<label>`
- Current pointer: `runtimes/unison/current`
- Shim command: `ucm`
- Runtime home env: `UNISON_HOME`

`envr` accepts both extracted layouts:

- `ucm(.exe)` at runtime root
- `bin/ucm(.exe)` under runtime root

## Config

`settings.toml`:

```toml
[runtime.unison]
path_proxy_enabled = true
```

When `path_proxy_enabled = false`, `ucm` shim bypasses envr-managed runtime selection and resolves from host `PATH`.

## Remote/index behavior

- GitHub API primary source with token support (`GITHUB_TOKEN` / `GH_TOKEN` / `ENVR_GITHUB_TOKEN`)
- Fallback chain: GitHub API -> releases HTML -> releases atom
- Cache / TTL knobs:
  - `ENVR_UNISON_RELEASES_CACHE_TTL_SECS` (legacy fallback: `ENVR_UNISON_INDEX_CACHE_TTL_SECS`)
  - `ENVR_UNISON_REMOTE_CACHE_TTL_SECS`
  - `ENVR_UNISON_GITHUB_RELEASES_URL` (API endpoint override)

## Smoke commands

```powershell
.\target\release\envr.exe remote unison -u
.\target\release\envr.exe install unison latest
.\target\release\envr.exe use unison latest
.\target\release\envr.exe which ucm
.\target\release\envr.exe exec --lang unison -- ucm version
.\target\release\envr.exe shim sync
ucm version
```

GUI smoke:

- Open Runtime -> Unison
- Verify remote list renders
- Install one version and set current
- Toggle PATH proxy and verify button/state behavior
