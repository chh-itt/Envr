# Luau Integration Plan

## Goal

Add envr-managed Luau runtime (`luau`) with cross-platform prebuilt binary distribution:

- Windows: `luau-windows.zip`
- Linux: `luau-ubuntu.zip`
- macOS: `luau-macos.zip`

Primary objective is fast end-to-end validation against `docs/architecture/new-runtime-playbook.md`.

## Scope

In scope:

- Runtime kind/descriptor registration (`luau`)
- Provider crate (`envr-runtime-luau`) with GitHub releases index + install/uninstall/current
- Runtime service and runtime registry wiring
- CLI resolver/run/exec/env template key integration
- Shim core command support for `luau` and `luau-analyze`
- Minimal GUI parity via descriptor-driven runtime surfaces

Out of scope (phase 2 if needed):

- Path proxy toggle (initially disabled)
- Complex mirror/index fallback beyond GitHub API pagination
- Runtime-specific settings block

## Runtime decisions

- Runtime key: `luau`
- Labels: `Luau` / `Luau`
- Core commands:
  - `luau`
  - `luau-analyze`
- Version source:
  - GitHub releases: `https://api.github.com/repos/luau-lang/luau/releases`
- Version grammar:
  - supports exact (`0.715`)
  - supports major prefix (`0`)
  - supports major.minor prefix (`0.71`)
  - supports `latest`
- Install layout:
  - `runtimes/luau/versions/<label>/...`
- Current pointer:
  - symlink + pointer-file fallback via shared helper
- Runtime home env:
  - `LUAU_HOME=<runtime_home>`

## Implementation checklist

- [ ] Domain/descriptor registration (`RuntimeKind::Luau`)
- [ ] New crate `crates/envr-runtime-luau`
- [ ] Runtime registry + core feature wiring
- [ ] Runtime service default provider registration
- [ ] Resolver/run/exec/missing-pin/template-key integration
- [ ] Shim command integration (`luau`, `luau-analyze`)
- [ ] CLI smoke path sanity
- [ ] GUI runtime visibility sanity
- [ ] Runtime docs (`docs/runtime/luau.md`) follow-up

## Acceptance criteria

- `envr remote luau` returns installable versions
- `envr install luau latest` installs and sets current
- `envr current luau` returns selected version
- `envr exec --lang luau -- luau --help` works
- `envr which luau` resolves to envr-managed shim when available
