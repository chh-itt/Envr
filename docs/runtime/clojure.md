# Clojure runtime (envr)

`envr` manages Clojure CLI tools (`clojure` / `clj`) as a first-class runtime key: `clojure`.

## Install and use

```bash
envr remote clojure
envr install clojure 1.12
envr use clojure 1.12.4.1629
```

Common checks:

```bash
envr exec --lang clojure -- clojure -Sdescribe
envr exec --lang clojure -- clj -h
```

## Runtime source and cache

- Upstream source: `clojure/brew-install` GitHub Releases.
- `envr remote clojure -u` forces live refresh.
- Cached release index lives under `cache/clojure/github_releases.json`.
- Unified CLI/GUI list behavior also uses `unified_version_list/full_installable_versions.json` snapshots.

## Java (JVM host) requirement

Clojure is JVM-hosted and requires global Java current:

- Minimum policy: **Java 8+**
- `envr` checks Java compatibility before `install` / `use` and in shim/exec/run/GUI paths.
- If Java is missing or too old, envr returns a friendly actionable message instead of raw JVM startup errors.

Example fix:

```bash
envr install java 21
envr use java 21
envr use clojure ...
```

## PATH proxy and shims

- Managed commands: `clojure`, `clj`.
- Runtime settings: `[runtime.clojure].path_proxy_enabled`.
  - `true`: envr shims dispatch to managed Clojure.
  - `false`: shims bypass to system PATH binaries.

## Project pin

```toml
[runtimes.clojure]
version = "1.12"
```

With pin + install-missing flows (`envr run`, sync/project actions), envr resolves the pinned Clojure line/version from managed installs.
