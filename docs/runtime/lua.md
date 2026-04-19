# Lua (PUC Rio) — managed runtime

envr installs **LuaBinaries** “Tools Executables” builds into:

`runtimes/lua/versions/<version>` with a global `runtimes/lua/current` symlink or Windows pointer file.

## Requirements

- **Windows x64**, **Linux x86_64 (glibc)**, or **macOS x64** (Intel tarball). Other hosts are rejected with a clear error (see `lua-integration-plan.md`).
- **LuaBinaries on Windows** often ships **`lua54.exe` / `luac54.exe`** (and **`lua55.exe` / `luac55.exe`** for 5.5.x) rather than `lua.exe`; envr treats these as a valid install and resolves the `lua` / `luac` shims to them.

## Commands

```bash
envr remote lua
envr remote lua --prefix 5.4
envr install lua 5.4.8
envr use lua 5.4.8
envr current lua
envr exec --lang lua -- lua -v
```

## Settings

`settings.toml`:

```toml
[runtime.lua]
path_proxy_enabled = true
```

When `path_proxy_enabled = false`, `lua` / `luac` shims bypass envr and resolve to the next matching binary on PATH outside envr shims (same model as Nim/Crystal).

## Pins

`.envr.toml`:

```toml
[runtimes.lua]
version = "5.4.8"
```

## Index / offline

Remote rows come from cached `https://luabinaries.sourceforge.net/download.html` stored under `{ENVR_RUNTIME_ROOT or default}/cache/lua/download_page.html` (TTL via `ENVR_LUA_INDEX_CACHE_TTL_SECS`, default 86400). In `mirror.mode = offline`, a warm cache is required (or go online once so the page can be fetched).
