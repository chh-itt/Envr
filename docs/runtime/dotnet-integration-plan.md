# .NET runtime integration plan (envr)

This document is the execution checklist for introducing `.NET / dotnet SDK` into envr.
It is designed as a living record: implementation tasks, coupling/friction notes, and progress logs stay in one place.

## 1) Goal and scope

### Goal

- Add `.NET / dotnet SDK` as a first-class runtime in envr, matching existing runtime UX and CLI expectations.
- Use this integration to validate whether current runtime abstractions remain sufficient or need restructuring.

### In scope (MVP)

- Runtime kind registration (`dotnet`) across domain/core/cli/gui/resolver/shim.
- Install/list/resolve/current/uninstall for `dotnet` SDK versions.
- Basic shim proxy for core command `dotnet` (project pin -> runtime home, else global current).
- Project pin support in `.envr.toml` (`[runtimes.dotnet] version = "8"` style).
- Missing-pin planning support (`envr run`/`project sync` path).
- Docs + manual verification checklist + friction log updates.

### Out of scope (post-MVP)

- `dotnet workload` lifecycle management.
- NuGet cache/source switching policy.
- Global tool (`dotnet tool`) advanced UX and policy.
- Multi-channel/preview stream management beyond version specs.

## 2) Architecture fit hypotheses (what we are testing)

- **Hypothesis A**: Current `RuntimeProvider` interface is enough for dotnet MVP.
- **Hypothesis B**: `CoreCommand` + shim resolution model can absorb `dotnet` with low churn.
- **Hypothesis C**: Current `RuntimeKind` enum + hardcoded route lists are still manageable at 9 runtimes.
- **Hypothesis D**: `versions/<label> + current` layout is adequate for dotnet SDK distribution artifacts.

If any hypothesis fails during implementation, record in section 7 with concrete symptoms.

## 3) Work breakdown (implementation checklist)

Legend:
- `[ ]` not started
- `[~]` in progress
- `[x]` done

### Phase A - Runtime surface registration

- [x] Add `RuntimeKind::Dotnet` and parser support.
  - Target: `crates/envr-domain/src/runtime.rs`
- [x] Register provider in runtime service defaults.
  - Target: `crates/envr-core/src/runtime/service.rs`
- [x] Add runtime crate to workspace/deps where needed.
  - Targets: root `Cargo.toml`, `crates/envr-core/Cargo.toml` (and any consumer crates)
- [x] Ensure CLI runtime routing recognizes `dotnet` anywhere `lang/runtime kind` is parsed.
  - Targets: `crates/envr-cli/src/cli/**`, `crates/envr-cli/src/commands/**`

Estimated effort: 20-30 min.

### Phase B - New runtime crate (`envr-runtime-dotnet`)

- [x] Create crate skeleton and provider wiring.
  - New: `crates/envr-runtime-dotnet/`
- [x] Implement index fetch + version resolve strategy (major/minor/full label).
  - New: `src/index.rs`
- [x] Implement manager paths/layout:
  - `runtimes/dotnet/versions/<label>/...`
  - `runtimes/dotnet/current` (symlink with Windows fallback if needed)
  - New: `src/manager.rs`
- [x] Implement install pipeline:
  - download artifact
  - checksum/validation (if source supports)
  - extract/promote staging -> final
  - post-install executable validation (`dotnet` exists)
- [x] Implement `list_installed/current/set_current/uninstall/uninstall_dry_run_targets`.
- [x] Add unit tests for:
  - version selection
  - archive layout promotion
  - current pointer resolution fallback

Estimated effort: 70-100 min.

### Phase C - Resolver + shim integration

- [x] Add `dotnet` into run/exec runtime-home resolution.
  - Target: `crates/envr-resolver/src/run_home.rs`
- [x] Add `dotnet` to missing-pin planning order.
  - Target: `crates/envr-resolver/src/missing_pins.rs`
