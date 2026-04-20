# Bun runtime — design & acceptance

This document locks **product decisions** and the **acceptance checklist** for Bun before further implementation and testing. Bun is intentionally aligned with **Deno** (multi-version list + settings); once Bun is shipped, run a **combined Deno + Bun** manual test pass.

## Decisions (agreed)

1. **Remote version list — full tag coverage**
   - The remote list must cover **all** relevant upstream release tags, not only one GitHub tags page.
   - **Implementation direction**: GitHub API pagination via `Link: rel="next"` (same strategy as Deno).

2. **PATH proxy**
   - Add/keep **`runtime.bun.path_proxy_enabled`** semantics consistent with Node/Python/Go/PHP/Deno:
     - **On**: `bun`/`bunx` shims resolve envr-managed `current`.
     - **Off**: shims bypass envr and resolve to the next matching `bun`/`bunx` on PATH outside envr shims.

3. **Package sources (包源) — MVP**
   - Bun users care about registry latency; MVP includes a **single “包源 / Package source”** preset that drives the relevant child-process env variables for Bun’s package fetch behavior.
   - **UI**: one control, presets **`auto | domestic | official | restore`** (same spirit as Node’s `npm_registry_mode`).
   - **Semantics**: **child-process env injection only** (no global shell env writes; no modification of user config files unless explicitly added later).

   Notes:
   - Exact env keys and CLI config knobs for Bun should be verified against the minimum Bun version supported; keep the MVP small and prefer env injection over file edits.
   - Follow-up (non-MVP): separate overrides for different registries/scopes if requested.

4. **Download source**
   - Bun installs are based on upstream release artifacts.
   - Mirror strategy should be consistent with repo conventions:
     - Prefer generic mirror proxy (`mirror.mode=auto/manual`) for HTTP downloads when available.
     - Avoid adding too many Bun-specific mirror URLs unless necessary.

## Scope (Bun milestone)

### In scope

- **`envr-runtime-bun`**:
  - Full remote tags pagination for list/resolve.
  - Install/uninstall/switch with layout under `runtimes/bun/versions/<semver>` and global `current`.
  - Windows/macOS/Linux host tuple selection must be correct for the downloaded artifact.
  - `read_current` supports symlink **and** pointer-file fallback on Windows when link creation is blocked (same rule as Deno).
- **Shim**:
  - `bun` and `bunx` launchers under `{runtime_root}/shims`.
  - PATH proxy bypass logic is wired for Bun (off → next on PATH).
  - When proxy is on, shim injects Bun package-source env (when configured).
- **CLI**:
  - `envr exec --lang bun` and merged `envr run/env` flows inject Bun package-source env when Bun is in scope.
- **GUI — Env Center**:
  - Installed list + suggested rows (latest patch per major line recommended for UX).
  - Bun settings strip: PATH proxy + package source (and download source if exposed).
  - Remote suggestions load from disk cache first then refresh in background (same UX as other runtimes).

### Out of scope (unless explicitly added later)

- Editing user-global Bun config files (`bunfig.toml`, etc.) as a default behavior.
- Per-scope registries / complex proxy chains beyond one preset control.
- Managing global installed package executables beyond existing shim sync behavior (handled separately).

## Acceptance checklist (must pass before calling Bun “done”)

Use a clean runtime root or a dedicated test profile where possible.

### A. Provider & remote index

- [ ] **A1** `list_remote` / resolve enumerates versions from **all** relevant upstream tags (not only first GitHub API page).
- [ ] **A2** `resolve` accepts: exact `x.y.z`, `v`-prefixed spec, and prefix specs if supported by existing runtime rules (`latest`, `1`, `1.2`, etc.).
- [ ] **A3** Network errors surface clearly (GUI shows a non-fatal inline message, not a silent empty list).

### B. Install / layout / integrity

- [ ] **B1** Fresh install downloads the correct OS/arch artifact for Bun (Windows x64/ARM64, Linux, macOS as applicable).
- [ ] **B2** Extracted tree passes `bun_installation_valid` and `bun --version` matches the selected version.
- [ ] **B3** Checksum validation runs when upstream publishes it (document behavior when unavailable).

### C. Current & multiple versions

- [ ] **C1** Switching current updates `runtimes/bun/current`; `bun --version` reflects the switch when using envr-managed `bun`.
- [ ] **C2** Uninstall non-current removes only that tree; uninstall current clears `current` without leaving a broken shim target.
- [ ] **C3** Windows link-restricted environment works via pointer-file `current` fallback (read + write).

### D. Shim & PATH proxy

- [ ] **D1** After shim sync, `bun`/`bunx` resolve to envr shim when proxy is **on**.
- [ ] **D2** With proxy **off**, `bun`/`bunx` resolve to the next `bun`/`bunx` on PATH outside envr shims.
- [ ] **D3** Project pin in `.envr.toml` for `bun` (if supported) resolves to the expected version.

### E. Package source (包源)

- [ ] **E1** Setting “包源” to `domestic` injects the expected env/config into Bun child processes (verify via `envr exec --lang bun -- cmd /c "set"` or an equivalent check).
- [ ] **E2** Setting to `restore` stops injecting package-source env (user environment is respected).

### F. GUI — Env Center

- [ ] **F1** Bun page shows installed versions and suggested rows when empty.
- [ ] **F2** Install / switch / uninstall flows complete without stale UI.
- [ ] **F3** Settings strip exposes PATH proxy and package source; persists across restart.
- [ ] **F4** GUI copy uses `tr_key` and is present in `locales/zh-CN.toml` + `locales/en-US.toml`.

### G. Regression & docs

- [ ] **G1** `cargo fmt`, `cargo check --workspace`, and tests pass.
- [ ] **G2** This document matches what shipped; update **Implementation status** after merge.

## Implementation status

- **Settings** (`settings.toml`): `[runtime.bun]` supports `package_source` (`auto|domestic|official|restore`) and `path_proxy_enabled`.
- **Remote tags**: GitHub API pagination with graceful degradation when deep pages fail; default page cap to reduce 403 risk (`ENVR_BUN_TAGS_MAX_PAGES`, default `2`).
- **Install network behavior**:
  - Release `SHASUMS256.txt` and zip download support mirror URL + official fallback.
  - Zip download retries transient connect/timeout errors (including common Windows socket errors like `10054`/`10060`).
- **Version resolution/install flow**:
  - Exact semver spec (`x.y.z`) bypasses tags lookup and installs directly (avoids tag API 403 for direct installs).
  - `0.x` is blocked on Windows in GUI and resolver path because Bun `0.x` has no official Windows release assets.
- **Archive layout compatibility**: installer handles both flat and single-wrapper extraction layouts (e.g. `bun-windows-x64/bun.exe`).
- **Shim + PATH proxy**: `bun`/`bunx` shims are managed and synced after successful install/use flows so global command resolution updates without manual proxy toggling.
- **GUI**:
  - Env Center hides Bun `0.x` major line on Windows and shows explicit support note.
  - Direct install input has front-end validation for Bun `0.x` on Windows (button disable + inline warning + submit guard).

