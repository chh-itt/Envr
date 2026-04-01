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
# Recommended (works across cargo-deny versions; see advisories note below):
cargo deny check licenses bans sources

# When your cargo-deny supports the advisory DB in use (CVSS 4.0 in newer RustSec entries):
cargo deny check

# Optional advisory-only:
cargo install cargo-audit
cargo audit
```

**Advisories:** Some RustSec advisories use CVSS 4.0 metadata. Older `cargo-deny` releases fail to parse the database; CI therefore runs `licenses bans sources` only. Upgrade `cargo-deny` locally when you want a full advisory scan, or use `cargo audit`.

## CI

Pull requests and `main` run **`cargo deny check licenses bans sources`** (see `.github/workflows/ci.yml`). Full advisories are run locally when possible (`cargo deny check advisories` or `cargo audit`).

## Upgrade plan when advisories fire

1. Run `cargo deny check` and note crate + advisory id.
2. Prefer **patch semver** updates (`cargo update -p <crate>`) that resolve the advisory.
3. If the fixed release is breaking, evaluate: isolate the crate behind a feature, replace the dependency, or document a time-boxed exception in `deny.toml` `ignore` with issue link.
4. Re-run `cargo deny check` and workspace tests before merge.

## Licenses

Allowed SPDX identifiers live in `deny.toml` under `[licenses] allow`. Adding a new license requires architect/owner review and an update to this section.

## Temporary advisory exceptions

- `RUSTSEC-2024-0384` (`instant`) and `RUSTSEC-2024-0436` (`paste`) are currently ignored in `deny.toml` because they arrive transitively from the `iced/wgpu` GUI stack and cargo-deny reports no safe upgrade path today.
- Remove these ignores when the GUI stack is upgraded to versions that eliminate those crates.