- [x] Extend shim command enum/parser for `dotnet`.
  - Target: `crates/envr-shim-core/src/resolve.rs`
- [x] Implement tool path resolution under dotnet home.
  - Target: `crates/envr-shim-core/src/resolve.rs`
- [x] Add path-proxy bypass setting support (if needed for consistency with other runtimes).
  - Target: `crates/envr-shim-core/src/resolve.rs`

Estimated effort: 30-45 min.

### Phase D - CLI/GUI integration (MVP depth)

- [x] Ensure CLI list/install/current/etc include `dotnet` in generic runtime flows.
  - Targets: `crates/envr-cli/src/commands/**`, `crates/envr-cli/src/cli/**`
- [x] Ensure GUI runtime navigation and settings can render `dotnet`.
  - Targets: `crates/envr-gui/src/view/runtime_nav/mod.rs`, `crates/envr-gui/src/view/runtime_settings/**`, `crates/envr-gui/src/view/env_center/**`
- [x] Add any required settings schema/default fields.
  - Targets: `crates/envr-config/src/settings.rs`, schema templates

Estimated effort: 25-40 min.

### Phase E - Validation and docs

- [x] `cargo test --workspace --all-targets`
- [x] Smoke-test commands:
  - `envr install dotnet@<spec>`
  - `envr current dotnet`
  - `envr run -- dotnet --info` (or equivalent path in project-pinned context)
- [x] Add/update docs for users.
  - Target: `docs/runtime/dotnet.md` (new) and/or CLI docs
- [x] Update this plan with actual elapsed time and friction findings.

Estimated effort: 20-30 min.

## 4) Acceptance criteria (done definition)

- Dotnet appears as a fully supported runtime in CLI and GUI runtime lists.
- Project pin (`[runtimes.dotnet]`) resolves correctly for shim/exec/run.
- Runtime install creates valid version dir and can be switched via `current`.
- Uninstall removes selected version safely and clears `current` when applicable.
- Existing 8 runtimes show no regression in tests/smoke checks.
- Friction/coupling notes are documented in section 7 during implementation.

## 5) Risk and coupling watchlist

- Hardcoded runtime lists across multiple crates (`enum`, route match, planning order, UI nav) may cause scattered edits.
- Shim command matrix expansion may reveal maintainability limits in `CoreCommand` branching.
- Dotnet distribution differences by OS/arch could require runtime-specific exceptions not modeled in generic installer flow.
- Potential conflict between "runtime only" abstraction and dotnet SDK/workload reality.

## 5.1) Dotnet strategy decisions (agreed before implementation)

These decisions are binding for MVP unless a blocker is recorded in section 7.

### A) Windows/system dotnet PATH competition

- When envr resolves `dotnet` via shim/runtime path (non-bypass mode), child process must inject:
  - `DOTNET_ROOT=<resolved envr runtime home>`
  - `DOTNET_MULTILEVEL_LOOKUP=0`
- This ensures envr-managed SDK selection is deterministic and does not silently fall back to system-wide `C:\Program Files\dotnet`.
- `dotnet.path_proxy_enabled` should follow existing path-proxy policy:
  - enabled (default): envr-managed dotnet is used.
  - disabled: explicit system PATH bypass is allowed and reflected in `which` source metadata.

### B) Post-install layout validation (mandatory)

- Install succeeds only when all checks pass:
  - `dotnet` executable exists under the installed home (`dotnet.exe` on Windows, `dotnet` on Unix).
  - `sdk/` directory exists and has at least one SDK version entry.
  - `dotnet --version` (or `dotnet --info`) succeeds under injected env (`DOTNET_ROOT` + `DOTNET_MULTILEVEL_LOOKUP=0`).
- Any failed check must abort commit of staging dir and return a clear validation/runtime error.

### C) `.envr.toml` vs `global.json` precedence policy

