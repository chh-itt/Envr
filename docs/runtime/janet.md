# Janet runtime

`envr` supports managed **Janet** as `janet` (and `jpm` when shipped in the same release artifact).

## Version source

- GitHub releases: `https://api.github.com/repos/janet-lang/janet/releases`
- Fallbacks: releases HTML + `releases.atom`

## Version labels

- Tag `v1.41.2` maps to `1.41.2`.
- `envr use janet 1` resolves to the latest `1.x.y` present in the **installable** index for your OS.

## Commands

```powershell
.\envr remote janet
.\envr remote janet -u
.\envr install janet 1
.\envr use janet 1
.\envr shim sync
janet --version
.\envr exec --lang janet -- janet --version
```

## Environment and shims

- Runtime home env: `JANET_HOME`
- Template key: `ENVR_JANET_VERSION`
- Core shims: `janet`, `jpm`
  - `jpm` is **optional** upstream; if the installed artifact does not include `jpm`, invoking `jpm` will error with “missing under managed layout”.

## Windows notes

Upstream ships a **`.msi`** (not a `.zip`) for Windows x64. envr unpacks it with `msiexec /a` into a staging directory, then copies `janet.exe` / `jpm.exe` / DLLs into the managed layout. If extraction fails, retry from an elevated shell only if your environment blocks non-interactive MSI admin installs.

## Unsupported host matrices

Recent GitHub assets target **Windows x64**, **Linux x64**, and **macOS x64 / aarch64**. Other targets may have **no installable row** in the remote index.

## Settings

```toml
[runtime.janet]
path_proxy_enabled = true
```

When `path_proxy_enabled = false`, `janet` / `jpm` shims defer to the next matching binary on PATH outside envr shims.
