# Nim runtime integration plan (envr)

## 1) Why Nim (and why this design)

- **Official install matrix** is published as HTML on `https://nim-lang.org/install.html` (same content as the old download page): each stable row links to **prebuilt archives on `nim-lang/nightlies` GitHub releases**.
- **URL is not a simple function of semver**: every version uses a **different** `.../releases/download/<tag>/nim-<ver>-<platform>.<ext>` tag path. envr therefore **parses the HTML index** (cached on disk with TTL) instead of guessing URLs.
- **Artifacts**: Windows `zip` (`windows_x64`, `windows_x32`), Unix `tar.xz` (`linux_*`, `macosx_*`). MVP maps **host → one primary slot** (e.g. `linux_x64`, `windows_x64`, `macosx_arm64`).
- **Unified major lines**: Nim uses **two-part** lines (`2.2`, `2.0`, `1.6`) like Go/Zig/Julia via `version_line_key_for_kind`.
- **Shim surface (MVP)**: **`nim` only** (not `nimble` / `choosenim`).

## 2) Goal and scope

### Goal

- `RuntimeKind::Nim`, crate `envr-runtime-nim`, provider registered in `RuntimeService`.
- Install / list / current / set / uninstall / resolve / remote + `list_remote_latest_per_major` for GUI.
- Shim + `exec` / `run` PATH; **no required `NIM_HOME`** (toolchain is self-contained under prefix `bin/` + `lib/`; optional env can be added later if tooling demands it).
- `.envr.toml`: `[runtimes.nim] version = "2.0.14"`.
- PATH proxy toggle in settings (same contract as Zig/Julia).

### In scope (MVP)

- Fetch + parse install HTML; cache under `cache/nim/install.html`.
- Download selected archive; extract (`zip` / `tar.xz`); hoist single root dir to `runtimes/nim/versions/<ver>/`.
- Validate `bin/nim` / `bin/nim.exe`.
- Optional **SHA256** when `.sha256` sidecar URL exists (same basename + `.sha256`).

### Out of scope

- **`choosenim`** as a managed tool inside envr.
- **`nimble`** shim (can be a follow-up).
- **Source builds** / nightlies `devel` channel.
- **Windows arm64** until upstream publishes a stable row for it.

## 3) Host → platform slot

| Host | Slot | Archive |
|------|------|---------|
| Windows x86_64 | `windows_x64` | `.zip` |
| Windows x86 | `windows_x32` | `.zip` |
| Linux x86_64 | `linux_x64` | `.tar.xz` |
| Linux aarch64 | `linux_arm64` | `.tar.xz` |
| Linux `arm` | `linux_armv7l` | `.tar.xz` |
| macOS x86_64 | `macosx_x64` | `.tar.xz` |
| macOS aarch64 | `macosx_arm64` | `.tar.xz` |

## 4) Layout

- `runtimes/nim/versions/<version>/` — hoisted Nim root (`bin/nim`, `lib/`, …).
- `runtimes/nim/current` — symlink or Windows pointer file (Zig/Julia pattern).

## 5) Acceptance / tests

- `cargo test -p envr-runtime-nim` with a **small HTML fixture** containing two URLs.
- Manual: `envr remote nim -u`, `envr install nim 2.0.14`, `envr use nim 2.0.14`, `envr exec --lang nim -- nim --version`.

## 6) Architecture / friction log

- **HTML as index**: brittle if nim-lang reformats the page; mitigate with **regex on stable URL substrings** + tests on fixture; cache full HTML for debugging.
- **Nightlies URL coupling**: installable set is defined by whatever the HTML links to—not by semver alone; document in playbook (new-runtime) under “index ≠ semver formula”.
- **GUI / settings**: PATH-proxy toggles still need a per-runtime settings section in the hub, but **reading** is centralized in `envr_config::runtime_path_proxy` (`RuntimeSettings::path_proxy_enabled_for_kind`, `PathProxyRuntimeSnapshot` for shims, single `RuntimeKind` map in `PathProxyRuntimeSnapshot::enabled_for_kind`).

### CLI / GUI follow-ups (post-merge smoke)

- **CLI**: Confirm `envr remote nim -u`, install/use/exec/shim on your host; `exec_dry_run_nim_resolves_project_pin` covers layout + pin only (no network).
- **GUI**: Nim tab → PATH proxy toggle → `shim sync` when re-enabled; unified major list after first cache fill.
- **Optional later**: `nimble` shim, `NIM_HOME` if downstream tools require it.

## 7) Development log

- [x] Plan (`docs/runtime/nim-integration-plan.md`).
- [x] Provider + index + manager + tests (`envr-runtime-nim`).
- [x] Domain / service / shim / resolver / CLI / GUI / settings / user doc (`docs/runtime/nim.md`).
- [x] Playbook: HTML-derived artifact URLs + optional checksum sidecar (`docs/architecture/new-runtime-playbook.md`).
