# Julia runtime integration plan (envr)

Execution plan for adding **Julia** as a first-class runtime, aligned with
`docs/architecture/new-runtime-playbook.md` and existing patterns (Zig, Go unified major lines).

## 1) Why Julia

- **Single official version index**: `https://julialang-s3.julialang.org/bin/versions.json` is a large JSON object keyed by version (`1.10.5`, …), each with `files[]` listing per-OS/arch artifacts (URLs + `sha256`).
- **Portable archives** (no silent `.exe` installer path for MVP): prefer **`tar.gz` on Linux/macOS** and **`zip` on Windows** with `kind: "archive"` (not the `.exe` installer rows).
- **Versioning**: stable semver `MAJOR.MINOR.PATCH` — **major line** for unified UI is **`1.10`** style (two-part), same contract as Go/Python/Zig via `version_line_key_for_kind`.
- **Narrow shim surface (MVP)**: **`julia`** only (no first-class `juliaup` shim).

## 2) Goal and scope

### Goal

- Add `julia` as `RuntimeKind` with provider crate `envr-runtime-julia`.
- Install / list / current / set current / uninstall / resolve / remote listing with disk cache + `list_remote_latest_per_major` for GUI rows.
- Shim / `exec` / `run` PATH + **`JULIA_HOME`** for the selected prefix.
- `.envr.toml`: `[runtimes.julia] version = "1.10.5"`.
- CLI + GUI parity with other unified-list runtimes; PATH proxy toggle in settings (like Zig).

### In scope (MVP)

- Parse `versions.json`, map host `(OS, ARCH)` → `(os, arch)` fields in JSON (`linux`/`mac`/`winnt`, `x86_64`/`aarch64`/…).
- Download selected archive, extract, hoist single root dir to `runtimes/julia/versions/<ver>/`.
- Validate `bin/julia` or `bin/julia.exe`.
- Cache `versions.json` under `cache/julia/versions.json` with TTL.

### Out of scope

- **`juliaup`** as a managed installer inside envr.
- **Source builds** from GitHub.
- **32-bit Windows** unless we add mapping when demand exists.
- **DMG** on macOS (prefer **`tar.gz`** rows only).

## 3) Metadata and artifact selection

### Canonical URL

- `DEFAULT_JULIA_VERSIONS_JSON_URL`: `https://julialang-s3.julialang.org/bin/versions.json`

### Host mapping (examples)

| Host | `os` | `arch` | Preferred artifact |
|------|------|--------|----------------------|
| Linux x86_64 | `linux` | `x86_64` | `tar.gz`, `kind: archive` |
| Linux aarch64 | `linux` | `aarch64` | `tar.gz` |
| macOS x86_64 | `mac` | `x86_64` | `tar.gz` |
| macOS aarch64 | `mac` | `aarch64` | `tar.gz` |
| Windows x86_64 | `winnt` | `x86_64` | **`zip`** (portable) |

### Stable keys

- Include only top-level keys that look like **semver `x.y.z`** (three numeric segments) and, when present, **`stable: true`** on the version object.

## 4) Install layout

- `runtimes/julia/versions/<version>/` — contents of extracted archive root (typically `julia-x.y.z/` hoisted), with **`bin/julia`** / **`bin/julia.exe`**.
- `runtimes/julia/current` — symlink or Windows pointer file (same strategy as Zig).

## 5) Environment

- `JULIA_HOME` = absolute runtime home (strip `\\?\` prefix on Windows where needed), wired in `runtime_home_env_for_key(..., "julia")`.
- PATH prepends `versions/<ver>/bin` via `runtime_bin_dirs_for_key`.

## 6) Acceptance / test notes

- `cargo test -p envr-runtime-julia` with a **small fixture** snippet of `versions.json`.
- Manual: `envr remote julia`, `envr install julia 1.10.5`, `envr use julia 1.10.5`, `envr exec --lang julia -- julia --version`.

## 7) Architecture / friction log (fill while implementing)

- **`versions.json` size (~1MB+)**: cache + TTL required; full parse via `serde_json::Value` is acceptable; avoid loading in hot shim paths (provider uses blocking client only for install/remote refresh).
- **Many file rows per version**: must pick **one** archive per host (prefer zip on Windows, tar.gz on Unix); do not surface `.exe` installers in MVP.
- **GUI**: another PATH-proxy-only settings block — duplicates Zig/Dotnet pattern (`JuliaRuntimeSettings`).
- **Playbook**: **JULIA_HOME** is now listed in `docs/architecture/new-runtime-playbook.md` §D alongside `JAVA_HOME` / `GOROOT` / `DOTNET_ROOT`.

## 8) Development log

- [x] Plan written (`docs/runtime/julia-integration-plan.md`).
- [x] Provider + index + manager + tests (`crates/envr-runtime-julia`).
- [x] Domain / service / shim / resolver / CLI / GUI / settings / user doc (`docs/runtime/julia.md`).
- [x] Playbook §D: `JULIA_HOME` example added.

### CLI / GUI follow-ups

- **PATH proxy off + `julia` shim on Windows**: if bypass PATH search treated envr `shims\julia.cmd` as the “host” binary (e.g. custom `ENVR_RUNTIME_ROOT` without `"envr"` in the parent path, or 8.3 PATH segments), `envr-shim` → `cmd /c` batch → shim again caused a hang and Ctrl+C spammed “Terminate batch job (Y/N)?”. **Fixed** in `envr-shim-core`: skip `ctx.runtime_root.join("shims")` using logical + `canonicalize` match; playbook §8.3 updated.

### Known friction (unchanged)

- Large `versions.json` → cache + TTL; pick one archive per host (zip vs tar.gz).
