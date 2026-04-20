# Clojure integration plan (Clojure CLI tools via GitHub releases)

## Goal

Add **`RuntimeKind::Clojure`** as a first-class runtime (CLI / GUI / shims / pins / `exec` / `run`), installing official Clojure CLI tools from **`clojure/brew-install`** releases into:

`runtimes/clojure/versions/<version>/` with `runtimes/clojure/current`.

Clojure is JVM-hosted: descriptor `host_runtime = Java`, `JAVA_HOME` merged in shim/exec/run, and Java compatibility checks surfaced through the shared JVM-hosted compatibility abstraction.

## Scope & non-goals

- **In scope:** Clojure CLI toolchain releases (`clojure-tools.zip` / `clojure-tools-<ver>.tar.gz`) from `clojure/brew-install`.
- **Out of scope:** Leiningen/specific build tools as managed runtimes, ClojureScript toolchains, Maven/Gradle project-specific bootstrap.

## Version/index shape

- **Remote source:** GitHub Releases API  
  `https://api.github.com/repos/clojure/brew-install/releases?per_page=100`
- **Installable row:** `(version_label, download_url)` where:
  - `version_label` comes from release `tag_name` (strip optional `v`);
  - asset URL prefers host-appropriate tool bundle (`clojure-tools.zip` on Windows, `clojure-tools-<ver>.tar.gz` on Unix, with fallback).
- **Cache:** `{runtime_root}/cache/clojure/github_releases.json` (TTL env knob, default 6h).

## Host JDK policy

- **Minimum:** Java **8+**.
- **JVM-family abstraction:** call sites should route through `envr_domain::jvm_hosted` helpers so Kotlin/Scala/Clojure keep one behavior surface.
- **Current assumption:** no Clojure-specific max-JDK cap in envr policy unless field reports justify adding one.

## Architecture / abstraction friction log

1. **JVM-hosted duplication risk:** every new hosted runtime currently touches shim + child_env + GUI checks; prefer central helpers (`jvm_hosted`) and avoid per-runtime copy branches.
2. **Asset naming asymmetry:** Clojure release asset names are less semver-embedded than Scala/Kotlin (`clojure-tools.zip` vs versioned names), so resolver must not overfit one filename shape.
3. **Remote-cache parity:** ensure plain `envr remote clojure` and `envr remote clojure -u` converge via unified full-installable snapshot persistence.
4. **Playbook drift:** JVM guidance should explicitly mention adding runtime key support in `jvm_hosted` helper, not only runtime-specific policy modules.

## Implementation checklist

### Phase A — Domain

- [x] Add `RuntimeKind::Clojure`, descriptor (`key: clojure`, `host_runtime: Java`, remote + path proxy flags).
- [x] Include Clojure in `version_line_key_for_kind` (major.minor lines).
- [x] Extend descriptor tests/count + host acyclicity assertions.
- [x] Extend `envr_domain::jvm_hosted` for `"clojure"`.

### Phase B — Provider crate `envr-runtime-clojure`

- [x] Create crate + provider implementation (`list_installed/current/set_current/list_remote/resolve/install/uninstall`).
- [x] Parse releases and select installable assets by host.
- [x] Validate install tree has runnable `clj` + `clojure` launchers.
- [x] Enforce Java preflight before `install` / `set_current`.

### Phase C — Core/CLI/resolver/shims

- [x] Register provider in `RuntimeService` + `envr-core/Cargo.toml`.
- [x] Shim commands: `clj`, `clojure`.
- [x] `runtime_bin_dirs_for_key` + `runtime_home_env_for_key` support (`CLOJURE_HOME` optional, `JAVA_HOME` via JVM-hosted merge).
- [x] `child_env` and run stack include `clojure` + `ENVR_CLOJURE_VERSION`.
- [x] Add to `missing_pins`, bundle/list/status parity.

### Phase D — Config/GUI

- [x] `[runtime.clojure] path_proxy_enabled` in settings + snapshot/schema wiring.
- [x] Env Center runtime panel + JVM-hosted Java hint/error behavior.
- [x] Ensure runtime layout/tests count update.

### Phase E — Docs/playbook polish

- [x] Add `docs/runtime/clojure.md` (install, JDK, remote/cache behavior, pins, commands).
- [x] Update playbook JVM section for Clojure and `jvm_hosted` extension checklist.
- [x] Append development friction outcomes in this plan.

## QA notes (to fill while validating)

- CLI smoke: `envr remote clojure`, `envr remote clojure -u`, `envr install clojure <ver>`, `envr use clojure <ver>`, `envr exec --lang clojure -- clojure -Sdescribe`, `envr exec --lang clojure -- clj -h`.
- GUI smoke: Clojure tab remote/install/use/current visibility, path-proxy toggle persistence, Java-host hint behavior.

## Development notes (actual)

- Added JVM-hosted matrix entry #3 (`kotlin` / `scala` / `clojure`) in `envr_domain::jvm_hosted`; GUI, shim, and child-env now consume one hosted-runtime gate.
- Clojure release asset mapping required host-specific candidate list because tags are versioned but Windows artifact is stable name (`clojure-tools.zip`).
- GUI friction: env-center still needs explicit message wiring per runtime (`Set*PathProxy`, `*JavaChecked`), though runtime descriptors already expose capability flags.
