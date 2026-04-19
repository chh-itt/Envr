# Kotlin (JVM compiler bundle) — managed runtime

envr installs the official **kotlin-compiler** zip from **JetBrains/kotlin** GitHub releases into:

`runtimes/kotlin/versions/<version>` with a global `runtimes/kotlin/current` symlink or Windows pointer file.

Kotlin **always uses an envr-managed JDK**: shims and `envr exec --lang kotlin` set **`JAVA_HOME`** to the same resolved home as the `java` / `javac` shims (see [ADR-0001: Runtime host dependencies & Kotlin on the JVM](../architecture/adr-0001-runtime-host-dependencies-kotlin.md)).

## Requirements

- **A JDK** must be installed and selected as **Java `current`** (`envr use java …`). Minimum **Java 8** for modern Kotlin lines.
- **JDK “too new” for the compiler bundle:** Some Kotlin **2.0.x** builds ship IntelliJ components whose `JavaVersion` parser can **fail on very new JDKs** (field report: **JDK 25+** with Kotlin **2.0.21**). envr applies a **conservative upper bound** for **Kotlin 2.0.x** (currently JDK **≤24** by directory-label heuristic) and surfaces a clear error in **install / use / shim / exec / run / GUI** when the combo is blocked.
- Remote index: GitHub API (`api.github.com/repos/JetBrains/kotlin/releases`), cached under `{runtime_root}/cache/kotlin/github_releases.json`. TTL: `ENVR_KOTLIN_INDEX_CACHE_TTL_SECS` (default **21600** seconds).

## Commands

Use **`envr remote kotlin`** (not `envr remote list kotlin`). With no local cache yet, the first **`envr remote kotlin`** performs the same network fetch as **`envr remote kotlin -u`** so you do not see an empty “（无）” list.

```bash
envr remote kotlin
envr remote kotlin -u
envr install kotlin 2.0.21
envr use kotlin 2.0.21
envr shim sync
kotlinc -version
envr exec --lang kotlin -- kotlinc -version
```

## Settings

`settings.toml`:

```toml
[runtime.kotlin]
path_proxy_enabled = true
```

When `path_proxy_enabled = false`, `kotlin` / `kotlinc` shims bypass envr-managed resolution (same model as Lua/Nim).

## Pins

`.envr.toml`:

```toml
[runtimes.kotlin]
version = "2.0.21"
```

Java can be pinned separately under `[runtimes.java]`; envr resolves **effective** Java for Kotlin the same way as `java` shims.

## Performance note (`kotlinc -version`)

Roughly **~1–2 seconds** on a cold JVM + Kotlin CLI startup is normal: most time is **Java process + kotlinc**, not envr’s PATH/`JAVA_HOME` merge. envr does not add a persistent daemon.

## GUI

Env Center shows a **JDK vs Kotlin compiler** hint when the global Java/Kotlin combo hits the bundled-compatibility heuristic (same policy as CLI). Ensure **Java** has a **`current`** before installing or switching Kotlin.
