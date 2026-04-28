# Diagnostics and support data

This document explains what maintainers expect when asking for `envr doctor` or `envr diagnostics export` output in support issues.

## `envr doctor`

`envr doctor` performs environment and runtime-root checks.
It is useful for reports involving installation, shims, PATH setup, current runtime pointers, and cache/runtime-root layout.

Common commands:

```bash
envr doctor
envr doctor --format json
envr doctor --fix-path
envr doctor --runtime-root <path>
```

Notes:

- `--format json` is preferred for automation or reproducible bug reports.
- `--fix` may perform safe repairs; mention if you ran it before collecting diagnostics.
- `--fix-path` prints commands for adding shims to PATH; review commands before applying them.
- On Windows, `--fix-path-apply` may interactively update the user PATH when used with `--fix-path`.

## `envr diagnostics export`

`envr diagnostics export` writes a diagnostic zip containing support data such as:

- `doctor.json`
- `system.txt`
- `environment.txt`
- recent `*.log` files when available

Common command:

```bash
envr diagnostics export
```

When filing a public issue, do not upload diagnostics blindly.
First review the archive and remove secrets, private paths, tokens, internal mirror URLs, or other sensitive data.
If you cannot share the archive, provide the path where it was generated and summarize relevant findings.

## What to include in bug reports

For runtime installation, shim, PATH, mirror, offline, or cache bugs, include:

- OS and architecture
- `envr --version` or commit SHA
- install method for `envr`
- exact command that failed
- runtime kind and version
- whether mirror/custom source settings are enabled
- whether offline mode, cached indexes, or bundle workflows are involved
- `envr doctor` output, preferably JSON when practical
- `envr diagnostics export` path or a sanitized summary

## Security note

If diagnostics reveal a suspected vulnerability, do not post details publicly.
Follow [`../../SECURITY.md`](../../SECURITY.md) instead.
