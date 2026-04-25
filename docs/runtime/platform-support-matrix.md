# Runtime Platform Support Matrix

This document tracks current runtime support across Windows/Linux/macOS, plus architecture coverage and whether envr supports **managed install** on that host.

Legend:

- `yes`: supported
- `partial`: supported with constraints (host/arch/upstream-asset dependent)
- `no`: not supported

> Scope note: this is a **current-state** matrix (implementation + docs), not a future roadmap.

## Matrix

| Runtime | Windows | Linux | macOS | Arch coverage (current) | Managed install |
|---|---|---|---|---|---|
| `bun` | yes | yes | yes | x64 on all; arm64 depends on upstream assets/host mapping | yes (host-dependent) |
| `deno` | yes | yes | yes | x64 + arm64 on mainstream hosts (subject to release artifacts) | yes |
| `dotnet` | yes | yes | yes | host-dependent by SDK artifact/RID | yes (host-dependent) |
| `dart` | yes | yes | yes | host-dependent by official archive naming | yes |
| `flutter` | yes | yes | yes | host-dependent by official feed assets | yes |
| `julia` | yes | yes | yes | Windows zip; Linux/macOS tar.gz; x64 + arm64 where upstream provides | yes |
| `zig` | yes | yes | yes | x64 + arm64 where `index.json` has rows | yes |
| `v` | yes | yes | yes | Windows x64; Linux x64/arm64; macOS x64/arm64 (with fallbacks) | yes |
| `unison` | yes | yes | yes | Windows x64; Linux/macOS x64 + arm64 | yes |
| `babashka` | yes | yes | yes | Windows x64; Linux x64; macOS x64/arm64 | yes |
| `sbcl` | yes | yes | yes | per `roswell/sbcl_bin` availability by host | yes |
| `haxe` | yes | yes | yes | Windows x64, Linux x64, macOS archive | yes |
| `gleam` | yes | yes | yes | host/asset dependent; common x64 + macOS arm64 lines | yes |
| `crystal` | yes | yes | yes | host/asset dependent; Windows assets use "unsupported" filename convention upstream | yes |
| `perl` | yes | yes | yes | Windows x64 + Unix variants; unsupported hosts (for example some ARM combos) rejected | yes (host-dependent) |
| `scala` | yes | yes | yes | platform-specific or universal assets; filtered by host installability | yes |
| `kotlin` | yes | yes | yes | compiler zip cross-platform; requires managed Java current | yes |
| `clojure` | yes | yes | yes | JVM-hosted; requires managed Java current | yes |
| `groovy` | yes | yes | yes | JVM-hosted; requires managed Java current | yes |
| `terraform` | yes | yes | yes | host-dependent `terraform_<ver>_<platform>.zip` | yes |
| `elm` | yes | yes | yes | Windows/Linux/macOS assets (incl. macOS arm64 in plan) | yes |
| `janet` | yes | yes | yes | Windows x64, Linux x64, macOS x64/arm64; some hosts may have no asset | yes (host-dependent) |
| `c3` | yes | partial | partial | Windows x64, Linux x64, macOS arm64 (no macOS x64 artifact in current mapping) | yes (only on mapped hosts) |
| `lua` | yes | partial | partial | Windows x64, Linux x64 (glibc), macOS x64; arm64 not in current mapping | yes (only on mapped hosts) |
| `php` | yes | partial | partial | Windows artifact install path; Unix uses discovery/registration of system installs | Windows: yes; Unix: no (discovery/register only) |
| `r` (`rlang`) | yes | no | no | Windows managed installer path (`R-<ver>-win.exe`) | Windows only |
| `racket` | yes | no | no | current provider mapping is Windows x64 artifact only | Windows only |

## Evidence pointers

- Provider registration list: `crates/envr-runtime-registry/src/lib.rs`
- C3 host mapping: `crates/envr-runtime-c3/src/index.rs`, `docs/runtime/c3-integration-plan.md`
- Lua host mapping: `crates/envr-runtime-lua/src/index.rs`, `docs/runtime/lua.md`, `docs/runtime/lua-integration-plan.md`
- PHP Unix strategy and Windows-only install path: `crates/envr-runtime-php/src/lib.rs`, `crates/envr-runtime-php/src/unix.rs`, `docs/runtime/php.md`
- R (rlang) Windows-only managed install: `crates/envr-runtime-rlang/src/manager.rs`, `docs/runtime/r.md`, `docs/runtime/r-integration-plan.md`
- Racket current mapping: `crates/envr-runtime-racket/src/index.rs`, `docs/runtime/racket.md`, `docs/runtime/racket-integration-plan.md`

## Maintenance notes

- When adding/changing a runtime provider, update this matrix in the same PR.
- If docs and implementation disagree, prefer implementation and add a TODO note in the runtime doc.
