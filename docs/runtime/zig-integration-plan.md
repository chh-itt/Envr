# Zig runtime integration plan (envr)

Execution plan for adding **Zig** as a first-class runtime, aligned with
`docs/architecture/new-runtime-playbook.md` and the post–unified-list / layout GUI work.

## 1) Why Zig (for this milestone)

- **Structured upstream metadata**: official `https://ziglang.org/download/index.json` lists stable
  releases and `master` nightlies per platform, with tarball URLs and `shasum` (single HTTP fetch
  for discovery — unlike Ruby’s HTML-first story).
- **Cross-platform but regular**: Unix uses `.tar.xz`, Windows uses `.zip`; archive root is
  predictable (`zig-<triple>-<version>/` with `zig` executable inside).
- **Version strings are expressive**: stable `0.14.1` vs nightly `0.17.0-dev.9+046002d1a` — good stress
  test for **remote list sorting**, **resolve**, and **GUI unified major-line** rules without JVM
  baggage.
- **Narrow core command surface (MVP)**: primarily `zig` (optional later: `zls` is out of scope).

## 2) Goal and scope

### Goal

- Add `zig` as a first-class `RuntimeKind` with provider crate `envr-runtime-zig`.
- Validate descriptor-driven registration, `RuntimeService`, resolver/shim/exec paths, and GUI
  parity (nav, env center, settings if any, remote cache / unified list where enabled).

### In scope (MVP)

- Install / list installed / current / set current / uninstall / resolve (full version spec).
- Remote listing from **official `index.json`** with disk cache and TTL consistent with other
  `supports_remote_latest` runtimes.
- Shim / `exec` / `run` for **`zig`**.
- `.envr.toml` pin: `[runtimes.zig] version = "0.14.1"` (exact spec grammar TBD in §4).
- CLI parity for the standard surface (`list`, `current`, `remote`, `install`, `use`, `uninstall`,
  `exec`, `run`, `env`, `which` as applicable).
- GUI: runtime hub (respect layout/hidden), env center page, dashboard row, optional small
  settings block (download source / mirror only if we implement domestic mirror in MVP).

### Out of scope (explicit deferrals)

- **`zls`** or other satellite tools as first-class shims.
- **`zig build`** project introspection, `build.zig.zon` version pinning semantics.
- **Bundled LLVM / libc** diagnostics beyond “archive extracted and `zig version` runs”.
- **Self-hosting bootstrap** archives (`zig-bootstrap-*` in `index.json`) — too large and not the
  user-facing toolchain; **do not** offer as default install artifact.
- **Source-only** tarball (`src` entry) as default — prefer platform **prebuilt** triple.

## 3) Zig-specific facts and decisions

### A. Runtime key and labels

- Runtime key: `zig`
- English label: `Zig`
- Chinese label: `Zig`

### B. Canonical metadata: `index.json`

- Primary URL: `https://ziglang.org/download/index.json` (JSON object).
- Top-level keys include:
  - **`master`**: rolling nightly; `version` string includes `-dev` and `+` commit suffix.
  - **Stable releases**: semver keys such as `0.14.1`, each with `date`, per-platform objects.
- Each platform object includes `tarball` (or `.zip` on Windows), `shasum`, `size`.

**Implementation decision (MVP):**

- Fetch and parse `index.json` once per refresh; cache normalized result under
  `cache/zig/` (path pattern consistent with other runtimes).
- **Default remote list for GUI “stable” experience**: include semver keys only, sorted by
  version descending. Optionally expose **`master`** behind explicit user spec or a separate
  toggle post-MVP — if included early, document churn (version string changes daily).

### C. Platform triple mapping (critical)

`index.json` uses keys like `x86_64-linux`, `aarch64-macos`, `x86_64-windows` (not Rust’s
`x86_64-unknown-linux-gnu`).

Required mapping from `(OS, ARCH)` → JSON platform key:

| envr host (examples) | `index.json` key (typical) |
|----------------------|----------------------------|
| Linux x86_64 | `x86_64-linux` |
| Linux aarch64 | `aarch64-linux` |
| macOS x86_64 | `x86_64-macos` |
| macOS aarch64 | `aarch64-macos` |
| Windows x86_64 | `x86_64-windows` |
| Windows aarch64 | `aarch64-windows` |

**Edge cases:**

- **Unsupported triple**: JSON may lack an entry for a rare arch; `install` / `list_remote` must
  return a clear error (“no official Zig build for this platform”) rather than panicking.
