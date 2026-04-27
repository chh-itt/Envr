# Architecture and design notes

This directory contains internal design material for `envr` contributors.
These files are primarily for maintainers, not end users.

## What belongs here

- ADRs and design decisions
- Internal blueprints for subsystems
- Refactoring notes and migration plans
- Implementation drafts for features that may still evolve

## Reading order for contributors

| Document | Purpose |
|---|---|
| [`new-runtime-playbook.md`](new-runtime-playbook.md) | Operational guide for adding a new runtime provider. |
| [`runtime-descriptor-refactor.md`](runtime-descriptor-refactor.md) | Runtime descriptor structure and refactor notes. |
| [`download-control-plane-blueprint.md`](download-control-plane-blueprint.md) | Download architecture blueprint. |
| [`structured-runtime-errors-blueprint.md`](structured-runtime-errors-blueprint.md) | Error model design notes. |
| [`adr-0001-runtime-host-dependencies-kotlin.md`](adr-0001-runtime-host-dependencies-kotlin.md) | Recorded design decision example. |

## Maturity guidance

These documents are not guaranteed to describe the current shipping behavior exactly.
When docs here disagree with implementation:

1. Prefer code and tests.
2. Update the relevant user-facing docs first if behavior changed externally.
3. Then refresh or archive the architecture note.

## Related directories

- Runtime-specific design plans live in [`../runtime/`](../runtime/), usually as `*-integration-plan.md` files.
- Historical refactor context lives in [`../../refactor docs/`](../../refactor%20docs/).
