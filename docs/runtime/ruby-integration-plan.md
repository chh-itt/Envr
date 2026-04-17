# Ruby runtime integration plan (envr)

This document is the execution plan for introducing `Ruby` as the next runtime used to validate
the post-`.NET` architecture improvements.

It intentionally follows `docs/architecture/new-runtime-playbook.md` so we can measure whether the
new descriptor/runtime-policy refactor actually reduced scattered edits and missed items.

## 1) Why Ruby

Ruby is a strong next candidate because it stresses different parts of the system than `.NET`:

- It is mainstream enough to matter, but not already covered by the current runtime mix.
- It introduces a package/tooling layer centered around `gem` and usually `bundle`.
- It has common project-local version conventions such as `.ruby-version`, which creates another
  local-config precedence question.
- Windows support exists, but distribution/layout can be more awkward than Node/Python/Go.
- It is a good test of whether "single runtime kind + multiple core commands" remains ergonomic.

In short: Ruby is not exotic, but it is different enough to be a useful architecture probe.

## 2) Goal and scope

### Goal

- Add `ruby` as a first-class runtime in envr.
- Use this integration to validate whether the refactor after `.NET` actually reduced churn,
  omissions, and GUI/CLI drift.

### In scope (MVP)

- Runtime kind registration: `ruby`
- Provider crate: `envr-runtime-ruby`
- Install/list/resolve/current/uninstall
- Remote version listing and version resolution
- Shim/core command support for:
  - `ruby`
  - `gem`
  - `bundle`
  - `irb`
- `.envr.toml` pin support: `[runtimes.ruby] version = "3.3"`
- CLI parity across install/current/remote/run/exec/which
- GUI parity in runtime nav, env center, settings block (if any), and cached remote list behavior
- Docs + focused tests + development log

### Out of scope (post-MVP)

- Full RubyGems mirror management policy
- Gemset emulation
- rbenv/rvm import/migration
- Native extension toolchain diagnostics beyond basic error surfacing
- Bundler global config management

## 3) Architecture fit hypotheses

- **Hypothesis A**: Descriptor-driven runtime registration now keeps Ruby surface wiring low-friction.
- **Hypothesis B**: Shared runtime-home PATH/env hooks are sufficient for Ruby without re-creating
  new run/exec/shim divergence.
- **Hypothesis C**: Multi-command runtimes (`ruby` + `gem` + `bundle` + `irb`) still fit the
  current `CoreCommand` model cleanly.
- **Hypothesis D**: GUI parity issues seen during `.NET` should now be much less likely because the
  runtime capability and iteration plumbing is centralized.

If any hypothesis fails, record it in the development log with exact hotspots.

## 4) Ruby-specific strategy decisions

These decisions should be treated as default implementation intent unless blocked by real data.

### A. Runtime key and labels

- Runtime key: `ruby`
- English label: `Ruby`
- Chinese label: `Ruby`

### B. Version source and spec grammar

Initial desired support:

- `major` like `3`
- `major.minor` like `3.3`
- full version like `3.3.6`

Recommendation:

- Do not assume Ruby has a Node-style `index.json` or `.NET`-style releases index.
- Preferred discovery order:
  - official structured source if one can be proven stable enough
  - official HTML pages such as `ruby-lang.org/en/downloads/` or `.../downloads/releases/`
  - official tarball directory / mirror listing such as `cache.ruby-lang.org/pub/ruby/`
- If official sources are still not structured enough, use a curated parser with explicit tests rather
  than depending on Git tags as the first choice.

Implementation note:

- Git tags from `ruby/ruby` may be useful as a fallback research source, but should not be the first
  production metadata source unless official release pages prove too brittle.

### C. Install layout

Target layout:

- Runtime home: `runtimes/ruby`
- Installed versions: `runtimes/ruby/versions/<version>`
- Current pointer: `runtimes/ruby/current`
- Cache: `cache/ruby`

Validation after install should require:

- `ruby` executable exists
- `gem` executable exists
- `bundle` executable presence is checked explicitly for the chosen distribution artifact
- basic `ruby --version` succeeds under envr-managed execution

