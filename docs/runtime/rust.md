# Rust runtime (MVP) — design notes

This document records the Rust runtime decisions and current implementation status in Envr.
It is intended as a living note for future iterations and testing.

## Scope lock (MVP)

### Goals

- **When system `rustup` exists**: Envr acts as a friendly front-end:
  - No Rust install/uninstall entrypoints in the GUI.
  - Provide channel switching (stable/beta/nightly), update, components, and targets.
  - Rust download source setting affects rustup via **child-process env injection** (no system env writes).
- **When system `rustup` does not exist**: Envr provides a managed install:
  - GUI offers **Install stable** (managed; `rustup-init` download shows in the same download panel as other runtime installs, with cancel wired through).
  - CLI: `envr rust install-managed` (stable default toolchain; uses the same stderr progress rules as `envr install` when appropriate).
  - GUI offers **Uninstall** for managed Rust only (delete managed directories under runtime root).
  - All other actions (channel/update/components/targets) operate on the managed installation.
- **Project constraints** via `.envr.toml`:
  - `warn` / `error` enforcement levels.
  - Constraints include `channel` and `rustc` `version_prefix`.
  - Enforced on `envr exec` / `envr run` paths.

### Explicitly NOT in scope (MVP)

- Cargo registry switching (we do **not** modify `~/.cargo/config.toml`).
- Rust PATH proxy/shims for `cargo/rustc/rustup` (no global terminal takeover).
- Multi-version UI lists / remote Rust toolchain listing.
- Background auto-update checks (only a manual Update action).
- Uninstall in system mode.

## Key product rules (agreed)

### Rule B (system vs managed)

- If **system `rustup` is available**, Envr uses it and does not offer managed install/uninstall.
- If **system `rustup` is not available**, Envr may install and use a **managed** rustup under the Envr runtime root.

### Mirrors / “download source”

- Mirrors are implemented as **process environment injection** to rustup (e.g. `RUSTUP_DIST_SERVER`).
- We do not write system-wide environment variables.

## What’s implemented (current)

### 1) Settings (`settings.toml`)

- **`runtime.rust.download_source`**: `auto | domestic | official`.
- The setting maps to rustup environment variables:
  - `RUSTUP_DIST_SERVER` (domestic only)
  - `RUSTUP_UPDATE_ROOT` (domestic only)

Implementation:

- `crates/envr-config/src/settings.rs`
  - `RustDownloadSource`, `RustRuntimeSettings`
  - `rustup_dist_server_from_settings()`
  - `rustup_update_root_from_settings()`

### 2) Runtime provider (`envr-runtime-rust`)

#### rustup mode detection

- `RustupMode::System` when `rustup --version` is runnable.
- `RustupMode::Managed` when managed `rustup` exists (and system rustup does not).

#### rustup command execution

- System mode: run `rustup` from PATH; inject mirror env (if configured).
- Managed mode: run managed `rustup` by absolute path; inject:
  - `RUSTUP_HOME = <runtime_root>/runtimes/rust/rustup`
  - `CARGO_HOME = <runtime_root>/runtimes/rust/cargo`
  - mirror env (if configured)

#### managed install (rustup-init)

- When system rustup is missing, managed install downloads `rustup-init` and runs it silently:
  - `-y --default-toolchain stable --no-modify-path`
  - plus `RUSTUP_HOME/CARGO_HOME` and mirror env.
- The rustup-init download URL is aligned with `runtime.rust.download_source`.
- Callers may pass an `InstallRequest` so the download reports **bytes** and supports **cancel** (GUI download panel, CLI `install_request_with_progress` / `envr rust install-managed`).

Implementation:

- `crates/envr-runtime-rust/src/manager.rs`
- `crates/envr-runtime-rust/src/installer.rs`
- `crates/envr-runtime-rust/src/lib.rs`
- `crates/envr-runtime-rust/Cargo.toml` (added deps needed for installer)

### 3) GUI — Runtime → Rust page

