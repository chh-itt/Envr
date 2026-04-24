# Runtime Install Pipeline Phase A

## Goal

Standardize the common blocking install flow used by runtime managers:

1. prepare cache directories
2. download artifact
3. optional checksum verification
4. extract + promote install layout
5. activate as current version

Phase A introduces a shared orchestrator while keeping runtime-specific hooks local.

## Design

`envr-domain/src/installer.rs` now provides:

- `ensure_not_cancelled(cancel)` for unified cooperative cancellation checks
- `execute_install_pipeline(cancel, prepare, download, verify, install_layout, activate)`

The orchestrator enforces stage ordering and inserts cancellation checks between major stages.
Each runtime manager supplies closures for stage details, so runtime-specific behavior remains explicit.

## Migration Scope

Phase A migrated three baseline managers to the shared pipeline:

- `envr-runtime-bun`
- `envr-runtime-deno`
- `envr-runtime-go`

Each manager now keeps version resolution and artifact selection logic unchanged, but delegates install stage orchestration to `execute_install_pipeline`.

## Why this shape

- avoids heavy trait hierarchy early
- keeps call sites readable and explicit
- reduces duplicated stage ordering and cancellation logic
- provides stable seam for later Phase B/C expansion (GitHub-release managers and consistency tests)

## Next steps (Phase B/C)

- migrate GitHub-release managers onto the same stage orchestration
- standardize optional checksum/fallback policies behind reusable helpers
- add cross-runtime install-pipeline behavior tests
