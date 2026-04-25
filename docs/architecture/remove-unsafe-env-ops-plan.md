# Remove Unsafe Env Mutations Plan

## Problem

- Global process env mutation (`std::env::set_var/remove_var`) is `unsafe` and non-isolated.
- Tests that mutate env can leak state under parallel execution.
- GUI startup mutation for `WGPU_BACKEND` modifies process-global state.

## Goals

- Remove explicit `unsafe` env mutations from codebase.
- Keep test behavior deterministic with scoped env overrides.
- Prefer command-level env injection over process-global mutation.

## Strategy

1. Replace env mutation in tests with scoped helpers (`temp_env::with_vars`).
2. Remove runtime/global env mutation in process startup paths where possible.
3. Keep env propagation explicit at process boundaries (`Command.env/.envs/.env_clear`).

## Implemented (this change set)

- `envr-shim-core` tests now use scoped env restoration via `temp_env::with_vars`:
  - `ENVR_ROOT`
  - `PATH`
- Removed GUI startup global env mutation for `WGPU_BACKEND`.
- Added `temp-env` dev dependency to `envr-shim-core`.

## Follow-up

- Optionally add CI grep guard to reject new `unsafe { std::env::set_var/remove_var }`.
- If needed, move GUI backend selection to launcher/command env instead of in-process mutation.
