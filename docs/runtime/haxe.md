# Haxe

Haxe is an envr-managed runtime.

## Versions

envr installs official binaries from `HaxeFoundation/haxe` GitHub Releases.

## Commands

- `envr remote haxe`: list remote versions
- `envr install haxe <version>`: install a version (supports `major.minor` like `4.3`)
- `envr use haxe <version>`: set current Haxe
- `envr uninstall haxe <version>`: uninstall a version
- `envr exec --lang haxe -- haxe ...`: run `haxe` inside envr runtime environment
- `envr run --lang haxe -- <cmd>`: run a command with Haxe on PATH

## Shims

Core shims:

- `haxe`
- `haxelib`

When PATH proxy is enabled, these resolve to the envr-managed Haxe under the current version.

## Environment variables

- `HAXE_HOME`: points to the resolved Haxe runtime home directory.
- `HAXE_STD_PATH`: points to the `std` directory under `HAXE_HOME`.
- `ENVR_HAXE_VERSION`: version template key used by `envr exec/run` resolution for `--lang haxe`.

## Settings

`settings.toml`:

```toml
[runtime.haxe]
path_proxy_enabled = true
```

When `path_proxy_enabled = false`, `haxe` / `haxelib` shims will passthrough to the next matching binaries on PATH outside envr shims.

