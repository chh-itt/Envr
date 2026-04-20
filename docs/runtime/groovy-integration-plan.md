# Groovy integration plan (Apache dist index + binary zip)

## Goal

Add **`RuntimeKind::Groovy`** as a first-class runtime (CLI / GUI / shims / pins / `exec` / `run`) and manage installs under:

`runtimes/groovy/versions/<version>/` with `runtimes/groovy/current`.

Groovy is JVM-hosted: descriptor `host_runtime = Java`; shim/exec/run must merge `JAVA_HOME` and enforce compatibility checks via shared JVM-hosted abstractions.

## Scope & non-goals

- **In scope:** Apache Groovy binary distribution (`apache-groovy-binary-<ver>.zip`) from Apache dist index.
- **Out of scope:** Gradle wrapper/toolchains, Groovy-Eclipse, project build plugin provisioning.

## Version/index shape

- **Primary source:** `https://dlcdn.apache.org/groovy/` directory index (stable release lines).
- **Fallback source:** `https://archive.apache.org/dist/groovy/` for wider historical coverage when needed.
- **Installable row:** `(version_label, download_url)` where URL resolves to:
  - `https://dlcdn.apache.org/groovy/<version>/distribution/apache-groovy-binary-<version>.zip`
  - fallback to archive host with same path shape.
- **Cache:** `{runtime_root}/cache/groovy/index_rows.json` (TTL env knob, default 6h).

## Host JDK policy

- **Minimum:** Java **8+** baseline, with Groovy 4.x+ enforced as Java **11+**.
- Keep compatibility call sites routed through `envr_domain::jvm_hosted`.
- Revisit min/max policy only if field reports show compiler/runtime launch breakage by Groovy major line.

## Architecture / abstraction friction log

1. **Index-source diversity:** Groovy uses Apache directory indexes instead of a convenient GitHub releases API; parser robustness and cache schema stability are the main risk.
2. **JVM-hosted matrix drift:** every new JVM runtime should avoid custom branches in shim/exec/run/gui and only extend matrix tables.
3. **GUI settings/message wiring:** Env Center still needs explicit per-runtime message and section branches; compile-time safe but repetitive.

## Implementation checklist

### Phase A — Domain

- [x] Add `RuntimeKind::Groovy` descriptor (`key=groovy`, `host_runtime=Java`, remote/path-proxy true).
- [x] Include Groovy in version line grouping (`major.minor`).
- [x] Extend descriptor tests/count/host acyclic checks.
- [x] Extend `envr_domain::jvm_hosted` and add `groovy_java` policy module.

### Phase B — Provider crate `envr-runtime-groovy`

- [x] Create crate + provider implementation.
- [x] Parse Apache index pages into installable versions.
- [x] Resolve/install binary zip and validate `groovy` + `groovyc` launchers.
- [x] Enforce Java preflight for install/set-current.

### Phase C — Core/CLI/resolver/shims

- [x] Register provider in runtime service + core Cargo wiring.
- [x] Shim commands: `groovy`, `groovyc`.
- [x] Add `runtime_bin_dirs_for_key`, `runtime_home_env_for_key` (`GROOVY_HOME`) and hosted Java merge.
- [x] Wire `ENVR_GROOVY_VERSION` + run stack + missing pins + list/bundle/status/shim sync parity.

### Phase D — Config/GUI

- [x] Add `[runtime.groovy] path_proxy_enabled` and snapshot/schema support.
- [x] Add Env Center settings block + hosted Java hint flow.
- [x] Ensure runtime layout/order tests include new runtime count.

### Phase E — Docs/playbook polish

- [x] Add `docs/runtime/groovy.md`.
- [x] Update playbook/ADR if JVM matrix guardrails need further clarification from Groovy bring-up.
- [x] Record concrete friction/edge cases found during implementation/testing.

## QA notes

- CLI smoke: `envr remote groovy`, `envr remote groovy -u`, `envr install groovy <spec>`, `envr use groovy <ver>`, `envr exec --lang groovy -- groovy --version`, `envr exec --lang groovy -- groovyc --version`.
- GUI smoke: Groovy tab remote/install/use/current, path proxy toggle persistence, Java-host hint behavior.

## Development notes (actual)

- Groovy release discovery uses Apache index pages (not GitHub releases), so remote integration risk moved from API auth/rate-limit toward HTML index parsing stability.
- JVM-family matrix now includes four hosted runtimes (`kotlin`/`scala`/`clojure`/`groovy`) via `envr_domain::jvm_hosted`; shim/child_env/gui call sites did not need bespoke compatibility branches.
- GUI friction still exists for runtime settings message wiring (`Set*PathProxy`, `*JavaChecked` per runtime), even with shared descriptor capabilities; this remains the main abstraction hotspot when adding new runtimes.
- Existing unrelated CLI JSON-contract failures (`final_sprint_json_contract` shell cases) remain in this environment and are not introduced by Groovy changes.
- JVM policy refinement: field validation showed Groovy 4.x on Java 8 still surfaced raw JNI errors; policy now gates Groovy 4.x+ to Java 11+ so shim/exec errors align with Kotlin/Clojure style messaging.
