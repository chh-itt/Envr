# PHP runtime (Windows MVP) — design notes

This document tracks PHP runtime decisions and implementation status for the current phase.

## Scope lock

### In scope (this phase)

- PHP management for **Windows artifacts** based on PHP Windows releases metadata.
- Runtime settings:
  - `download_source` (`auto` / `domestic` / `official`)
  - `windows_build` (`nts` / `ts`)
  - `path_proxy_enabled` (same semantics as Node/Python/Go)
- Env Center integration:
  - PHP settings section (download source + TS/NTS + PATH proxy)
  - Suggested rows when nothing is installed (latest **stable** patch per **minor line**, e.g. one row for `8.4.*`, filtered by NTS vs TS availability on this arch)
- Core shim support for `php` with PATH proxy bypass support.

### Out of scope (this phase)

- Composer registry/source switching.
- Native install flows for Linux/macOS package managers.
- PHP extension management UX.

## Version list (Env Center left column)

- Rows are **not** the full `releases.json` leaf list. They are **one suggested stable version per `major.minor` line** (e.g. `8.5`, `8.4`, …), using the newest patch that exists for that line in the index.
- If the official index only lists **8.5 down through 8.0** (plus e.g. **7.4**), showing just those lines is **expected** — it reflects what the feed still ships for Windows, not “missing” newer majors elsewhere.
- **NTS and TS use separate lists** (separate cache + different filtered rows): a minor line appears only if the Windows index contains a zip for that build (VS + arch). The UI labels rows with **`· NTS` / `· TS`** so the active build is obvious.
- Very old lines (e.g. `7.3` and below) are often **absent** from the current Windows builds feed; only lines present in the index can appear.

## Product rules

- TS/NTS choice is explicit in GUI settings.
- Install/resolve uses current TS/NTS selection for Windows zip choice.
- PATH proxy semantics match other managed runtimes:
  - On: `php` shim resolves envr-managed runtime
  - Off: `php` shim bypasses envr and resolves next `php` on PATH

## Notes for follow-up

- Domestic download URL can be switched to a dedicated mirror once validated.
- Linux/macOS strategy should be decided separately (system package manager vs prebuilt bundles).

## Testing status

- Automated compile/test checks are required before merge.
- Manual Windows verification should cover:
  - TS/NTS switching + install behavior
  - PATH proxy on/off behavior
  - Suggested major rows when no local installs

