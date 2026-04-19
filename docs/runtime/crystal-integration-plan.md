# Crystal runtime integration (envr)

## 1) Naming and scope

- **`RuntimeKind::Crystal`**, descriptor key **`crystal`** (`[runtimes.crystal]`, `envr install crystal 1.20.0`, `envr exec --lang crystal`).
- **Index**: GitHub **`crystal-lang/crystal` Releases API** (`/repos/crystal-lang/crystal/releases?per_page=100`), not a custom JSON feed. Parse `tag_name`, `draft`, `prerelease`, and `assets[]` (`name`, `browser_download_url`, `digest`).
- **Install**: download selected asset → **`envr_download::extract::extract_archive`** (`.tar.gz` / `.zip`) → **single-root promote** into `runtimes/crystal/versions/<semver>/` (same layout contract as Zig).
- **Shim**: **`crystal`** only (PATH proxy like Zig). Validation: `bin/crystal` (Unix) or `bin/crystal.exe` (Windows) under version home.
- **Checksum**: use GitHub asset `digest` when present (`sha256:...` → hex for `verify_sha256_hex`).

## 2) Platform asset selection (MVP)

Upstream filenames vary by OS/arch and include a **Debian-style middle revision** (`-1-`). Selection is **suffix/pattern based**, not a hand-built URL template:

| Host | Preferred asset pattern |
|------|---------------------------|
| Linux x86_64 | `*-linux-x86_64.tar.gz`, exclude `bundled` |
| Linux aarch64 | `*-linux-aarch64.tar.gz`, exclude `bundled` |
| macOS (universal) | `*-darwin-universal.tar.gz` |
| Windows x86_64 | `*-windows-x86_64-msvc-unsupported.zip` (portable tree; avoid `.exe` for extract-only flow) |
| Windows aarch64 | `*-windows-aarch64-gnu-unsupported.zip` |

Unsupported / missing mapping → clear **`EnvrError::Validation`** (same class as Zig host map).

## 3) Version semantics

- Remote list = **semver tags** that have an **installable asset for this host** (intersect tag index with asset matrix).
- **Two-part line** keys (`1.20`, `1.19`) via `version_line_key_for_kind` with **`RuntimeKind::Crystal`** (group with Zig/Julia/…).
- **Resolve**: exact `x.y.z`, `x.y` (latest patch on line), `x` (latest on major).

## 4) Caching / env

- Disk cache: `cache/crystal/releases_<host_slug>.json` (normalized `CrystalReleaseRow` list after fetch) + TTL **`ENVR_CRYSTAL_RELEASES_CACHE_TTL_SECS`** (default 1h; legacy alias `ENVR_CRYSTAL_INDEX_CACHE_TTL_SECS`).
- Remote latest-per-major cache: same pattern as Zig/R (`remote_latest_per_major_*.json` + **`ENVR_CRYSTAL_REMOTE_CACHE_TTL_SECS`**).
- `run` / `exec` template: **`ENVR_CRYSTAL_VERSION`**.

## 5) Acceptance / tests

- `cargo test -p envr-runtime-crystal` with **fixture JSON** (subset of GitHub releases + assets) for parse + asset pick + resolve.
- `exec_run_env_integration` dry-run with fake `runtimes/crystal/versions/.../bin/crystal(.exe)`.

## 6) Architecture / friction log

- **Cache vs parser shape**: raw GitHub JSON and normalized `CrystalReleaseRow` JSON are different; the on-disk TTL cache must deserialize with the same type written after fetch (implemented as `parse_cached_install_rows`).
- **GitHub API coupling**: rate limits, pagination, and asset naming drift (`bundled`, `unsupported`, `darwin-universal` vs older darwin targets). Mitigate with TTL cache + explicit pattern docs + `ENVR_CRYSTAL_GITHUB_RELEASES_URL` override for mirrors/proxies.
- **Windows builds labeled “unsupported”**: still the official distribution channel for portable zips; document in user-facing `crystal.md` so expectations stay clear.
- **Pagination**: MVP may cap pages (e.g. 3×100); document if `remote crystal` truncates very old majors.

## 7) Development log

- [x] Plan (`docs/runtime/crystal-integration-plan.md`).
- [x] Crate `envr-runtime-crystal` + wire domain/config/core/shim/resolver/cli/gui/tests/docs/playbook.

### Implementation notes (done)

- Disk cache under `cache/crystal/releases_<host_slug>.json` stores **`Vec<CrystalReleaseRow>`** (serde), not raw GitHub JSON, so TTL reload does not depend on re-parsing the API shape.
- Releases TTL env: **`ENVR_CRYSTAL_RELEASES_CACHE_TTL_SECS`** (with legacy **`ENVR_CRYSTAL_INDEX_CACHE_TTL_SECS`** fallback in code).

### CLI / GUI follow-ups (post-merge)

- If GitHub adds/changes MSVC vs GNU primary zip, adjust patterns and extend fixture tests.
- Optional: domestic mirror base URL preset (same class of work as other runtimes).
