# Lua runtime — integration plan

## Goal

Ship **PUC Rio Lua** (LuaBinaries prebuilt) under envr’s standard layout: `runtimes/lua/versions/<semver>`, global `current`, shims for `lua` / `luac`, PATH proxy parity with Nim/Crystal, CLI + GUI Env Center.

## Product decisions

1. **Runtime key**: `lua` (CLI/GUI/shim/`.envr.toml` `[runtimes.lua]`).
2. **Upstream / index**: Fetch and cache `https://luabinaries.sourceforge.net/download.html`, parse **`lua-X.Y.Z_Win64_bin.zip`** links to derive the set of **installable** semver labels (single source of truth for “can install”). Do not trust lua.org FTP alone (source-only).
3. **Download URL construction**: After a version label is chosen, build the SourceForge `downloads.sourceforge.net` URL for the **host artifact** using deterministic naming rules (Win64 zip, Linux `Linux54_64` vs `Linux319_64` glibc tarball, macOS `MacOS1011` Intel tarball). Same version list for all hosts; install fails fast if the host has no artifact rule.
4. **Version grouping**: GUI/remote major lines use **`major.minor`** (e.g. `5.4`, `5.3`) via `version_line_key_for_kind(RuntimeKind::Lua, …)`.
5. **Core commands**: `lua`, `luac` (shims + `exec --lang lua`).
6. **Runtime-home env**: None required for stock Lua; optional later: `LUA_PATH` presets — out of scope.
7. **Unsupported hosts (MVP)**:
   - Windows ARM64, Linux aarch64, macOS aarch64 **without** a published LuaBinaries row in our mapping → clear validation error (LuaBinaries ships x64 Windows/Linux and Intel macOS in the matrix we target).

## Implementation checklist (playbook cross-walk)

- [x] Domain: `RuntimeKind::Lua`, descriptor (`supports_remote_latest`, `supports_path_proxy`), `parse_runtime_kind("lua")`.
- [x] Crate `envr-runtime-lua`: provider, index parse tests, install/extract/`current` pointer fallback.
- [x] `RuntimeService` registration (`with_defaults` / `with_runtime_root`).
- [x] Shim: `CoreCommand::Lua` / `Luac`, resolution, PATH proxy snapshot field `lua`.
- [x] Resolver / run stack: `RUN_STACK_LANG_ORDER`, `RUNTIME_PLAN_ORDER`, `runtime_bin_dirs` / `ENVR_LUA_VERSION` template key.
- [x] Config: `[runtime.lua] path_proxy_enabled`, schema template, `PathProxyRuntimeSnapshot`.
- [x] GUI: Env Center path-proxy section + `SetLuaPathProxy`; hub order picks up descriptor default merge.
- [x] Docs: this plan + user-facing `lua.md` + playbook §2/§8.11 addendum.

## Architecture / friction notes (post bring-up)

| Area | Note |
|------|------|
| Index shape | **HTML matrix + constructed download URLs** — same class as Nim’s scraped index; TTL `ENVR_LUA_INDEX_CACHE_TTL_SECS` on `cache/lua/download_page.html`. |
| Descriptor vs wiring | PATH proxy still required synchronized updates: `RuntimeSettings.lua`, `PathProxyRuntimeSnapshot`, `CoreCommand`, GUI `EnvCenterMsg` + settings strip — same §8.8-style repetition. |
| Platform matrix | Linux `Linux319_64` vs `Linux54_64` is **version-derived** in code (`linux_tools_middle_label`); macOS uses Intel `MacOS1011` tarball only — **aarch64 macOS rejected** until a policy exists. |
| CLI vs GUI cache | Remote list goes through `LuaManager::load_version_list()` so CLI `remote` and GUI `refresh_runtimes` share the same on-disk HTML cache (avoids Zig-style split). |
| Known product gaps | No LuaJIT / LuaRocks; no `.lua-version` native file precedence (only `.envr.toml`). |

## Known limitations (user-visible)

- Apple Silicon **native** macOS builds are not in the LuaBinaries shortcuts we parse; arm64 mac hosts are rejected until a supported artifact policy exists.
- No **LuaJIT** / **LuaRocks** in this milestone.

## CLI smoke (copy-paste)

See `docs/runtime/lua.md` after build.

## Development log

- Initial integration follows Nim-style **scrape + cache** index, with **install-time URL construction** (fewer moving parts than storing three URLs per version in JSON).

## CLI / GUI validation notes (for testers)

- **GUI**: Lua tab uses the same descriptor-driven `supports_remote_latest` path as Zig/Nim (unified major rows + `runtime_page_enter_tasks` batch); settings fold only exposes PATH proxy (no download-source knob yet).
- **CLI**: `remote lua` uses the same provider as install; first run may take ~1–2s to fetch `download.html` (then cached).
