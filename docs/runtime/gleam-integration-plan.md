# Gleam integration plan

## Goal

Add **`RuntimeKind::Gleam`** (`key = "gleam"`) as a first-class managed runtime (CLI / GUI / shims / pins / `exec` / `run`) using official GitHub release binaries.

Install layout:

`runtimes/gleam/versions/<label>` with global `runtimes/gleam/current` symlink or Windows pointer file.

## Upstream and artifacts

Gleam distribution source:

- repo: `gleam-lang/gleam`
- API: `https://api.github.com/repos/gleam-lang/gleam/releases`
- fallback: `releases.atom` + paginated releases HTML

Host assets (candidate matching by current OS/ARCH):

- Windows x64: `gleam-v{version}-x86_64-pc-windows-msvc.zip`
- Linux x64: `gleam-v{version}-x86_64-unknown-linux-musl.tar.gz` (or gnu fallback)
- macOS x64/arm64: `gleam-v{version}-<arch>-apple-darwin.tar.gz`

## Version labels and spec grammar

- tag `v1.11.2` -> label `1.11.2`
- supports exact label install and shorthand line resolution (`1.11` -> latest `1.11.x`)

## Index/caching

- row: `GleamInstallableRow { version, url }`
- cache: `{runtime_root}/cache/gleam/`
- index TTL: `ENVR_GLEAM_RELEASES_CACHE_TTL_SECS` / legacy `ENVR_GLEAM_INDEX_CACHE_TTL_SECS`
- latest-per-major TTL: `ENVR_GLEAM_REMOTE_CACHE_TTL_SECS`
- API override: `ENVR_GLEAM_GITHUB_RELEASES_URL`
- tokens: `GITHUB_TOKEN` / `GH_TOKEN` / `ENVR_GITHUB_TOKEN`

## Install layout and validation

- Download host archive (`.zip` / `.tar.gz`) and extract atomically into version directory.
- Valid install contains `gleam` executable at root or `bin/`.

## Shims/env/settings

- shim command: `gleam`
- home env: `GLEAM_HOME`
- template key: `ENVR_GLEAM_VERSION`
- settings: `[runtime.gleam].path_proxy_enabled`

## Erlang/OTP host dependency

- Gleam compiles/targets BEAM and requires Erlang/OTP toolchain availability.
- Install phase should fail fast with explicit guidance if `erl` is missing/unrunnable.
- Runtime descriptor host relation is `gleam -> erlang` (same idea as other host-dependent runtimes).

## CLI / GUI smoke

```bash
envr remote gleam
envr remote gleam -u
envr install gleam 1
envr use gleam 1
envr shim sync
gleam --version
envr exec --lang gleam -- gleam --version
```

## Architecture / abstraction friction log

1. GitHub-backed standalone runtime still needs API + HTML + atom fallback chain for 403/rate-limit/proxy reliability.
2. Runtime-level host dependency (`gleam -> erlang`) is declarative, but install-time prerequisite checks remain provider-specific.
3. Archive root layout differs by release artifact and needs shared promotion logic (root vs nested directory).

## CLI / GUI friction log

- CLI:
  - Pending operator smoke in real shell (`remote/install/use/shim sync/gleam --version/exec`).
- GUI:
  - Pending operator smoke in Env Center tab (install/use + PATH proxy toggle persistence).

