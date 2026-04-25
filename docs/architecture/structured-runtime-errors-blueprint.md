# Structured Runtime Errors Blueprint

## Why Now

- Runtime modules still emit many ad-hoc string errors (`Validation(format!(...))`), which are hard to aggregate and monitor.
- Mixed-language final strings leak from low-level modules, making CLI/GUI localization inconsistent.
- `EnvrError` already has category-level codes, but lacks stable diagnostic-level codes for runtime/index/resolve failures.

## Goals

- Introduce stable, machine-readable error codes for high-frequency runtime failures.
- Keep source error chains and attach runtime context (spec, url, version, platform).
- Move final language rendering to CLI/GUI layers over stable keys/codes.

## Design

### 1) Error code table (cross-runtime)

Add diagnostic-level codes in `envr-error::ErrorCode`:

- `runtime_version_spec_invalid`
- `runtime_version_not_found`
- `remote_index_fetch_failed`
- `remote_index_parse_failed`

These complement existing broad categories (`validation`, `runtime`, `download`), and become the primary grouping key for diagnostics.

### 2) Runtime-local typed errors

Each runtime can define a local error enum in index/manager modules (e.g. `NodeIndexError`), then map to `EnvrError` via `From`.

Pattern:

- enum variant stores structured fields (`url`, `spec`, `version`, `os`, `arch`, `status`).
- `Display` is a stable English template for logs.
- `From<LocalError> for EnvrError` maps to a stable `ErrorCode`.

### 3) Translation boundary

- Runtime layer returns `EnvrError` with stable code and structured source chain.
- CLI/GUI map `ErrorCode` (plus optional context fields) to localized text.
- No locale-specific final message should be assembled inside runtime index/manager modules.

## Migration Plan

### Phase A (implemented in this change set)

- Add the four diagnostic-level codes above.
- Migrate Node/Go/PHP index paths (`fetch`, `parse`, `resolve`) to local typed errors + code mapping.

### Phase B

- Expand to manager install flow for Node/Go/PHP (artifact selection, checksum, layout).
- Add JSON error contract tests ensuring key commands emit expected `ErrorCode`.

### Phase B Status (partial)

- Node/Go/PHP manager high-frequency "not found/layout" errors now map to structured code:
  - `runtime_version_not_found`
- Node manager SHASUMS format validation now maps to:
  - `runtime_version_spec_invalid`
- Added manager-level code assertion tests (Node/PHP).

### Phase C

- Rollout template to other runtimes (Deno/Bun/Elixir/Erlang first, then remaining runtimes).
- Add CI guard to discourage new ad-hoc `Validation(format!(...))` in runtime modules.

## Concrete Checklist

- [x] Add fine-grained error codes in `envr-error`.
- [x] Create architecture blueprint doc for structured runtime errors.
- [x] Node: convert index fetch/parse/resolve errors to typed local enum.
- [x] Go: convert index fetch/parse/resolve errors to typed local enum.
- [x] PHP: convert index fetch/parse/resolve errors to typed local enum.
- [x] Add/adjust tests to assert returned `ErrorCode` for key invalid-spec/index-failure paths.
- [ ] Wire CLI/GUI translation table by `ErrorCode` (follow-up).
