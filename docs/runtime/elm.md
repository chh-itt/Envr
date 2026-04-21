# Elm (managed)

envr installs prebuilt **Elm compiler** binaries into:

`runtimes/elm/versions/<label>` with a global `runtimes/elm/current` symlink or Windows pointer file.

Upstream: `elm/compiler` GitHub Releases.

## Version labels

- Upstream tag `0.19.1` / `v0.19.1` maps to `0.19.1`.
- `envr use elm 0.19` resolves to latest matching `0.19.x`.

## Remote index and caching

- Default API URL: `https://api.github.com/repos/elm/compiler/releases`
- Override: `ENVR_ELM_GITHUB_RELEASES_URL`
- On API failures (including 403), envr falls back to releases HTML pagination and then `releases.atom`.
- Index TTL: `ENVR_ELM_RELEASES_CACHE_TTL_SECS` or `ENVR_ELM_INDEX_CACHE_TTL_SECS` (default 3600)
- Latest-per-major TTL: `ENVR_ELM_REMOTE_CACHE_TTL_SECS` (default 86400)
- Optional token: `GITHUB_TOKEN` / `GH_TOKEN` / `ENVR_GITHUB_TOKEN`

## Commands

```bash
envr remote elm
envr remote elm -u
envr install elm 0.19.1
envr use elm 0.19
envr shim sync
elm --version
envr exec --lang elm -- elm --version
```

## Settings

```toml
[runtime.elm]
path_proxy_enabled = true
```

When `path_proxy_enabled = false`, the `elm` shim resolves to the next matching `elm` on system PATH outside envr shims.

## Environment

- `ELM_HOME` points to resolved runtime home during shim/run/exec.
- `ENVR_ELM_VERSION` is available as template key.

