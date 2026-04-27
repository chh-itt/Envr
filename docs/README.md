# Documentation guide

This directory contains the public and maintainer documentation for `envr`.
If you are new to the project, start with the root [`README.md`](../README.md).

## Information architecture

Docs are organized by audience and stability:

| Layer | Audience | Stability | Where |
|---|---|---|---|
| Product docs | Users, operators, CI authors | Intended to match current behavior | [`cli/`](cli/), [`runtime/*.md`](runtime/), [`paths-and-caches.md`](paths-and-caches.md), [`release/`](release/) |
| Integration contracts | Tooling authors, maintainers | Stable enough for tests and scripts | [`cli/output-contract.md`](cli/output-contract.md), [`schemas/`](schemas/) |
| Contributor docs | Contributors and maintainers | Current process guidance | [`../CONTRIBUTING.md`](../CONTRIBUTING.md), [`qa/`](qa/), [`i18n/`](i18n/), [`perf/`](perf/) |
| Design history and plans | Maintainers | May be historical, draft, or partially implemented | [`architecture/`](architecture/), [`runtime/*-integration-plan.md`](runtime/), [`../refactor docs/`](../refactor%20docs/) |

## Recommended reading paths

### End users

- Command overview: [`cli/README.md`](cli/README.md), [`cli/commands.md`](cli/commands.md)
- Common workflows: [`cli/recipes.md`](cli/recipes.md)
- Configuration: [`cli/config.md`](cli/config.md)
- Offline usage and bundles: [`cli/offline.md`](cli/offline.md), [`cli/bundle.md`](cli/bundle.md)
- Paths, caches, and runtime root layout: [`paths-and-caches.md`](paths-and-caches.md)
- Platform/runtime coverage: [`runtime/README.md`](runtime/README.md), [`runtime/platform-support-matrix.md`](runtime/platform-support-matrix.md)
- Release notes and known issues: [`release/README.md`](release/README.md)
- Support and issue reporting: [`../SUPPORT.md`](../SUPPORT.md)
- Security policy and vulnerability reporting: [`../SECURITY.md`](../SECURITY.md)

### Script and CI authors

- CLI output contract: [`cli/output-contract.md`](cli/output-contract.md)
- JSON schemas: [`schemas/README.md`](schemas/README.md)
- Scripting workflows: [`cli/scripting.md`](cli/scripting.md)
- Offline and bundle workflows: [`cli/offline.md`](cli/offline.md), [`cli/bundle.md`](cli/bundle.md)

### Contributors

- Contribution workflow: [`../CONTRIBUTING.md`](../CONTRIBUTING.md)
- Architecture index: [`architecture/README.md`](architecture/README.md)
- Runtime integration docs: [`runtime/README.md`](runtime/README.md), [`architecture/new-runtime-playbook.md`](architecture/new-runtime-playbook.md)
- QA docs: [`qa/README.md`](qa/README.md)
- i18n docs: [`i18n/README.md`](i18n/README.md)
- Performance docs: [`perf/README.md`](perf/README.md)

## Directory overview

- [`cli/`](cli/) — CLI behavior, command reference, output contract, config, scripting, offline/bundle docs.
- [`runtime/`](runtime/) — per-runtime user notes, platform matrix, and integration plans.
- [`release/`](release/) — release notes, platform release notes, known issues, and packaging notes.
- [`architecture/`](architecture/) — design notes, ADRs, migration plans, and blueprints.
- [`perf/`](perf/) — performance notes and investigations.
- [`qa/`](qa/) — regression, bug triage, and diagnostics reproduction notes.
- [`i18n/`](i18n/) — glossary, style guidance, and allowlists.
- [`schemas/`](schemas/) — schema documentation and generated contract references.

## Document maturity rules

- User-facing docs should describe current behavior, not plans.
- Files ending in `*-integration-plan.md`, `*-blueprint.md`, `*-draft.md`, and most files under `architecture/` are maintainer-facing.
- If a plan doc conflicts with a user doc, prefer the user doc only after checking implementation and tests.
- If implementation changes external behavior, update the product docs in the same change.

When in doubt, link users to the root [`README.md`](../README.md) and keep draft/history material behind contributor-facing indexes.
