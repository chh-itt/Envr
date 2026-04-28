# Security policy

English | [简体中文](SECURITY.zh-CN.md)

## Supported versions

`envr` is currently pre-1.0 and does not yet maintain long-term support branches.

At this stage, security fixes are expected to land on the default development branch first.
We generally consider the following versions supported for security reporting and fixes:

| Version | Supported |
|---|---|
| Current default branch / latest unreleased work | yes |
| Latest tagged release, if any | best effort |
| Older tags / historical commits | no |

If a vulnerability is reported against an older revision, maintainers may ask you to verify whether it still reproduces on the latest code before triage is completed.

## Reporting a vulnerability

Please do **not** open a public GitHub issue for suspected security vulnerabilities.

Instead, report vulnerabilities privately using one of the following channels:

1. **GitHub Security Advisory / private vulnerability reporting**, if enabled for this repository.
2. If private advisory reporting is not available, contact the maintainers through a private channel before publishing details.

If you are unsure whether something is security-sensitive, prefer private reporting first.

## What to include in a report

Please include as much of the following as possible:

- affected `envr` version, commit, or branch
- operating system and architecture
- runtime/provider involved
- configuration details relevant to the issue
- exact commands or steps to reproduce
- proof of concept, sample archive, or example mirror/index when safe to share
- impact assessment: code execution, path hijack, archive traversal, checksum bypass, privilege boundary confusion, local secret exposure, denial of service, and so on
- whether the issue depends on a malicious mirror, compromised upstream, local filesystem access, or untrusted project files

## Security-sensitive areas in `envr`

Because `envr` is a runtime manager, we treat the following areas as especially sensitive:

- downloading remote binaries and metadata
- mirror and upstream index selection
- checksum validation and integrity verification
- archive extraction and destination path handling
- shim generation and executable resolution
- `PATH` manipulation and shell integration
- project-local configuration such as `.envr.toml`
- local cache reuse and cache recovery behavior
- diagnostic export and environment/path disclosure

Reports involving any of these areas should be sent privately.

## Disclosure policy

Please allow maintainers reasonable time to investigate and prepare a fix before public disclosure.

Our preferred process is:

1. private report received
2. maintainer triage and impact confirmation
3. fix prepared and reviewed
4. release or advisory published
5. coordinated public disclosure

We ask reporters not to publish exploit details before a fix or mitigation is available, unless coordinated with maintainers.

## Expected response times

Current response targets are best effort:

- initial acknowledgement: within **7 calendar days**
- first triage update: within **14 calendar days**
- status updates for confirmed issues: at reasonable intervals until resolution

Complex issues involving upstream runtime providers, mirrors, archive formats, or cross-platform behavior may take longer to resolve.

## Scope notes

### In scope

Examples of issues likely to be treated as security vulnerabilities:

- archive extraction path traversal or overwrite outside intended directories
- checksum verification bypass or integrity downgrade
- unsafe trust of unverified mirrors or metadata leading to unintended binary execution
- shim or `PATH` behavior that enables unexpected command hijacking beyond documented trust assumptions
- unsafe handling of project-local config that can trigger privileged or surprising execution flows
- information leaks through diagnostics or exported configuration beyond intended behavior

### Usually out of scope or lower severity

These may still be bugs, but are not always treated as security issues by themselves:

- failures that require the user to intentionally run an obviously malicious local binary
- behavior fully explained by a hostile local administrator or already-compromised machine
- denial of service limited to deleting the user's own cache or runtime root
- breakage caused only by unsupported platforms or unsupported custom mirrors without a security boundary bypass

## Hardening expectations for users

Until `envr` reaches a more stable release process, users should take standard precautions:

- prefer trusted networks and trusted mirrors
- review project-local `.envr.toml` files before using them in untrusted repositories
- avoid sharing a writable runtime root between unrelated trust domains
- keep runtime root and cache directories writable only by the expected user account
- verify release artifacts and checksums when official release packaging is available

## Dependency advisory policy

CI and release packaging are expected to run `cargo deny check`, including RustSec advisories.
Any temporary advisory ignore in `deny.toml` must have a documented risk-acceptance reason and review plan in [`docs/deps/security-and-licenses.md`](docs/deps/security-and-licenses.md).

Currently accepted advisory ignores are limited to transitive GUI stack issues with no compatible safe upgrade path. These must be reviewed before release cuts and whenever the GUI dependency stack is upgraded.

## Security updates

Security-relevant fixes will be documented in release notes when applicable.
If a fix requires operational follow-up, maintainers should also document recommended mitigation or cleanup steps.
