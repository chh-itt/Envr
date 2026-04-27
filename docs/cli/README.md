# CLI documentation

This directory contains user-facing command docs plus automation and maintainer references for the `envr` CLI.

## Start here

| Document | Audience | Purpose |
|---|---|---|
| [`commands.md`](commands.md) | Users | Command map grouped by lifecycle, project workflow, automation, and diagnostics. |
| [`recipes.md`](recipes.md) | Users | Task-oriented examples for common workflows. |
| [`config.md`](config.md) | Users | `settings.toml`, `envr config`, mirrors, paths, and preferences. |
| [`scripting.md`](scripting.md) | Users / CI | Using `envr` in scripts and subprocess workflows. |
| [`offline.md`](offline.md) | Users / CI | Offline index and cache behavior. |
| [`bundle.md`](bundle.md) | Users / CI | Portable offline bundles. |
| [`output-contract.md`](output-contract.md) | Integrators | Text/JSON output contract and error envelope expectations. |

## Maintainer references

| Document | Purpose |
|---|---|
| [`automation-matrix.md`](automation-matrix.md) | Checklist for command output modes and automation behavior. |
| [`v1.0-definition.md`](v1.0-definition.md) | Product definition and acceptance scope for the v1.0 target. |
| [`v1.0-metrics.md`](v1.0-metrics.md) | Suggested metrics and observability signals for v1.0 readiness. |

## Documentation status

- The first table is intended to be stable enough for users and external integrators.
- Maintainer references may include planned or aspirational details; verify against current CLI behavior with `envr --help` and tests.