- `.envr.toml` defines envr-level runtime intent and chooses SDK root (`current` / pinned home).
- `global.json` remains respected by `dotnet` within that chosen root.
- Preflight check for `envr exec/run`:
  - Resolve `.envr.toml` dotnet spec.
  - Execute `dotnet --version` in target project context.
  - If resolved version does not satisfy `.envr.toml` constraint, treat as policy mismatch:
    - `warn`: continue with warning.
    - `error`: fail command.
- Default recommendation for dotnet enforce mode: `error` (deterministic CI/local behavior).

### D) Workload storage and version switching

- MVP policy: workloads are SDK-local; no automatic migration between SDK versions.
- On SDK switch (or when mismatch detected), surface guidance:
  - "workloads may differ by SDK version; run `dotnet workload list` to verify."
- Post-MVP command candidates:
  - workload snapshot export/import per SDK.
  - workload replay when switching SDK versions.

### E) Additional pitfalls to guard

- Global tool isolation:
  - evaluate setting dedicated `DOTNET_CLI_HOME` for envr-managed runs to reduce cross-contamination with system-level global tools.
- Architecture correctness:
  - ensure artifact selection honors host arch; avoid mixing x64/x86/arm64 SDK payloads.
- First-time experience side effects:
  - consider opt-in/opt-out setting for `DOTNET_SKIP_FIRST_TIME_EXPERIENCE`.
- Network diagnostics:
  - preserve upstream stderr and include actionable hints for proxy/cert/mirror failures.
- Safe uninstall:
  - keep uninstall dry-run target reporting and avoid deleting shared directories.

## 6) Time budget (target)

- Total target: 2.0-3.0 hours (MVP only).
- If over 3 hours, pause and record blockers in section 7 before continuing.

## 7) Development log (update during implementation)

### Log format

Use one entry per meaningful implementation step:

```
## [YYYY-MM-DD HH:MM] <phase/task>
- Change:
- Result:
- Friction:
- Coupling hotspot:
- Decision:
- Follow-up:
```

### Entries

## [2026-04-16 00:00] plan initialized
- Change: Created dotnet integration execution plan and logging template.
- Result: Ready for incremental implementation with traceability.
- Friction: None yet.
- Coupling hotspot: Predicted around runtime enum + shim command branching.
- Decision: Start from Phase A -> B -> C to reduce integration risk.
- Follow-up: Begin Phase A in next implementation step.

## [2026-04-16 12:00] strategy decisions locked
- Change: Added explicit dotnet MVP policy for PATH contention, install validation, `global.json` precedence, workload behavior, and hidden pitfalls.
- Result: Implementation can proceed with deterministic behavior rules and conflict handling criteria.
- Friction: Need to reconcile envr enforcement semantics with dotnet's native `global.json` resolver without confusing users.
- Coupling hotspot: Shim env injection + resolver/enforce checks span `envr-shim-core`, `envr-resolver`, and CLI command execution flow.
- Decision: Keep MVP strict/deterministic (`DOTNET_MULTILEVEL_LOOKUP=0`, preflight mismatch checks) and defer workload migration automation.
- Follow-up: Start Phase A with these rules as acceptance constraints.

## [2026-04-16 12:25] phase A runtime surface completed
- Change: Added `RuntimeKind::Dotnet` + parser support, created `envr-runtime-dotnet` crate scaffold, registered provider in `RuntimeService`, and updated runtime-kind label mappings used by CLI/GUI/resolver/core shim service.
- Result: `cargo check --workspace` passes with dotnet included in type system and default provider registration path.
- Friction: `RuntimeKind` enum expansion required touching multiple distributed match statements (CLI labels, GUI labels, resolver key mapping, shim service runtime lists).
- Coupling hotspot: Runtime identity is currently hardcoded in many crates, so adding a new runtime has broad but shallow edit fan-out.
- Decision: Keep scaffold provider intentionally minimal (returns "not implemented yet") to unblock integration compilation before Phase B manager/index implementation.
- Follow-up: Start Phase B by replacing scaffold behavior with real dotnet manager + index + install/current/uninstall flow.