Windows packaging note:

- Do not assume the Windows path can use the same artifact type as Unix.
- Official Ruby release pages primarily point to source releases, while Windows distribution commonly
  comes from RubyInstaller.
- RubyInstaller provides `.exe` installers, but also downloadable `7z` archives; MVP should prefer
  archive artifacts over interactive installers when possible.
- If Windows archive artifacts are insufficient for a portable envr layout, record that as a hard
  blocker instead of silently switching to an interactive installer flow.

### D. Core command surface

MVP core commands:

- `ruby`
- `gem`
- `bundle`
- `irb`

Open question to confirm during implementation:

- Modern Ruby includes Bundler as a default gem, but we still need to confirm whether the chosen
  artifact/distribution exposes a working `bundle` executable consistently on each target platform.
- If `bundle` is missing in a target artifact, fail validation with a clear diagnostic rather than
  silently degrading the command surface.

### E. Runtime-home env policy

Expected initial stance:

- Ruby likely does not need a mandatory runtime-home env variable analogous to `JAVA_HOME`
- However, we should evaluate whether a managed `GEM_HOME` / `GEM_PATH` policy is desirable

Recommendation for MVP:

- Do not invent global gem isolation yet unless required for correctness
- Start with runtime executable/path correctness first
- Record whether system gem contamination becomes a real problem during testing

### F. PATH proxy policy

Recommendation:

- Support `path_proxy_enabled = true/false` for consistency with most other runtimes
- Default to `true`

Why:

- Ruby users often already have system Ruby installed
- We want explicit envr-vs-system control, especially on Windows

### G. `.envr.toml` vs `.ruby-version`

This should be decided before implementation, not after.

Recommended policy:

- `.envr.toml` is envr's source of truth for runtime selection
- `.ruby-version` may be read for diagnostics or future import UX, but should not silently override
  `.envr.toml`
- If both exist and disagree:
  - MVP recommendation: warn clearly
  - possible future stricter mode: error
- If only `.ruby-version` exists and `.envr.toml` does not:
  - do not silently treat it as an envr pin in MVP
  - optional future UX: offer import/sync guidance

### H. Bundler and gem behavior

Potential friction areas:

- `bundle` may expect project-local gems and lockfile conventions
- `gem install` may write outside envr-managed runtime if no policy is set

MVP recommendation:

- Keep runtime installation/switching first-class
- Do not promise full gem/bundler environment isolation in MVP
- Document limitations explicitly if discovered

## 5) Work breakdown

Legend:

- `[ ]` not started
- `[~]` in progress
- `[x]` done

### Phase A - Runtime surface registration

- [x] Add `RuntimeKind::Ruby` and descriptor entry
- [x] Register provider in runtime service defaults
- [x] Add crate wiring to workspace/consumers
- [x] Ensure generic CLI parsing recognizes `ruby`

Target files:

- `crates/envr-domain/src/runtime.rs`
- `crates/envr-core/src/runtime/service.rs`
- workspace/Cargo files as needed

### Phase B - Provider crate

- [x] Create `crates/envr-runtime-ruby/`
- [~] Implement release metadata fetch and version resolution
- [ ] Decide and document production metadata source by platform:
  - official release HTML / tarball listing for Unix-like installs
  - RubyInstaller archive source for Windows installs
- [ ] Implement install pipeline and validation
- [x] Implement current pointer behavior with Windows fallback if needed
- [~] Implement installed/current/uninstall flows
- [~] Add focused unit tests for version resolution and layout validation
- [ ] Add explicit validation/tests for `bundle` command availability

### Phase C - Resolver / run / exec integration

- [x] Add runtime-home resolution for `ruby`
- [x] Add missing-pin planning participation if appropriate
- [x] Ensure PATH layout flows through shared runtime policy hooks
- [ ] Decide whether Ruby needs extra runtime-home env keys
- [~] Ensure `envr exec --lang ruby` and project-pinned `run` both work

### Phase D - Shim integration

