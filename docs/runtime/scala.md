# Scala 3 (JVM) ??managed runtime

envr installs **Scala 3** from **`scala/scala3`** GitHub releases into:

`runtimes/scala/versions/<version>` with a global `runtimes/scala/current` symlink or Windows pointer file.

Scala **requires a JDK** managed by envr like other JVM-hosted runtimes: shims and `envr exec --lang scala` set **`JAVA_HOME`** to the same resolved home as the `java` shims, and set **`SCALA_HOME`** to the Scala install root. See [ADR-0001: Runtime host dependencies & Kotlin on the JVM](../architecture/adr-0001-runtime-host-dependencies-kotlin.md) (same pattern as Kotlin).

## Requirements

- **Java `current`** must be set (`envr use java ...`). Runtime baseline is **Java 8+**, and Scala compiler compatibility is checked by version line: **Scala 3.3+ requires Java 17+** (to avoid raw `UnsupportedClassVersionError` from `scalac`), while Scala 3.0-3.2 stays on Java 8+ in this policy.
- **Remote index:** GitHub **releases REST API** (`api.github.com/repos/scala/scala3/releases`), cached under `{runtime_root}/cache/scala/github_releases.json`. TTL: **`ENVR_SCALA_INDEX_CACHE_TTL_SECS`** (default **21600** seconds). Requests use the same mitigations as Crystal: optional token (**`GITHUB_TOKEN`**, **`GH_TOKEN`**, or **`ENVR_GITHUB_TOKEN`**), optional **`ENVR_GITHUB_API_PROXY_PREFIX`** / **`ENVR_SCALA_GITHUB_RELEASES_URL`**, and if the API returns an error (e.g. **403** in some regions), a fallback to **`https://github.com/scala/scala3/releases.atom`** plus synthetic `releases/download/...` URLs for the current platform.
- **Artifacts:** Releases may ship **platform-specific** archives (`scala3-x.y.z-x86_64-pc-win32.zip`, Linux/macOS `.tar.gz`) or **universal** `scala3-x.y.z.zip` / `.tar.gz`. envr tries platform names first, then universal fallbacks.

## Commands

Use **`envr remote scala`** (not `envr remote list scala`). With an empty local cache, the first single-runtime **`envr remote scala`** follows the same blocking refresh path as other unified remote runtimes (see CLI `remote` implementation).

**Why the list can look ?short??** `list_remote` only includes versions **installable on this machine** (GitHub assets must match the ordered filename candidates for your OS/arch, e.g. Windows `x86_64-pc-win32` or universal `scala3-*.zip`). Releases that only ship Linux/macOS archives are omitted. **`envr remote scala`** (without `-u`) prefers a cached **full installable** snapshot under `cache/scala/unified_version_list/full_installable_versions.json` when present; otherwise it falls back to **one version per major line** (`3.8`, `3.7`, ?? from `remote_latest_per_major.json`, which is why you may see a single row like `3.8.3` until a full snapshot exists?use **`envr remote scala -u`** once to refresh the full installable list (the CLI persists that snapshot for subsequent plain `envr remote scala`).

```bash
envr remote scala
envr remote scala -u
envr install scala 3.4.3
envr use scala 3.4.3
envr shim sync
scala -version
envr exec --lang scala -- scala -version
```

## Settings

`settings.toml`:

```toml
[runtime.scala]
path_proxy_enabled = true
```

When `path_proxy_enabled = false`, `scala` / `scalac` shims bypass envr-managed resolution (same model as Kotlin / Lua / Nim).

## Pins

`.envr.toml`:

```toml
[runtimes.scala]
version = "3.4.3"
```

Java can be pinned under `[runtimes.java]`; envr resolves **effective** Java for Scala the same way as for Kotlin shims.

## GUI

Env Center shows a short **Scala and JDK** hint when Scala is selected and a global **Java `current`** is missing (after install/switch, the hint is refreshed).