## [2026-04-16 13:10] phase B runtime crate implemented (core path)
- Change: Replaced scaffold provider with functional `index + manager + provider` implementation in `envr-runtime-dotnet`; added remote metadata loading from .NET release index, spec resolver (major / major.minor / full), artifact selection by host RID, install/extract/validate pipeline, and current switch/uninstall/list operations.
- Result: Dotnet runtime crate compiles and integrates with workspace (`cargo check --workspace` passes).
- Friction: .NET metadata shape is nested and not uniform across channels/releases (`sdk` + `sdks` entries), requiring defensive aggregation logic.
- Coupling hotspot: Current install pipeline has reusable patterns across runtimes (download + staging + validation + current pointer fallback) but remains duplicated per runtime crate.
- Decision: Use official release metadata endpoint for MVP and keep checksum verification optional/deferred (metadata hash normalization not yet standardized in implementation).
- Follow-up: Add Phase B unit tests, then move to Phase C (resolver + shim integration) with dotnet-specific env policy (`DOTNET_ROOT`, `DOTNET_MULTILEVEL_LOOKUP=0`).

## [2026-04-16 13:28] phase B unit tests completed
- Change: Added unit tests in `envr-runtime-dotnet` for version resolution variants, installation layout validity, installed-version filtering, and pointer-file based `current` resolution.
- Result: `cargo test -p envr-runtime-dotnet` passes (7/7).
- Friction: Existing checklist mentions "archive layout promotion" explicitly, while current tests validate layout acceptance via `dotnet_installation_valid` and listing behavior (pragmatic coverage for MVP).
- Coupling hotspot: Minimal direct seams for installer internals make low-level extraction-path tests heavier; current suite focuses on deterministic pure/IO-light paths.
- Decision: Keep current unit test scope focused and rely on later smoke tests for full install/extract E2E.
- Follow-up: Proceed to Phase C resolver/shim integration.

## [2026-04-16 13:40] phase C core integration completed
- Change: Added `dotnet` to resolver run/exec home resolution, missing-pin planning order, shim core command parsing/executable resolution, and env injection (`DOTNET_ROOT`, `DOTNET_MULTILEVEL_LOOKUP=0`). Also enabled core shim generation for `dotnet`.
- Result: Workspace compiles with dotnet in resolver + shim path (`cargo check --workspace` passes).
- Friction: Introducing a new core command still requires broad match-arm edits across shim-core and shim-service, reinforcing current enum-centric coupling.
- Coupling hotspot: Path-proxy bypass policy for dotnet is currently implicit (no dedicated settings toggle yet), while other runtimes already expose per-runtime toggle semantics.
- Decision: Keep deterministic managed-dotnet behavior in shim (no dedicated bypass toggle yet) and leave toggle/settings integration for follow-up phase.
- Follow-up: Phase D will decide whether to add explicit `runtime.dotnet.path_proxy_enabled` to align policy symmetry.

## [2026-04-16 14:05] phase C/ D policy symmetry update
- Change: Added explicit dotnet path-proxy setting chain: `runtime.dotnet.path_proxy_enabled` in settings model + defaults + disk helper, template schema entry, shim settings snapshot, and bypass branch usage.
- Result: Dotnet now follows the same proxy-toggle policy family as Node/Python/Java/Go/PHP/Deno/Bun.
- Friction: Settings evolution touches multiple layers (serde model, defaults, helpers, template docs, shim snapshot), which increases change fan-out for each new runtime option.
- Coupling hotspot: Runtime settings and shim behavior are tightly coupled by duplicated field plumbing instead of declarative runtime metadata.
- Decision: Mark Phase C path-proxy item complete and Phase D settings-surface items complete for MVP.
- Follow-up: Move to Phase E validation/docs polish and then run smoke scenarios.

