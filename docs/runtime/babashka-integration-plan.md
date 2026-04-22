# Babashka integration plan

## Goal

Integrate **Babashka** (`bb`) from `babashka/babashka` GitHub releases with full envr surface:

- CLI: `remote/install/use/uninstall/exec/run/which`
- GUI: Env Center support + path proxy toggle
- Shims: `bb` core shim
- Cache: releases + latest-per-major line cache

## Upstream

- Repo: `https://github.com/babashka/babashka`
- Releases API: `https://api.github.com/repos/babashka/babashka/releases`
- Atom fallback: `https://github.com/babashka/babashka/releases.atom`

## Version and asset policy

- Tag style: `v1.12.218` -> label `1.12.218`
- Only non-draft releases are considered installable.
- Ignore sidecar assets like `.sha256`, plus non-runtime artifacts (`reflection.json`, `standalone.jar`).

### Target assets by platform

- Windows x64: `babashka-<version>-windows-amd64.zip`
- Linux x64: prefer `babashka-<version>-linux-amd64-static.tar.gz`, fallback `babashka-<version>-linux-amd64.tar.gz`
- macOS x64: `babashka-<version>-macos-amd64.tar.gz`
- macOS arm64: `babashka-<version>-macos-aarch64.tar.gz`
- Linux arm64 (optional support): `babashka-<version>-linux-aarch64-static.tar.gz`

## Install layout strategy

- Download to `cache/babashka/`
- Extract with `envr_download::extract::extract_archive`
- Promote extracted root to `runtimes/babashka/versions/<label>/`
- Validate install by finding executable:
  - `bb.exe` or `bb` in root, fallback `bin/`

## Shim and env contract

- Runtime key: `babashka`
- Core command: `bb`
- PATH entries: `[home/bin, home]`
- Runtime env var: `BABASHKA_HOME`
- Path proxy setting: `[runtime.babashka].path_proxy_enabled`

## Cache behavior

- `cache/babashka/releases.json`
- `cache/babashka/latest_per_major.json`
- Clear latest-per-major cache when releases cache is refreshed.
- Validate latest cache against fresh rows before reuse.

## Friction log (implementation notes)

- [x] Multi-asset Linux policy (`-static` vs non-static) implemented with deterministic priority (`-static` preferred, normal tarball fallback).
- [x] Runtime key `babashka` vs command `bb` mapping wired across provider/shim/CLI/GUI; playbook updated with an explicit checklist item for key/stem divergence.
- [x] GUI path-proxy wiring required dual touch points (`EnvCenterMsg` + panel section + app update handler); all three are now connected.
- [x] Non-runtime release assets are filtered by explicit filename suffix rules; `.sha256`/`reflection.json`/`standalone.jar` are not install candidates.

## CLI / GUI notes

- No regression observed in compile-time checks (`cargo check --workspace`) and focused runtime/shim tests.
- GUI currently follows existing path-proxy UX: disabling proxy blocks managed Use / Install & Use for Babashka.

## CLI / GUI smoke commands

```powershell
.\envr remote babashka
.\envr remote babashka -u
.\envr install babashka 1.12
.\envr use babashka 1.12
.\envr shim sync
bb --version
.\envr which --lang babashka
.\envr exec --lang babashka -- bb --version
```

