# Gleam runtime

`envr` supports managed **Gleam** as `gleam`.

## Version source

- GitHub releases: `https://api.github.com/repos/gleam-lang/gleam/releases`
- Fallbacks: releases HTML pagination + `releases.atom`

## Version labels

- Upstream tag `v1.11.2` maps to label `1.11.2`.
- `envr use gleam 1` resolves to latest matching `1.x`.
- `envr use gleam 1.11` resolves to latest matching `1.11.x`.

## Which versions show up as “remote”

The installable index only lists releases that ship a **matching prebuilt archive for your OS/arch** (e.g. `gleam-v…-x86_64-pc-windows-msvc.zip` on Windows x64). That is **not** a Windows vs “1.15 and below” limitation; older lines appear as long as upstream published that artifact. `envr remote gleam -u` forces a **network refresh** of that index (bypassing the on-disk TTL cache).

## Commands

```powershell
.\envr remote gleam
.\envr remote gleam -u
.\envr install gleam 1
.\envr use gleam 1
.\envr shim sync
gleam --version
.\envr exec --lang gleam -- gleam --version
```

## Environment and shims

- Runtime home env: `GLEAM_HOME`
- Template key in run/exec env: `ENVR_GLEAM_VERSION`
- Core shim: `gleam`

## Erlang/OTP prerequisite

Gleam requires Erlang/OTP (`erl`) to be available on PATH for normal BEAM workflows.  
If `erl` is missing or not runnable, envr will report a clear prerequisite error during install.

## Settings

`settings.toml`:

```toml
[runtime.gleam]
path_proxy_enabled = true
```

When `path_proxy_enabled = false`, `gleam` shim bypasses managed runtime resolution and defers to system PATH.

## Troubleshooting

If the GUI suggests a version (e.g. `1.14.0`) but install reports **unknown gleam version spec**, the remote index cache was likely out of sync with the “latest per line” cache. Run `.\envr remote gleam -u` and retry; current builds also invalidate the line cache whenever the installable index is saved, and `-u` forces re-fetch of the installable list (not only the line summary).