- **Windows vs Unix archive**: select `.zip` vs `.tar.xz` by platform; reuse existing download +
  extract helpers where possible.

### D. Version and spec grammar

- **Stable**: `MAJOR.MINOR.PATCH` (e.g. `0.14.1`).
- **Master** (if enabled later): `0.17.0-dev.N+hash` style strings — treat as opaque labels for
  storage; semver crates may not parse; use **string ordering** only with care, or pin “master” as
  a single moving target.

**MVP recommendation:**

- `install` / `use` accept **full stable version** first (`0.14.1`).
- `resolve` may allow `0.14` → latest `0.14.x` if trivially derivable from the index (same as other
  runtimes’ “partial spec” policy).

**Unified / major-line GUI:**

- Major key for Zig is likely **`0.14`** (two-part prefix) not a single integer; align with
  `version_line_key_for_kind` / unified list rules — **add a Zig branch** if today’s logic assumes
  Node/Python-style keys only.

### E. Install layout under envr runtime root

Target (consistent with other languages):

- Home: `runtimes/zig/`
- Versions: `runtimes/zig/versions/<version>/` containing extracted tree (single root folder
  flattened or normalized so `zig` is at `versions/<ver>/zig` or `versions/<ver>/zig.exe`).
- Current: `runtimes/zig/current` → symlink (Unix) or junction / documented Windows strategy
  matching Go/Node patterns already in tree.

**Post-install validation:**

- `zig version` succeeds.
- On Windows, ensure `zig.exe` resolves when `current` is updated.

### F. Checksum and integrity

- `index.json` provides `shasum` (hex). Prefer **verifying after download** before promote (same
  class of guarantee as other HTTP-installed runtimes).

### G. Child env / PATH proxy

- **Default assumption**: putting `zig` (and `lib/` relative layout shipped in archive) on PATH is
  sufficient for MVP — Zig is designed to be relocatable.
- If resolver injects runtime-home for other languages, follow the same helper for `zig`.
- **PATH proxy**: follow descriptor `supports_path_proxy` decision (likely `true` like Go/Deno
  unless product chooses `false` for minimal surface — **decide before coding** and match shim
  bypass rules).

### H. Domestic / mirror (optional MVP+)

- Official JSON is on `ziglang.org`. For China-friendly flows, evaluate whether a vetted mirror
  exposes the **same** `index.json` shape; if not, keep **official only** for MVP and add
  `ZigDownloadSource` later mirroring Node/Go patterns.

### I. Project-local version files (defer)

- Community may use ad-hoc `.zig-version` files; **not** in MVP unless explicitly prioritized.
- Document as follow-up to avoid precedence wars with `.envr.toml`.

## 4) Special situations / risks (read before coding)

1. **`master` vs stable**: Installing `master` means the resolved URL changes over time; uninstall
   and “current” labels must show the **opaque version string** from JSON at install time.
2. **JSON schema drift**: If Zig adds/removes platform keys, mapping table must fail soft with good
   errors; add a **contract test** that parses a checked-in fixture snippet of `index.json`.
3. **Semver `0.x`**: Ordering must not use naive float parsing (`0.14` ≠ `0.140`).
4. **Archive root directory name**: Extracted folder name includes triple and version; normalize
   into `versions/<spec>/` deterministically (same class of bug as other tarball providers).
5. **GUI skeleton / tab enter**: After Zig exists, ensure `runtime_page_enter_tasks` and
   `OpenRuntime` paths both refresh; follow playbook §H.1.
6. **`zig fmt` / `zig test`**: Not separate shims in MVP — users invoke via `zig fmt` etc.

## 5) Implementation phases (suggested)

1. **Domain + descriptor**: `RuntimeKind::Zig`, `RUNTIME_DESCRIPTORS`, `parse_runtime_kind`,
   `version_line_key_for_kind` / `major_line_remote_install_blocked` updates if needed.
2. **Provider crate** `envr-runtime-zig`: fetch `index.json`, map triple, download+verify+extract,
   `list_remote`, `resolve`, `install`, `uninstall`, `current`. — **done** (2026-04-18).
3. **Service registration** in `envr-core` runtime service.
4. **Resolver / shim / exec** wiring for `zig`.
5. **CLI smoke** in temp runtime root.
6. **GUI** (nav, dashboard, env center, unified list if `supports_remote_latest: true`).
7. **Docs** `docs/runtime/zig.md` + tests + development log updates in this file.

