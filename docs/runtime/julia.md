# Julia runtime (envr)

## Overview

Julia is installed from the official **`versions.json`** index published on `julialang-s3`, using **portable archives**:

- Linux / macOS: **`tar.gz`** (`kind: archive`)
- Windows: **`zip`** (portable layout; the `.exe` installer rows are not used by envr)

Installations live under:

- `runtimes/julia/versions/<version>/`
- `runtimes/julia/current` → selected version

The active prefix is exposed to child processes as **`JULIA_HOME`** (and `bin/` is prepended to `PATH`).

## Configuration

### `.envr.toml`

```toml
[runtimes.julia]
version = "1.10.5"
```

Partial specs follow the same rules as other unified-line runtimes (e.g. `1.10` → latest `1.10.x` for your platform).

### `settings.toml`

```toml
[runtime.julia]
path_proxy_enabled = true
```

## Caching

- `cache/julia/versions.json` — downloaded `versions.json` (TTL configurable via `ENVR_JULIA_VERSIONS_CACHE_TTL_SECS`, default 1h).
- `cache/julia/remote_latest_per_major_<os>_<arch>.json` — latest patch per major line (TTL `ENVR_JULIA_REMOTE_CACHE_TTL_SECS`, default 24h).

## CLI quick test

```bash
envr remote julia --format json
envr install julia 1.10.5
envr use julia 1.10.5
envr exec --lang julia -- julia --version
envr shim sync
```

Ensure `{runtime_root}/shims` is on `PATH` so the `julia` shim resolves.
