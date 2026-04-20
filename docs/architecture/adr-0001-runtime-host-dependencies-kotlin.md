# ADR-0001: Runtime host dependencies & Kotlin on the JVM

- **Status:** Accepted  
- **Date:** 2026-04-19  
- **Scope:** Product model + domain/shim/install/GUI contracts for **JVM-hosted languages**, with **Kotlin** as the first consumer and **Scala** (`RuntimeKind::Scala`, `scala/scala3` releases) as the second (same `host_runtime = Java`, `JAVA_HOME` merge, install/use preflight).  
- **Related:** `docs/architecture/runtime-descriptor-refactor.md`, `docs/architecture/new-runtime-playbook.md` §2.1, `envr-domain::RuntimeDescriptor`, `envr-shim-core::runtime_home_env_for_key` (today sets `JAVA_HOME` only for key `"java"`).

---

## Context

1. **User mental model**  
   Users expect `envr install kotlin` / `envr use kotlin` to work like other first-class runtimes, not “a hidden subfolder of Java”.

2. **Technical reality**  
   Kotlin/JVM toolchains **need a JDK** at execution time (and often at compile time). The JDK version must be **compatible** with the Kotlin distribution (minimum supported Java version, sometimes maximum for older stacks).

3. **Existing envr shape**  
   - Each `RuntimeKind` maps to a **descriptor** (`RuntimeDescriptor`) and usually an **`envr-runtime-*`** provider.  
   - Java shims already inject **`JAVA_HOME`** pointing at the resolved Java **home** (not only `bin`). See `runtime_home_env_for_key` for `"java"` in `envr-shim-core`.

4. **Why an ADR**  
   Adding Kotlin touches **descriptor catalog**, **install preconditions**, **shim env composition**, **GUI hub rows**, and possibly **resolver/pins**. Doing this ad hoc risks drift between CLI and GUI and between “installed” vs “runnable”.

---

## Decision (summary)

| Topic | Decision |
|--------|-----------|
| **RuntimeKind** | **Kotlin is its own `RuntimeKind::Kotlin`.** It is a first-class runtime in CLI/GUI/registry/shim vocabulary. |
| **Dependency on Java** | **Declared dependency** from Kotlin → Java (host), enforced in **install / use / exec** paths as specified below—not an implicit “hope PATH works”. |
| **Where dependency lives** | Extend **`RuntimeDescriptor`** (or a closely linked static table in `envr-domain`) with a **small, declarative host field**. |
| **JDK compatibility** | **Preflight + policy**: detect global Java `current` (or pin-resolved Java) version; compare to Kotlin’s **documented minimum** (and optional max if needed); emit **warn** or **hard error** per setting. |
| **Shims** | For `kotlin` / `kotlinc` (names TBD), **compose `extra_env`**: keep Kotlin’s own home-derived vars if any, and **set `JAVA_HOME` to the resolved Java home** (same semantics as Java shims today). |
| **GUI** | On the Kotlin hub row, show **secondary line or muted text**, e.g. **“宿主: Java 21”** (i18n), without expanding the full JDK panel there. |

---

## Decision detail

### 1. `RuntimeKind::Kotlin` (first-class)

- **Rationale:** Matches install/use vocabulary and keeps metrics, cache keys, and GUI tabs consistent with other languages.  
- **Implementation note:** Internal crates may call into **`envr-runtime-java`** helpers** (or shared `envr-platform` “JDK layout” helpers) without exposing “Kotlin is a subcommand of Java” in the UI.

### 2. Declaring “Kotlin requires Java”

**Proposal (evolution-friendly):**

- **Phase A (Kotlin-only):** add a single optional host on the descriptor, e.g.  
  `host_runtime: Option<RuntimeKind>`  
  where **`Some(RuntimeKind::Java)`** for Kotlin.

- **Phase B (if needed):** generalize to **ordered list**  
  `host_runtimes: &'static [RuntimeKind]`  
  when a future runtime needs more than one host (rare), or needs **ordered** resolution (e.g. “Java then something else”).  
  Empty slice = no host dependency.

**Why not stop at `Option` forever?**  
`Option<RuntimeKind>` is enough for Kotlin and most JVM cousins **if** we only ever need “one JDK”. If we later need “Java + something else”, a slice avoids another breaking rename.

**Rules:**

- **Acyclic:** `host_runtime(s)` must not introduce cycles (Kotlin → Java → Kotlin). Validation in domain or resolver startup.  
- **Resolution order:** When installing or activating Kotlin, **resolve Java first** (global `current` or project pin, same rules as today for `exec --lang java`).

### 3. Install / switch preflight (“JDK incompatible?”)

**Minimum viable behavior (MVP):**

1. **Before** `install kotlin` / `use kotlin` / first `exec` with Kotlin pin:  
   - Resolve **effective Java home** (same as shim would for `java`).  
   - Read **Java major version** (parse `release` file or `java -version` / equivalent—implementation detail in ADR implementation phase).

2. Compare to **Kotlin distribution metadata** (static table or embedded in index): **`java_min_major`**, optionally **`java_max_major`**.

