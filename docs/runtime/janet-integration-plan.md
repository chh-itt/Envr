# Janet integration plan

## Goal

Managed **Janet** (`janet`, optional `jpm`) from official GitHub releases (`janet-lang/janet`), with CLI/GUI/shims/`exec`/`run`, PATH proxy, remote index + install, and docs aligned with the new-runtime playbook.

## Upstream

- Repo: `https://github.com/janet-lang/janet`
- Releases API: `https://api.github.com/repos/janet-lang/janet/releases`
- Fallbacks: GitHub releases HTML pagination + `releases.atom` (same pattern as Gleam/Odin).

## Version labels

- Tags `v1.41.2` → canonical label `1.41.2`.
- `envr use janet 1` / `1.41` resolve to latest matching patch line (numeric semver).

## Installable artifacts (by host)

| Host | Asset pattern | Notes |
|------|----------------|-------|
| Windows x64 | `janet-{version}-windows-x64-installer.msi` | **No** portable `.zip` in upstream; version in MSI filename has **no** `v` prefix. |
| Linux x64 | `janet-v{version}-linux-x64.tar.gz` | `v` prefix in tarball name. |
| macOS aarch64 | `janet-v{version}-macos-aarch64.tar.gz` | |
| macOS x64 | `janet-v{version}-macos-x64.tar.gz` | |

Linux aarch64 / Windows aarch64: no first-class assets observed on recent releases → **empty installable index** on those hosts until upstream ships artifacts (document in user doc).

## Windows install strategy

- Download MSI to persistent cache under `cache/janet/`.
- **Administrative unpack** via `msiexec /a <msi> /qn TARGETDIR=<staging>` (no full product install UI; avoids GUI needing elevation like some `.exe` installers).
- Locate `janet.exe` under `TARGETDIR` (WiX layout varies), then copy `janet.exe`, `jpm.exe` (if present), and `*.dll` from that directory into `versions/<label>/bin/`.
- Validate with `janet --version`.

## Layout & shims

- Runtime home: `runtimes/janet/versions/<label>/` with `bin/janet` (or `janet.exe`).
- `JANET_HOME` injected for managed resolution.
- Core shims: `janet`, `jpm` (when present in MSI/tarball).

## Caching

- `cache/janet/releases.json` — full installable rows (TTL, `force_index_refresh` from `remote -u`).
- `cache/janet/latest_per_major.json` — line summary; **invalidate when releases cache is saved**; validate multiset vs current rows before reuse (Gleam lesson).

## Friction log (fill during implementation)

- [x] Asset naming asymmetry: MSI uses `janet-1.x.y-...` while tarballs use `janet-v1.x.y-...` (handled in index filename builder + GitHub asset match).
- [x] Windows MSI layout variance after `msiexec /a` (mitigation: recursive search for `janet.exe`, then copy sibling `.exe`/`.dll` from that directory).
- [x] `jpm` optional across versions — `jpm` shim errors at resolve time if the MSI/tarball did not ship `jpm` under the same directory as `janet` (expected).

## CLI / GUI notes (operator)

- Path proxy toggle: **Settings → Janet** (`[runtime.janet] path_proxy_enabled`) and Env Center fold match Gleam-style behavior.
- Resolver `RUNTIME_PLAN_ORDER` / `resolve_run_lang_home` were missing several runtimes already in `RUN_STACK_LANG_ORDER`; Janet is added, and `resolve_run_lang_home` now includes **odin / purescript / elm / gleam / racket / lua** so `envr run` / missing-pin resolution matches the CLI run stack for those keys.

## CLI / GUI smoke (operator)

```powershell
.\envr remote janet
.\envr remote janet -u
.\envr install janet 1
.\envr use janet 1
.\envr shim sync
janet --version
.\envr exec --lang janet -- janet --version
```

## Playbook

- If MSI administrative extract proves flaky on some machines, capture mitigations under Windows installer / archive notes.
