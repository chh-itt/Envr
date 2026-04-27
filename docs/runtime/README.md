# Runtime documentation

This directory contains runtime-specific documentation for `envr`.

There are two main document types here:

1. Stable runtime notes, named like `<runtime>.md`
2. Maintainer planning notes, named like `<runtime>-integration-plan.md`

## How to use this directory

### If you are a user

Start with:

- [`platform-support-matrix.md`](platform-support-matrix.md) — current support by OS/host
- The runtime file you care about, such as [`zig.md`](zig.md), [`deno.md`](deno.md), or [`flutter.md`](flutter.md)

Stable runtime docs usually cover:

- what `envr` installs
- host/platform requirements
- common commands
- project pin examples
- install layout and environment variables
- cache behavior

### If you are a contributor

Use the matching `*-integration-plan.md` file when present.
These plan files capture implementation quirks, acceptance checklists, upstream artifact rules, and rollout notes.
They are maintainer-facing and may describe work that is incomplete or intentionally deferred.

## Naming convention

| Pattern | Meaning |
|---|---|
| `<runtime>.md` | User-facing runtime behavior/reference document |
| `<runtime>-integration-plan.md` | Maintainer-facing integration plan or implementation checklist |
| `platform-support-matrix.md` | Cross-runtime support summary |

## Maintenance rule

When changing runtime behavior:

1. Update the user-facing `<runtime>.md` file if external behavior changed.
2. Update `platform-support-matrix.md` if host support changed.
3. Update or close out the matching `*-integration-plan.md` if it is still relevant.
