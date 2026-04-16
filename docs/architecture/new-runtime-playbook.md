# New Runtime Playbook

This document is the reusable execution checklist for adding a new runtime to envr.
It exists to prevent the exact class of omissions that appeared during `.NET` integration:
runtime list cache behavior, missing GUI settings blocks, missing current-version action parity,
scattered capability registration, and incomplete run/exec/shim environment coverage.

Use this playbook together with the runtime-specific design doc for the candidate runtime.

## 1) Purpose

- Provide one consistent flow for adding a runtime end-to-end.
- Reduce "I added the provider, but forgot the GUI/doctor/shim/help/docs/test path" regressions.
- Force explicit decisions for runtime-specific exceptions before coding starts.

## 2) Before coding

Create a runtime-specific plan first. It should answer:

- Runtime key: for example `ruby`
- Display labels: English + Chinese
- Core command surface: for example `ruby`, `gem`, `bundle`, `irb`
- Version source and spec grammar:
  - major only?
  - major.minor?
  - full version?
  - preview/prerelease handling?
- Install layout assumptions under `runtimes/<key>/versions/<label>`
- Whether `current` is a symlink, pointer file fallback, or custom marker
- Whether the runtime needs extra child env:
  - for example `JAVA_HOME`, `GOROOT`, `DOTNET_ROOT`
- Whether the runtime needs package registry/proxy env derived from settings
- Whether PATH proxy toggle is supported
- Whether remote latest / major-line cache is supported
- Whether project-local config outside `.envr.toml` can override runtime selection
  - for example `.ruby-version`, `Gemfile`, toolchain files, etc.

Do not start coding before these decisions are written down.

## 3) Standard implementation checklist

Legend:

- `[ ]` not started
- `[~]` in progress
- `[x]` done

### A. Domain and descriptor registration

- [ ] Add `RuntimeKind::<NewRuntime>`
- [ ] Add descriptor entry in `RUNTIME_DESCRIPTORS`
- [ ] Set:
  - `key`
  - `label_en`
  - `label_zh`
  - `supports_remote_latest`
  - `supports_path_proxy`
- [ ] Ensure `parse_runtime_kind()` accepts the runtime key
- [ ] Confirm descriptor-driven lists automatically pick it up where expected

Files commonly involved:

- `crates/envr-domain/src/runtime.rs`

### B. Runtime provider crate

- [ ] Create `crates/envr-runtime-<key>/`
- [ ] Add dependencies and workspace wiring
- [ ] Implement provider surface:
  - `list_installed`
  - `current`
  - `set_current`
  - `list_remote`
  - `list_remote_majors` if applicable
  - `list_remote_latest_per_major` if applicable
  - `resolve`
  - `install`
  - `uninstall`
  - `uninstall_dry_run_targets`
- [ ] Validate install layout after extraction/promote
- [ ] Validate core executable exists
- [ ] Add Windows fallback for `current` if symlink creation can fail

Typical targets:

- `crates/envr-runtime-<key>/src/lib.rs`
- `crates/envr-runtime-<key>/src/index.rs`
- `crates/envr-runtime-<key>/src/manager.rs`

### C. Runtime service registration

- [ ] Register provider in runtime service defaults
- [ ] Confirm `RuntimeService::new()` / `with_defaults()` both include it

Typical target:

- `crates/envr-core/src/runtime/service.rs`

### D. Resolver / run / exec / missing-pin integration

- [ ] Add runtime-home resolution for `run` / `exec`
- [ ] Add missing-pin planning order if the runtime participates
- [ ] Ensure run/exec env path construction can include the runtime
- [ ] If runtime requires home-specific env vars, wire them through the shared Phase 4 helpers
- [ ] If runtime needs extra registry/proxy env, extend settings-derived tooling env rules

Typical targets:

- `crates/envr-resolver/src/run_home.rs`
- `crates/envr-resolver/src/missing_pins.rs`
- `crates/envr-resolver/src/merge_env.rs`
- `crates/envr-cli/src/commands/child_env.rs`
- `crates/envr-shim-core/src/resolve.rs`

### E. Shim and core command integration

- [ ] Decide whether the runtime has only one core command or several
- [ ] Extend `CoreCommand` if the runtime participates in envr shims
- [ ] Add command parsing
- [ ] Add tool executable resolution under runtime home
- [ ] Add path-proxy bypass routing if supported
- [ ] Add runtime-home env injection through shared helper
- [ ] Add absolute executable fallback for CLI `exec` if Windows lookup ordering can bite
- [ ] Ensure core shim generation includes all expected command stems

Typical targets:

- `crates/envr-shim-core/src/resolve.rs`
- `crates/envr-cli/src/commands/exec.rs`
- `crates/envr-core/src/shim_service.rs`

### F. Settings and capability exposure

- [ ] Add `settings.toml` runtime section if the runtime has configurable behavior
- [ ] Add defaults
- [ ] Add schema/template docs
- [ ] Add read-from-disk helper if the setting participates in shim snapshots
- [ ] Confirm descriptor capability flags match actual implementation

Typical targets:

- `crates/envr-config/src/settings.rs`
- `crates/envr-config/templates/settings.schema.zh.toml`