Rust uses a **specialized** view rather than the generic “version list” UI.

Displayed:

- Detected mode: `system` / `managed` / `none`
- Active toolchain (best-effort)
- `rustc` version (best-effort)
- Rust download source buttons (Auto/Domestic/Official)
- Channel actions: stable/beta/nightly (install-or-switch semantics)
- Update action
- Components tab (install/uninstall)
- Targets tab (install/uninstall)
- Managed-only install stable and uninstall actions

Implementation:

- `crates/envr-gui/src/view/env_center/panel.rs`
  - `RustStatus`, `RustTab`, Rust-specific messages and view
- `crates/envr-gui/src/gui_ops.rs`
  - `rust_refresh`, `rust_load_components`, `rust_load_targets`
  - `rust_channel_install_or_switch`, `rust_update_current`
  - `rust_managed_install_stable`, `rust_managed_uninstall`
  - `rust_component_toggle`, `rust_target_toggle`
- `crates/envr-gui/src/app.rs`
  - Message handling and page-enter tasks for `RuntimeKind::Rust`
- `crates/envr-gui/src/view/shell/mod.rs`
  - passes `runtime.rust` settings into `env_center_view`
- `crates/envr-gui/Cargo.toml`
  - adds dependency on `envr-runtime-rust`

### 4) Project constraints (`.envr.toml`) — warn/error

We extended the project runtime config shape for Rust-only constraints:

```toml
[runtimes.rust]
channel = "stable"        # optional: stable|beta|nightly
version_prefix = "1.78"   # optional: rustc semver prefix
enforce = "warn"          # warn|error (defaults to warn)
```

Behavior:

- `warn`: prints a warning but continues.
- `error`: returns a validation error (blocks `envr exec` / `envr run`).

Implementation:

- `crates/envr-config/src/project_config.rs`
  - extends `RuntimeConfig` with rust-only fields
  - `RustEnforceMode`
- `crates/envr-cli/src/commands/exec.rs`
- `crates/envr-cli/src/commands/run_cmd.rs`

## What’s NOT implemented yet (future ideas)

- Cargo registry switching (rsproxy.cn), with mandatory backup + exact restore semantics.
- Background update checks (e.g., daily) and “update available” UX.
- PATH proxy/shims for Rust core tools (cargo/rustc/rustup) with an enable/disable toggle.
- More robust RustStatus:
  - derive exact channel consistently
  - show more diagnostics (e.g., toolchain list)
- “Core component” protection (prevent uninstalling non-removable components) and better parsing/labels.
- Additional rustup mirrors (TUNA, etc.) if desired.

## Testing status

### Unit/CI verification

- The workspace builds and tests pass (`cargo test --workspace --all-targets`).

### Manual testing (NOT yet done)

Due to lack of a clean machine, the following scenarios have not been validated end-to-end:

1. **Clean machine without rustup**
   - Rust page shows “Install stable”.
   - Managed install succeeds (rustup-init download and silent install).
   - Channel switching + update + components + targets all work on managed install.
   - Managed uninstall deletes only managed directories and leaves system untouched.
2. **Machine with system rustup**
   - Rust page does **not** show install/uninstall.
   - Channel switching + update + components + targets operate on system rustup correctly.
3. **Mirror effects**
   - Switching `runtime.rust.download_source` changes rustup-init download base and rustup dist/update env.

### Suggested manual test checklist

- **Mode detection**
  - Verify mode label matches environment: system vs managed vs none.
- **Managed install**
  - Install stable; ensure toolchain exists; verify `rustc -V`.
- **Channel switch**
  - Switch stable → beta → stable.
- **Update**
  - Run Update; ensure it completes and does not break toolchain.
- **Components**
  - Add/remove `clippy`, `rustfmt` (if supported by toolchain).
- **Targets**
  - Add/remove `wasm32-unknown-unknown`.
- **Project constraints**
  - Add `[runtimes.rust]` constraints and verify warn/error behavior in `envr exec` and `envr run`.

