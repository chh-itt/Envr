# Gleam runtime

`envr` supports managed **Gleam** as `gleam`.

## Version source

- GitHub releases: `https://api.github.com/repos/gleam-lang/gleam/releases`
- Fallbacks: releases HTML pagination + `releases.atom`

## Version labels

- Upstream tag `v1.11.2` maps to label `1.11.2`.
- `envr use gleam 1` resolves to latest matching `1.x`.
- `envr use gleam 1.11` resolves to latest matching `1.11.x`.

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

If the GUI suggests a version (e.g. `1.14.0`) but install reports **unknown gleam version spec**, the remote index cache was likely out of sync with the “latest per line” cache. Run `.\envr remote gleam -u` and retry; newer envr builds invalidate the line cache whenever the installable index is refreshed.

