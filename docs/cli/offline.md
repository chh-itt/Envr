# Offline usage

This doc explains how to run `envr` with limited or no network access.

## Offline index cache

`envr` can use a shared index cache directory to make these operations work offline:

- `envr remote ...`
- version resolve (the "which version should I install/use" step)

### Cache directory

Index cache directory is resolved as:

- `ENVR_INDEX_CACHE_DIR` (when set), else
- `{runtime_root}/cache/indexes`

### Populate cache (administrator / online machine)

Run:

- `envr cache index sync --all`

Or one runtime:

- `envr cache index sync node`
- `envr cache index sync deno`
- `envr cache index sync bun`

Check status:

- `envr cache index status`

### Use cache (offline machine / restricted network)

Set:

- `ENVR_INDEX_CACHE_DIR=/path/to/shared/indexes` (optional, if not using default)
- `envr config set mirror.mode offline`

Then `envr remote ...` will read cached indexes/tags and will not attempt network access.

## Bundle workflow (recommended)

If you also need to transport runtime binaries and project config, use `envr bundle`:

- `envr bundle create --include-indexes --output envr-bundle.zip`
- copy `envr-bundle.zip` to the offline machine
- `envr bundle apply envr-bundle.zip`

