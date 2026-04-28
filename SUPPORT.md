# Support

Thanks for using `envr`.

This document explains where to ask questions, where to report bugs, and how to report security issues.

## Before opening an issue

Please check the following first:

- the root [`README.md`](README.md)
- the docs index at [`docs/README.md`](docs/README.md)
- CLI command docs in [`docs/cli/`](docs/cli/)
- runtime support notes in [`docs/runtime/`](docs/runtime/)
- known issues in [`docs/release/KNOWN-ISSUES.md`](docs/release/KNOWN-ISSUES.md)

If you are reporting a CLI contract or automation regression, also review [`CONTRIBUTING.md`](CONTRIBUTING.md) and [`docs/cli/output-contract.md`](docs/cli/output-contract.md).

## Getting help

Use the issue tracker for:

- installation problems
- runtime installation failures
- platform support questions
- unclear documentation
- behavior that looks wrong but may not be a confirmed bug yet
- feature requests and usability feedback

When asking for help, include:

- your OS and architecture
- the `envr` version or commit
- how you installed `envr`
- the runtime kind and version involved
- the exact command you ran
- the output you expected
- the output you actually saw
- whether you are using a custom runtime root, mirror, offline/cache workflow, or project-local `.envr.toml`

For environment, shim, PATH, mirror, offline, or cache issues, also include `envr doctor` output and, when useful, a sanitized `envr diagnostics export` summary. See [`docs/qa/diagnostics.md`](docs/qa/diagnostics.md).

## Reporting bugs

Please open a normal GitHub issue for non-security bugs.

Helpful reports usually include:

- reproduction steps
- expected behavior
- actual behavior
- logs or diagnostics when relevant
- whether the problem is reproducible with a clean runtime root or fresh project directory

If the bug involves machine-readable output, include whether you used:

- `--format json`
- `--porcelain`
- `--quiet`

## Reporting security issues

Do **not** report security vulnerabilities in public issues.

Examples include:

- malicious archive extraction behavior
- checksum or integrity verification bypass
- unsafe mirror or remote metadata trust behavior
- shim or `PATH` behavior that could lead to unintended execution
- local configuration handling that crosses an expected trust boundary
- unexpected exposure of secrets or environment data

Please follow the private reporting instructions in [`SECURITY.md`](SECURITY.md).

## Scope expectations

`envr` is a runtime manager, so support requests often sit in one of these buckets:

- `envr` bug
- unsupported host/platform combination
- upstream runtime packaging change
- mirror/index/network issue
- project configuration issue
- local environment or shell integration issue

Maintainers may redirect a report if the root cause is upstream or outside the supported scope.

## Response expectations

Support is best effort.

- Questions and bug reports may not receive immediate replies.
- Security reports should follow the timing expectations in [`SECURITY.md`](SECURITY.md).
- Pre-1.0 behavior may still change, especially in advanced automation and newer runtime providers.

## Documentation feedback

If the main problem is that the docs are confusing, incomplete, or outdated, opening an issue is encouraged.
Documentation problems are valid support issues.
