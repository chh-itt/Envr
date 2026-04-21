# Racket runtime

`envr` supports managed **Racket (Minimal Racket)** as `racket`.

## Version source

- Index page: `https://download.racket-lang.org/all-versions.html`
- Archive template (Windows x64):  
  `https://download.racket-lang.org/releases/{version}/installers/racket-minimal-{version}-x86_64-win32-cs.tgz`

## Version labels

- Labels are numeric release strings from the index page (for example `9.1`, `8.18`, `8.11.1`).
- `envr use racket 9` resolves to the latest `9.x` line in the cached remote list.
- `envr use racket 8.16` resolves to the latest `8.16.x`.

## Commands

```powershell
.\envr remote racket
.\envr remote racket -u
.\envr install racket 9.1
.\envr use racket 9
.\envr shim sync
racket --version
raco --version
.\envr exec --lang racket -- racket --version
```

Note: `raco --version` output format can vary by release/command context; treat `racket --version` as the primary runtime health signal.

## Environment and shims

- Runtime home env: `RACKET_HOME`
- Template key in run/exec env: `ENVR_RACKET_VERSION`
- Core shims: `racket`, `raco`

## Settings

`settings.toml`:

```toml
[runtime.racket]
path_proxy_enabled = true
```

When `path_proxy_enabled = false`, `racket`/`raco` shims bypass managed runtime resolution and defer to system PATH.

