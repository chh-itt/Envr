# Paths, settings, and caches

This document describes where `envr` stores global settings, runtime installs, shims, logs, and caches.

## Data root

The data root is the base directory used for user-level configuration, logs, and general cache data.
It is selected by `EnvrPaths::runtime_root` defaults unless overridden by `ENVR_ROOT`.

| Platform | Default data root |
|---|---|
| Windows | `%APPDATA%\envr`, then `%LOCALAPPDATA%\envr`, then `%USERPROFILE%\.envr` as fallback |
| macOS | `~/Library/Application Support/envr` |
| Linux | `$XDG_DATA_HOME/envr`, otherwise `~/.local/share/envr` |

If `ENVR_ROOT` is set to a non-empty value, the whole data tree is rooted there.

Standard subdirectories:

| Path | Purpose |
|---|---|
| `{data_root}/config/` | User settings and aliases. |
| `{data_root}/cache/` | General cache root. |
| `{data_root}/logs/` | Logs. |

## User settings

| File | Purpose |
|---|---|
| `{data_root}/config/settings.toml` | Main settings: mirrors, download concurrency, locale, appearance, runtime root override, GUI state, and related preferences. |
| `{data_root}/config/aliases.toml` | User-defined CLI argv aliases. |

Use `envr config` to inspect and edit settings where possible.

## Runtime root

The runtime root is where `envr` stores installed runtimes, shims, runtime-specific caches, and `current` selections.
It can be different from the data root.

Resolution order:

1. `ENVR_RUNTIME_ROOT`, if non-empty.
2. `paths.runtime_root` in `settings.toml`, if non-empty.
3. The platform data root.

Typical contents:

| Path | Purpose |
|---|---|
| `{runtime_root}/runtimes/` | Installed runtime versions. |
| `{runtime_root}/shims/` | Shim executables/scripts. |
| `{runtime_root}/cache/` | Runtime download and remote-index caches. |

## Project configuration

Project-local configuration is stored in the project tree, not under the global data root.

Common files:

| File | Purpose |
|---|---|
| `.envr.toml` | Project runtime pins, env overlays, scripts, and profile settings. |
| `.envr.local.toml` | Optional local overrides that should generally stay out of version control. |

Use `envr init`, `envr project`, `envr check`, `envr status`, and `envr why` to create, validate, and explain project configuration.

## Remote index and runtime caches

Runtime providers may cache upstream indexes and derived version lists under `{runtime_root}/cache/<runtime>/`.
For example, Node.js caches live under `{runtime_root}/cache/node/`.

Examples:

| File or directory | Purpose |
|---|---|
| `index_body_<fingerprint>.json` | Cached upstream index response body. |
| `remote_majors_<os>_<arch>.json` | Cached major-version list for the host. |
| `remote_latest_per_major_<os>_<arch>.json` | Cached latest patch version per major for the host. |
| `<version>/...` | Download cache or temporary install artifacts for a runtime version. |

Runtime-specific TTL environment variables may control cache behavior. For Node.js, `ENVR_NODE_REMOTE_CACHE_TTL_SECS=0` disables disk-cache reads and forces refresh attempts.

## Cache recovery contract

Remote-list caches are considered rebuildable. The intended behavior is:

- Writes use atomic-style helpers where available: write temporary file, flush, rename, and best-effort sync parent directory.
- Reads validate TTL, JSON parsing, and caller-provided invariants.
- Expired caches are ignored.
- Corrupt or invalid caches are deleted and rebuilt on the next refresh path.
- TTL `0` means “do not read disk cache”.

This keeps bad cache files from poisoning behavior until the normal TTL expires.

## GUI state

Some GUI preferences are persisted in `settings.toml`, including downloads panel visibility, expanded state, and position.
Transient UI drafts are only written when settings are saved.

## Quick reference

- Global settings: `{data_root}/config/settings.toml`
- User aliases: `{data_root}/config/aliases.toml`
- Runtime installs and shims: `{runtime_root}/runtimes/`, `{runtime_root}/shims/`
- Runtime indexes and downloads: `{runtime_root}/cache/`
- Project pins: `.envr.toml`
