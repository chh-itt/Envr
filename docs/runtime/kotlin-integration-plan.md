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
5. **Index / artifacts:** **JetBrains GitHub releases API** → `kotlin-compiler-<ver>.zip` per tag; cached under `cache/kotlin/github_releases.json` (TTL via `ENVR_KOTLIN_INDEX_CACHE_TTL_SECS`, default 6h).
6. **Layout:** `runtimes/kotlin/versions/<label>/` with standard **`current`** (symlink or Windows pointer file).
7. **JDK compatibility:** Preflight uses **Java `current` resolution** (same as shims) and parses **major** from the JDK version directory label (`21…` → 21, `1.8…` → 8). MVP minimum: **Java 8+** for all Kotlin lines (no per-Kotlin-line table yet).
8. **Shims:** `resolve_core_shim_command_with_settings` merges **`JAVA_HOME`** from resolved Java home whenever the guest key is `kotlin` (ADR: same source as `java` shims). `exec` / `run` merge the same way in `child_env`.
9. **GUI:** Env Center **PATH proxy** strip for Kotlin (like Lua). **Host subtitle** (`宿主: Java …`) and blocking row CTA are **not** in this MVP (see Development log).
10. **PATH proxy:** `supports_path_proxy: true`; `[runtime.kotlin] path_proxy_enabled` + schema zh template.

## Implementation checklist (playbook + ADR cross-walk)

### Phase 0 — Domain & policy

- [x] `RuntimeKind::Kotlin` + `RUNTIME_DESCRIPTORS` entry (`label_en` / `label_zh`, flags).
- [x] `RuntimeDescriptor::host_runtime` + **acyclic** validation in domain tests (MVP: single hop only).
- [x] `parse_runtime_kind("kotlin")`, `version_line_key_for_kind` → **major.minor** (e.g. `2.0.21` → `2.0`).
- [x] Settings: `[runtime.kotlin] path_proxy_enabled` + schema template (zh).
- [ ] JDK mismatch policy (`warn` | `error`) as a dedicated settings enum (still **hard error** below Java 8 only).
- [x] `PathProxyRuntimeSnapshot` + `path_proxy_enabled_for_kind(Kotlin)`.

### Phase 1 — Provider crate `envr-runtime-kotlin`

- [x] Crate: `index` (GitHub releases JSON), `manager` (install zip, promote, `kotlin_installation_valid`).
- [x] `RuntimeProvider` impl + **`default_provider_boxes`** (`envr-core` `service.rs`).
- [x] **Preflight:** `install` / `set_current` require resolvable Java home + major ≥ 8.

### Phase 2 — Resolver / exec / run / child env

- [x] `RUN_STACK_LANG_ORDER` (**kotlin** after **java**), `RUNTIME_PLAN_ORDER`, `resolve_run_lang_home` / `resolve_exec_lang_home` delegation.
- [x] `runtime_bin_dirs_for_key("kotlin", …)` → `bin/`.
- [x] **`JAVA_HOME`** for `exec --lang kotlin` and for **kotlin** layer in `run` when Java resolves.

### Phase 3 — Shims

- [x] `CoreCommand::Kotlin` / `Kotlinc`, parser, `core_tool_executable`, PATH-proxy bypass stems.
- [x] **`JAVA_HOME`** merged in `resolve_core_shim_command_with_settings` for `kotlin` (not only via `runtime_home_env_for_key("kotlin")`).
- [x] `shim_service` stems.

### Phase 4 — CLI / GUI / registry / locales

- [x] CLI: `remote` / `list` / `shim` / `bundle` parity lists; `help_registry/table.inc` + `root.rs` doc strings mention `kotlin`.
- [x] GUI: Env Center Kotlin settings (PATH proxy), `runtime_layout` default count **19**, JDK↔Kotlin **compat hint** (install/switch/kind pick).
- [ ] `envr doctor`: Kotlin+Java sanity row (optional follow-up).

### Phase 5 — Docs & tests

- [x] User doc [`kotlin.md`](kotlin.md) (install limits, Java requirement, pins, `remote` UX, perf note).
- [x] Domain tests: descriptor / `version_line_key` / host acyclicity + `kotlin_java` JDK/Kotlin combo tests.
- [ ] Integration test: `exec --lang kotlin --dry-run` with temp roots (optional follow-up).
- [ ] Manual smoke matrix (Windows + one Unix) recorded below.

## Acceptance criteria

- With **Java `current` set** to a compatible JDK, `envr install kotlin <ver>` + `envr use kotlin <ver>` + `envr exec --lang kotlin -- kotlinc -version` succeed.
- With **Java missing** or **below Java 8**, install/use preflight fails with an actionable message.
- **`cargo test --workspace`** green.

## Risks & watchlist

| Risk | Mitigation |
|------|------------|
| Two indices (GitHub API vs installable assets) | Only rows with matching `kotlin-compiler-{label}.zip` asset. |
| Kotlin native vs JVM-only | MVP scope: **JVM compiler bundle** only unless ADR extended. |
| `JAVA_HOME` vs `JDK_JAVA_OPTIONS` | Start with ADR-mandated `JAVA_HOME`; add knobs only if real failures. |
| GUI row without Java | Subtitle/CTA deferred; user still sees Java tab separately. |

## Open questions (carry from ADR until closed)

1. Per-Kotlin-line **`java_min_major`** table vs static minimum.
2. Shim surface beyond `kotlin` / `kotlinc`.
3. `.envr.toml` **only Kotlin pin** — preflight uses **effective** Java via `resolve_runtime_home_for_lang` (global `current` + project pin for `java` when present).

## Development log

- **2026-04-19:** Integration plan created; ADR-0001 Accepted; playbook §2.1 linked.
- **2026-04-19 (implementation):**
  - **Friction — `runtime_home_env_for_key("kotlin")`:** left empty on purpose; **`JAVA_HOME` is merged in the shim resolver and in `child_env`** next to Kotlin home resolution so it always matches the Java resolver (ADR) without overloading `runtime_home_env_for_key` with `ShimContext`.
  - **Friction — `RUN_STACK_LANG_ORDER`:** **java** must appear **before** **kotlin** so `collect_run_env` can reuse Java’s `JAVA_HOME` when both layers resolve; extra merge for `exec --lang kotlin`-only and kotlin-only pins when Java layer is skipped is handled explicitly in `child_env`.
  - **Follow-up — upper JDK vs bundled compiler:** `envr-domain::kotlin_java` adds a **Kotlin 2.0.x → JDK ≤24** heuristic (directory labels) after reports that **JDK 25+** can crash kotlinc startup; enforced in manager preflight, shim resolve, `exec`/`run` env build, and GUI **`check_kotlin_jdk_compat`** after pick/install/use.
  - **CLI `remote <kind>` cold UX:** single-runtime **`envr remote kotlin`** with empty cache now **blocks on `list_remote` once** (same data path as `-u`) instead of printing empty rows.
  - **GUI:** hub-row **「宿主: Java xx」** subtitle still optional; compat card covers JDK-too-new vs Kotlin instead.

## Manual verification (fill in during QA)

| Step | Command / action | Result |
|------|-------------------|--------|
| 1 | | |
| 2 | | |
