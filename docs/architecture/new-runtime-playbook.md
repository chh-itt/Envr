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
  - for example `JAVA_HOME`, `GOROOT`, `DOTNET_ROOT`, `JULIA_HOME`
- Whether the runtime needs package registry/proxy env derived from settings
- Whether PATH proxy toggle is supported
- Whether remote latest / major-line cache is supported
- Whether project-local config outside `.envr.toml` can override runtime selection
  - for example `.ruby-version`, `Gemfile`, toolchain files, etc.
- **Index / URL discovery shape**: many runtimes use one JSON or simple URL rules, but some ship installable artifacts only through a **scraped HTML matrix** or other non-formulaic index (example: Nim’s `install.html` on nim-lang.org → nightlies GitHub assets). Document parsing, caching, TTL, and optional checksum sidecars (`.sha256`) up front.
- **Installer-backed Windows runtimes** (no portable zip): some vendors only ship an `.exe` setup (examples: CRAN `R-*-win.exe` Inno; Rust **`rustup-init.exe`**). Plan for **spawn installer with documented silent flags**, **target directory layout**, **post-install validation**, and **Windows `current` pointer-file fallback** when symlinks are blocked—do not assume `extract_archive` alone can install.

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
- `crates/envr-config/src/runtime_path_proxy.rs` (PATH-proxy snapshot + `RuntimeSettings::path_proxy_enabled_for_kind`; extend when adding a PATH-proxy runtime)
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

Remote/cache parity addendum (CLI vs GUI):

- [ ] Confirm CLI and GUI read from the same **primary** remote cache tier for non-prefix list paint.
- [ ] If GUI uses unified major/children caches, ensure CLI `remote` can reuse unified full-installable snapshot before provider-local fallbacks.
- [ ] Decide whether CLI exposes a force-refresh switch (for example `remote -u/--update`) and verify it bypasses stale-first behavior.
- [ ] Keep prefix query contract explicit in docs/help/examples (`--prefix`, not positional argument).
- [ ] Verify stale-first and force-refresh paths both preserve JSON schema compatibility.

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
- `crates/envr-gui/src/view/runtime_layout.rs` (order / visibility resolution)

Additional GUI checks (hub + dashboard layout, `settings.toml`):

- [ ] Descriptor `key` matches the strings used in `[gui.runtime_layout]` (`order` / `hidden`); new runtimes are merged into default order on load.
- [ ] Runtime appears in the **horizontal hub** when not hidden; hidden runtimes are omitted there but may still appear in the dashboard “hidden” tail.
- [ ] **Dashboard overview** card line uses the localized “installed count · current” pattern; users should not see a bare `N · version` without context.
- [ ] Entering the runtime from a **dashboard card** (`OpenRuntime`) triggers the same initial load as **sidebar → Runtime** (`Navigate(Runtime)` + `runtime_page_enter_tasks`), including when the target kind is already the selected tab (avoid empty list + endless skeleton).
- [ ] If `supports_remote_latest`: follow §8.1 unified major-line / cache / `runtime_page_enter_tasks` wiring, not only `refresh_runtimes`.

See also: `docs/architecture/runtime-ui-layout-plan.md`.

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

Remote/cache regression tests:

- [ ] Offline test: pre-seeded remote cache returns non-empty `remote` rows without network.
- [ ] Prefix test: `remote --prefix` falls back to local snapshot only when live fetch fails/times out.
- [ ] Force-refresh test: `remote -u/--update` disables stale/fallback hints and returns live path semantics.
- [ ] Cache-source parity test: CLI `remote` and GUI unified list overlap on top installable versions for the same runtime.

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

## 8) Lessons from Ruby (Windows / RubyInstaller)

Ruby integration surfaced repeatable gaps. Treat this as an addendum to sections **B**, **H**, and **§4 anti-omission** for any runtime whose **installable artifact set** is not identical to a separate “upstream release index”.

### 8.1 Single source of truth for versions users can install

If installation downloads from **vendor A** (e.g. RubyInstaller `.7z` links on the downloads page), then on Windows:

- **`resolve` / `install` / `list_remote` / `list_remote_latest_per_major`** should not rely only on **vendor B** (e.g. ruby-lang.org release HTML) unless you explicitly intersect or merge the two.
- Otherwise users see versions in lists or GUI that **cannot** be installed yet (“language released, installer not published”), especially on trailing minors.

**Concrete checks:**

- [ ] Remote “latest per major” rows match artifacts your parser can actually download.
- [ ] Full GUI Env Center wiring for `supports_remote_latest`: dedicated `*_remote_latest` / `*_refreshing` state, `recompute_derived_lists` merge branch, `PickKind`, `runtime_page_enter_tasks`, `RemoteLatestDiskSnapshot` / `RemoteLatestRefreshed`, and motion/skeleton subscription—not only `refresh_runtimes` in a catch-all `else`.

