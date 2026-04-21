# Elm integration plan

## Goal

Add **`RuntimeKind::Elm`** (`key = "elm"`) as a first-class managed runtime (CLI / GUI / shims / pins / `exec` / `run`) using prebuilt Elm compiler binaries.

Install layout:

`runtimes/elm/versions/<label>` with global `runtimes/elm/current` symlink or Windows pointer file.

## Upstream and artifacts

Elm compiler releases are from GitHub:

- Repo: `elm/compiler`
- API: `https://api.github.com/repos/elm/compiler/releases`
- Non-API fallback: releases HTML pagination + `releases.atom`

Tag format:

- stable: `0.19.1` / `v0.19.1`

Asset names:

- Windows x86_64: `binary-for-windows-64-bit.gz`
- Linux x86_64: `binary-for-linux-64-bit.gz`
- macOS x86_64: `binary-for-mac-64-bit.gz`
- macOS arm64: `binary-for-mac-64-bit-ARM.gz`

## Version labels and spec grammar

- tag `0.19.1` or `v0.19.1` -> label `0.19.1`
- supports exact label install and shorthand line resolution (`0.19` -> latest `0.19.x`)

## Index/caching

- row: `ElmInstallableRow { version, url }`
- cache: `{runtime_root}/cache/elm/`
- index TTL: `ENVR_ELM_RELEASES_CACHE_TTL_SECS` / legacy `ENVR_ELM_INDEX_CACHE_TTL_SECS`
- latest-per-major TTL: `ENVR_ELM_REMOTE_CACHE_TTL_SECS`
- API override: `ENVR_ELM_GITHUB_RELEASES_URL`
- tokens: `GITHUB_TOKEN` / `GH_TOKEN` / `ENVR_GITHUB_TOKEN`

## Install layout and validation

- Download `.gz`, unpack to `elm` / `elm.exe`.
- Valid install contains `elm` executable at root or `bin/`.

## Shims/env/settings

- shim command: `elm`
- home env: `ELM_HOME`
- template key: `ENVR_ELM_VERSION`
- settings: `[runtime.elm].path_proxy_enabled`

## CLI / GUI smoke

```bash
envr remote elm
envr remote elm -u
envr install elm 0.19.1
envr use elm 0.19
envr shim sync
elm --version
envr exec --lang elm -- elm --version
```

## Architecture / abstraction friction log

1. Elm ships raw `.gz` single binaries (not zip/tar), so installer needs explicit gzip inflate path.
2. GitHub API 403 must fallback automatically; atom/history truncation requires HTML pagination fallback.
3. Keep shorthand spec resolution in-provider to avoid CLI special-casing.

## CLI / GUI friction log

- CLI:
  - No workspace-test regressions after Elm runtime wiring.
  - Pending operator smoke in real shell (`remote/install/use/shim sync/elm --version/exec`).
- GUI:
  - No GUI compile/runtime regressions in workspace tests.
  - Pending operator smoke in Env Center tab (install/use + PATH proxy toggle persistence).

