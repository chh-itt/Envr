# C3 integration plan

## Goal

Integrate **C3** compiler (`c3c`) from upstream GitHub releases (`c3lang/c3c`) with:

- CLI: `remote/install/use/uninstall/exec/run/which`
- GUI: Env Center unified list + PATH proxy toggle
- Shims: `c3c` core shim + PATH proxy bypass support
- Installers: `.zip` (Windows/macOS) and `.tar.gz` (Linux) archives

## Upstream

- Repo: `https://github.com/c3lang/c3c`
- Releases API: `https://api.github.com/repos/c3lang/c3c/releases`
- Atom fallback: `https://github.com/c3lang/c3c/releases.atom`

### Release policy note

Upstream’s **latest** is often marked **prerelease** (e.g. `v0.7.11` as of 2026-03). For envr:

- Accept **non-draft** releases even if `prerelease = true` (otherwise remote list becomes empty).
- Version label: strip leading `v` from `tag_name` when it matches semver (e.g. `v0.7.11` → `0.7.11`).

## Installable artifacts (by host)

Asset names are stable across releases (no version embedded); URLs are per-tag:
`https://github.com/c3lang/c3c/releases/download/<tag>/<asset>`

| Host | Asset | Format |
|------|-------|--------|
| Windows x64 | `c3-windows.zip` | zip |
| Linux x64 | `c3-linux.tar.gz` | tar.gz |
| macOS aarch64 | `c3-macos.zip` | zip |

If the host lacks an official artifact (e.g. macOS x64), envr returns an empty installable index for that host.

## Install strategy

- Download artifact to `cache/c3/`
- Extract using `envr_download::extract::extract_archive`
- Promote extracted layout into `runtimes/c3/versions/<label>/`
  - Validate by locating `c3c(.exe)` in `home/` or `home/bin/`
  - Handle archive root variance (flat root vs single nested directory) using the Odin/Gleam promote pattern.

## Layout & shims

- Runtime home: `runtimes/c3/versions/<label>/`
- Expected executable: `c3c` / `c3c.exe` (primary), plus any sibling tools if shipped.
- PATH entries: `[home/bin, home]` (similar to Gleam).
- Runtime home env: `C3_HOME`
- Core shims:
  - `c3c`

## Caching

Use the same disk cache shape as Gleam/Janet:

- `cache/c3/releases.json`: installable rows (TTL, supports `remote -u` force refresh)
- `cache/c3/latest_per_major.json`: latest per `major.minor` line; **invalidate when releases cache is refreshed** and validate cached labels match current rows before reuse.

## Friction log (fill during implementation)

- [ ] Upstream prereleases are the common “latest” signal → we must include prereleases.
- [ ] Asset filenames are versionless → fallback URL construction must be tag-aware.
- [ ] Archive root variance (`c3c` at root vs `bin/`) → promote needs discovery.
- [ ] Windows `.cmd` shims and PATH proxy bypass behavior.

## CLI / GUI smoke (operator)

```powershell
.\envr remote c3
.\envr remote c3 -u
.\envr install c3 0.7
.\envr use c3 0.7
.\envr shim sync
c3c --version
.\envr exec --lang c3 -- c3c --version
```

## Playbook gaps to watch

- Optional prerelease acceptance rules (when upstream marks latest as prerelease) should be explicit in `new-runtime-playbook.md` if it isn’t already.

# C3 integration plan

## Goal

Integrate **C3** compiler (`c3c`) from upstream GitHub releases (`c3lang/c3c`) with:

- CLI: `remote/install/use/uninstall/exec/run/which`
- GUI: Env Center unified list + PATH proxy toggle
- Shims: `c3c` core shim + PATH proxy bypass support
- Installers: `.zip` (Windows/macOS) and `.tar.gz` (Linux) archives

## Upstream

- Repo: `https://github.com/c3lang/c3c`
- Releases API: `https://api.github.com/repos/c3lang/c3c/releases`
- Atom fallback: `https://github.com/c3lang/c3c/releases.atom`

### Release policy note

Upstream’s **latest** is often marked **prerelease** (e.g. `v0.7.11` as of 2026-03). For envr:

- Accept **non-draft** releases even if `prerelease = true` (otherwise remote list becomes empty).
- Version label: strip leading `v` from `tag_name` when it matches semver \(e.g. `v0.7.11` → `0.7.11`\).

## Installable artifacts (by host)

Asset names are stable across releases (no version embedded); URLs are per-tag:
`https://github.com/c3lang/c3c/releases/download/<tag>/<asset>`

| Host | Asset | Format |
|------|-------|--------|
| Windows x64 | `c3-windows.zip` | zip |
| Linux x64 | `c3-linux.tar.gz` | tar.gz |
| macOS aarch64 | `c3-macos.zip` | zip |

If the host lacks an official artifact (e.g. macOS x64), envr returns an empty installable index for that host.

## Install strategy

- Download artifact to `cache/c3/`
- Extract using `envr_download::extract::extract_archive`
- Promote extracted layout into `runtimes/c3/versions/<label>/`
  - Validate by locating `c3c(.exe)` in `home/` or `home/bin/`
  - Handle archive root variance (flat root vs single nested directory) using the Odin/Gleam promote pattern.

## Layout & shims

- Runtime home: `runtimes/c3/versions/<label>/`
- Expected executable: `c3c` / `c3c.exe` (primary), plus any sibling tools if shipped.
- PATH entries: `[home/bin, home]` (similar to Gleam).
- Runtime home env: `C3_HOME`
- Core shims:
  - `c3c`

## Caching

Use the same disk cache shape as Gleam/Janet:

- `cache/c3/releases.json`: installable rows (TTL, supports `remote -u` force refresh)
- `cache/c3/latest_per_major.json`: latest per `major.minor` line; **invalidate when releases cache is refreshed** and validate cached labels match current rows before reuse.

## Friction log (fill during implementation)

- [ ] Upstream prereleases are the common “latest” signal → we must include prereleases.
- [ ] Asset filenames are versionless → fallback URL construction must be tag-aware.
- [ ] Archive root variance (`c3c` at root vs `bin/`) → promote needs discovery.
- [ ] Windows `.cmd` shims and PATH proxy bypass behavior.

## CLI / GUI smoke (operator)

```powershell
.\envr remote c3
.\envr remote c3 -u
.\envr install c3 0.7
.\envr use c3 0.7
.\envr shim sync
c3c --version
.\envr exec --lang c3 -- c3c --version
```

## Playbook gaps to watch

- Optional prerelease acceptance rules (when upstream marks latest as prerelease) should be explicit in `new-runtime-playbook.md` if it isn’t already.