- [x] Extend `CoreCommand` for `ruby`, `gem`, `bundle`, `irb`
- [x] Add tool resolution under runtime home
- [x] Add path-proxy bypass support
- [~] Confirm `envr which` source reporting stays correct
- [x] Confirm `shim_service` generates expected command stems

### Phase E - Settings and GUI

- [x] Add `runtime.ruby.path_proxy_enabled`
- [x] Add defaults and schema docs
- [~] Add runtime nav/dashboard labels
- [~] Add env center rendering
- [x] Add Ruby settings section if needed
- [ ] Confirm GUI remote list cache behavior
- [~] Confirm current-version row button parity

This phase must explicitly re-check the omissions seen in `.NET`:

- [ ] cached remote list persists across tab revisit
- [ ] settings block exists
- [ ] current version does not show incorrect action buttons

### Phase F - Validation and docs

- [ ] Add `docs/runtime/ruby.md`
- [x] Update this plan with actual friction notes
- [ ] Add focused integration tests:
  - `exec --dry-run`
  - `run --dry-run`
  - project pin path
- [x] Add precedence tests for:
  - `.envr.toml` only
  - `.ruby-version` only
  - both present and conflicting
- [ ] Manual smoke test:
  - `envr remote ruby`
  - `envr install ruby <spec>`
  - `envr current ruby`
  - `envr use ruby <version>`
  - `envr exec --lang ruby -- ruby --version`
  - `envr exec --lang ruby -- gem --version`
  - `envr exec --lang ruby -- bundle --version`

## 6) Acceptance criteria

- Ruby appears in CLI and GUI runtime surfaces without ad-hoc hardcoded gaps.
- `.envr.toml` project pin for Ruby works in `run`, `exec`, and shim resolution.
- PATH proxy toggle works and bypass source is visible in diagnostics/which flows.
- Core commands `ruby`, `gem`, `bundle`, and `irb` resolve correctly under managed runtime home.
- GUI remote list cache and current-version action parity do not regress.
- Focused tests prove the run/exec/shim policy stays aligned.
- Development log records actual friction and remaining structural pain points.

## 7) Risk watchlist

- Upstream Ruby release metadata may be less automation-friendly than Node or `.NET`
- Ruby may require different artifact acquisition strategies by platform, especially Windows
- Windows artifact layout may differ from Unix enough to require special validation logic
- `bundle` should usually exist on modern Ruby, but executable availability may still vary by chosen
  artifact/distribution and must be validated explicitly
- Gem install location semantics may expose another abstraction gap not covered by current MVP
- `.ruby-version` precedence may become a product decision hotspot

## 8) Suggested time budget

- Target MVP effort: 2.0-3.5 hours
- If the work exceeds 3.5 hours, pause and log exactly which hypothesis failed

## 9) Development log template

Use the same format as `.NET`:

```text
## [YYYY-MM-DD HH:MM] <phase/task>
- Change:
- Result:
- Friction:
- Coupling hotspot:
- Decision:
- Follow-up:
```

## 11) Development log

## [2026-04-16 00:00] phase-a runtime skeleton wired
- Change: Added `RuntimeKind::Ruby`, descriptor catalog entry, `envr-runtime-ruby` crate skeleton, and runtime service registration.
- Result: Workspace now recognizes Ruby as a first-class runtime kind; the minimal provider skeleton compiles.
- Friction: A compile error still appeared in `envr-core/src/shim_service.rs` because core shim entries remain explicitly matched by runtime kind.
- Coupling hotspot: `shim_service` still requires one manual edit per new runtime even after descriptor/catalog refactor.
- Decision: Keep `RuntimeKind::Ruby => &[]` for now, then add real Ruby core command entries in shim phase instead of over-centralizing too early.
- Follow-up: Continue into provider/index work and measure whether additional hidden hardcoded runtime lists still surface.

