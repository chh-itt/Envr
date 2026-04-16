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

- [ ] Add `RuntimeKind::Dotnet` and parser support.
  - Target: `crates/envr-domain/src/runtime.rs`
- [ ] Register provider in runtime service defaults.
  - Target: `crates/envr-core/src/runtime/service.rs`
- [ ] Add runtime crate to workspace/deps where needed.
  - Targets: root `Cargo.toml`, `crates/envr-core/Cargo.toml` (and any consumer crates)
- [ ] Ensure CLI runtime routing recognizes `dotnet` anywhere `lang/runtime kind` is parsed.
  - Targets: `crates/envr-cli/src/cli/**`, `crates/envr-cli/src/commands/**`

Estimated effort: 20-30 min.

### Phase B - New runtime crate (`envr-runtime-dotnet`)

- [ ] Create crate skeleton and provider wiring.
  - New: `crates/envr-runtime-dotnet/`
- [ ] Implement index fetch + version resolve strategy (major/minor/full label).
  - New: `src/index.rs`
- [ ] Implement manager paths/layout:
  - `runtimes/dotnet/versions/<label>/...`
  - `runtimes/dotnet/current` (symlink with Windows fallback if needed)
  - New: `src/manager.rs`
- [ ] Implement install pipeline:
  - download artifact
  - checksum/validation (if source supports)
  - extract/promote staging -> final
  - post-install executable validation (`dotnet` exists)
- [ ] Implement `list_installed/current/set_current/uninstall/uninstall_dry_run_targets`.
- [ ] Add unit tests for:
  - version selection
  - archive layout promotion
  - current pointer resolution fallback

Estimated effort: 70-100 min.

### Phase C - Resolver + shim integration

- [ ] Add `dotnet` into run/exec runtime-home resolution.
  - Target: `crates/envr-resolver/src/run_home.rs`
- [ ] Add `dotnet` to missing-pin planning order.
  - Target: `crates/envr-resolver/src/missing_pins.rs`
- [ ] Extend shim command enum/parser for `dotnet`.
  - Target: `crates/envr-shim-core/src/resolve.rs`
- [ ] Implement tool path resolution under dotnet home.
  - Target: `crates/envr-shim-core/src/resolve.rs`
- [ ] Add path-proxy bypass setting support (if needed for consistency with other runtimes).
  - Target: `crates/envr-shim-core/src/resolve.rs`

Estimated effort: 30-45 min.

### Phase D - CLI/GUI integration (MVP depth)

- [ ] Ensure CLI list/install/current/etc include `dotnet` in generic runtime flows.
  - Targets: `crates/envr-cli/src/commands/**`, `crates/envr-cli/src/cli/**`
- [ ] Ensure GUI runtime navigation and settings can render `dotnet`.
  - Targets: `crates/envr-gui/src/view/runtime_nav/mod.rs`, `crates/envr-gui/src/view/runtime_settings/**`, `crates/envr-gui/src/view/env_center/**`
- [ ] Add any required settings schema/default fields.
  - Targets: `crates/envr-config/src/settings.rs`, schema templates

Estimated effort: 25-40 min.

### Phase E - Validation and docs

- [ ] `cargo test --workspace --all-targets`
- [ ] Smoke-test commands:
  - `envr install dotnet@<spec>`
  - `envr current dotnet`
  - `envr run -- dotnet --info` (or equivalent path in project-pinned context)
- [ ] Add/update docs for users.
  - Target: `docs/runtime/dotnet.md` (new) and/or CLI docs
- [ ] Update this plan with actual elapsed time and friction findings.

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

## 8) Post-MVP refactor candidates (triggered by observed friction)

- Convert hardcoded runtime registration to declarative runtime metadata table.
- Reduce shim branch explosion via per-runtime resolver registry pattern.
- Unify "current symlink vs pointer file fallback" helpers across runtime crates.
- Extract reusable installer pipeline traits/helpers for archive-based runtimes.

