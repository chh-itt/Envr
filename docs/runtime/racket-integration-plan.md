# Racket integration plan

## Goal

Add **`RuntimeKind::Racket`** (`key = "racket"`) as a first-class managed runtime (CLI / GUI / shims / pins / `exec` / `run`) using official Minimal Racket portable archives.

Install layout:

`runtimes/racket/versions/<label>` with global `runtimes/racket/current` symlink or Windows pointer file.

## Upstream and artifacts

Racket distribution source:

- versions index: `https://download.racket-lang.org/all-versions.html`
- release path template: `https://download.racket-lang.org/releases/{version}/installers/`

Windows x64 installable asset (MVP target):

- `racket-minimal-{version}-x86_64-win32-cs.tgz`

## Version labels and spec grammar

- labels come from all-versions table (`9.1`, `8.18`, `8.11.1`, ...)
- supports exact label install and shorthand line resolution (`8.16` -> latest `8.16.x`)

## Index/caching

- row: `RacketInstallableRow { version, url }`
- cache: `{runtime_root}/cache/racket/`
- index TTL: `ENVR_RACKET_INDEX_CACHE_TTL_SECS`
- latest-per-major TTL: `ENVR_RACKET_REMOTE_CACHE_TTL_SECS`
- index override: `ENVR_RACKET_ALL_VERSIONS_URL`

## Install layout and validation

- Download Windows archive (`.tgz`) and extract atomically into version directory.
- Valid install contains `racket.exe` (root or `bin/`).

## Shims/env/settings

- shim commands: `racket`, `raco`
- home env: `RACKET_HOME`
- template key: `ENVR_RACKET_VERSION`
- settings: `[runtime.racket].path_proxy_enabled`

## CLI / GUI smoke

```bash
envr remote racket
envr remote racket -u
envr install racket 9.1
envr use racket 9
envr shim sync
racket --version
raco --version
envr exec --lang racket -- racket --version
```

## Architecture / abstraction friction log

1. Racket release discovery is HTML-first (non-GitHub), so provider needs a custom parser path.
2. Upstream "installers" page includes both installer EXE and portable archives; GUI/non-admin flow is more reliable with archive extraction than EXE silent install.
3. Runtime abstraction currently assumes mostly single entrypoint; Racket needs at least `racket` + `raco` shims.
4. Redirect-heavy upstream requires stronger download guardrails (identity encoding, mirror fallback, gzip magic check) than typical GitHub release providers.

## CLI / GUI friction log

- CLI:
  - Verified: `racket --version` works as expected.
  - Note: `raco --version` may print command/help context depending on subcommand and should not be treated as install failure.
- GUI:
  - Verified: install/use and PATH proxy toggle now pass.

