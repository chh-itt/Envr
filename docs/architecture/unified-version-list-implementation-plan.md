# Unified Version List Implementation Plan

This plan refines `unified-version-list-interface-draft.md` into executable work items.

## Current Status

- Phase 1 complete: shared runtime domain contracts and semver/key helpers landed.
- Phase 2 complete: `RuntimeService` unified list facade methods landed.
- Phase 3 complete: cache schema + SWR-style cached-first then refresh behavior landed.
- Phase 4 complete: GUI unified major/children state and lazy child loading are active.
- Phase 5 complete for non-Rust runtimes in rollout scope:
  - Node, Python, Java, Go, Ruby, Elixir, Erlang, PHP, Deno, Bun, Dotnet.
- Phase 6 partial:
  - obsolete key-mode/derived rendering branches removed from GUI main path.
  - remote-latest GUI state plumbing removed.
  - telemetry hooks are still pending.

## Scope

- Replace runtime-specific list rendering branches with one shared major/child list pipeline.
- Keep existing runtime install/uninstall/use behaviors unchanged.
- Keep lazy loading and cache-first UX as hard constraints.

## Non-goals

- No immediate migration of every runtime in one PR.
- No redesign of download/install business logic in this phase.

## Phase 1: Shared Domain Contracts

1. Add shared contracts under `envr-domain`:
   - `MajorVersionRecord`
   - `VersionRecord`
   - `VersionListAdapter` trait (or equivalent service-facing contract)
2. Add host installability helper contract:
   - `is_installable_on_host(kind, version)`
3. Add semver helpers that accept extra numeric segments:
   - support `x.y.z` and `x.y.z.w` (Erlang-safe)

Acceptance:

- Contracts compile without changing current GUI behavior.
- Unit tests for parsing and key derivation pass.

## Phase 2: Runtime Service Facade

1. Add facade methods in `envr-core::RuntimeService`:
   - `list_major_rows_cached(kind)`
   - `refresh_major_rows_remote(kind)`
   - `list_children_cached(kind, major_key)`
   - `refresh_children_remote(kind, major_key)`
2. Preserve existing `list_remote_*` methods; facade maps from current provider outputs.
3. Apply installability filtering before returning list rows.

Acceptance:

- Existing CLI commands remain unchanged and passing.
- Facade returns stable rows for at least Node, Ruby, Elixir, Erlang.

## Phase 3: Cache Schema and SWR

1. Add minimal render-oriented cache files:
   - `cache/<kind>/major_rows.json`
   - `cache/<kind>/children/<major_key>.json`
   - `cache/<kind>/unified_version_list/full_installable_versions.json` (full installable remote list; reused for every expanded major line within TTL so child refresh does not re-fetch the upstream index per expand)
2. Cache payload keeps only required fields.
3. Implement SWR behavior:
   - render cache immediately
   - refresh in background
   - merge without list blanking
4. Keep env override for upstream page coverage.
5. TTL defaults (overridable by env; disk payloads remain version strings only):
   - `ENVR_UNIFIED_LIST_MAJOR_DISK_TTL_SECS` (default 600): “fresh” window for `major_rows.json`; after expiry the file is still read for **stale-while-revalidate** paint.
   - `ENVR_UNIFIED_LIST_CHILDREN_DISK_TTL_SECS` (default 300): same for `children/<major>.json`.
   - `ENVR_UNIFIED_LIST_FULL_REMOTE_TTL_SECS` (default 300): how long to reuse `full_installable_versions.json` before re-running `list_remote` for child refresh.
6. Leaving the Runtime route clears **in-memory** unified list VM (`unified_major_rows_by_kind`, children map, expanded keys); on-disk unified cache is retained.

Acceptance:

- Cold start with cache shows list quickly.
- Remote failure does not clear rendered rows.

## Phase 4: GUI State Migration

1. In `EnvCenterState`, introduce unified VM state:
   - `major_rows: Vec<MajorRowVm>`
   - `children_rows_by_major: HashMap<String, Vec<ChildRowVm>>`
   - `expanded_majors: HashSet<String>`
2. Replace current derived key pipeline in runtime list rendering path.
3. Keep current-specific fallback:
   - if installed scan lags, inject current row into VM.
4. Preserve existing action wiring:
   - install / install&use / use / uninstall

Acceptance:

- Visual behavior matches current production behavior for non-expanded mode.
- Expanded major rows render child list on demand only.

## Phase 5: Progressive Runtime Rollout

Order:

1. Node
2. Ruby
3. Elixir
4. Erlang
5. Remaining runtimes

For each runtime:

- connect adapter/facade
- validate installable filtering
- verify expanded child rows and actions

Acceptance:

- Runtime-specific branch code in panel is reduced each rollout step.
- No regression in install/switch/uninstall flow.

## Phase 6: Cleanup and Guardrails

1. Remove obsolete key-mode strategy branches no longer needed.
2. Keep Bun/Windows host support guard in shared installability filter.
3. Add telemetry:
   - time to first major rows render
   - child rows load latency
   - cache hit ratio

Acceptance:

- Panel code path is unified and shorter.
- Existing tests and new unified list tests pass.

## Test Plan

- Unit tests:
  - semver key parsing for 3/4 segments
  - major key and child key derivation
  - SWR merge behavior
- Integration tests:
  - facade output with cached + remote fallback
  - installable filtering for host/platform
- Manual:
  - enter runtime page (cache-first render)
  - expand/collapse major rows (on-demand child load)
  - install/use/uninstall from both major row and child row contexts
  - leave page and return (render state reset, cache retained)

## PR Strategy

- PR-1: contracts + facade + cache schema (no UI switch yet)
- PR-2: GUI unified VM + Node rollout
- PR-3: Ruby/Elixir/Erlang rollout
- PR-4: remaining runtimes + cleanup

Progress note:

- PR-1/PR-2/PR-3/PR-4 implementation work has been merged into current working tree changes.
- Remaining follow-up is mainly observability/telemetry guardrails and optional polishing.
