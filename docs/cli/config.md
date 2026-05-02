# CLI Config Management

`envr config` provides a declarative interface for reading and writing user settings in `settings.toml`.

Project configuration is separate: use `envr init`, `envr status`, `envr project validate`, `envr project sync`, `envr why`, and `envr profile` for `.envr.toml`, `.envr.local.toml`, lockfiles, profiles, and runtime pins.

## Commands

- `envr config path`
- `envr config show`
- `envr config validate`
- `envr config keys`
- `envr config get <KEY>`
- `envr config set <KEY> <VALUE> [--type <TYPE>]`
- `envr config edit`
- `envr config schema`

## Key Format

Use dotted paths, for example:

- `mirror.mode`
- `mirror.manual_id`
- `runtime.node.download_source`
- `runtime.bun.path_proxy_enabled`

You can list all writable keys with:

`envr config keys`

These keys belong to `settings.toml`, not `.envr.toml`.

## Value Parsing

`config set` supports two parsing modes.

- Auto mode (default): tries JSON, bool, int, float, then falls back to string.
- Explicit mode: pass `--type` for stable scripting behavior.

Supported `--type` values:

- `string`
- `bool`
- `int`
- `float`
- `json`

Examples:

- `envr config set mirror.mode manual`
- `envr config set mirror.manual_id cn-fast`
- `envr config set runtime.bun.path_proxy_enabled false --type bool`
- `envr config set download.retry_max 5 --type int`
- `envr config set gui.downloads_panel.x_frac 0.35 --type float`
- `envr config set runtime.go.private_patterns "[\"corp.example.com/*\"]" --type json`

## Validation Behavior

Writes are validated before saving. If a key/value combination violates schema rules, the write is rejected and file contents are unchanged.

Example: `mirror.mode = manual` requires `mirror.manual_id`.

`envr config validate` checks only the user settings file. Use `envr status` or `envr project validate` to inspect project config, profiles, pins, and lockfiles.

## Migration and Backup

Before handling any `envr config` subcommand, the CLI attempts a lightweight migration for known legacy fields in `settings.toml`.

If migration changes are applied:

- Original file is backed up to `settings.toml.bak`.
- Migrated file is written back atomically.

Current migration rules include:

- `runtime_root` -> `paths.runtime_root`
- `mirror.strategy` -> `mirror.mode`
- `mirror.manual` -> `mirror.manual_id`
- `download.max_concurrent` -> `download.max_concurrent_downloads`
- Top-level runtime tables (`[node]`, `[python]`, ...) -> `[runtime.<name>]`

## Automation Notes

- For machine-readable output, use `--format json`.
- For plain script-friendly text, use `--porcelain` when applicable.