### 8.2 Large binary downloads and HTTP resume

If the provider uses **HTTP Range** resume for archives:

- [ ] On **`416 Range Not Satisfiable`**, retry without `Range` after deleting or truncating the partial file (stale length vs CDN/object).

### 8.3 Shim bypass error copy

Shared helpers like `find_on_path_outside_envr_shims` are used by **all** runtimes. Avoid hardcoding one runtime name (e.g. “Node”) in user-visible strings.

**Skipping envr shims on PATH-proxy bypass:** the managed shims directory must be detected from **`ShimContext.runtime_root.join("shims")`** (compare case-insensitively on Windows and, when both paths exist, via **`fs::canonicalize`** so short 8.3 PATH entries still match the long layout). Do **not** rely only on a parent-path substring such as `"envr"`: custom `ENVR_RUNTIME_ROOT` layouts (e.g. `...\plaindata\runtimes\shims`) omit that substring, and 8.3 segments like `ENVIRON~1` can omit it even for default roots—then bypass resolves to `julia.cmd` in shims, `cmd /c` re-enters the shim, and Ctrl+C can flood “Terminate batch job (Y/N)?”.

### 8.4 GUI: what “one row per major” looks like

For runtimes that expose **latest installable version per semver major** (e.g. Ruby from installer artifacts), the left column may show **Ruby 4**, **Ruby 3**, etc.—only majors for which at least one installable artifact exists. Missing **Ruby 2** simply means the installer index no longer lists that line (or your filter excludes it). This is expected, not a broken remote list.

### 8.5 Architecture friction worth improving later

These are not blockers, but they increased integration cost:

- **GUI Env Center** still requires a **per-runtime** branch for remote-latest state and tasks; easy to forget for a new `RuntimeKind` even when `supports_remote_latest` is true in the descriptor. A descriptor-driven or table-driven “remote latest wiring” would reduce omission.
- **Two indices** (language releases vs installer artifacts) without a shared abstraction forced a second pass (RubyInstaller-only lists + download resume fix). A small internal contract (“install candidate versions”) per provider would make the rule explicit in code.

### 8.7 Zig bring-up notes (CLI/GUI cache strategy)

Zig integration surfaced one more cross-cutting friction that applies to future runtimes:

- GUI already used unified cache (`cache/<runtime>/unified_version_list/full_installable_versions.json`) for stale-first rendering, while CLI `remote` initially read provider-local `remote_latest_per_major*`.
- Outcome: GUI showed populated version rows but CLI could return temporary empty lists with `remote_refreshing=true`.
- Guardrail for new runtimes: pick **one primary remote cache abstraction** (prefer unified full-installable snapshot when unified-list UX exists), and make both GUI + CLI consume it first.

CLI contract lessons from Zig:

- Keep prefix search as explicit option syntax (`remote <runtime> --prefix <value>`).
- Provide a documented force-refresh path (`remote -u/--update`) for operators who prefer deterministic "fetch now then display" behavior.

### 8.6 Elixir bring-up notes (Hex builds)

Elixir integration validated that the earlier refactors did reduce misses, but two friction points remain visible:

- `RuntimeKind` expansion is still multi-point for GUI settings (`EnvCenterMsg`, settings fold sections, path-proxy guards). Compilation catches omissions, but this remains repetitive.
- New provider crates can silently ship with **zero parser tests** unless explicitly added. Require at least: index parse smoke test, version resolution test, and latest-per-major test.

Additional concrete frictions found during Elixir bring-up:

- **GUI derived list omissions**: Even when `RemoteLatestRefreshed` is wired, a runtime can still show an empty left list if `recompute_derived_lists` does not merge that runtime’s remote rows into key sets (e.g. missing `RuntimeKind::Elixir` branch). This is a common “data loaded but UI blank” failure mode.
- **Upstream index variance**: `builds.txt` includes multiple tag shapes (e.g. `main-otp-27`, `v1.19.5-otp-27`, `v1.0.0` without `-otp-`). Parsers must handle all relevant shapes, and OTP filtering should degrade gracefully (prefer configured OTP, but fall back to available OTP lines when absent).
- **External prerequisite runtime**: Elixir requires Erlang/OTP (`erl.exe`). Add a **preflight check** (GUI + CLI) so installs fail fast with actionable guidance, instead of failing at post-install validation.
- **Windows batch quoting trap**: `elixir.bat` uses `"%ERTS_BIN%erl.exe"`. When `ERTS_BIN` is empty, it becomes `"erl.exe"` and `cmd` will not resolve it via `PATH`. Ensure env injection sets `ERTS_BIN` (found from host PATH) so the bat resolves to an absolute `...\\erl.exe`.

Suggested guardrails for the next runtime:

