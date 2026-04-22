# SBCL (Steel Bank Common Lisp)

SBCL is an envr-managed runtime.

## Versions

envr installs **prebuilt SBCL binaries** from `roswell/sbcl_bin` GitHub Releases (chosen for consistent Windows/Linux/macOS binary availability).

## Commands

- `envr remote sbcl`: list remote versions
- `envr install sbcl <version>`: install a version (supports `major.minor` like `2.6`)
- `envr use sbcl <version>`: set current SBCL
- `envr uninstall sbcl <version>`: uninstall a version
- `envr exec --lang sbcl -- sbcl ...`: run `sbcl` inside the envr runtime environment
- `envr run --lang sbcl -- <cmd>`: run a command with SBCL on PATH

## Shims

Core shim:

- `sbcl`

When PATH proxy is enabled, `sbcl` resolves to the envr-managed SBCL under the current version.

## Environment variables

- `SBCL_HOME`: points to the resolved SBCL runtime home directory (the same directory that contains `bin/sbcl`).
- `ENVR_SBCL_VERSION`: version template key used by `envr exec/run` resolution for `--lang sbcl`.

## Settings

`settings.toml`:

```toml
[runtime.sbcl]
path_proxy_enabled = true
```

When `path_proxy_enabled = false`, `sbcl` shim will passthrough to the next matching `sbcl` on PATH outside envr shims.

