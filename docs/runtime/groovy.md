# Groovy runtime support

`envr` supports Groovy as a managed runtime (`RuntimeKind::Groovy`), including:

- remote version discovery (`envr remote groovy`)
- managed install/use (`envr install groovy <spec>`, `envr use groovy <version>`)
- shim commands (`groovy`, `groovyc`)
- `exec` / `run` environment merging (`GROOVY_HOME`, `JAVA_HOME`)

## Remote source and cache

- Primary source: Apache distribution index `https://dlcdn.apache.org/groovy/`
- Fallback source: Apache archive index `https://archive.apache.org/dist/groovy/`
- Install artifact pattern: `distribution/apache-groovy-binary-<version>.zip`

Cache location:

- `{runtime_root}/cache/groovy/index_rows.json`

TTL environment variable:

- `ENVR_GROOVY_INDEX_CACHE_TTL_SECS` (default 21600 seconds / 6h)

Optional source overrides:

- `ENVR_GROOVY_PRIMARY_INDEX_URL`
- `ENVR_GROOVY_ARCHIVE_INDEX_URL`

## Java compatibility

Groovy is JVM-hosted. Current envr policy:

- Groovy 4.x+ requires Java 11+
- Older lines default to Java 8+

Before installing/using Groovy, ensure a global Java current version is selected:

```powershell
envr install java 21
envr use java 21
```

## PATH proxy toggle

`settings.toml`:

```toml
[runtime.groovy]
path_proxy_enabled = true
```

When disabled, `groovy` / `groovyc` shims passthrough to system PATH, and managed "Use / Install & Use" actions are blocked in GUI for safety.

## Quick checks

```powershell
envr remote groovy
envr remote groovy -u
envr install groovy 4.0
envr use groovy 4.0.31
envr exec --lang groovy -- groovy --version
envr exec --lang groovy -- groovyc --version
```
