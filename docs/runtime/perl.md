# Perl (managed)

envr installs a **portable Perl** into:

`runtimes/perl/versions/<label>` with a global `runtimes/perl/current` symlink or Windows pointer file.

- **Windows x64:** [StrawberryPerl/Perl-Dist-Strawberry](https://github.com/StrawberryPerl/Perl-Dist-Strawberry) release assets `strawberry-perl-*-64bit-portable.zip`.
- **Linux / macOS:** [skaji/relocatable-perl](https://github.com/skaji/relocatable-perl) archives `perl-linux-amd64.tar.xz` (and siblings), with `.tar.gz` fallback when an `.xz` asset is absent.

Unsupported hosts (for example Windows ARM64 without a portable zip contract) return a validation error from the provider; see `docs/runtime/perl-integration-plan.md`.

## Remote index and caching

- Default GitHub Releases API URL is chosen by OS. Override with **`ENVR_PERL_GITHUB_RELEASES_URL`** if you mirror the API.
- If the **REST API** returns **403** (or all API candidates fail), **Windows Strawberry** falls back to **`https://github.com/StrawberryPerl/Perl-Dist-Strawberry/releases.atom`** and builds portable zip URLs from stable `SP_xxxxx_64bit` release tags (same idea as Crystal / relocatable-perl atom fallback).
- Index cache TTL: **`ENVR_PERL_RELEASES_CACHE_TTL_SECS`** or **`ENVR_PERL_INDEX_CACHE_TTL_SECS`** (default 3600 seconds).
- Latest-per-major disk cache TTL: **`ENVR_PERL_REMOTE_CACHE_TTL_SECS`** (default 86400 seconds).
- Optional GitHub token: **`GITHUB_TOKEN`**, **`GH_TOKEN`**, or **`ENVR_GITHUB_TOKEN`** (same as other GitHub-backed runtimes).

## Commands

```bash
envr remote perl
envr remote perl -u
envr install perl 5.42.2.0
envr use perl 5.42.2.0
envr shim sync
perl -v
envr exec --lang perl -- perl -v
```

## PATH and shims

The `perl` shim lives under **`{ENVR_RUNTIME_ROOT}/shims`** (after `envr shim sync`). Your shell **PATH** must include that directory so a bare **`perl`** resolves to envr’s shim—not a system Strawberry/ActivePerl elsewhere on PATH.

- **`perl -v`** (no path prefix) uses PATH; ensure the shims directory is **before** other Perl installs if you need the managed version by default.
- **`.\perl`** from an arbitrary working directory (for example a build output folder) is **not** the shim; use **`perl`** on PATH or **`envr exec --lang perl -- …`** when you want the resolved managed runtime regardless of cwd.

Major lines follow **`major.minor`** (for example `5.42`) via `version_line_key_for_kind`.

## Settings

```toml
[runtime.perl]
path_proxy_enabled = true
```

When `path_proxy_enabled = false`, the `perl` shim resolves to the next matching `perl` on **system PATH** outside envr shims (same model as Crystal / Nim).

## Pins

```toml
[runtimes.perl]
version = "5.42.2.0"
```

## Environment

- Shims and `envr run` set **`PERL_HOME`** to the resolved install root when Perl is on the stack.
- Template key **`ENVR_PERL_VERSION`** is set to the version directory label (same family as `ENVR_RUBY_VERSION`, `ENVR_CRYSTAL_VERSION`, etc.).
