# Kotlin runtime — integration plan

## Goal

Add **Kotlin** as a first-class **`RuntimeKind::Kotlin`** (CLI / GUI / shims / `.envr.toml` `[runtimes.kotlin]`), while **reusing envr-managed Java** as the JVM host: declarative dependency, install/`use` preflight, shim-time `JAVA_HOME`, and a compact **host** line in the Env Center hub.

**Normative architecture:** [ADR-0001: Runtime host dependencies & Kotlin on the JVM](../architecture/adr-0001-runtime-host-dependencies-kotlin.md) (Accepted).

**Execution checklist:** [New runtime playbook](../architecture/new-runtime-playbook.md) §3 + §2.1.

## Product decisions (MVP targets)

1. **Runtime key:** `kotlin` (parse key, cache dir, `[runtimes.kotlin]`).
2. **Independence:** Kotlin is its own `RuntimeKind`; users run `envr install kotlin` / `envr use kotlin`.
3. **Host:** `RuntimeDescriptor` gains **`host_runtime: Option<RuntimeKind>`** (Kotlin → `Some(Java)`); future JVM languages reuse the same field or migrate to `host_runtimes: &'static [RuntimeKind]` per ADR Phase B.
4. **Core commands (MVP):** `kotlin`, `kotlinc` shims + `exec --lang kotlin` (defer `kapt`, REPL variants, until needed).
5. **Index / artifacts:** TBD in implementation PR — likely **JetBrains GitHub releases** (`https://github.com/JetBrains/kotlin/releases`) or official **kotlin-compiler** bundles; document exact URL matrix, supported hosts (align with envr host policy: Windows x64, Linux x64 glibc, macOS tier as for Java/Lua policy).
6. **Layout:** `runtimes/kotlin/versions/<label>/` with standard **`current`** (symlink or Windows pointer file).
7. **JDK compatibility:** Static or index-derived **`java_min_major`** per Kotlin release; preflight on `install` / `use` / shim resolve compares **effective** Java from `RuntimeKind::Java` `current` (or pin); policy **`warn` vs `error`** via settings (key TBD — see ADR open questions).
8. **Shims:** Resolve Kotlin binary under Kotlin home; **`extra_env` includes `JAVA_HOME`** set to the **same resolved Java home** as the `java` shim (`envr-shim-core`: extend `runtime_home_env_for_key` / shared helper per ADR).
9. **GUI:** Kotlin row shows secondary text for host JDK, e.g. `宿主: Java 21` / i18n equivalent; blocking hint if Java missing.
10. **PATH proxy:** Descriptor `supports_path_proxy` — default **true** if Kotlin binaries must be managed like Java (confirm during wiring).

## Implementation checklist (playbook + ADR cross-walk)

### Phase 0 — Domain & policy

- [ ] `RuntimeKind::Kotlin` + `RUNTIME_DESCRIPTORS` entry (`label_en` / `label_zh`, flags).
- [ ] `RuntimeDescriptor::host_runtime` (or slice) + **acyclic** validation helper.
- [ ] `parse_runtime_kind("kotlin")`, `version_line_key_for_kind` if unified major UX applies.
- [ ] Settings: Kotlin runtime section + **JDK mismatch policy** (`warn` | `error`) + schema template + zh schema.
- [ ] `PathProxyRuntimeSnapshot` + `path_proxy_enabled_for_kind(Kotlin)` if supported.

### Phase 1 — Provider crate `envr-runtime-kotlin`

- [ ] Crate layout: `index` (fetch/parse releases), `manager` (install, promote, `kotlin_installation_valid`), `mirror` if needed.
- [ ] `RuntimeProvider` impl: list/install/uninstall/current/set_current/resolve + remote list semantics consistent with artifact set.
- [ ] **Preflight:** before install/commit `current`, resolve Java home + major; compare `java_min_major`; emit warn or error.
- [ ] Register in **`default_provider_boxes`** (`envr-core` `service.rs`).

### Phase 2 — Resolver / exec / run / child env

- [ ] `RUN_STACK_LANG_ORDER`, `RUNTIME_PLAN_ORDER`, merge env templates.
- [ ] `runtime_bin_dirs_for_key("kotlin", …)`.
- [ ] Ensure **`JAVA_HOME`** (and `PATH` bin dirs) appear in merged child env when Kotlin participates — same source as shims.

### Phase 3 — Shims

- [ ] `CoreCommand::Kotlin` / `Kotlinc` (names aligned with stems).
- [ ] Parser + `core_tool_executable` + PATH-proxy bypass map.
- [ ] **`runtime_home_env_for_key`:** for `kotlin`, append **`JAVA_HOME`** from shared **Java home resolver** (do not duplicate Java-only branch logic).
- [ ] `shim_service` / core stems for new commands.

### Phase 4 — CLI / GUI / registry / locales

- [ ] CLI: `remote` / `install` / `use` / … parity per playbook §G; **argv sample** + `COMMAND_SPEC_REGISTRY` + `help_registry/table.inc` + `cli.ok.*` locale keys.
- [ ] GUI: nav, dashboard, Env Center, `runtime_layout` default order merge, **host subtitle** component, preflight errors.
- [ ] `envr doctor`: optional Kotlin+Java sanity row.

### Phase 5 — Docs & tests

- [ ] User doc `docs/runtime/kotlin.md` (install limits, Java requirement, pins).
- [ ] Unit tests: index parse, resolve, preflight matrix (Java 8 vs 21 vs missing).
- [ ] Integration: `exec --lang kotlin --dry-run`, shim `extra_env` contains expected `JAVA_HOME`.
- [ ] Manual smoke matrix (Windows + one Unix) recorded below.

## Acceptance criteria

- With **Java `current` set** to a compatible JDK, `envr install kotlin <ver>` + `envr use kotlin <ver>` + `envr exec --lang kotlin -- kotlinc -version` succeed.
- With **Java missing** or **below `java_min_major`**, behavior matches configured policy (warn vs error) with actionable messages.
- **GUI** shows Kotlin versions and **host JDK** metadata without duplicating the Java editor.
- **`cargo test --workspace`** green after registry/help/locale updates.

## Risks & watchlist

| Risk | Mitigation |
|------|------------|
| Two indices (GitHub API vs installable assets) | Single installable row list; cap/pagination documented (playbook §2 GitHub note). |
| Kotlin native vs JVM-only | MVP scope: **JVM compiler bundle** only unless ADR extended. |
| `JAVA_HOME` vs `JDK_JAVA_OPTIONS` | Start with ADR-mandated `JAVA_HOME`; add knobs only if real failures. |
| GUI row without Java | Clear CTA to install/use Java; link to Java tab. |

## Open questions (carry from ADR until closed)

1. Exact **release feed** and **OS/arch matrix** for MVP.
2. Shim surface beyond `kotlin` / `kotlinc`.
3. `.envr.toml` **only Kotlin pin** — confirm preflight uses **effective** Java global current.
4. Settings key names for host mismatch policy.

## Development log

- **2026-04-19:** Integration plan created; ADR-0001 Accepted; playbook §2.1 linked.

## Manual verification (fill in during QA)

| Step | Command / action | Result |
|------|-------------------|--------|
| 1 | | |
| 2 | | |