3. **Policy knob** (settings or env, exact name TBD):  
   - **`warn`** (default): print / log structured warning; still proceed if user insists (`--force` optional).  
   - **`error`**: refuse `use` / refuse starting shim for Kotlin until Java is upgraded.

**Transparency first:** even when default is `warn`, messages must state **observed Java version**, **required minimum**, and **how to fix** (`envr use java …`).

### 4. Shims: `JAVA_HOME` without duplicating Java logic

**Intent:** Kotlin binaries must see the **same** `JAVA_HOME` as `java` / `javac` shims for the **currently selected Java**.

**Approach:**

- Introduce (or reuse) a **single internal helper** used by shim resolution, e.g.  
  `fn java_home_for_effective_selection(ctx: &ShimContext) -> EnvrResult<PathBuf>`  
  mirroring today’s Java home resolution.

- For **`runtime_home_env_for_key("kotlin", kotlin_home)`** (or for `CoreCommand::Kotlin` branch):  
  return **at least**  
  `("JAVA_HOME", java_home_string)`  
  in addition to any Kotlin-specific vars (if any).

- **Path:** `runtime_bin_dirs_for_key` for `"kotlin"` lists Kotlin’s `bin` (and roots if needed), same pattern as other languages.

**Note:** Today `runtime_home_env_for_key` only sets `JAVA_HOME` for **`"java"`**. This ADR explicitly extends the contract: **host env injection may be keyed off the child runtime** when that child declares `host_runtime = Java`.

### 5. GUI

- **Primary:** Kotlin version / current / actions—unchanged pattern vs other runtimes.  
- **Secondary:** one line like `宿主: Java 21` / `Host: Java 21` (major from resolved JDK), driven by the **same resolved Java** as shims.  
- **If Java missing:** show **blocking hint** on Kotlin row (icon + short text + link to install/use Java), not a second full JDK editor.

---

## Alternatives considered

| Alternative | Why not (for MVP) |
|-------------|-------------------|
| Kotlin as subcommand of `java` | Breaks `envr install kotlin` mental model; complicates GUI tabs and metrics. |
| Kotlin bundles its own JBR only | Possible vendor-specific path later; still need a story for “which JBR” and upgrades; does not remove version checks. |
| Implicit “PATH has java” | Non-reproducible; breaks offline/pin story; hard to support in shims. |

---

## Consequences

### Positive

- One **declarative** place for “this runtime needs that host”.  
- Shims and CLI/GUI can share **one resolution path** for Java home.  
- Clear path to **Scala / Clojure / Groovy** by reusing the same host mechanism.

### Negative / cost

- **Resolver & install** must become **dependency-aware** (order, errors, messaging).  
- **Testing matrix** grows: Kotlin × Java major versions × Windows/Linux.  
- **Documentation** must explain host line and policies (`warn` vs `error`).

---

## Implementation phases (non-binding checklist)

1. **Domain:** `RuntimeKind::Kotlin`, descriptor field `host_runtime` / `host_runtimes`, cycle check, helpers `runtime_hosts(kind)`.  
2. **Install service:** topological or fixed order (Java check before Kotlin unpack/commit).  
3. **Shim:** `CoreCommand::Kotlin` / `kotlinc`, `runtime_bin_dirs_for_key`, merged `JAVA_HOME` via shared Java resolution.  
4. **GUI:** host subtitle + empty-state when Java missing.  
5. **Cross-runtime compatibility abstraction:** keep per-runtime policies (e.g. `kotlin_java`, `scala_java`) but route callers through a shared hosted-runtime helper (`envr_domain::jvm_hosted`) to avoid drift across shim/exec/run/GUI code paths.
6. **Docs + playbook:** update `docs/architecture/new-runtime-playbook.md` with “hosted runtime” section.

---

## Open questions (to close before coding)

1. **Kotlin distribution source** (GitHub releases, JetBrains Toolbox API, etc.) and **supported OS matrix** (must match existing envr host policy).  
2. **Exact shim names:** `kotlin`, `kotlinc`, `kapt`? (start minimal.)  
3. **Project pins:** can `.envr.toml` pin **Kotlin without** pinning Java, or do we **auto-pin** Java when Kotlin is pinned? (ADR leans: **allow both**, but preflight always evaluates **effective** Java.)  
4. **Settings key** for `warn` vs `error` (global vs per-runtime).

---

## Stakeholder alignment (discussion recap)

The following stakeholder proposals are **accepted as the default direction** for this ADR, with the small generalizations above (`Option` → possible `&'static [RuntimeKind]` later; centralized `JAVA_HOME` helper; explicit preflight policy).

- Independent **`RuntimeKind::Kotlin`**.  
- **Declarative** host dependency on **Java**.  
- **Preflight** with warn/error configurability.  
- **Shim `JAVA_HOME`** aligned with Java `current`.  
- **GUI** `宿主: Java {major}` as compact metadata.

---

## Status transitions

- **Accepted** (2026-04-19): baseline for Kotlin and future JVM-hosted runtimes.  
- **Superseded** (future): if the host model moves to a richer graph (e.g. version constraints on edges, multiple ordered hosts).