## [2026-04-16 00:05] phase-a refactor effect check
- Change: Compared this Ruby bring-up against the `.NET` bring-up starting from descriptor/runtime-policy refactor state.
- Result: Phase A was narrower than before; labels, most full-runtime iteration flows, and run/exec/shim env policy did not require repeated edits just to recognize the new kind.
- Friction: The remaining manual runtime-touchpoints are now more localized, but not eliminated.
- Coupling hotspot: Core shim command registration is still distributed separately from runtime descriptor metadata.
- Decision: Treat this as acceptable for now and record it as a real remaining abstraction seam rather than trying to solve it prematurely mid-runtime.
- Follow-up: Revisit after Ruby core commands are fully wired; if the same seam hurts again in Phase D, promote it into a follow-up architecture task.

## [2026-04-16 00:20] phase-b metadata and local manager baseline
- Change: Replaced the Ruby provider skeleton with a real baseline implementation for remote version parsing, version resolution, local installed-version discovery, `current` reading, `set_current`, and `uninstall`.
- Result: Ruby now has a usable provider baseline instead of all-`not implemented` stubs; `envr-runtime-ruby` and `envr-core` compile with the new code.
- Friction: Official Ruby release discovery is immediately less structured than Node or `.NET`; parsing the official releases HTML is workable, but clearly more fragile than a JSON index.
- Coupling hotspot: None serious in the refactored runtime registration path; the main complexity moved into runtime-specific metadata parsing, which is expected and acceptable.
- Decision: Use the official Ruby releases page as the first remote-version source for now, while keeping Windows artifact acquisition as a separate explicit decision for the install phase.
- Follow-up: Add focused tests and then move into install artifact selection, especially the Windows RubyInstaller archive path.

## [2026-04-16 00:23] phase-b implementation friction classification
- Change: Classified the first non-architectural implementation bug encountered while wiring Ruby semver comparison.
- Result: The issue was a local helper mistake, fixed quickly without touching shared runtime abstractions.
- Friction: This did not reveal new architecture debt; it was ordinary implementation churn.
- Coupling hotspot: None.
- Decision: Do not over-interpret routine coding mistakes as refactor failure signals.
- Follow-up: Keep distinguishing between "normal implementation bug" and "multi-file architecture friction" in later Ruby logs.

## [2026-04-16 00:35] phase-b remote cache behavior probe
- Change: Smoke-tested `envr remote ruby` and `envr remote ruby --prefix 3.3` against the new provider.
- Result: Prefix queries work and return real official Ruby versions; no-prefix remote rows still show empty on a cold cache because the CLI path prefers disk snapshot + background refresh semantics.
- Friction: This is a subtle UX friction point for new runtimes: implementing remote discovery is not enough, the cached latest-per-major path must also be exercised through the caller behavior.
- Coupling hotspot: CLI remote UX still has implicit expectations about on-disk snapshot availability and process lifetime of background refresh work.
- Decision: Record this as a real integration friction, but do not broaden scope into a CLI remote command redesign during Ruby provider bring-up.
- Follow-up: Keep Ruby cache persistence implemented and revisit whether the caller-side cold-cache UX needs a later global fix.

## [2026-04-16 00:45] phase-cd resolver and shim command wiring
- Change: Added Ruby to run-home resolution, missing-pin planning, run/exec env stack ordering, `CoreCommand`, shim executable resolution, and generated shim stems (`ruby`, `gem`, `bundle`, `irb`).
- Result: The Ruby command surface is now wired through resolver, shim-core, core shim generation, and CLI compile paths.
- Friction: The remaining edits were focused and localized; the refactor noticeably reduced the number of unrelated runtime list/label/capability files touched at this stage.
- Coupling hotspot: Core command registration is still an explicit seam spanning `envr-shim-core` and `envr-core::shim_service`, but it behaved predictably.
- Decision: Keep this seam explicit for now because Ruby is a good test of a multi-command runtime and the current cost was acceptable.
- Follow-up: Add focused tests around `parse_core_command`, fake runtime-home tool lookup, and then continue toward install pipeline / real end-to-end smoke validation.

