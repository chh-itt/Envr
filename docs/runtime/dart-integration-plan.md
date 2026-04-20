# Dart integration plan (Google dart-archive SDK zip)

## Goal

Add **`RuntimeKind::Dart`** as a first-class managed runtime (CLI / GUI / shims / pins / `exec` / `run`) with install layout:

`runtimes/dart/versions/<version>/` and `runtimes/dart/current`.

Dart is a standalone runtime in envr (not modeled as host runtime of Flutter).

## Scope & non-goals

- **In scope:** official Dart SDK archives from `storage.googleapis.com/dart-archive`.
- **Out of scope:** pub package cache management, Dart beta/dev/main channels in first pass, SDK source builds.

## Version/index shape

- **Primary source:** GCS bucket object listing API  
  `https://storage.googleapis.com/storage/v1/b/dart-archive/o?prefix=channels/stable/release/&delimiter=/`
- **Latest hint source:**  
  `https://storage.googleapis.com/dart-archive/channels/stable/release/latest/VERSION`
- **Install artifact URL shape:**  
  `https://storage.googleapis.com/dart-archive/channels/stable/release/<version>/sdk/dartsdk-<platform>-<arch>-release.zip`
- **Cache:** `{runtime_root}/cache/dart/index_rows.json` (TTL env knob, default 6h).

## Architecture / abstraction friction log

1. **Non-GitHub index shape:** Dart stable version discovery comes from GCS prefix listing, unlike GitHub release JSON/atom runtimes.
2. **Single binary + SDK tree layout:** install validation should check `dart` executable under SDK layout (`bin/`) rather than root-level binary assumptions.
3. **Future Dart/Flutter overlap:** keep `dart` and `flutter` as independent runtime kinds; avoid hidden remapping in first pass.

## Implementation checklist

### Phase A — Domain

- [x] Add `RuntimeKind::Dart` descriptor (`key=dart`, remote/path proxy true).
- [x] Include Dart in version line grouping (`major.minor`).
- [x] Extend descriptor count/tests.

### Phase B — Provider crate `envr-runtime-dart`

- [x] Create crate + provider implementation.
- [x] Parse GCS stable release prefixes into installable rows.
- [x] Resolve/install host SDK zip and validate `dart` executable.
- [x] Add cache + TTL knobs.

### Phase C — Core/CLI/resolver/shims

- [x] Register provider in runtime service + core Cargo wiring.
- [x] Add core shim command `dart`.
- [x] Wire runtime bin dirs + runtime home env key (`DART_HOME`).
- [x] Add `ENVR_DART_VERSION` + list/bundle/status/shim sync/missing-pins/run-home/run-stack parity.

### Phase D — Config/GUI

- [x] Add `[runtime.dart] path_proxy_enabled` (settings + snapshot + schema).
- [x] Add Env Center settings section and toggle handling.
- [x] Update runtime layout count test.

### Phase E — Docs/playbook polish

- [x] Add `docs/runtime/dart.md`.
- [x] Record development friction + CLI/GUI observations.
- [x] Patch playbook if Dart index-source differences reveal missing checklist items.

## QA notes

- CLI smoke:
  - `envr remote dart`
  - `envr remote dart -u`
  - `envr install dart 3.11`
  - `envr use dart <version>`
  - `envr exec --lang dart -- dart --version`
  - `envr which dart`
- GUI smoke:
  - Dart tab remote/install/use/current
  - path-proxy toggle persistence and behavior

## Development notes (actual)

- GCS listing API returns both semver-style release prefixes and raw numeric revision-like prefixes under stable release root.
- We added strict semver filtering (`major.minor[.patch]`) both at parse time and cache-load normalization time.
- Existing local cache from pre-filter code can still surface stale rows until user forces refresh with `envr remote dart -u`; after refresh, cache is clean.
