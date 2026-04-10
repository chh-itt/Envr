# Deno runtime — design & acceptance

This document locks **product decisions** and the **acceptance checklist** before implementation. **Bun** is tracked separately but is expected to follow the same UX pattern (multi-version list + settings); **test Deno and Bun together** after both land.

## Decisions (agreed)

1. **Remote version list — full tag coverage**  
   Resolve and list remote versions from **all** relevant Deno release tags (same spirit as Node’s full index), not only the first page of GitHub results.  
   **Implementation direction**: use GitHub API pagination (`Link` header and/or `page=` loops) until no more `rel="next"`, or an equivalent single source that enumerates all semver tags. Cap with a documented upper bound only if needed for abuse protection (e.g. max pages).

2. **PATH proxy**  
   Add **`runtime.deno.path_proxy_enabled`** (or the same naming pattern as Node/Go/PHP):  
   - **On**: `deno` shim resolves the envr-managed `current` tree.  
   - **Off**: shim bypasses envr and uses the next `deno` on PATH.  
   Settings validation and disk round-trip follow existing runtime settings conventions.

3. **Package sources (包源) — MVP**  
   In addition to **binary download source** (where `deno` zip comes from) and PATH proxy, the Deno settings strip includes **one** control for package registries:

   **UI & settings (recommended)**  
   - **Single dropdown** — label e.g. **「包源」/ Package source**, presets **`auto | domestic | official`** (same spirit as `runtime.node.npm_registry_mode`).  
   - **Not** two separate dropdowns for npm vs JSR in MVP.

   **Why one control, not two**  
   - Matches how users describe intent (“use domestic mirrors” vs “official”), and keeps the settings row small next to download source + PATH proxy.  
   - Aligns with Node’s single `npm_registry_mode` pattern instead of fragmenting registry UX.  
   - Implementation still applies **two** env vars from the **same** preset: npm and JSR mirrors are switched **together** via a fixed URL table per preset (`domestic` / `official`; `auto` follows the same detection rules as Node where applicable).

   **Child env**  
   - **npm**: **`NPM_CONFIG_REGISTRY`** ([Deno docs](https://docs.deno.com/runtime/manual/basics/env_variables/)).  
   - **JSR**: mirror for the JSR origin (exact env name **must be verified at implementation time** against the managed Deno release; names like `JSR_URL` have appeared in upstream—confirm in Deno sources/docs for the minimum version we ship).  

   **Semantics**: child-process env injection only (no global `~/.config` writes), consistent with Rust/Node mirrors in this repo.

   **Follow-up (non-MVP)**: separate npm vs JSR overrides if users need asymmetric mirrors; not required for the first ship.

4. **Order of work**  
   Implement and validate **Deno first**, then **Bun** in the same style; **one combined manual test pass** when both are ready.

## Scope (Deno milestone)

### In scope

- **`envr-runtime-deno`**: full remote tag enumeration, install from `dl.deno.land` zip per platform triple, `runtimes/deno/versions/<semver>` + global `current`, uninstall, optional checksum from `.sha256sum` (behavior aligned with existing crate).
- **Shim**: `deno` launcher under `{runtime_root}/shims`, dispatch via `envr-shim`, resolution consistent with CLI (`resolve` / `current` / project pin).
- **CLI**: `PATH` / child env injection behavior consistent with other proxied runtimes when proxy is on; **package registry env** (`NPM_CONFIG_REGISTRY` and JSR mirror var when applicable) applied when spawning `deno` under envr-managed flows.
- **GUI — Env Center**: multi-version list + suggested rows (from full remote list / per-major strategy as implemented), install/switch/uninstall, download progress where applicable, **Deno-specific settings strip**: **binary download source**, **PATH proxy**, **one Package source dropdown** (`auto | domestic | official`) driving both npm + JSR env injection.
- **`read_current`**: support symlink **and** pointer-file / junction fallback on Windows where creation of symlinks fails (same idea as PHP/Node).

### Out of scope (unless explicitly added later)

- **Independent** npm vs JSR dropdowns (asymmetric mirrors); MVP uses the **single** Package source preset only.
- Per-scope **npm** registries (scoped packages to different hosts) — only preset URLs / mirror mode as above.
- **deno.json** / workspace project wizardry inside envr.
- Automating `deno install` global script shims beyond the core `deno` binary (optional follow-up).

## Acceptance checklist (must pass before calling Deno “done”)

Use a clean runtime root or a dedicated test profile where possible.

### A. Provider & remote index

- [ ] **A1** `list_remote` / resolve returns versions that match **all** published semver tags from the chosen source (spot-check against GitHub `denoland/deno` tags or official release list), not only the first API page.
- [ ] **A2** `resolve` accepts at least: exact `x.y.z`, `v`-prefixed spec, major-only / major.minor if supported by the same rules as documented in code.
- [ ] **A3** Network failure surfaces a clear error (no silent empty list without explanation in GUI when appropriate).

### B. Install / layout / integrity

- [ ] **B1** Fresh install of a chosen version downloads the correct **platform triple** zip (Windows x64/ARM64, Linux, macOS as applicable).
- [ ] **B2** Extracted tree passes `deno_installation_valid` and `deno --version` matches the selected version.
- [ ] **B3** Optional: `.sha256sum` verification when the upstream file exists; document behavior when missing or mismatch.

### C. Current & multiple versions

- [ ] **C1** **Switch current** to another installed version updates global `current`; `deno --version` reflects the switch when using envr-managed `deno`.
- [ ] **C2** **Uninstall** non-current version removes only that tree; **uninstall current** clears `current` and does not leave a broken shim target.

### D. Shim & PATH proxy

- [ ] **D1** After `envr shim sync` (or equivalent), **`deno` on PATH** resolves to envr shim when proxy is **on**.
- [ ] **D2** With proxy **off**, invocations use the **next** `deno` on system PATH (document test: temporarily ensure envr shims precede or follow PATH as designed).
- [ ] **D3** Project pin in `.envr.toml` for `deno` (if supported) resolves to the expected version directory.

### E. GUI — Env Center

- [ ] **E1** Deno page shows **installed** versions and **suggested** rows when nothing is installed (or per agreed list strategy).
- [ ] **E2** Install / switch / uninstall flows complete without stale UI; download panel shows progress for install jobs if wired.
- [ ] **E3** Settings strip exposes **binary download source**, **PATH proxy**, and **one Package source** control; all persist across restart.
- [ ] **E4** Changing **Package source** preset updates **both** `NPM_CONFIG_REGISTRY` and the JSR mirror env together; `envr exec` / GUI-spawned `deno` resolves `npm:` / `jsr:` against the expected endpoints for that preset (when the managed Deno supports the JSR variable).

### F. CLI / doctor

- [ ] **F1** `envr` commands that merge runtime PATH include Deno when installed and proxy rules say so.
- [ ] **F2** `envr doctor` (if applicable) reports sensible status for Deno (installed / current / shims).
- [ ] **F3** Child env for Deno includes **package registry** variables when settings are non-default (spot-check with `envr exec -- ...` or a tiny script that prints `NPM_CONFIG_REGISTRY` / JSR-related env if exposed).

### G. Regression & docs

- [ ] **G1** `cargo fmt`, `cargo check --workspace`, and existing tests pass.
- [ ] **G2** This file’s **Decisions** and **Scope** match what was shipped; update **Implementation status** subsection when implementation exists.

## Bun (follow-up)

- Reuse the same checklist shape (remote list completeness per Bun’s upstream, PATH proxy, shims, Env Center).  
- **Combined test pass**: run sections **D–E** for **both** Deno and Bun after Bun lands.

## Implementation status

- **Settings** (`settings.toml`): `[runtime.deno]` — `download_source` (`auto` \| `domestic` \| `official`), `package_source` (reuses Node’s `npm_registry_mode` values: `auto` \| `domestic` \| `official` \| `restore`), `path_proxy_enabled`.
- **Remote tags**: GitHub API pagination via `Link: rel="next"` in `envr-runtime-deno` (`fetch_all_tags`), with resilience for rate-limit/forbidden deep pages plus configurable page cap (`ENVR_DENO_TAGS_MAX_PAGES`, default `2`).
- **Binary zip**: `deno_release_zip_url` — official `dl.deno.land/release/...` or domestic `registry.npmmirror.com/-/binary/deno/v{version}/...`.
- **Download fallback**: if domestic/primary URL fails, installer retries official `dl.deno.land` URL for the same version.
- **Package env**: `deno_package_registry_env` sets `NPM_CONFIG_REGISTRY` and `JSR_URL` (unless `restore`).
- **Shim**: `CoreCommand::Deno` + PATH proxy from disk; **CLI**: `child_env` injects package env when Deno is on merged PATH.
- **Version policy**:
  - Exact semver spec (`x.y.z`) bypasses tags lookup and installs directly (avoids tag API 403 for direct installs).
  - `0.x` is blocked for managed install and now front-validated in GUI direct install input with explicit hint.
- **GUI**: Env Center Deno page — single **包源** control, download source, PATH proxy; remote suggestions via `list_remote_latest_per_major`; install success triggers shim sync to keep command availability consistent.
