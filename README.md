# envr

English | [简体中文](README.zh-CN.md)

`envr` is a Rust-based language runtime manager for developers and automation.
It installs and switches toolchain/runtime versions, resolves project pins from `.envr.toml`, and provides script-friendly CLI output for CI and other tools.

The project is currently pre-1.0. The CLI surface is already broad, but contracts and behavior may still change while the workspace is being stabilized.

## Platform status

- **Current first-release target:** Windows x86_64.
- **CLI:** the Rust CLI workspace is intended to build on Windows, Linux, and macOS, but packaged end-user releases are not yet published as a stable multi-platform channel.
- **GUI:** `envr-gui` is built on cross-platform Rust GUI/runtime libraries, so the implementation is not designed as Windows-only. In principle, the GUI should remain portable to most desktop platforms supported by the current `iced`/`wgpu`/native-dialog stack, but only Windows packaging and release validation are currently in scope.
- **Linux/macOS:** source builds are part of the intended technical direction, but packaged releases and platform-specific support promises are not yet declared.
- **Runtime providers:** support is runtime-by-runtime and host-dependent; some providers are cross-platform, while others currently have Windows-only or partial host coverage.

See [`docs/runtime/platform-support-matrix.md`](docs/runtime/platform-support-matrix.md) for the current runtime matrix, and [`docs/release/README.md`](docs/release/README.md) for current release packaging scope.

## What envr does

- Installs and manages language runtimes under a single runtime root.
- Switches global defaults with `envr use` and resolves project-local pins with `.envr.toml`.
- Provides shims so tools can find the selected runtime consistently.
- Runs commands inside merged runtime/project environments with `envr exec`, `envr run`, `envr env`, and shell hooks.
- Supports offline-oriented workflows through remote index caching and portable bundles.
- Emits human-readable text by default and JSON envelopes for automation with `--format json`.

## Supported runtimes

`envr` has providers for many runtimes, including Node.js, Python, Java, Kotlin, Scala, Clojure, Groovy, Terraform, Deno, Bun, Dart, Flutter, Go, Rust, Ruby, Elixir, Erlang, PHP, .NET, Zig, Julia, Janet, C3, Babashka, SBCL, Haxe, Lua, Nim, Crystal, Perl, Unison, R, Racket, Elm, Gleam, PureScript, Odin, V, and Luau.

Support varies by operating system, architecture, and upstream release artifacts. See [`docs/runtime/platform-support-matrix.md`](docs/runtime/platform-support-matrix.md) for the current implementation matrix.

## Installation

`envr` does not yet publish a stable multi-platform installation channel for end users.
Today, the supported path is to build from source; Windows packaging docs exist for maintainers preparing release artifacts.

### Build from source

On Windows:

```powershell
cargo build --release -p envr-cli
.\target\release\envr.exe --help
```

On Unix-like systems:

```bash
cargo build --release -p envr-cli
./target/release/envr --help
```

The workspace currently uses Rust 2024 edition and requires Rust **1.88 or newer** (see `rust-version = "1.88"` in [`Cargo.toml`](Cargo.toml)). Older local toolchains will fail before the workspace can build.

### Release packaging status

- End-user install packages are not yet documented as a stable distribution channel.
- Maintainer packaging notes for Windows zip/MSI/setup artifacts live in [`docs/release/README.md`](docs/release/README.md).
- Known release limitations and issues live in [`docs/release/KNOWN-ISSUES.md`](docs/release/KNOWN-ISSUES.md).

## Quick start

```bash
# Show commands and global flags
envr --help

# List remote versions for a runtime
envr remote node

# Install a runtime version
envr install node 22.0.0

# Set the global default
envr use node 22.0.0

# Check the selected version
envr current node

# Create project config in the current directory
envr init

# Run a command in the resolved runtime/project environment
envr exec node -- node --version
```

Use `envr help shortcuts` for built-in argv shortcuts and command aliases.