- [ ] Add provider tests in the same PR as parser/index code (not as follow-up).
- [ ] Verify GUI `Set<Runtime>PathProxy` branch + shim sync is wired when descriptor enables `supports_path_proxy`.
- [ ] When a runtime has an external prerequisite (e.g. OTP), add `doctor`/GUI preflight checks and a crisp error message before download/extract work.

### 8.7 Post-Elixir hardening (settings + version list strategy)

Recent follow-up optimizations produced two practical rules for future runtime bring-up and refactors:

- **Unify settings persistence in GUI handlers**:
  - Avoid per-branch hand-written `clone -> mutate -> validate -> persist` code in `handle_env_center`.
  - Route common write paths through shared helpers (for example, one helper for generic runtime setting updates and one for path-proxy toggles that also performs shim sync when re-enabled).
  - This reduces omission risk when adding a new runtime-specific `Set*` message.

- **Keep derived version-list behavior strategy-based, not branch-heavy**:
  - `recompute_derived_lists` should rely on small reusable strategy functions (key extraction, query parsing, sorting, host-compat filtering) instead of large `match` blocks duplicated across installed/remote/filter phases.
  - Add focused unit tests for these helpers and for at least one remote-key merge case (e.g. runtime rows present remotely but not installed).
  - Minimum GUI regression tests:
    - key grouping for line-based runtimes (e.g. `major.minor`),
    - query matching rules (major/minor input behavior),
    - remote-only rows still producing visible keys.

### 8.8 Erlang/OTP bring-up notes (GitHub tag + release asset model)

Erlang integration validated the current provider abstraction on another runtime with non-trivial release naming:

- **Tag-to-installable mapping must be explicit**:
  - Upstream tags are `OTP-x.y.z(.p)` while install assets are `otp_win64_x.y.z(.p).zip`.
  - Do not assume “tag string == filename”; keep a dedicated normalization + URL builder layer.

- **Platform support should fail fast in provider/index layer**:
  - Current managed install path is Windows-first (`otp_win64_*.zip`).
  - Non-Windows hosts should return a clear platform error before download/extract logic starts.

- **RuntimeKind expansion is still multi-point for PATH-proxy runtimes**:
  - Adding a new proxy-enabled runtime requires synchronized updates in:
    - settings snapshot + disk-read helper,
    - shim core command enum + parser + bypass map + executable resolver,
    - GUI Env Center settings message/section + `path_proxy_on` branching.
  - Compile-time exhaustiveness catches misses, but repetitive wiring remains a future refactor target.

- **Minimum provider test bar should stay mandatory**:
  - Parse/normalize test (tag formats),
  - resolver test (major/minor/full specs),
  - latest-per-major selection test,
  - manager-level install-layout sanity checks (binary existence + current pointer read).

### 8.9 Erlang follow-up friction (remote coverage + version-key parsing)

Post-integration real-machine validation surfaced two subtle but important guardrails:

- **Remote "latest-per-major" data depends on upstream page coverage**:
  - GitHub tags are recency-ordered. If page cap is too small, GUI may only show newest major (for example OTP 28) and appear to "lose" older still-supported majors (OTP 27).
  - Runtime providers that build "latest per major" from paginated tags should use a safe default page window and keep an env override for constrained environments.

- **Version-key parsing must match runtime version shape, not generic SemVer assumptions**:
  - Erlang uses `major.minor.patch.build` (e.g. `27.3.4.10`); strict 3-segment parsing can silently drop installed/current entries from grouped GUI lists.
  - Grouping/parsing helpers should accept additional numeric segments when only major/minor (or major) keys are needed.

- **Current-version visibility should be resilient to transient installed-scan lag**:
  - Even if installed scan is temporarily empty, active `current` version should still be merged into derived list keys to avoid "Current shown, but row missing" UX breaks.

### 8.10 R (CRAN Windows) / Inno installer-backed bring-up

R validates the **installer-exe** pattern called out in §2 “Before coding”:

- **Provider**: `envr-runtime-rlang` — JSON index (rversions) + `cran_windows_r_installer_url` + silent Inno run + `bin/R.exe` / `bin/Rscript.exe` validation.
- **Settings**: TOML table **`[runtime.r]`** maps to `RuntimeSettings.r` (field name `r`); do not confuse with `RuntimeKind::Rust` (`rust`).
- **Shims**: two core commands (`R`, `Rscript`) sharing runtime key `r`; **`R_HOME`** in `runtime_home_env_for_key`.
- **Friction logged in** `docs/runtime/r-integration-plan.md`: third-party index, non-Windows policy, PATH substring ambiguity in dry-run output when the runtime directory segment is literally `...\r\...`.
- **Shared primitive**: `envr_platform::links::ensure_runtime_current_symlink_or_pointer` centralizes symlink-then-pointer-file `current` updates for several zip-style runtimes (Julia/Nim/Zig/Deno/Bun/R and others with the same contract).
