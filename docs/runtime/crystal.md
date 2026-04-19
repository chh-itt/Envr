# Crystal (managed)

envr installs official Crystal builds from the **GitHub** [`crystal-lang/crystal` releases](https://github.com/crystal-lang/crystal/releases) API, selects the tarball or zip for your OS/arch, extracts under `runtimes/crystal/versions/<semver>/`, and points `runtimes/crystal/current` at the active version.

## Commands

- `envr install crystal <spec>` — install (supports exact `x.y.z`, line `x.y`, or major `x`).
- `envr use crystal <version>` — switch global `current`.
- `envr remote crystal` / GUI env-center — browse remote rows (cached).
- `envr exec --lang crystal -- …` — run with pinned or global Crystal on `PATH`.
- Shim: **`crystal`** (PATH proxy toggle in settings, `[runtime.crystal]`).

## Windows note

Official Windows archives are often named with **`unsupported`** in the filename; this is still the normal portable distribution channel, not a signal that envr skipped support.

## Environment

- `ENVR_CRYSTAL_VERSION` — set by `envr run` / template expansion when Crystal is on the stack.
- `ENVR_CRYSTAL_GITHUB_RELEASES_URL` — optional override for the releases API base URL (mirrors/proxies).
- `ENVR_CRYSTAL_RELEASES_CACHE_TTL_SECS` — TTL for the merged releases cache (default 1 hour). Legacy alias: `ENVR_CRYSTAL_INDEX_CACHE_TTL_SECS`.
- `ENVR_CRYSTAL_REMOTE_CACHE_TTL_SECS` — TTL for latest-per-major-line cache (default 24 hours).

See `docs/runtime/crystal-integration-plan.md` for asset matrix and design details.