## [2026-04-16 11:45] phase-b windows install robustness
- Change: For Windows RubyInstaller `.7z` installs, increased HTTP timeout to `180s`, added Range-based resume and up-to-3 retry attempts on request/read failures, and auto-promoted single-root extraction layouts (to avoid nested `ruby/` discovery failures).
- Result: `cargo test -p envr-runtime-ruby` passes; the provider install path is now resilient to slow/unstable large-asset downloads.
- Friction: End-to-end `envr install ruby <ver>` is still sensitive to the environment's connectivity to GitHub release assets (not an abstraction-layer issue).
- Coupling hotspot: Ruby install provider still uses its own synchronous download implementation, so it does not automatically inherit the project's async GUI download / mirror probe behavior.
- Decision: Keep the change localized to provider correctness + robustness; revisit shared download abstractions later if installs remain flaky.
- Follow-up: Re-run `envr install ruby 3.3.11` + `current/exec` smoke; if it still fails, capture the exact reqwest error type and decide whether to implement shared download/mirror logic in the Ruby provider.

## [2026-04-16 12:20] phase-d ruby path-proxy bypass + shim routing
- Change: Extended `ShimSettingsSnapshot` with `ruby_path_proxy_enabled` and wired `uses_path_proxy_bypass` for `CoreCommand::{Ruby,Gem,Bundle,Irb}`; updated pinned runtime-home selection so `.ruby-version` acts as a Ruby-only fallback when `.envr.toml` does not pin.
- Result: Ruby PATH proxy toggle now affects shim routing; `envr which` source can report `PathProxyBypass` for ruby/gem/bundle/irb when configured.
- Friction: A Rust lifetime mismatch appeared while adapting the Ruby pinned-spec fallback; fixed by switching to an owned `Option<String>` internally.
- Coupling hotspot: PATH-proxy bypass behavior requires extending the shim settings snapshot per runtime, not only GUI toggles.
- Decision: Keep the Ruby `.ruby-version` fallback logic local to `runtime_home_for_key` (no new global env policy yet).
- Follow-up: Add/extend `envr which` behavior tests (not just the bypass predicate) once the integration harness is expanded.

## [2026-04-16 12:35] phase-e GUI: ruby settings block + persisted toggle
- Change: Added `RuntimeSettings.runtime.ruby.path_proxy_enabled`, updated `settings.schema.zh.toml` template with `[runtime.ruby]`, and implemented the runtime settings section in `envr-gui` for the PATH proxy toggle (including `EnvCenterMsg::SetRubyPathProxy` + shim sync when enabled).
- Result: GUI now persists and applies the Ruby PATH proxy toggle consistently with other runtimes.
- Friction: `envr-gui` contains several explicit `match RuntimeKind` branches (remote skeleton/refresh + view layout) that required Ruby arms to keep compilation exhaustive.
- Coupling hotspot: GUI view code still has “explicit per-runtime match arms” in a few places, so new runtime kinds can cause multiple localized edits.
- Decision: For MVP, keep Ruby remote list behavior minimal in the env center UI (no additional Ruby-specific remote state was added yet).
- Follow-up: When adding full Ruby remote caching, revisit `EnvCenterState` to track `ruby_remote_latest*` like other runtimes.

## [2026-04-16 12:55] phase-f regression test: ruby proxy bypass from disk
- Change: Added `ruby_path_proxy_bypass_follows_settings_disk` unit test in `envr-shim-core`, toggling `[runtime.ruby].path_proxy_enabled` via a temporary `ENVR_ROOT/config/settings.toml` and asserting bypass for `ruby/gem/bundle/irb`.
- Result: Prevents regressions where GUI/app toggles are wired but shim bypass logic forgets to check the Ruby setting.
- Friction: Rust marked env var writes as `unsafe` in this workspace; handled safely in the test by using a global mutex.
- Coupling hotspot: Shim-core tests depend on the settings path resolution via `ENVR_ROOT`.

## 10) Success criteria for the refactor itself

This runtime should be used to judge whether the refactor was worth it.

Questions to answer after Ruby MVP:

- How many files had to change compared with `.NET`?
- Did any generic runtime list/label/capability omissions still happen?
- Did run/exec/shim env policy require duplicate edits?
- Did GUI parity issues still appear?
- Did a new runtime-specific exception reveal the next architectural limitation?

If Ruby lands with low churn and without the `.NET` omissions repeating, the refactor was likely
successful.