## Core commands

| Area | Commands |
|---|---|
| Runtime lifecycle | `install`, `use`, `list`, `current`, `uninstall`, `which`, `remote`, `doctor` |
| Project environment | `init`, `check`, `status`, `project`, `why`, `resolve`, `exec`, `run`, `env`, `template`, `shell`, `hook`, `deactivate` |
| Configuration | `config`, `alias`, `profile`, `import`, `export` |
| Data and offline workflows | `shim`, `cache`, `bundle`, `prune` |
| Diagnostics and tooling | `debug`, `diagnostics`, `completion`, `help`, `update` |

For the full command map and command tiers, see [`docs/cli/commands.md`](docs/cli/commands.md).

## Automation and JSON output

Most automation-facing commands support the global `--format json` flag:

```bash
envr --format json current node
```

The JSON output is designed as a stable envelope with command-specific `data` payloads. See:

- [`docs/cli/output-contract.md`](docs/cli/output-contract.md)
- [`docs/cli/scripting.md`](docs/cli/scripting.md)
- [`docs/schemas/README.md`](docs/schemas/README.md)

## Configuration, paths, and caches

- User settings are managed by `envr config`.
- Project pins live in `.envr.toml`.
- Runtime installs, shims, cache entries, and remote indexes live under the runtime root.

Relevant docs:

- [`docs/cli/config.md`](docs/cli/config.md)
- [`docs/paths-and-caches.md`](docs/paths-and-caches.md)
- [`docs/cli/offline.md`](docs/cli/offline.md)
- [`docs/cli/bundle.md`](docs/cli/bundle.md)

## Documentation map

Start with [`docs/README.md`](docs/README.md) for the default English documentation guide.
Chinese documentation entry points begin at [`docs/README.zh-CN.md`](docs/README.zh-CN.md).

Quick links:

- CLI usage and recipes: [`docs/cli/README.md`](docs/cli/README.md)
- Runtime support and per-runtime notes: [`docs/runtime/README.md`](docs/runtime/README.md)
- Release notes and known issues: [`docs/release/README.md`](docs/release/README.md)
- Support policy and where to ask for help: [`SUPPORT.md`](SUPPORT.md)
- Security policy and vulnerability reporting: [`SECURITY.md`](SECURITY.md)
- Contribution workflow and governance: [`CONTRIBUTING.md`](CONTRIBUTING.md)
- Architecture notes and ADRs: [`docs/architecture/README.md`](docs/architecture/README.md)
- QA and regression notes: [`docs/qa/README.md`](docs/qa/README.md)
- Diagnostics collection for support issues: [`docs/qa/diagnostics.md`](docs/qa/diagnostics.md)
- Historical refactor notes: [`refactor docs/`](refactor%20docs/)

## Development

Common checks:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo test --workspace
```

CLI-facing changes should also follow [`CONTRIBUTING.md`](CONTRIBUTING.md), especially the JSON contract and governance checklist.

## Community and project policies

- For contribution workflow and maintainer checks, see [`CONTRIBUTING.md`](CONTRIBUTING.md).
- For community behavior expectations, see [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md).
- For ordinary questions, bug reports, feature requests, and support expectations, see [`SUPPORT.md`](SUPPORT.md).
- For issue intake behavior, see [`.github/ISSUE_TEMPLATE/config.yml`](.github/ISSUE_TEMPLATE/config.yml).
- For suspected vulnerabilities, do not open a public issue; follow [`SECURITY.md`](SECURITY.md).

## Project status

`envr` is actively evolving toward a stable CLI and runtime-provider architecture. Expect some documentation under `docs/architecture/`, `refactor docs/`, and `tasks*.md` to be design history or implementation planning rather than end-user documentation.

## License

This workspace is licensed under either of Apache License, Version 2.0 or MIT license at your option. See [LICENSE](LICENSE) for details.
