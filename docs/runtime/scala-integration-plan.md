# Scala integration plan (Scala 3 via GitHub releases)

## Goal

Add **`RuntimeKind::Scala`** as a first-class runtime (CLI / GUI / shims / pins / `exec` / `run`), installing the official **Scala 3** distribution from **`scala/scala3` GitHub releases** into:

`runtimes/scala/versions/<version>/` with `runtimes/scala/current`.

Scala on the JVM **requires a JDK**: follow **[ADR-0001](../architecture/adr-0001-runtime-host-dependencies-kotlin.md)** ??descriptor `host_runtime = Java`, **`JAVA_HOME`** merged like Kotlin (no Kotlin-specific bundled-parser JDK cap unless we later discover a Scala-version ??JDK ceiling worth encoding).

## Scope & non-goals (this iteration)

- **In scope:** Scala **3.x** lines published under `scala/scala3` (tags like `3.4.3`).
- **Out of scope (follow-up):** Scala **2.13** from `scala/scala` (second index + artifact matrix); **Coursier-only** workflows; **sbt** as a managed tool (separate runtime or doc-only).

## Version / index shape

- **Remote source:** GitHub Releases API  
  `https://api.github.com/repos/scala/scala3/releases?per_page=100` (paginate later if needed).
- **Installable row:** `(version_label, download_url)` where `version_label` is the release tag without leading `v`, and `download_url` is chosen from **release assets** for the **current OS/arch**, with fallbacks to **universal** archives when vendors ship `scala3-<ver>.zip` / `.tar.gz` without platform suffixes.
- **Cache:** `{runtime_root}/cache/scala/github_releases.json` + normalized rows as needed; TTL **`ENVR_SCALA_INDEX_CACHE_TTL_SECS`** (default **21600**).
- **Major-line key:** same pattern as Kotlin ??`major.minor` from [`version_line_key_for_kind`](../../crates/envr-domain/src/runtime.rs) (e.g. `3.4`).

## Host JDK policy

- **Minimum:** Java **8+** directory-label heuristic (reuse `envr_domain::kotlin_java::jdk_dir_label_effective_major` for parsing only).
- **Scala compiler floor policy:** `envr_domain::scala_java` enforces **Scala 3.3+ -> Java 17+** (Scala 3.0-3.2 remains Java 8+) to surface a friendly envr validation instead of upstream JNI/classfile crashes.
- **No JDK max cap** currently (unlike Kotlin 2.0.x + JDK 25 policy).

## Architecture / abstraction friction (working log)

Record concrete pain points while wiring ??this section is intentionally blunt.

1. **GitHub asset naming drift:** Scala 3 has shipped both **platform-specific** (`scala3-x.y.z-x86_64-pc-win32.zip`) and **universal** (`scala3-x.y.z.zip`) assets across releases. The provider must **try ordered candidates** + universal fallback, not a single glob.
2. **`runtime_home_env_for_key`:** Today only `"java"` sets `JAVA_HOME`. Hosted JVM languages merge Java in **shim resolve** / **`child_env`** (ADR). Optional: set **`SCALA_HOME`** to the Scala install root for scripts that expect it ??keep consistent with ?guest home + host JAVA_HOME??
3. **`RUN_STACK_LANG_ORDER`:** **`java` must stay before `scala`** (same as Kotlin) so `collect_run_env` can reuse `JAVA_HOME` when both layers resolve.
4. **GUI parity:** JVM cousins should share a single ?hosted runtime??pattern long-term; Kotlin currently has **JDK-compat** card + Elixir has **OTP** card ??Scala MVP uses a **lightweight ?need Java current??* hint only when Scala is selected/after install/use (no duplicate Kotlin-style cap unless needed).
5. **Playbook:** JVM matrix is easy to under-document ??update **[new-runtime-playbook](../architecture/new-runtime-playbook.md)** when adding Scala so ?hosted runtime??+ asset fallback rules stay visible.

## Implementation checklist

### Phase A ??Domain

- [x] `RuntimeKind::Scala`, descriptor (`key: scala`, `host_runtime: Java`, remote + path proxy flags).
- [x] `version_line_key_for_kind` includes Scala (major.minor lines).
- [x] Tests: descriptor count, host acyclicity, optional version-line smoke.

### Phase B ??Provider crate `envr-runtime-scala`

- [x] `list_installed` / `current` / `set_current` / `install` / `uninstall` / remote lists / resolve.
- [x] Extract + promote tree (`bin/scala`, `bin/scalac` validation).
- [x] Windows `current` pointer fallback (reuse platform helpers).

### Phase C ??Core / CLI / resolver / shims

- [x] Register provider in `RuntimeService`.
- [x] Shims: `scala`, `scalac`; `JAVA_HOME` merge; optional `SCALA_HOME`.
- [x] `child_env` / `run` stack: java before scala; merge Java env for `exec`/`run`.
- [x] `RUN_STACK_LANG_ORDER`: insert `scala` after `kotlin`.
- [x] `missing_pins` / `bundle` / `list` parity lists as needed.

### Phase D ??Config / GUI

- [x] `[runtime.scala] path_proxy_enabled` + path-proxy snapshot wiring.
- [x] Env Center: PATH proxy strip + Scala hint task after pick/install/use.
- [x] `runtime_layout` default order picks up new descriptor key automatically; tests expect **20** runtimes.

### Phase E ??Docs

- [x] User doc `docs/runtime/scala.md` (install, JDK, remote cold-start parity with `remote` command, pins).
- [x] Update ADR-0001 header/body to mention Scala as second JVM-hosted consumer (same contracts).

## CLI / GUI verification notes (fill during QA)

- **CLI:** `envr remote scala`, `envr install scala <ver>`, `envr use scala <ver>`, `envr exec --lang scala -- scala -version`, `envr run --dry-run` in a pinned project.
- **GUI:** Env Center Scala tab loads; settings toggler persists; shim sync writes `scala`/`scalac` launchers.

## Known follow-ups

- [ ] Scala 2.13 **second index** (`scala/scala`) if users request legacy lines.
- [ ] Optional **doctor** row: Scala + Java sanity (ADR mentions Kotlin row; Scala can share shape).
