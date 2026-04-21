# Odin integration plan

## Goal

Add **`RuntimeKind::Odin`** (`key = "odin"`) as a first-class managed runtime (CLI / GUI / shims / pins / `exec` / `run`), installing prebuilt Odin toolchains into:

`runtimes/odin/versions/<label>` with a global `runtimes/odin/current` symlink or Windows pointer file.

## Upstream and artifacts

Odin is distributed via GitHub Releases:

- Repo: `odin-lang/Odin`
- API: `https://api.github.com/repos/odin-lang/Odin/releases`
- Atom fallback: `https://github.com/odin-lang/Odin/releases.atom`

Release tags are typically monthly dev tags like:

- `dev-2026-04`
- `dev-2025-12a` (letter suffix within a month can exist)

Assets are host-specific archives:

- Windows: `odin-windows-amd64-<tag>.zip`
- Linux/macOS: `odin-<os>-<arch>-<tag>.tar.gz`

Where `<tag>` is the GitHub tag name (examples above).

## Version labels and spec grammar

envr exposes a **numeric dotted label** so remote summaries can group by “major.minor” via `version_line_key_for_kind`:

- Tag `dev-YYYY-MM` → label **`YYYY.MM`**
- Tag `dev-YYYY-MM<suffix>` where `<suffix>` is a single ASCII letter (e.g. `a`) → label **`YYYY.MM.<n>`**
  - `a` → `1`, `b` → `2`, ...

Examples:

- `dev-2026-04` → `2026.04`
- `dev-2025-12a` → `2025.12.1`

`envr install odin <spec>` accepts:

- Exact labels like `2026.04` / `2025.12.1`
- `latest` / `latest@2026.04` (if unified-latest-per-major is enabled for Odin)

## Normalized index row and caching

- **Normalized row:** `OdinInstallableRow { version, url }`
- **Cache directory:** `{runtime_root}/cache/odin/`
- **Index cache TTL:** `ENVR_ODIN_RELEASES_CACHE_TTL_SECS` or legacy `ENVR_ODIN_INDEX_CACHE_TTL_SECS` (default 3600)
- **Latest-per-major disk cache TTL:** `ENVR_ODIN_REMOTE_CACHE_TTL_SECS` (default 86400)
- **API override:** `ENVR_ODIN_GITHUB_RELEASES_URL`
- **GitHub token support:** `GITHUB_TOKEN` / `GH_TOKEN` / `ENVR_GITHUB_TOKEN`

Fallback policy:

- Try GitHub Releases API (pagination, auth token).
- If all API candidates fail (e.g. **403/rate-limit/proxy blocks**), use `releases.atom` tag extraction and construct synthetic download URLs:
  - `https://github.com/odin-lang/Odin/releases/download/<tag>/<asset>`

## Install layout and validation

After extraction, promotion normalizes common archive shapes:

- Single-root directory vs flat-root archives.

Validation rule:

- Runtime home is valid if it contains:
  - Windows: `odin.exe` (or `bin/odin.exe`)
  - Unix: `odin` (or `bin/odin`)

PATH entries for shims should include the runtime home (and `bin/` if present).

## Shims and environment

- **Core command surface:** `odin`
- **Runtime home env:** set **`ODIN_ROOT`** to the resolved runtime home when Odin is on the stack.
- **Template key:** set **`ENVR_ODIN_VERSION`** to the selected version label (same family as `ENVR_PERL_VERSION`).
- **PATH proxy toggle:** supported via `[runtime.odin].path_proxy_enabled` (same model as V/Nim/Crystal).

## CLI / GUI smoke (acceptance)

CLI:

```bash
envr remote odin
envr remote odin -u
envr install odin 2026.04
envr use odin 2026.04
envr shim sync
odin version
envr exec --lang odin -- odin version
```

GUI:

- Odin tab exists in Env Center.
- Toggle PATH proxy, persisted to `settings.toml`.
- Install and switch versions; Env Center reflects current and remote list.

## Architecture / abstraction friction log (fill during implementation)

1. **Non-semver tags**: monthly `dev-YYYY-MM` plus occasional `dev-YYYY-MM<suffix>` required mapping to dotted numeric labels (`YYYY.MM(.n)`) so existing `numeric_version_segments` and “major.minor” grouping (`version_line_key_for_kind`) could be reused.
2. **Remote filter shape**: `RemoteFilter` is prefix-only (no `force_refresh` flag), so “refresh” stays a responsibility of higher layers (runtime service cache invalidation) rather than provider APIs.
3. **Cache serde boundary**: domain `RuntimeVersion` is not `serde`-serializable; provider caches must persist **strings** or a dedicated `serde` row type (not `Vec<RuntimeVersion>`).
4. **Archive extraction dispatch**: archive extraction dispatches by file extension, so downloads should preserve the upstream filename (or at least an extension) when writing to disk.
5. **Asset selection**: host mapping is table-driven (OS/ARCH → acceptable asset prefixes + archive extensions); avoid hardcoding a single full filename template.
6. **GitHub resilience**: API pagination + token + atom fallback; fallback constructs synthetic `releases/download/<tag>/<asset>` URLs and must still produce *installable* rows for the host (otherwise return a clear “unsupported host” style error).
7. **Archive root variance**: promotion tolerates both flat-root and single-root archives; validation checks for `odin` under root or `bin/`.

## CLI / GUI friction log (fill after smoke)

- CLI:
  - ...
- GUI:
  - ...

## Playbook corrections (if needed)

- If Odin integration surfaces a missing generic checklist item, add it to `docs/architecture/new-runtime-playbook.md` under the most relevant “follow-up friction” section.

