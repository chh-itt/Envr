# Zig runtime (envr)

envr installs official Zig builds from **`https://ziglang.org/download/index.json`** (cached under your envr runtime root, typically `cache/zig/`).

## Requirements

- Network for `envr remote zig`, `envr install zig …`, and first index fetch.
- A supported host triple; if the index has no build for your OS/arch, install fails with a clear error.

## Common commands

```bash
envr remote zig
envr install zig 0.14.1
envr use zig 0.14.1
envr exec --lang zig -- zig version
```

Partial stable specs (e.g. `0.14` → latest `0.14.x`) follow the same resolve rules as other runtimes when the index allows it.

## Project pin (`.envr.toml`)

```toml
[runtimes.zig]
version = "0.14.1"
```

Then `envr run` / `envr project sync` can treat Zig like other pinned languages.

## Layout

Under `ENVR_RUNTIME_ROOT` (or the default data dir):

- Installed trees: `runtimes/zig/versions/<version>/`
- Global selection: `runtimes/zig/current` (symlink on Unix; pointer file on Windows, same family as Deno)

The `zig` binary is expected at the version root (or under `bin/`), matching the promoted archive layout.

## Environment

- **`envr run`**: Zig’s install directory is prepended on `PATH` when Zig is part of the stack (and optional `ENVR_ZIG_VERSION` for template collection).
- No extra variables are required for a normal `zig` CLI beyond `PATH` (unlike e.g. `JAVA_HOME` / `DOTNET_ROOT`).

## Caching

- Index JSON: disk cache + TTL (override with `ENVR_ZIG_INDEX_CACHE_TTL_SECS` where supported by the provider).
- Remote “latest per major” GUI/CLI cache: `ENVR_ZIG_REMOTE_CACHE_TTL_SECS` where applicable.

## See also

- [Zig integration plan](zig-integration-plan.md) — implementation phases and acceptance checklist.
- [New runtime playbook](../architecture/new-runtime-playbook.md) — how other runtimes are wired.
