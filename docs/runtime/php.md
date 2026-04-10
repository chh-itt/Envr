# PHP runtime — design notes

This document tracks PHP runtime decisions and implementation status for **Windows** and **Unix** (Linux / macOS).

## Windows (MVP)

### In scope

- PHP management for **Windows artifacts** based on PHP Windows releases metadata (`releases.json`).
- Runtime settings:
  - `download_source` (`auto` / `domestic` / `official`)
  - `windows_build` (`nts` / `ts`)
  - `path_proxy_enabled` (same semantics as Node/Python/Go)
- Env Center:
  - PHP settings: download source + TS/NTS + PATH proxy
  - Suggested rows when nothing is installed (latest **stable** patch per **minor line**, filtered by NTS vs TS for this arch)
- Core shim for `php` with PATH proxy bypass.

### Out of scope (Windows)

- Composer registry/source switching.
- PHP extension management UX.

### Version list (Env Center, Windows)

- Rows are **not** the full `releases.json` leaf list. They are **one suggested stable version per `major.minor` line** (e.g. `8.5`, `8.4`, …), using the newest patch for that line in the index.
- If the index only lists **8.5 down through 8.0** (plus e.g. **7.4**), that is **expected** for the Windows feed.
- **NTS and TS use separate lists** (separate cache + filtered rows). The UI labels rows with **`· NTS` / `· TS`**.

### Product rules (Windows)

- TS/NTS is explicit in GUI settings and matches the Windows zip artifact.
- There is a **single global** `current` pointer for PHP (one active version for the whole runtime, not one per NTS/TS).
- PATH proxy: on → shim uses envr-managed PHP; off → shim passes through to the next `php` on PATH.

---

## Unix (Linux / macOS) — strategy

Unix is **not** modeled like Windows: upstream ships **source**; **ZTS vs non-ZTS** is a **`./configure` compile-time choice**, not a download-time artifact. Typical stacks (**PHP-FPM + Nginx**, **Homebrew PHP**) already align with **NTS-style** builds. envr therefore **does not** offer a Windows-equivalent NTS/TS toggle on Unix.

### Product principles

1. **No parity with Windows downloads** in v1: no `releases.json`-driven zip install on Linux/macOS.
2. **Discover and select** PHP installs that already exist (Homebrew prefixes, `PATH`, common distro layouts).
3. **Register** discovered trees under `runtimes/php/versions/` as **symlinks** to the real prefix (envr does not replace the package manager).
4. **Single global `current`** (same idea as Windows): one active PHP for shims and Env Center.
5. **PATH proxy** semantics are unchanged: on → managed selection; off → next `php` on PATH.
6. **Suggested “remote” rows** from the Windows index are **not** used on Unix; the left column is driven by **registered / discovered** installs (and may be empty until discovery runs).

### macOS (v1 direction)

- Prefer discovering **Homebrew** layouts (`brew --prefix php`, `php@8.x` when present).
- Supplement with **`which php`** and resolve a stable **prefix** (`…/bin/php` → install root).
- envr does **not** install or upgrade Homebrew formulae in v1.

### Linux (v1 direction)

- Discover **`which php`** and common prefixes; do **not** run `apt`/`dnf`/`pacman` from envr in v1.
- UI copy can direct users to install PHP with the distro tools, then pick it in env Center.

### Explicitly out of scope (Unix, near term)

- Building PHP from source inside envr.
- Second “TS” build line in the UI (use distro/brew if you need ZTS).
- Automating FPM pool configuration.

### Implementation notes

- Settings keys `download_source` and `windows_build` may remain in `settings.toml` for cross-platform files; the **GUI hides** download source and NTS/TS on non-Windows.
- Shim **project pin** resolution on Unix uses the same semver picking as other runtimes (`pick_version_home`), not Windows NTS/TS directory suffixes.

### Implementation status (Unix, in repo)

- **`list_installed`**: runs `brew --prefix` (common `php` / `php@*` names) + `which php` → prefix; symlinks new prefixes under `runtimes/php/versions/<version>` (deduped by canonical target).
- **`set_current` / `uninstall`**: same global `current` model as Windows; uninstall removes the **registration symlink** only (does not remove Homebrew/distro packages).
- **`install` / `resolve` / Windows remote listing**: not supported on Unix (`install` returns a clear platform error; remote APIs return empty where appropriate).
- **`read_current`**: prefers the registered `versions/` entry name when resolving Homebrew Cellar paths so the label matches Env Center rows.

---

## Notes for follow-up

- Domestic Windows download URL can be switched to a dedicated mirror once validated.
- Unix: optional “remote” hints (e.g. Homebrew API) as a separate feature.
- `composer`/extension UX remains out of scope unless scheduled.

## Testing status

- Automated compile/test checks are required before merge.
- **Windows** manual checks: TS/NTS + install/uninstall/switch + PATH proxy + suggested rows.
- **Unix** manual checks: discovery populates versions, switch updates `php -v` with proxy on, proxy off passes through.
