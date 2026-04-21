# PureScript (managed)

envr installs a prebuilt **PureScript compiler** into:

`runtimes/purescript/versions/<label>` with a global `runtimes/purescript/current` symlink or Windows pointer file.

Upstream: `purescript/purescript` GitHub Releases.

## Version labels

- Upstream tag `v0.15.15` maps to `0.15.15`.
- Prerelease tags are filtered from installable rows by default.

## Remote index and caching

- Default GitHub Releases API URL: `https://api.github.com/repos/purescript/purescript/releases`
- Override with `ENVR_PURESCRIPT_GITHUB_RELEASES_URL` if needed.
- If API discovery fails, envr falls back to `https://github.com/purescript/purescript/releases.atom` and constructs synthetic download URLs.
- Index cache TTL: `ENVR_PURESCRIPT_RELEASES_CACHE_TTL_SECS` or legacy `ENVR_PURESCRIPT_INDEX_CACHE_TTL_SECS` (default 3600 seconds).
- Latest-per-major cache TTL: `ENVR_PURESCRIPT_REMOTE_CACHE_TTL_SECS` (default 86400 seconds).
- Optional GitHub token: `GITHUB_TOKEN`, `GH_TOKEN`, or `ENVR_GITHUB_TOKEN`.

## Commands

```bash
envr remote purescript
envr remote purescript -u
envr install purescript 0.15.15
envr use purescript 0.15.15
envr shim sync
purs --version
envr exec --lang purescript -- purs --version
```

## Settings

```toml
[runtime.purescript]
path_proxy_enabled = true
```

When `path_proxy_enabled = false`, the `purs` shim resolves to the next matching `purs` on system PATH outside envr shims.

## Environment

- Shims and `envr run` set `PURESCRIPT_HOME` to the resolved install root.
- Template key `ENVR_PURESCRIPT_VERSION` is set to the version directory label.

