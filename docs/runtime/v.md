# V runtime

`envr` can manage official [V](https://vlang.io/) compiler releases as a standalone runtime.

## What is managed

- Remote discovery from `vlang/v` GitHub Releases
- Install under `ENVR_RUNTIME_ROOT/runtimes/v/versions/<version>`
- Global current pointer at `ENVR_RUNTIME_ROOT/runtimes/v/current`
- Shim + env integration (`v`, `V_HOME`)

## Version resolution

Supported install/use specs:

- exact: `0.5.1`
- major.minor line: `0.5` (resolves to latest patch in that line)
- major: `0` (resolves to latest visible stable in major 0)

## Host artifact mapping

`envr` chooses host assets by platform:

- Windows: `v_windows.zip`
- Linux x86_64: `v_linux.zip`
- Linux arm64: `v_linux_arm64.zip` (fallback `v_linux.zip`)
- macOS arm64: `v_macos_arm64.zip` (fallback `v_macos_x86_64.zip`)
- macOS x86_64: `v_macos_x86_64.zip`

## Cache knobs

- `ENVR_V_INDEX_CACHE_TTL_SECS` (default `21600`, i.e. 6h)
- `ENVR_V_GITHUB_RELEASES_MAX_PAGES` (default `8`)

## CLI quick checks

```powershell
envr remote v
envr remote v -u
envr install v 0.5
envr use v 0.5.1
envr shim sync
envr exec --lang v -- v version
envr which v
```

## GUI notes

- Env Center has a V runtime page with the standard remote/install/use flow.
- `runtime.v.path_proxy_enabled` controls whether `v` shim is envr-managed or PATH passthrough.
