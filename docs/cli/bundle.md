# Bundle and environment snapshot

`envr bundle` creates a portable zip that can be applied on another machine.

## Create

Create a bundle zip:

- `envr bundle create --output envr-bundle.zip --include-indexes`

Optional:

- `--path <DIR>`: search upward for `.envr.toml` starting from DIR
- `--profile <NAME>`: use `[profiles.<name>]` overlay when reading project config
- `--include-shims`: also include `{runtime_root}/shims`
- `--full`: include all runtimes under `{runtime_root}/runtimes` (larger)
- `--no-current`: do not include global `current` selections (project pins only)

The bundle contains:

- `envr-bundle/manifest.json`
- `envr-bundle/runtime_root/runtimes/...` (precise by default; all when `--full`)
- `envr-bundle/index_cache/indexes/...` when `--include-indexes` is set
- `envr-bundle/project/.envr.toml` and `.envr.local.toml` when found

## Apply

Apply on the target machine:

- `envr bundle apply envr-bundle.zip`

Override destination dirs:

- `envr bundle apply envr-bundle.zip --runtime-root /data/envr`
- `envr bundle apply envr-bundle.zip --index-cache-dir /shared/envr-indexes`

## Notes

- `apply` merges directories (it does not delete existing installs).
- For fully offline behavior of `remote/resolve`, also set `envr config set mirror.mode offline` and ensure index cache is present.

