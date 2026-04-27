# Dependency security and licenses (T904)

This document matches the workspace policy in `Cargo.toml` (`[workspace.metadata.envr.dependency_policy]`) and `refactor docs/07-依赖选择与原则.md`.

## Tools

| Tool | Purpose |
|------|---------|
| [`cargo-deny`](https://embarkstudios.github.io/cargo-deny/) | License allow-list, duplicate crate warnings, advisory DB (RustSec), yanked crates |
| [`cargo-audit`](https://github.com/RustSec/rustsec/tree/main/cargo-audit) | Advisory-only scan (optional; overlaps advisories in `cargo-deny`) |

Configuration: repository root **`deny.toml`**.

## Local commands

```bash
# Recommended full check:
cargo deny check

# If you need to isolate categories while debugging policy failures:
cargo deny check licenses bans sources advisories

# Optional advisory-only:
cargo install cargo-audit
cargo audit
```

`envr` now expects a recent `cargo-deny` release that can parse the current advisory database, including RustSec entries that use newer metadata.

## CI

Pull requests and `main` run **`cargo deny check`** in CI. The release workflow also runs a full `cargo deny check` before packaging artifacts.

## Upgrade plan when advisories fire

1. Run `cargo deny check` and note crate + advisory id.
2. Prefer **patch semver** updates (`cargo update -p <crate>`) that resolve the advisory.
3. If the fixed release is breaking, evaluate: isolate the crate behind a feature, replace the dependency, or document a time-boxed exception in `deny.toml` `ignore` with issue link.
4. Re-run `cargo deny check` and workspace tests before merge.

## Licenses

Allowed SPDX identifiers live in `deny.toml` under `[licenses] allow`. Adding a new license requires architect/owner review and an update to this section.

## Temporary advisory exceptions

The following RustSec advisories are currently ignored in `deny.toml` as an explicit risk-acceptance decision:

- `RUSTSEC-2024-0384` (`instant`)
  - Source: transitive dependency from the `iced` GUI stack.
  - Current reason: no compatible safe upgrade path is available in the current GUI dependency set.
  - Acceptance scope: accepted only while `envr` continues to depend on the affected `iced` stack.
  - Review plan: re-check on every GUI dependency upgrade and before each release cut.

- `RUSTSEC-2024-0436` (`paste`)
  - Source: transitive dependency from the `wgpu` / `metal` GUI stack.
  - Current reason: no compatible safe upgrade path is available in the current GUI dependency set.
  - Acceptance scope: accepted only while `envr` continues to depend on the affected graphics stack.
  - Review plan: re-check on every GUI dependency upgrade and before each release cut.

These ignores should be removed as soon as the GUI stack can be upgraded to versions that eliminate or replace the affected crates.
