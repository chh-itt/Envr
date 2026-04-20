# V integration plan (GitHub Releases zip bundles)

## Goal

Add **`RuntimeKind::V`** as a first-class managed runtime (CLI / GUI / shims / pins / `exec` / `run`) with install layout:

`runtimes/v/versions/<version>/` and `runtimes/v/current`.

V is treated as a standalone runtime in envr (no JVM host coupling).

## Scope & non-goals

- **In scope:** official release artifacts from `vlang/v` GitHub Releases (`v_windows.zip`, `v_linux.zip`, `v_macos_*.zip`).
- **Out of scope:** vup/self-update flows, project-level package management (`v install` ecosystem), alternative mirrors.

## Version/index shape

- **Source:** GitHub Releases API (`vlang/v`), stable releases only.
- **Installable row:** `(version_label, asset_url)` from release tag + host-mapped asset name.
- **Cache:** `{runtime_root}/cache/v/github_releases.json` and derived remote-line cache (TTL env knob, default 6h).
- **Resolution policy:** support exact / `major` / `major.minor` shorthand.

## Architecture / abstraction friction log

1. **Standalone-zip variation:** V bundles are ZIP archives with runtime-specific root layout; extraction promotion and executable validation must be explicit.
2. **Path-proxy runtime wiring repetition:** adding one runtime still touches multiple tables (settings snapshot, shim command mapping, GUI runtime settings section).
3. **Potential host compiler expectations:** V toolchain behavior for downstream builds can depend on C toolchain details; envr runtime integration should keep scope to V runtime installation/selection.

## Implementation checklist

### Phase A — Domain

- [x] Add `RuntimeKind::V` descriptor (`key=v`, remote/path proxy true).
- [x] Include V in version line grouping (`major.minor`).
- [x] Extend descriptor count/tests.

### Phase B — Provider crate `envr-runtime-v`

- [x] Create crate + provider implementation.
- [x] Parse GitHub releases and map host to correct asset.
- [x] Resolve/install zip and validate `v` executable layout.
- [x] Add cache + TTL knobs.

### Phase C — Core/CLI/resolver/shims

- [x] Register provider in runtime service + core Cargo wiring.
- [x] Add core shim command `v`.
- [x] Wire runtime bin dirs + runtime home env key (`V_HOME`).
- [x] Add `ENVR_V_VERSION` + list/bundle/status/shim sync/missing-pins/run-home/run-stack parity.

### Phase D — Config/GUI

- [x] Add `[runtime.v] path_proxy_enabled` (settings + snapshot + schema).
- [x] Add Env Center settings section and toggle handling.
- [x] Update runtime layout count test.

### Phase E — Docs/playbook polish

- [x] Add `docs/runtime/v.md`.
- [x] Record development friction + CLI/GUI observations.
- [x] Patch playbook if standalone runtime guidance needs extension.

## QA notes

- CLI smoke:
  - `envr remote v`
  - `envr remote v -u`
  - `envr install v 0.5`
  - `envr use v <version>`
  - `envr exec --lang v -- v version`
  - `envr which v`
- GUI smoke:
  - V tab remote/install/use/current
  - path-proxy toggle persistence and behavior

## Development notes (actual)

- V release assets are host-name-driven (`v_windows.zip`, `v_linux_arm64.zip`, etc.) rather than embedding version/platform in one strict template, so asset-candidate tables are the most fragile part and must stay explicit.
- ZIP layout can vary between “single root directory” and “flat root”; installer promotion now handles both before validating the `v` executable.
- Path-proxy runtime wiring remains repetitive across settings snapshot, shim-core command mapping, and GUI settings sections; compile-time exhaustiveness helps, but abstraction friction is still present.
- One unrelated existing test remains environment-sensitive in this workspace (`hook_env_import_export_json_contract::env_json_emits_vars_for_posix_shell`) due ambient runtime validation, not due V integration changes.
