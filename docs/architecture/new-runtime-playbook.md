# New Runtime Playbook (Slim)

This is the operational playbook for adding a new runtime to envr.
Goal: keep one short, reliable path from design to release, and avoid omission-driven regressions.

If a runtime has complex upstream quirks, keep those details in `docs/runtime/<key>-integration-plan.md` rather than growing this file.

## 1) Scope and outcomes

A runtime is considered integrated only when all of the following are true:

- It is descriptor-visible and parseable by key.
- It is installable/usable through provider + runtime service.
- It works across `run` / `exec` / shim paths with consistent env behavior.
- CLI and GUI both expose expected behavior and states.
- Docs and focused tests are present.

## 2) Before coding (required decisions)

Write a runtime-specific plan first (`docs/runtime/<key>-integration-plan.md`) and lock these decisions:

- Runtime key and labels (`key`, `label_en`, `label_zh`).
- Core commands (for example: `ruby`, `gem`, `bundle`).
- Version grammar (`major`, `major.minor`, full semver, prerelease policy).
- Grouping grammar for GUI major lines, including non-semver/single-number versions (for example `696`).
- Upstream index/discovery shape (API, HTML, atom feed, static URL rules).
- Install artifact types by host (`zip`, `tar.*`, `exe`, `msi`) and install layout.
- `current` strategy (symlink, pointer fallback, or custom marker).
- Runtime home env contract (for example `JAVA_HOME`, `GOROOT`, `DOTNET_ROOT`, `R_HOME`).
- Settings/proxy/registry requirements and whether path proxy is supported.
- Hosted-runtime dependency (for example JVM-hosted languages) and preflight policy.

Do not start coding until these choices are explicit.

## 3) Implementation flow (current architecture)

Follow this order. It matches current code boundaries and reduces backtracking.

### A. Domain descriptor and kind

- Add `RuntimeKind::<NewRuntime>` and descriptor entry.
- Confirm `parse_runtime_kind()` accepts the key.
- Set capability flags correctly (`supports_remote_latest`, `supports_path_proxy`, `host_runtime`).

Primary file:

- `crates/envr-domain/src/runtime.rs`

### B. Provider crate

- Add `crates/envr-runtime-<key>/` and workspace wiring.
- Implement provider behavior: installed/current/resolve/remote/install/uninstall.
- Validate final layout and required executables after install.
- Handle Windows `current` fallback when symlink is unavailable.

Typical files:

- `crates/envr-runtime-<key>/src/lib.rs`
- `crates/envr-runtime-<key>/src/index.rs`
- `crates/envr-runtime-<key>/src/manager.rs`

### C. Runtime service registration

- Register provider in runtime service defaults.
- Verify both construction paths include it.

Primary file:

- `crates/envr-core/src/runtime/service.rs`

### D. Resolver / run / exec / env consistency

Keep these synchronized for the same runtime key and intent:

- `crates/envr-cli/src/commands/run_env_builder.rs` (`RUN_STACK_LANG_ORDER`)
- `crates/envr-resolver/src/missing_pins.rs` (`RUNTIME_PLAN_ORDER`)
- `crates/envr-resolver/src/run_home.rs`
- `crates/envr-cli/src/commands/child_env.rs`
- `crates/envr-shim-core/src/resolve.rs` (`runtime_home_env_for_key`)

Required checks:

- `run`/`exec` can resolve runtime home consistently.
- Required home env vars are injected in shared helpers.
- Hosted-runtime extra env merge is correct (if applicable).

### E. Shim and command mapping

- Add/extend core command mapping when runtime is shim-managed.
- Keep runtime key vs executable stem mapping explicit (they can differ).
- Ensure resolution + bypass behavior is correct when path proxy is off.
- Confirm shim generation includes all intended command stems.

Primary files:

- `crates/envr-shim-core/src/resolve.rs`
- `crates/envr-core/src/shim_service.rs`
- `crates/envr-cli/src/commands/exec.rs`

### F. Settings and capability exposure

- Add runtime settings block only when behavior is configurable.
- Add defaults and schema/template updates.
- Wire path-proxy snapshot/read helpers when `supports_path_proxy = true`.

