# Nim runtime (envr)

## Overview

Nim stable binaries are discovered from the official **`https://nim-lang.org/install.html`** matrix (links point at **`nim-lang/nightlies`** GitHub release assets). envr caches that HTML, parses version → download URL for your platform, then installs under:

- `runtimes/nim/versions/<version>/`
- `runtimes/nim/current` → selected version

`bin/` is prepended to `PATH` for `exec` / `run` / shims. No separate `NIM_HOME` is set in MVP (the compiler layout is relative to the prefix).

## Configuration

### `.envr.toml`

```toml
[runtimes.nim]
version = "2.0.14"
```

Partial specs follow the same rules as other **two-part line** runtimes (e.g. `2.0` → latest `2.0.x` installable for your host).

### `settings.toml`

```toml
[runtime.nim]
path_proxy_enabled = true
```

## Caching

- `cache/nim/install.html` — downloaded index (TTL `ENVR_NIM_INDEX_CACHE_TTL_SECS`, default 1h).
- `cache/nim/remote_latest_per_major_<slot>.json` — latest patch per line (TTL `ENVR_NIM_REMOTE_CACHE_TTL_SECS`, default 24h).

## CLI quick test

```bash
envr remote nim -u
envr install nim 2.0.14
envr use nim 2.0.14
envr exec --lang nim -- nim --version
envr shim sync
```

Ensure `{runtime_root}/shims` is on `PATH` for the `nim` shim.
