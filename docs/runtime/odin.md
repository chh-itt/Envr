# Odin (managed)

envr installs a prebuilt **Odin toolchain** into:

`runtimes/odin/versions/<label>` with a global `runtimes/odin/current` symlink or Windows pointer file.

Upstream: `odin-lang/Odin` GitHub Releases.

## Version labels

Odin upstream tags are monthly dev tags like `dev-2026-04` (and sometimes `dev-2025-12a`).

envr maps tags into dotted numeric labels so remote summaries can group lines:

- `dev-YYYY-MM` → `YYYY.MM`
- `dev-YYYY-MM<suffix>` (`a`, `b`, …) → `YYYY.MM.<n>` where `a=1`, `b=2`, …

Examples:

- `dev-2026-04` → `2026.04`
- `dev-2025-12a` → `2025.12.1`

## Remote index and caching

- Default GitHub Releases API URL: `https://api.github.com/repos/odin-lang/Odin/releases`
- Override with `ENVR_ODIN_GITHUB_RELEASES_URL` if you mirror/proxy the API.
- If API candidates fail (e.g. 403/rate-limit/proxy blocks), envr falls back to `https://github.com/odin-lang/Odin/releases.atom` and constructs synthetic download URLs.
- Index cache TTL: `ENVR_ODIN_RELEASES_CACHE_TTL_SECS` or legacy `ENVR_ODIN_INDEX_CACHE_TTL_SECS` (default 3600 seconds).
- Latest-per-major disk cache TTL: `ENVR_ODIN_REMOTE_CACHE_TTL_SECS` (default 86400 seconds).
- Optional GitHub token: `GITHUB_TOKEN`, `GH_TOKEN`, or `ENVR_GITHUB_TOKEN`.

## Commands

```bash
envr remote odin
envr remote odin -u
envr install odin 2026.04
envr use odin 2026.04
envr shim sync
odin version
envr exec --lang odin -- odin version
```

## Settings

```toml
[runtime.odin]
path_proxy_enabled = true
```

When `path_proxy_enabled = false`, the `odin` shim resolves to the next matching `odin` on **system PATH** outside envr shims.

## Environment

- Shims and `envr run` set **`ODIN_ROOT`** to the resolved install root when Odin is on the stack.
- Template key **`ENVR_ODIN_VERSION`** is set to the version directory label.

On Windows, `odin version` may print an executable path with a `\\?\` prefix. That is a normal canonical path form and does not indicate an install problem.

