# Terraform integration plan (HashiCorp Releases single-binary zip)

## Goal

Add **`RuntimeKind::Terraform`** as a first-class managed runtime (CLI / GUI / shims / pins / `exec` / `run`) with install layout:

`runtimes/terraform/versions/<version>/` and `runtimes/terraform/current`.

Terraform is a standalone binary runtime (no JVM/host-runtime coupling).

## Scope & non-goals

- **In scope:** HashiCorp official Terraform CLI (`terraform`) from `https://releases.hashicorp.com/terraform/`.
- **Out of scope:** provider plugins/cache mirrors, Terraform Cloud auth bootstrap, OpenTofu compatibility layer.

## Version/index shape

- **Source:** HashiCorp releases HTML index page:
  - `https://releases.hashicorp.com/terraform/`
- **Install artifact URL shape:**  
  `https://releases.hashicorp.com/terraform/<version>/terraform_<version>_<platform>.zip`
- **Cache:** `{runtime_root}/cache/terraform/index_versions.json` (TTL env knob, default 6h).
- **Resolution policy:** support exact / `major` / `major.minor` shorthand (e.g. `1`, `1.14`).

## Architecture / abstraction friction log

1. **HTML index parsing vs JSON API:** unlike many runtimes, Terraform release discovery relies on HTML parsing; parser stability and fallback behavior need explicit tests.
2. **Single-binary layout mismatch:** existing runtimes mix root/bin multi-file layouts; Terraform needs a concise reusable helper for root single-binary validation.
3. **GUI runtime settings boilerplate:** each new path-proxy runtime still requires explicit runtime settings section wiring in env-center.

## Implementation checklist

### Phase A — Domain

- [x] Add `RuntimeKind::Terraform` descriptor (`key=terraform`, remote/path proxy true).
- [x] Include Terraform in version line grouping (`major.minor`).
- [x] Extend descriptor tests/count assertions.

### Phase B — Provider crate `envr-runtime-terraform`

- [x] Create crate + provider implementation.
- [x] Parse release versions from HashiCorp index page.
- [x] Resolve/download host artifact zip and install validated binary (`terraform`).
- [x] Add cache + TTL knobs.

### Phase C — Core/CLI/resolver/shims

- [x] Register provider in runtime service + core Cargo wiring.
- [x] Add core shim command `terraform`.
- [x] Add `runtime_bin_dirs_for_key`, `runtime_home_env_for_key` (`TERRAFORM_HOME`).
- [x] Wire `ENVR_TERRAFORM_VERSION` + list/bundle/status/shim sync/missing-pins/run-home/run-stack parity.

### Phase D — Config/GUI

- [x] Add `[runtime.terraform] path_proxy_enabled` (settings + snapshot + schema).
- [x] Add Env Center settings section and toggle handling.
- [x] Update runtime layout count test.

### Phase E — Docs/playbook polish

- [x] Add `docs/runtime/terraform.md` (install, remote source, cache knobs, usage).
- [x] Update playbook for standalone single-binary runtime checklist (if gaps found).
- [x] Record actual friction and post-change CLI/GUI observations.

## QA notes

- CLI smoke:
  - `envr remote terraform`
  - `envr remote terraform -u`
  - `envr install terraform 1.14`
  - `envr use terraform <version>`
  - `envr exec --lang terraform -- terraform version`
  - `envr which terraform`
- GUI smoke:
  - Terraform tab remote/install/use/current
  - path-proxy toggle persistence and behavior

## Development notes (actual)

- Terraform reinforces the "HTML index parser" pattern for remote discovery; unlike GitHub API-driven runtimes, this path is simple but parser-regex stability matters.
- Single-binary zip extraction differs from nested SDK layouts; install validation remains per-runtime, suggesting a future shared helper for "root contains tool.exe" runtimes.
- GUI/runtime settings still require one explicit runtime section for standalone tools (path-proxy hint/toggle wiring); descriptor-driven rendering could reduce this repetition later.
- CLI/GUI integration behaved consistently after wiring: no JVM-host coupling and no extra host-runtime preflight branch needed.

