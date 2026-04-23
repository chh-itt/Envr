# Haxe integration plan

## Goal

Integrate **Haxe** (compiler + bundled `haxelib`) as an envr-managed runtime with:

- CLI: `remote/install/use/uninstall/exec/run/which`
- GUI: Env Center hub + PATH proxy toggle
- Shims: `haxe` + `haxelib`
- Version pinning: `.envr.toml` + `ENVR_HAXE_VERSION`
- Cache: releases index + latest-per-major-line cache

## Upstream source

Use **HaxeFoundation GitHub Releases**:

- Repo: `HaxeFoundation/haxe`
- API: `https://api.github.com/repos/HaxeFoundation/haxe/releases`
- HTML/Atom fallbacks: `https://github.com/HaxeFoundation/haxe/releases` / `.../releases.atom`

Rationale: official, consistent cross-platform binaries (`win64.zip`, `linux64.tar.gz`, `osx.tar.gz`).

## Version labels and grouping

- Tag style: `4.3.7`, `4.3.6`, etc (no leading `v`)
- Label: keep numeric tag unchanged (e.g. `4.3.7`)
- Major-line grouping: `major.minor` (e.g. `4.3`) for `remote --latest-per-major` and list grouping

## Installable artifacts (expected)

Pick by OS/arch tuple and prefer archive formats envr already handles:

- Windows x64: `haxe-<ver>-win64.zip`
- Windows x86 (optional): `haxe-<ver>-win.zip` (not required for envr’s primary target)
- Linux x64: `haxe-<ver>-linux64.tar.gz`
- macOS: `haxe-<ver>-osx.tar.gz` (may be universal in newer releases)

Ignore installers (`*.exe`, `*.pkg`) since archives are portable and match envr’s layout approach.

## Expected layout (post-extract)

Typically the archives unpack into a single root directory:

```
haxe-<ver>/
  haxe(.exe)
  haxelib(.exe)
  std/
  # may include neko/hashlink payload depending on release
```

envr will “promote” to:

`{runtime_root}/runtimes/haxe/versions/<label>/`

with binaries either at root or under `bin/` (we will probe both).

## Shim and env contract

- Runtime key: `haxe`
- Core commands: `haxe`, `haxelib`
- PATH entries: `[home/bin, home]`
- Runtime env vars:
  - `HAXE_HOME=<home>`
  - `HAXE_STD_PATH=<home>/std` (critical for portable installs)
- Template key: `ENVR_HAXE_VERSION`
- Settings: `[runtime.haxe].path_proxy_enabled`

## Caching

Same shape as C3/Babashka/SBCL:

- `cache/haxe/releases.json`
- `cache/haxe/latest_per_major.json`
- Refresh invalidates latest cache when releases cache is updated

## Friction log (fill during implementation)

- [ ] Confirm `HAXE_STD_PATH` is required for all platforms (portable archives) and decide whether to always set it.
- [ ] Asset naming stability across Haxe 4.x and Haxe 5 previews (ignore previews by default?).
- [ ] Validate whether `haxelib` default repository path needs extra env (`HAXEPATH` / `HAXELIB_PATH`) for fully portable behavior.
- [ ] Decide if we should also provide shims for bundled `neko`/`hl` when present, or keep scope to `haxe`/`haxelib` only.

## CLI smoke commands

```powershell
.\envr remote haxe
.\envr remote haxe -u
.\envr install haxe 4.3
.\envr use haxe 4.3
.\envr shim sync
haxe --version
haxelib version
.\envr which --lang haxe
.\envr exec --lang haxe -- haxe --version
```

## Playbook gaps to watch

- If we must set `HAXE_STD_PATH` for correctness, document “portable stdlib path env var” as a recurring runtime pattern in `docs/architecture/new-runtime-playbook.md`.