## 6) Acceptance criteria (MVP)

- [x] `envr remote zig` returns stable versions for this host’s triple.
- [x] `envr install zig 0.14.1` + `envr use zig 0.14.1` + `envr exec --lang zig -- zig version`.
- [ ] GUI: selecting Zig loads installed/remote without blanking on refresh; dashboard card shows
  correct counts after doctor/dashboard refresh.
- [x] Uninstall removes the version directory and clears `current` when it pointed there.
- [x] At least one integration-style test for resolve/exec dry-run or install layout validation.

## 7) Open questions (resolve during implementation)

- Include **`master`** in default remote list or hide behind flag / advanced spec?
- `supports_path_proxy`: final `true`/`false` for Zig MVP?
- Domestic mirror: skip vs implement alongside `index.json` compatibility proof?

## 8) Development log

| Date | Note |
|------|------|
| 2026-04-18 | Phase 2: `envr-runtime-zig` — `index.json` fetch + disk cache (TTL), host→JSON platform key mapping, stable-only remote list (excludes `master` unless spec), semver resolve (`0.14`→latest patch), SHA256 verify + zip/tar.xz extract + single-root promote under `runtimes/zig/versions/<label>/`, `current` symlink/pointer, uninstall; contract tests on `tests/fixtures/zig_index_snippet.json`; `envr run` registers `zig` in `resolve_run_lang_home`. |
| 2026-04-18 | Follow-up: `RUNTIME_PLAN_ORDER` / `RUN_STACK_LANG_ORDER` / `ENVR_ZIG_VERSION` template key; bundle `global_current` includes Ruby/Elixir/Erlang/Dotnet/Zig; `shim sync` + `doctor --fix` core shim list aligned with all `ShimService` kinds; `docs/runtime/zig.md`; CLI integration test `exec_dry_run_zig_resolves_project_pin`. |
| 2026-04-18 | `RUNTIME_PLAN_ORDER` adds `elixir` + `erlang` (with `zig`); CLI dev-test `remote_zig_offline` seeds `cache/zig/remote_latest_per_major_<plat>.json` and asserts `envr remote --format json zig` is offline and sets `cached_snapshot: true`. |
| 2026-04-18 | Hardened tests: `RUN_STACK_LANG_ORDER` adds `dotnet`; `remote_zig_offline` validates envelope/data schemas and asserts `cached_snapshot=true` with non-prefix background refresh (`remote_refreshing=true`) and no prefix fallback; `envr-runtime-zig` unit test confirms uninstall removes version dir and clears `current`. |
| 2026-04-18 | CLI remote strategy aligned with GUI unified cache path: non-prefix `remote` prefers unified full-installable snapshot for instant paint, keeps stale-first + async refresh behavior, and adds `remote -u/--update` to force live fetch + cache update before rendering. |

## 9) Architecture and abstraction friction notes

### A. Runtime cache layers diverged between CLI and GUI

- GUI already consumed unified-list cache (`cache/<runtime>/unified_version_list/full_installable_versions.json`), while CLI `remote` initially read provider-local `remote_latest_per_major*` snapshots.
- Result: GUI could display rich version lists while CLI still returned empty rows with `remote_refreshing=true`.
- Fix direction used in this rollout: make CLI `remote` prefer unified full-installable snapshot, then fall back to major-row/provider snapshots.

### B. `resolve` vs remote-intent mismatch in user expectations

- Users often try `resolve zig --spec 0.14` expecting remote resolution.
- Actual behavior is local runtime-home resolution (project pin / global current); without installed versions it should fail.
- Action taken: keep semantics unchanged, but document command intent and provide `remote --prefix` + `remote -u` as the remote discovery path.

### C. Cache status command semantics were narrower than expected

- `cache index status` reports only the classic index-cache domain (`node`/`deno`/`bun`), not unified runtime caches.
- This is a naming/UX friction point, not a data corruption bug.
- Follow-up recommendation: add a dedicated runtime/unified cache status command rather than overloading existing index status semantics.

## 10) CLI command changes from Zig rollout

- `remote` now supports `-u, --update` to force live fetch and refresh cache before rendering.
- Prefix filtering remains `--prefix <value>` (for example: `envr remote zig --prefix 0.14`), not positional `envr remote zig 0.14`.
- Default `remote` path stays "stale-first then refresh" for fast paint:
  - with cache: returns cached rows immediately and refreshes in background;
  - without cache: may return temporary empty rows while refresh warms cache.
