# Runtime Descriptor Refactor Plan

## Why this refactor

Adding `.NET` exposed recurring friction:

- Runtime metadata is duplicated in many crates (`cli`, `gui`, `resolver`, `shim`, `core`).
- Small behavior changes (labels, path-proxy, PATH/bin rules) require scattered `match` edits.
- CLI and GUI can drift because each maintains its own runtime capability map.

Goal: move from "enum + distributed match" to "enum + centralized descriptor + focused integration points".

## Design target

Introduce a single source of truth in `envr-domain`:

- `RuntimeDescriptor` with:
  - `kind`
  - `key` (parse key, e.g. `dotnet`)
  - `label_en` / `label_zh`
  - `supports_remote_latest`
  - `supports_path_proxy`
- `RUNTIME_DESCRIPTORS` static catalog
- helper APIs:
  - `runtime_descriptor(kind)`
  - `runtime_kinds_all()`

Then incrementally migrate call-sites to consume the catalog.

## Phased implementation

### Phase 1 (started in this change)

- Add runtime descriptor model + catalog in `envr-domain`.
- Refactor `parse_runtime_kind` to use descriptor keys.
- Migrate GUI nav and labels to descriptor-driven rendering.

Acceptance:

- `cargo check` passes for updated crates.
- Runtime tabs and labels render unchanged from user perspective.
- `parse_runtime_kind` behavior remains backward-compatible.

### Phase 2 (completed)

- Migrate CLI human labels to descriptor (`kind_label` style helpers).
- Replace hardcoded runtime iteration arrays where full catalog is intended.
- Keep explicit local overrides where behavior is intentionally non-uniform.

Acceptance:

- No user-visible behavior regressions in CLI output.
- No duplicated label maps in CLI/GUI modules touched by this phase.

### Phase 3 (completed)

  - Descriptor-backed capability gating:
  - path proxy support
  - remote latest support
- Remove duplicated capability `match` trees from GUI state handlers.

Acceptance:

- `.NET`/Node/Python/... capability behavior is unchanged.
- Adding a new runtime requires fewer file edits.

### Phase 4 (completed)

- Unify PATH/env planning entry for `run`/`exec`/shim via shared runtime policy hooks.
- Move runtime bin-dir and runtime-specific env injection policies behind descriptor-linked helpers.

Acceptance:

- dotnet/go/java environment behavior remains verified by smoke tests.
- no duplicated runtime env injection logic across `cli` and `shim` paths.

## Risks and mitigations

- Risk: Over-centralization can hide runtime-specific exceptions.
  - Mitigation: keep descriptor minimal; use explicit per-runtime hooks where needed.
- Risk: behavior drift during migration.
  - Mitigation: phase-by-phase compile/test + keep existing tests green.

## Changes made now (Phase 1)

- Added descriptor catalog and helpers in `crates/envr-domain/src/runtime.rs`.
- Updated parse flow to use descriptor keys.
- Switched GUI runtime nav iteration to `runtime_kinds_all()`.
- Switched GUI runtime labels in env-center/dashboard to descriptor labels.

## Additional progress (Phase 2 started)

- Migrated CLI runtime kind labels to descriptor key lookup:
  - `crates/envr-cli/src/commands/common.rs`
  - `crates/envr-cli/src/app/runtime_installation.rs`
- Migrated default `envr list` runtime iteration to descriptor catalog (`runtime_kinds_all()`), which also fixes omission of `.NET` in default list output.
- Migrated GUI dashboard runtime overview iteration to descriptor catalog (`runtime_kinds_all()`).

## Additional progress (Phase 2 continued)

- Migrated CLI runtime iteration lists to descriptor catalog in:
  - `commands/current.rs`
  - `commands/prune.rs`
  - `commands/remote.rs` (filtered by `supports_remote_latest`)
  - `commands/doctor.rs` + dependent loops in `doctor_fixer.rs` / `doctor_analyzer.rs`
- Migrated core shim stem aggregation in `envr-core/src/shim_service.rs` to descriptor-driven runtime iteration.
- Smoke-verified `current/remote/prune --help/doctor --format json` after migration.

## Phase 3 progress (capability gating)

- GUI action gating:
  - `envr-gui/src/app.rs`: `runtime_path_proxy_blocks_use` delegates to `envr_config::runtime_path_proxy::path_proxy_blocks_managed_use` (descriptor `supports_path_proxy` + consolidated settings read).
  - included `Php/Deno/Bun` and kept `Dotnet`.
- GUI view gating:
  - `envr-gui/src/view/env_center/panel.rs`: remote error banner + skeleton “waiting remote” conditions now depend on `runtime_descriptor(state.kind).supports_remote_latest` (instead of hardcoded kind lists).
- GUI skeleton shimmer state in `app.rs` now uses descriptor-based `supports_remote_latest` gating for the currently selected kind.

## Phase 4 progress (shared runtime policy hooks)

- Shared runtime-home policy moved into `envr-shim-core`:
  - `runtime_bin_dirs_for_key(home, key)`
  - `runtime_home_env_for_key(home, key)`
- `CoreCommand::project_runtime_key()` is now public so CLI exec resolution can reuse the same runtime/tool mapping as shims.
- `envr-resolver/src/merge_env.rs` now delegates runtime PATH layout to `envr-shim-core`, eliminating the duplicate bin-dir match table there.
- `envr-cli/src/commands/child_env.rs` now reuses shared runtime-home env policy for:
  - single-runtime `exec` env
  - merged `run` env
  - hook restore key tracking now includes `.NET` env keys (`DOTNET_ROOT`, `DOTNET_MULTILEVEL_LOOKUP`)
- `envr-shim-core/src/resolve.rs` now seeds `extra_env` from the shared runtime-home env helper, removing duplicated Java/Go/.NET env injection branches.
- `envr-cli/src/commands/exec.rs` now resolves absolute core tool paths via `parse_core_command` + `core_tool_executable`, removing local Go/.NET path builders.
- Verification added:
  - `envr-shim-core`: targeted helper test for Go/.NET runtime-home env + PATH policy
  - `envr-cli`: integration tests verifying `exec --dry-run` includes `.NET` runtime-home env and `run --dry-run` includes `GOROOT` for pinned Go
  - CLI help/env wording updated so it no longer implies only `JAVA_HOME` is managed

## Next coding step

Phase 4 completed. Next natural step is to decide whether to keep iterating on descriptor scope or stop the refactor here and return to feature work.