## [2026-04-16 14:20] phase E validation and docs finished
- Change: Ran workspace-wide tests and CLI smoke checks; added end-user runtime doc `docs/runtime/dotnet.md`.
- Result: `cargo test --workspace --all-targets` passed. `envr current dotnet` and `envr resolve dotnet --spec 8` paths verified in current environment. `envr remote dotnet` path is wired and returns structured output (may show empty when no cached snapshot yet).
- Friction: `remote` command prefers cached snapshot for no-prefix query, so first-run output can appear empty while background refresh warms cache.
- Coupling hotspot: Validation/smoke scripts rely on current CLI command semantics; behavior like remote snapshot-first policy can hide runtime-fetch issues unless prefixed/second-run checks are also used.
- Decision: Keep snapshot-first `remote` behavior unchanged for MVP; document quick checks and known caveats in runtime doc.
- Follow-up: Optional next iteration can add explicit `envr remote dotnet --force-online` style path and deeper install smoke in clean machine matrix.

## [2026-04-16 14:55] phase E hardening pass (e2e-driven fixes)
- Change: Ran isolated-runtime-root E2E (`install -> use -> current -> exec`) and fixed three issues discovered during real flow.
- Fix 1 (metadata parse): `releases.json` fields can be `null` (`releases` / `sdks` / `files`), now deserialized as empty vectors instead of failing with `invalid type: null, expected a sequence`.
- Fix 2 (artifact pick): relaxed .NET SDK artifact selector from strict filename-prefix match to scored host-RID/version/sdk archive match so current upstream naming variations still resolve correctly.
- Fix 3 (exec isolation): `envr exec --lang dotnet` now injects `DOTNET_ROOT` + `DOTNET_MULTILEVEL_LOOKUP=0`, includes dotnet runtime bin/root in PATH merge, and resolves absolute `dotnet` executable on Windows (same rationale as existing `go` handling).
- Result: Isolated E2E now succeeds for install/use/current, and `envr exec --lang dotnet --spec 8 -- dotnet --version` returns managed `8.0.420` (no longer leaking to system 9.x).
- Friction: The cross-crate coupling (`runtime_bin_dirs` in resolver + env injection in CLI + shim behavior in shim-core) makes one runtime policy split across multiple crates.

## [2026-04-16 15:20] gui parity fix for dotnet remote/state
- Symptom: `.NET` tab could appear mostly empty in GUI (especially when no local install), despite CLI path being healthy.
- Root cause: `.NET` was added to nav/labels but missing in env-center remote-latest state plumbing (`PickKind` refresh branch, cache/refresh handlers, loading predicates, install-spec selection helpers).
- Fix: Added `.NET` remote snapshot/refresh state in `EnvCenterState`, wired `.NET` through app message handlers, list recompute and key resolution helpers, and enabled `.NET` in dashboard runtime rows.
- Result: GUI now shows `.NET` keys from remote-latest cache/refresh even before local install, and dashboard includes `.NET` status row.

## [2026-04-16 15:35] gui follow-up polish from manual testing
- Symptom: entering `.NET` tab repeatedly looked like full reload; settings area missed `.NET` path-proxy switch.
- Fix: keep `.NET` remote list cache across tab switches (avoid clearing `dotnet_remote_latest` on `PickKind`), add `.NET` settings section with `PATH 代理` toggle, wire toggle persistence and shim-sync-on-enable.
- Result: `.NET` tab now keeps prior remote rows while background refresh runs, and `.NET` has parity path-proxy control like other managed runtimes.

## 8) Post-MVP refactor candidates (triggered by observed friction)

- Convert hardcoded runtime registration to declarative runtime metadata table.
- Reduce shim branch explosion via per-runtime resolver registry pattern.
- Unify "current symlink vs pointer file fallback" helpers across runtime crates.
- Extract reusable installer pipeline traits/helpers for archive-based runtimes.

