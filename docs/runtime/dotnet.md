# .NET runtime (MVP) - design and current behavior

This document describes the current `.NET / dotnet SDK` integration in envr.

## Scope (MVP)

- Managed runtime kind: `dotnet`
- Version spec resolution: `major` / `major.minor` / full SDK version (for example: `8`, `8.0`, `8.0.420`)
- Install / uninstall / current switch under envr runtime root
- Shim execution for `dotnet` with deterministic environment injection
- Project pin support via `.envr.toml`

## Runtime layout

- Runtime home: `runtimes/dotnet`
- Installed versions: `runtimes/dotnet/versions/<sdk-version>`
- Current pointer: `runtimes/dotnet/current`
- Cache: `cache/dotnet`

## Shim execution policy

When envr resolves `dotnet` through managed runtime path (non-bypass mode), child process gets:

- `DOTNET_ROOT=<resolved dotnet home>`
- `DOTNET_MULTILEVEL_LOOKUP=0`

This prevents silent fallback to system-installed dotnet locations.

## Path proxy setting

`settings.toml`:

```toml
[runtime.dotnet]
path_proxy_enabled = true
```

- `true`: envr shim resolves managed `dotnet`
- `false`: shim bypasses envr and resolves `dotnet` from system PATH

## Resolver integration

- `envr run` / `envr exec` support `dotnet` runtime-home resolution
- Missing-pin planning includes `dotnet`
- `.envr.toml` pin key: `[runtimes.dotnet]`

## Validation behavior

Install validation requires:

- `dotnet` executable exists under installed home
- `sdk/` directory exists and is non-empty
- `dotnet --version` succeeds with managed env (`DOTNET_ROOT`, `DOTNET_MULTILEVEL_LOOKUP=0`)

## Known MVP limitations

- Workload lifecycle (`dotnet workload`) is not yet managed by envr
- Workload migration across SDK versions is not automated
- Artifact checksum verification is not finalized for all metadata sources

## Quick manual checks

- `envr current dotnet`
- `envr resolve dotnet --spec 8`
- `envr remote dotnet`
- `envr install dotnet 8` (network + disk dependent)
- `envr use dotnet <resolved-version>`