Primary files:

- `crates/envr-config/src/settings.rs`
- `crates/envr-config/src/runtime_path_proxy.rs`
- `crates/envr-config/templates/settings.schema.zh.toml`

### G. CLI parity

Minimum command verification:

- `envr remote <key>`
- `envr install <key> <spec>`
- `envr current <key>`
- `envr use <key> <version>`
- `envr exec --lang <key> -- <core-command> --version`
- `envr run --dry-run`
- `envr env --lang <key> --json`

Also verify `doctor`/`cache` behavior if the runtime uses remote cache or host prerequisites.

### H. GUI parity

In current GUI architecture, runtime work usually touches:

- `crates/envr-gui/src/app/pages/env_center.rs`
- `crates/envr-gui/src/app/pages/downloads.rs`
- `crates/envr-gui/src/app/env_center_ops.rs` (`runtime_page_enter_tasks`)
- `crates/envr-gui/src/gui_ops.rs`
- `crates/envr-gui/src/view/env_center/panel.rs`
- `crates/envr-gui/src/view/runtime_nav/mod.rs`
- `crates/envr-gui/src/view/dashboard/panel.rs`

Required checks:

- Runtime appears in nav/layout and enters with real data load.
- Installed/remote/current rows are consistent (including when current version uses a different grammar).
- Remote version grouping is stable for both semver and non-semver versions.
- Runtime-specific settings area appears when expected.
- Download/cancel/error states are coherent.

### I. Docs and tests

- Add `docs/runtime/<key>.md`.
- Keep/update `docs/runtime/<key>-integration-plan.md`.
- Add focused provider tests (parser/resolve/latest-per-major where applicable).
- Add at least one integration test proving env behavior.
- Run a smoke pass on a temporary runtime root.

## 4) Anti-omission release gate

Before calling the runtime done, all must be "yes":

- [ ] Descriptor + parse key wired (`envr-domain`).
- [ ] Provider registered (`envr-core` runtime service).
- [ ] Install layout validated beyond extraction success.
- [ ] `run`/`exec`/shim all work (not only one path).
- [ ] Runtime home env vars come from shared helpers.
- [ ] GUI runtime page loads and actions are complete.
- [ ] Current version is always visible/selectable in GUI lists, even when version grammar is irregular.
- [ ] Settings/path-proxy behavior matches descriptor flags.
- [ ] If `supports_path_proxy = true`, descriptor + settings + snapshot + GUI toggle + shim bypass are all wired.
- [ ] Project pin (`.envr.toml`) behavior is verified.
- [ ] Runtime docs and known limitations are updated.
- [ ] Focused tests and smoke commands are recorded.

If any item is "no", integration is incomplete.

## 5) Suggested execution rhythm

1. Write runtime-specific plan.
2. Implement provider + register service.
3. Align resolver/run/exec/shim env behavior.
4. Smoke CLI in isolated runtime root.
5. Wire GUI and verify states.
6. Finish docs + tests.
7. Record runtime-specific lessons in runtime docs, not in this playbook.

## 6) Where runtime-specific complexity lives

Keep this playbook short. Put per-runtime details in:

- `docs/runtime/<key>-integration-plan.md`
- `docs/runtime/<key>.md`

Useful architecture references:

- `docs/architecture/adr-0001-runtime-host-dependencies-kotlin.md`
- `docs/architecture/runtime-ui-layout-plan.md`
- `docs/architecture/unified-version-list-implementation-plan.md`
- `docs/architecture/github-release-runtime-template.md`

---

## Appendix: common high-risk areas (quick reminders)

Use this as a reminder list, not a full case archive:

- Upstream release index and installable assets can diverge; verify installability, not just visibility.
- GitHub-backed indexes need pagination/rate-limit fallback strategy.
- Installer-based Windows runtimes need explicit silent-install + validation flow.
- Cross-drive promotion on Windows requires copy fallback (`os error 17` scenarios).
- Hosted runtimes should fail fast on host prerequisite checks.
- Runtime key may differ from executable stem; keep mapping consistent across resolver/shim/CLI/GUI/env keys.
