# Ruby runtime (MVP) - design and current behavior

This document describes the current `Ruby` integration in envr, including version selection, installation layout, shim routing, and the GUI/CLI behaviors that affect ruby/gem/bundle/irb.

## Scope (MVP)

- Managed runtime kind: `ruby`
- Version spec resolution (examples):
  - `major` (for example: `3`)
  - `major.minor` (for example: `3.3`)
  - full version (for example: `3.3.6`)
- Install / uninstall / current switch under envr runtime root
- Shim execution for:
  - `ruby`
  - `gem`
  - `bundle`
  - `irb`
- Project pin support via `.envr.toml`
- `.ruby-version` fallback when `.envr.toml` does not pin ruby
- PATH proxy setting (envr shims intercept vs bypass to system `PATH`)

## Runtime layout

- Runtime home: `runtimes/ruby`
- Installed versions: `runtimes/ruby/versions/<version>`
- Current pointer: `runtimes/ruby/current`
- Cache: `cache/ruby`

## Version source and precedence

- Project pin (hard precedence):
  - `.envr.toml` key: `[runtimes.ruby].version = "<version spec>"`
- Fallback when project pin is absent:
  - `.ruby-version` in the project working directory
- If both exist and disagree:
  - `.envr.toml` wins (Ruby shim must follow the project pin)

## Path proxy setting

In `settings.toml`:

```toml
[runtime.ruby]
path_proxy_enabled = true
```

- `true`: envr shims manage/route `ruby` / `gem` / `bundle` / `irb`
- `false`: envr bypasses the Ruby shims and resolves these tools from the system `PATH`

## Validation behavior (install)

Install validation requires:

- `ruby` executable exists under the installed home
- `gem` executable exists under the installed home
- `bundle` executable presence is checked explicitly (for the chosen Ruby distribution/layout)
- `ruby --version` succeeds under envr-managed execution

## Known MVP limitations / notes

- Windows distribution uses RubyInstaller artifacts (commonly `.7z`); extraction relies on the platform `bsdtar.exe` tool and includes layout “hoisting” to ensure envr finds executables under the expected paths.
- GUI remote latest-per-major cache behavior for Ruby may still be minimal compared to other runtimes (design note: Ruby remote state was intentionally kept light for MVP).

## Quick manual checks

- `envr current ruby`
- `envr resolve ruby --spec 3.3`
- `envr remote ruby`
- `envr install ruby <spec>`
- `envr use ruby <resolved-version>`
- `envr exec --lang ruby -- ruby --version`
- `envr exec --lang ruby -- gem --version`
- `envr exec --lang ruby -- bundle --version`
- `envr exec --lang ruby -- irb --version`

