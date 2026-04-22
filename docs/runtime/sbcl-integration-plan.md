# SBCL integration plan

## Goal

Integrate **SBCL** (Steel Bank Common Lisp) as an envr-managed runtime with:

- CLI: `remote/install/use/uninstall/exec/run/which`
- GUI: Env Center hub + PATH proxy toggle
- Shims: `sbcl` core shim (optionally `sbc` / helpers if shipped, but start with `sbcl` only)
- Cache: releases index + latest-per-major-line cache

## Upstream (artifact source decision)

SBCL upstream has multiple “release surfaces”:

1. **`sbcl/sbcl` GitHub releases**: mostly source artifacts (not sufficient for envr’s “no system toolchain” constraint).
2. **Official binaries**: published via SBCL download pages and SourceForge (Windows `.msi`, Linux `.tar.bz2`, macOS builds exist but may lag).
3. **`roswell/sbcl_bin` GitHub releases**: provides prebuilt SBCL binaries for many platforms/arches, used by Roswell.

### envr policy choice

For envr’s constraints (Windows/Linux/mac, no toolchain), we will consume **`roswell/sbcl_bin`** as the binary artifact source:

- Repo: `https://github.com/roswell/sbcl_bin`
- Releases: `https://api.github.com/repos/roswell/sbcl_bin/releases`

This is a pragmatic choice: it gives consistent cross-platform binaries when “official” macOS binaries lag.

## Version labels and grouping

- Tag style in `sbcl_bin`: typically `2.6.1`, `2.6.0`, etc.
- Label: keep semver-like numeric label (e.g. `2.6.1`).
- Major-line grouping: `major.minor` (e.g. `2.6`) to match other runtimes like Julia/Janet/Babashka.

## Installable artifacts (expected)

`sbcl_bin` ships many assets per release. We will pick by OS/arch tuple and prefer archives that are easiest to promote:

- Windows x64: `sbcl-<ver>-x86-64-windows-*.zip` (or `.7z` if zip absent)
- Linux x64: `sbcl-<ver>-x86-64-linux-*.tar.*`
- macOS:
  - arm64: `sbcl-<ver>-arm64-darwin-*.tar.*`
  - x64: `sbcl-<ver>-x86-64-darwin-*.tar.*`

During implementation we will confirm the exact filename patterns for the current latest release and codify them as deterministic match rules.

## Install strategy

- Download artifact to `cache/sbcl/`
- Extract using `envr_download::extract::extract_archive`
  - If upstream uses `.7z` for Windows, we may need a new extraction path or choose an alternate asset format.
- Promote extracted layout into `runtimes/sbcl/versions/<label>/`
  - Validate by locating `sbcl(.exe)` under `home/`, `home/bin/`, or known SBCL subdirs.
  - Handle “one nested root directory” archives with the standard promote/discovery pattern.

## Shim and env contract

- Runtime key: `sbcl`
- Core command: `sbcl`
- PATH entries: `[home/bin, home]`
- Runtime env var: `SBCL_HOME` (TBD: whether SBCL expects `SBCL_HOME` to point at `lib/sbcl/` vs install root; confirm from layout)
- Settings: `[runtime.sbcl].path_proxy_enabled`

## Caching

Use the same disk cache shape as C3/Babashka:

- `cache/sbcl/releases.json`
- `cache/sbcl/latest_per_major.json`
- Invalidate latest cache when releases cache is refreshed

## Friction log (fill during implementation)

- [x] SBCL binary upstream choice is not “official” GitHub repo; documented and implemented via `roswell/sbcl_bin` (GitHub Releases API + HTML/Atom fallback).
- [x] Asset naming heterogeneity handled via tuple match rules; prefer `.tar.bz2` everywhere to avoid MSI and keep one extractor path.
- [ ] `SBCL_HOME` semantics depend on layout (root vs `lib/sbcl/`); current implementation points to runtime home root (same folder containing `bin/sbcl`). Validate with real runs.
- [x] Added `.tar.bz2` extraction support to `envr-download` (TarBz2 + `bzip2`).

### CLI/GUI notes

- CLI: `envr remote/install/use/exec/run` wired via runtime key `sbcl` and template key `ENVR_SBCL_VERSION`.
- GUI: Env Center shows SBCL and includes PATH proxy toggle (`[runtime.sbcl].path_proxy_enabled`).

## CLI / GUI smoke commands

```powershell
.\envr remote sbcl
.\envr remote sbcl -u
.\envr install sbcl 2.6
.\envr use sbcl 2.6
.\envr shim sync
sbcl --version
.\envr which --lang sbcl
.\envr exec --lang sbcl -- sbcl --version
```

## Playbook gaps to watch

- If we need to formalize “non-official binary upstream” as a supported pattern, add guidance to `docs/architecture/new-runtime-playbook.md`.

