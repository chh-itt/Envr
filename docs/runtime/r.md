# R runtime (envr)

## Overview

CRAN **Windows** builds ship as Inno Setup installers (`R-x.y.z-win.exe`). envr downloads the installer, runs a silent install into `runtimes/r/versions/<version>/`, and selects `runtimes/r/current`.

Version discovery uses [rversions.r-pkg.org](https://rversions.r-pkg.org/) (`r-versions`, `r-release-win`) and follows CRAN URL rules for `base/` vs `base/old/<ver>/`.

**Non-Windows hosts:** managed `list_remote` / `install` return a clear validation error (no silent empty catalog).

## Configuration

### `.envr.toml`

```toml
[runtimes.r]
version = "4.4.2"
```

Partial specs use the same **two-part line** rules as Julia/Nim (for example `4.4` → latest `4.4.x` in the Windows index).

### `settings.toml`

```toml
[runtime.r]
path_proxy_enabled = true
```

## Shims and environment

- Shims: **`R`** and **`Rscript`** (under `{runtime_root}/shims`).
- `exec` / `run` / shims set **`R_HOME`** to the resolved prefix and prepend `bin` on `PATH`.
- Template key for pinned version in merged run env: **`ENVR_R_VERSION`**.

## Caching / TTL

- `cache/r/r-versions.json`, `cache/r/r-release-win.json` — index bodies (TTL `ENVR_RLANG_INDEX_CACHE_TTL_SECS`, default 1h).
- `cache/r/remote_latest_per_major_win.json` — latest patch per line for GUI/CLI (TTL `ENVR_RLANG_REMOTE_CACHE_TTL_SECS`, default 24h).

## CLI quick test (Windows)

```powershell
envr remote r -u
envr install r 4.4.2
envr use r 4.4.2
envr exec --lang r -- Rscript --version
envr shim sync
```

Ensure `{runtime_root}/shims` is on `PATH` for the `R` / `Rscript` shims.
