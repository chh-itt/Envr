# PureScript integration plan

## Goal

Add **`RuntimeKind::Purescript`** (`key = "purescript"`) as a first-class managed runtime (CLI / GUI / shims / pins / `exec` / `run`), installing prebuilt PureScript compiler toolchains into:

`runtimes/purescript/versions/<label>` with a global `runtimes/purescript/current` symlink or Windows pointer file.

## Upstream and artifacts

PureScript compiler releases are from GitHub:

- Repo: `purescript/purescript`
- API: `https://api.github.com/repos/purescript/purescript/releases`
- Atom fallback: `https://github.com/purescript/purescript/releases.atom`

Version tags are semver-like:

- `v0.15.15` (stable)
- `v0.15.16-8` (pre-release; usually excluded from installable rows)

Primary assets (host-specific):

- Windows x86_64: `win64.tar.gz`
- Linux x86_64: `linux64.tar.gz`
- Linux arm64: `linux-arm64.tar.gz`
- macOS x86_64: `macos.tar.gz`
- macOS arm64: `macos-arm64.tar.gz`

## Version labels and spec grammar

- `v0.15.15` -> `0.15.15`
- install spec supports exact labels (`0.15.15`) and `latest`/major-line flows.

## Index/caching

- Normalized row: `PurescriptInstallableRow { version, url }`
- Cache dir: `{runtime_root}/cache/purescript/`
- Index TTL: `ENVR_PURESCRIPT_RELEASES_CACHE_TTL_SECS` (legacy: `ENVR_PURESCRIPT_INDEX_CACHE_TTL_SECS`, default 3600)
- Latest-per-major TTL: `ENVR_PURESCRIPT_REMOTE_CACHE_TTL_SECS` (default 86400)
- API override: `ENVR_PURESCRIPT_GITHUB_RELEASES_URL`
- Tokens: `GITHUB_TOKEN` / `GH_TOKEN` / `ENVR_GITHUB_TOKEN`
- Fallback: when API fails, parse `releases.atom` + paged GitHub Releases HTML tags and synthesize `.../releases/download/<tag>/<asset>`

## Install layout and validation

- Extract archive, normalize single-root or flat-root layout.
- Valid install home must contain `purs` / `purs.exe` (root or `bin/`).

## Shims/env/settings

- Core shim command: `purs`
- Runtime home env: `PURESCRIPT_HOME`
- Template key: `ENVR_PURESCRIPT_VERSION`
- Settings: `[runtime.purescript].path_proxy_enabled`

## CLI / GUI smoke

```bash
envr remote purescript
envr remote purescript -u
envr install purescript 0.15.15
envr use purescript 0.15.15
envr shim sync
purs --version
envr exec --lang purescript -- purs --version
```

GUI:

- Env Center has PureScript tab and PATH proxy toggle
- install/use current view matches CLI state

## Architecture / abstraction friction log

1. GitHub release assets for PureScript use short host names (`win64`, `macos`) unlike many runtimes; mapping must be table-driven.
2. Cross-drive rename fallback stays in shared install layout, not runtime-specific.
3. Pre-release filtering should remain explicit (`draft/prerelease` skip) to avoid unstable rows.

## CLI / GUI friction log

- CLI:
  - No integration regression in workspace tests.
  - Pending operator smoke in real shell: `remote/install/use/shim sync/purs --version/exec`.
  - `use/install purescript 0.15` should resolve to latest known `0.15.x` (line spec convenience).
- GUI:
  - No compile/runtime regression observed in GUI test suite.
  - Pending operator smoke in Env Center tab (install/use + PATH proxy toggle persistence).

