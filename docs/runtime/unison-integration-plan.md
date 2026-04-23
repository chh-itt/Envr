# Unison runtime integration plan

## Goal

Add **`RuntimeKind::Unison`** (`key = "unison"`) as a first-class managed runtime (CLI / GUI / shims / pins / `exec` / `run`) using official GitHub release binaries.

Install layout:

`runtimes/unison/versions/<label>` with global `runtimes/unison/current` symlink or Windows pointer file.

## Upstream and artifacts

Unison distribution source (UCM – Unison Codebase Manager):

- repo: `unisonweb/unison`
- releases API: `https://api.github.com/repos/unisonweb/unison/releases`
- typical tags: `release/1.2.0` (note the slash)
- assets (as of 2026):  
  - `ucm-windows-x64.zip`
  - `ucm-macos-x64.tar.gz`, `ucm-macos-arm64.tar.gz`
  - `ucm-linux-x64.tar.gz`, `ucm-linux-arm64.tar.gz`

The runtime we manage is the **`ucm`** executable (and any bundled support files shipped alongside it).

## Version labels and spec grammar

- Tag `release/1.2.0` → label `1.2.0`
- Support:
  - exact: `1.2.0`
  - major: `1` → latest `1.x.y`
  - major.minor: `1.2` → latest `1.2.y`
  - `latest`

## Index/caching

- row: `UnisonInstallableRow { version, url }`
- cache: `{runtime_root}/cache/unison/`
- index TTL env: `ENVR_UNISON_RELEASES_CACHE_TTL_SECS` (fallback legacy `ENVR_UNISON_INDEX_CACHE_TTL_SECS`)
- latest-per-line TTL env: `ENVR_UNISON_REMOTE_CACHE_TTL_SECS`
- API override env: `ENVR_UNISON_GITHUB_RELEASES_URL`
- tokens: `GITHUB_TOKEN` / `GH_TOKEN` / `ENVR_GITHUB_TOKEN`
- fallback chain (reliability): API → releases HTML → releases.atom (same pattern as Gleam)

## Install layout and validation

- Download the host archive (`.zip` / `.tar.gz`) and extract atomically into version directory.
- Accept both layouts:
  - `ucm(.exe)` at root
  - `bin/ucm(.exe)` in subdir
- Valid install: `ucm` executable exists and `ucm version` (or `--version`) returns success.

## Shims/env/settings

- shim command: `ucm`
- home env: `UNISON_HOME` (points to `runtimes/unison/versions/<label>`)
- template key: `ENVR_UNISON_VERSION`
- settings: `[runtime.unison].path_proxy_enabled`

## CLI / GUI smoke

```powershell
.\target\release\envr.exe remote unison -u
.\target\release\envr.exe install unison latest
.\target\release\envr.exe use unison latest
.\target\release\envr.exe which ucm
.\target\release\envr.exe exec --lang unison -- ucm version
.\target\release\envr.exe shim sync
ucm version
```

GUI: open **Runtime** → **Unison**, confirm remote rows, install, set current, and PATH proxy toggle.

## Architecture / abstraction friction log (living)

- **Tag shape**: tags are `release/<semver>` (contains `/`), so label parsing must not assume `vX.Y.Z`.
- **Release asset name stability**: UCM assets are simple but must be selected by OS/arch reliably.
- **Fallback chain**: GitHub API can fail/rate-limit; keep HTML/ATOM fallbacks consistent with other runtimes.
- **Promotion logic**: archives may contain nested root directories; reuse existing “find executable in extracted tree then commit” approach.

## CLI / GUI friction log

- CLI:
  - Pending operator smoke after implementation.
- GUI:
  - Pending operator smoke after implementation.