### G. CLI parity checklist

- [ ] `envr list`
- [ ] `envr current`
- [ ] `envr remote`
- [ ] `envr install`
- [ ] `envr use`
- [ ] `envr uninstall`
- [ ] `envr prune`
- [ ] `envr doctor`
- [ ] `envr cache` if the runtime has offline index/cache behavior
- [ ] `envr resolve`
- [ ] `envr exec`
- [ ] `envr run`
- [ ] `envr env`
- [ ] `envr which`

Do not assume descriptor refactor means all CLI work is automatic. Verify each command.

### H. GUI parity checklist

This is the section most likely to be missed.

- [ ] Runtime appears in left nav
- [ ] Runtime label is correct in Chinese and English
- [ ] Runtime dashboard row appears correctly
- [ ] Env Center page loads installed versions
- [ ] Remote list loads
- [ ] Remote list cache is reused when revisiting the tab
- [ ] Remote refresh does not blank out already known data unnecessarily
- [ ] Runtime-specific settings block renders if applicable
- [ ] Path Proxy toggle appears when supported
- [ ] Current version row matches all other runtimes:
  - no unexpected `Use`
  - no unexpected `Uninstall`
- [ ] Non-current versions show expected action buttons
- [ ] Busy/loading/skeleton states are correct
- [ ] Remote error banner behavior is correct

Typical targets:

- `crates/envr-gui/src/view/runtime_nav/mod.rs`
- `crates/envr-gui/src/view/dashboard/panel.rs`
- `crates/envr-gui/src/view/env_center/panel.rs`
- `crates/envr-gui/src/app.rs`
- `crates/envr-gui/src/gui_ops.rs`
- `crates/envr-gui/src/view/shell/mod.rs`

### I. Project pinning and local policy

- [ ] Add `.envr.toml` pin support: `[runtimes.<key>]`
- [ ] Decide precedence against runtime-native local config files
- [ ] Test project pin with `run` / `exec` / shim / GUI current view
- [ ] Decide whether mismatch is warning or error

Examples of runtime-native files:

- `.ruby-version`
- `.python-version`
- `global.json`

### J. Docs and help text

- [ ] Add runtime doc under `docs/runtime/<key>.md`
- [ ] Add runtime-specific integration plan if work is non-trivial
- [ ] Update CLI help wording if new env variables or behavior are now part of the generic surface
- [ ] Record known limitations explicitly

### K. Tests and verification

- [ ] Unit tests for provider logic
- [ ] Unit tests for tool executable resolution
- [ ] Integration tests for `exec --dry-run`
- [ ] Integration tests for `run --dry-run`
- [ ] Integration tests for current pointer/switch/uninstall behavior
- [ ] GUI manual checklist
- [ ] Smoke commands on a temporary runtime root

Minimum smoke matrix:

- `envr remote <key>`
- `envr install <key> <spec>`
- `envr current <key>`
- `envr use <key> <version>`
- `envr exec --lang <key> -- <core-command> --version`
- project pin via `.envr.toml`

## 4) Explicit anti-omission checklist

Before calling the runtime "done", answer every item below with `yes`:

- [ ] Did I add the runtime descriptor?
- [ ] Did I register the provider?
- [ ] Did I validate install layout, not just archive extraction?
- [ ] Did I wire run/exec/shim all three, not just one path?
- [ ] Did I expose required runtime-home env vars through shared helpers?
- [ ] Did I verify CLI `exec` absolute executable resolution on Windows-sensitive paths?
- [ ] Did I verify GUI remote cache behavior on tab switching?
- [ ] Did I verify GUI settings area exists when the runtime supports settings?
- [ ] Did I verify current-version button parity in GUI?
- [ ] Did I verify `.envr.toml` pinning manually?
- [ ] Did I write runtime docs and note known limitations?
- [ ] Did I add at least one focused integration test proving the new runtime env behavior?

If any answer is `no`, the runtime is not done.

## 5) Recommended execution rhythm

1. Write runtime-specific plan.
2. Implement provider + service registration.
3. Wire resolver/shim/exec/run before touching GUI.
4. Smoke-test CLI in an isolated temp root.
5. Implement GUI only after CLI path is stable.
6. Add docs and focused tests before declaring completion.
7. Record friction/coupling notes for future refactors.

## 6) Output documents to keep

For each non-trivial runtime, create and keep:

- `docs/runtime/<key>-integration-plan.md`
- `docs/runtime/<key>.md`

Recommended sections:

- Goal and scope
- Runtime-specific decisions
- Phased checklist
- Acceptance criteria
- Risk watchlist
- Development log
- Manual verification notes

## 7) What improved after `.NET`

The recent descriptor/runtime-policy refactor means the following are now easier than before:

- Runtime labels and capability exposure are descriptor-driven
- Full-catalog CLI iteration is less scattered
- Shared PATH/runtime-home env policy is centralized
- CLI `exec` and shim command resolution can share more logic

What is still intentionally explicit:

- runtime-native config precedence decisions
- GUI runtime-specific settings sections
- core command surface expansion
- provider-specific install/index logic

That explicit work is acceptable. Hidden omissions are not.
