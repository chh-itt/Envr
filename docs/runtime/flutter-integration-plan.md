# Flutter integration plan (for Dart coexistence design)

## Goal

Define a clean integration strategy for **`RuntimeKind::Flutter`** while avoiding conflict with standalone `RuntimeKind::Dart`.

This plan is intentionally design-first and cross-references Dart integration decisions.

## Core policy

- Treat `dart` and `flutter` as **independent runtime kinds** in envr.
- Default behavior:
  - `dart` shim resolves standalone Dart (`RuntimeKind::Dart`).
  - `flutter` shim resolves Flutter SDK (`RuntimeKind::Flutter`).
- Do **not** implicitly reroute `dart` shim to Flutter-embedded Dart in MVP.

## Scope & non-goals (future phase)

- **In scope (future):** Flutter SDK archive install/use/current, `flutter` shim, optional explicit access to embedded Dart in `exec --lang flutter`.
- **Out of scope (future):** FVM compatibility layer, channel migration UX (`flutter channel`), plugin/cache management.

## Version/index shape (candidate)

- Use official Flutter release metadata feed by platform/channel (stable first).
- Resolve archive URLs by host tuple and selected version.
- Cache rows under `{runtime_root}/cache/flutter/`.

## Decision notes: deleting `$FLUTTER_ROOT/.git`

User suggestion: remove `$FLUTTER_ROOT/.git` after install to reduce accidental self-managed mutation.

Recommendation:

- **Default: yes, remove `.git` in envr-managed Flutter installs** to discourage `flutter upgrade/channel` inside managed directories.
- Keep this behavior explicit in docs and guarded by an opt-out env knob (future), e.g. `ENVR_FLUTTER_KEEP_GIT=1`.
- Rationale:
  - aligns source of truth with envr-managed versions;
  - reduces drift between `envr current` and Flutter internal update state.

## CLI/GUI risk notes (pre-implementation)

- CLI risk: users may expect `dart` command to switch to Flutter-embedded Dart automatically after `envr use flutter`; MVP should keep behavior explicit and documented.
- GUI risk: runtime panels for Dart and Flutter need short coexistence copy to prevent confusion when both are installed.

## Architecture / abstraction friction log

1. **Dart overlap by design:** Flutter ships embedded Dart; shim/env precedence must be explicit to avoid ambiguous behavior.
2. **Tool self-management conflicts:** Flutter CLI encourages `upgrade/channel`; envr policy needs clear user-facing constraints.
3. **Settings/runtime wiring repetition:** new path-proxy runtime still touches config snapshot + shim + GUI section.

## Planned implementation checklist (future)

### Phase A — Domain

- [ ] Add `RuntimeKind::Flutter` descriptor (`key=flutter`, remote/path proxy true).
- [ ] Define whether Flutter uses major-only or major.minor grouping in UI.

### Phase B — Provider crate `envr-runtime-flutter`

- [ ] Parse stable release metadata.
- [ ] Install SDK archive and validate `flutter` executable.
- [ ] Remove `$FLUTTER_ROOT/.git` after successful promote (default-on).

### Phase C — Core/CLI/resolver/shims

- [ ] Register provider + add `flutter` shim.
- [ ] Add `FLUTTER_HOME` and runtime bin-dir mapping.
- [ ] Add `ENVR_FLUTTER_VERSION` parity in run/list/status/bundle/missing-pins.

### Phase D — Config/GUI

- [ ] Add `[runtime.flutter] path_proxy_enabled`.
- [ ] Add Env Center settings section with explicit Dart coexistence note.

### Phase E — Docs/playbook polish

- [ ] Add `docs/runtime/flutter.md` (include “envr-managed flutter does not use flutter self-upgrade” guidance).
- [ ] Add a Dart/Flutter coexistence matrix doc section.
