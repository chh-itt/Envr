# Perl integration plan

## Goal

Add **`RuntimeKind::Perl`** (`key = "perl"`) as a first-class managed runtime (CLI / GUI / shims / pins / `exec` / `run`), installing:

- **Windows x86_64:** **Strawberry Perl** portable ZIPs from GitHub **`StrawberryPerl/Perl-Dist-Strawberry`** (assets named like `strawberry-perl-<ver>-64bit-portable.zip`; version label parsed from the filename).
- **Linux / macOS:** **`skaji/relocatable-perl`** release tarballs (`perl-linux-amd64.tar.xz`, `perl-linux-arm64.tar.xz`, `perl-darwin-amd64.tar.xz`, `perl-darwin-arm64.tar.xz`, with `.tar.gz` fallback when present).

Layout: `runtimes/perl/versions/<label>/` with `runtimes/perl/current`. Validation: **`bin/perl`** (Windows: **`bin/perl.exe`**).

## Scope and non-goals

- **In scope:** Single interpreter shim **`perl`**, `PERL_HOME`, `ENVR_PERL_VERSION` in run/exec template keys, PATH proxy toggle, unified remote list UX (major.minor lines like Dart).
- **Out of scope (follow-up):** Windows **ARM64** Strawberry (no portable asset contract in-tree); **system Perl** / **plenv** / **perlbrew** integration; **CPAN** mirror knobs (use upstream / env only).

## Version / index shape

- **Normalized row:** `PerlReleaseRow { version, download_url, sha256_hex? }` cached as JSON (same contract as Crystal: dedicated serde type, not raw GitHub JSON round-trip).
- **Cache files:** under `{runtime_root}/cache/perl/` with slug `strawberry_win64` or `reloc_<stem>`.
- **TTL:** `ENVR_PERL_RELEASES_CACHE_TTL_SECS` or legacy `ENVR_PERL_INDEX_CACHE_TTL_SECS` (default **3600**). Remote latest-per-major disk cache TTL: **`ENVR_PERL_REMOTE_CACHE_TTL_SECS`** (default **86400**).
- **API override:** `ENVR_PERL_GITHUB_RELEASES_URL` (must point at the correct repo for the host: Strawberry vs relocatable-perl).
- **GitHub:** paginated `?per_page=100&page=N`; optional token **`GITHUB_TOKEN` / `GH_TOKEN` / `ENVR_GITHUB_TOKEN`**. For **relocatable-perl**, if the REST index fails, fallback to **`releases.atom`** plus synthetic `releases/download/<tag>/<asset>` URLs (Crystal-style).

## Architecture friction (working log)

1. **Dual upstream:** Windows and Unix use different orgs, asset naming, and version label shapes (Strawberry four-part build vs relocatable tag). The provider picks upstream from host and keeps one normalized row type.
2. **ZIP layout variance (Strawberry):** Portable trees may be flat, under `perl/`, or otherwise nested. Promotion scans **staging root + top-level dirs + one level of children** and picks the shallowest valid `bin/perl` home when multiple candidates exist.
3. **403 / rate limits:** Same playbook as other GitHub-backed runtimes: token, proxy-stripped API URL, then **`releases.atom`** fallback. **Strawberry** uses `StrawberryPerl/Perl-Dist-Strawberry/releases.atom` and stable `SP_<5digits>_64bit` tags to synthesize `strawberry-perl-<ver>-64bit-portable.zip` download URLs (beta / `dev_` tags skipped).
4. **GUI / CLI parity:** PATH proxy card mirrors Crystal/Nim; no JVM host card.

## Implementation checklist

- [x] Domain: `RuntimeKind::Perl`, descriptor, `version_line_key_for_kind` major.minor, descriptor count **27**.
- [x] Crate `envr-runtime-perl`: index + manager + provider + tests.
- [x] Core: register provider; shims `perl`; `child_env` / run stack / resolver / missing pins / bundle / list / shim sync lists.
- [x] Config: `[runtime.perl] path_proxy_enabled`, snapshot wiring, schema template.
- [x] GUI: Env Center PATH proxy; shell passes `runtime.perl` slice.
- [x] Docs: this plan, `docs/runtime/perl.md`, playbook subsection.

## CLI / GUI smoke

- `envr remote perl` / `envr install perl <label>` / `envr use perl <label>` / `envr shim sync` / `perl -v`
- GUI: Perl tab, toggle PATH proxy, persist `settings.toml`.
