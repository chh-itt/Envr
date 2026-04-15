# CLI automation output matrix (P0 contract)

This matrix is the **maintainer checklist** for script-facing behavior. It complements [output-contract.md](./output-contract.md) (normative rules) and [schemas/README.md](../schemas/README.md) (JSON Schema files).

## Legend

| Column | Meaning |
|--------|---------|
| **JSON ok** | Success path uses the standard envelope on stdout (`schema_version`, `success: true`, `message`, `data`, …) via `emit_ok` / equivalent. |
| **JSON err** | Business/runtime failures use the same envelope with `success: false` and a stable `code` (e.g. `validation`, `runtime`). |
| **Porcelain** | Documented in [output-contract.md § Porcelain](./output-contract.md#porcelain-contract) or command-specific notes; otherwise “—” (not a stability target). |

## New command / automation checklist

When adding or materially changing a subcommand that scripts might drive (`--format json` and/or `--porcelain`):

1. Add or update a stable entry in [`Command::trace_name`](../../crates/envr-cli/src/cli/command/mod.rs) (registry in [`metadata.rs`](../../crates/envr-cli/src/cli/metadata.rs)) and keep `cli::command_trace_tests` passing.
2. Add a row (or extend notes) in this matrix under **Matrix** / **Phase A coverage map**.
3. For each new success `emit_ok` / `write_envelope` **`message`** id, add `schemas/cli/data/<message>.json` (enforced by `json_emit_ok_message_schema_files`).
4. Extend or add an integration test in `json_schema_contract`, `json_envelope`, and/or `automation_matrix` for the JSON and porcelain paths you claim.
5. Document porcelain line shapes or JSON `data` fields in [output-contract.md](./output-contract.md) if they are stability targets.

6. If the command adds a **`--json`** (or equivalent) shorthand: extend [`Command::legacy_json_shorthand`](../../crates/envr-cli/src/cli/command/mod.rs), wire [`cloned_with_legacy_json`](../../crates/envr-cli/src/cli/global.rs) in dispatch/handler, and add a `cli` module test in [`mod.rs`](../../crates/envr-cli/src/cli/mod.rs) (see [CONTRIBUTING.md](../../CONTRIBUTING.md)).

**JSON parsing:** rely on `success`, `schema_version`, `code` (failures), success `message` as a stable id, and typed **`data`** per schema — not on localized failure `message` text (see [output-contract § JSON envelope](./output-contract.md#json-envelope-contract)).

## Parse errors (clap)

If the user invokes `envr` with **invalid flags or missing required positional arguments**, default clap behavior prints a usage/error message to **stderr** with non-zero exit.

When JSON is explicitly requested (`--format json`, or supported legacy shorthand such as `doctor --json`), `envr` emits a JSON failure envelope on stdout with `code = "argv_parse_error"`; see `argv_parse_json.rs` and [output-contract.md](./output-contract.md).

## Matrix (high-traffic commands)

| Command | JSON ok | JSON err | Porcelain |
|---------|---------|----------|-----------|
| `list` | yes | yes | yes (see contract) |
| `current` | yes | yes | yes |
| `remote` | yes | yes | — |
| `doctor` | yes (`--format json` or `doctor --json`) | yes | — |
| `status` | yes | yes | — |
| `resolve` | yes | yes | yes |
| `which` | yes | yes | yes |
| `config path` | yes | yes | — |
| `config get` / `set` / `show` / `keys` | yes | yes | — |
| `deactivate` | yes | yes | — |
| `hook prompt` | yes | yes | — |
| `template` | yes | yes | — |
| `update` | yes | yes | — |
| `run` | yes (`child_completed` / …) | yes | partial (prefer JSON for structure) |
| `exec` | yes | yes | partial |
| `install` / `use` / `uninstall` | yes | yes | — |
| `cache` subcommands | yes (per subcmd) | yes | — |
| `shim sync` | yes | yes | — |
| `bundle` | yes | yes | — |
| `project` / `prune` / … | yes | yes | — |

**Note:** `install` JSON **success** is not in the offline regression suite (needs download or a large fixture); **`use`**, **`uninstall --dry-run`**, and **`project add` / `project validate`** cover the surrounding JSON paths.

## Regression tests

- **Command result pipeline (Phase C):** dispatch builds [`CommandOutcome`](../../crates/envr-cli/src/command_outcome.rs) with [`from_result`](../../crates/envr-cli/src/command_outcome.rs) (`EnvrResult<i32>`) or [`from_exit_code`](../../crates/envr-cli/src/command_outcome.rs) (bare `i32`); optional one-liner [`finish_cli_cmd`](../../crates/envr-cli/src/command_outcome.rs) is re-exported as [`envr_cli::finish_cli_cmd`](../../crates/envr-cli/src/lib.rs) for embedders.
- **Metadata registry invariants:** `cargo test -p envr-cli --lib` runs [`cli::metadata::registry_alignment_tests`](../../crates/envr-cli/src/cli/metadata.rs) (unique `CommandKey` / `trace_name`, fixed row count, [`metadata_for_key`](../../crates/envr-cli/src/cli/metadata.rs) matches each static row) and extends [`command_key_mapping_round_trips_against_registry`](../../crates/envr-cli/src/cli/mod.rs) so the argv sample table length matches the registry.
- Envelope shape and selected payloads: `crates/envr-cli/tests/json_envelope.rs`, `json_schema_contract.rs`.
- **Every `emit_ok` / success `write_envelope` message** has `schemas/cli/data/<message>.json`: `crates/envr-cli/tests/json_emit_ok_message_schema_files.rs`.
- **Every `emit_failure_envelope` code literal** has `schemas/cli/data/failure_<code>.json`: `crates/envr-cli/tests/failure_contract_guards.rs`.
- `schemas/cli/index.json` stays aligned with source literals and schema files (message ids + failure codes): `crates/envr-cli/tests/schema_index_contract.rs` + `python scripts/generate_cli_schema_index.py --check`.
- Shared schema fragments (e.g. `schemas/cli/fragments/error_object.json`) stay aligned with selected failure schemas: `python scripts/check_cli_schema_fragments.py`.
- Governance index schema validation: `python scripts/check_cli_governance_index_schema.py`.
- Governance index/docs sync: `python scripts/check_cli_governance_index_sync.py` keeps `schemas/cli/governance-index.json` aligned with this markdown matrix and `output-contract.md` tier block.
- PR gate for schema/index diffs: `python scripts/check_cli_contract_gate.py` (fail-closed on unreadable changed schema JSON; breaking-like changes require `Migration note` in `docs/cli/output-contract.md` and mention changed ids/codes; use `--explain` for first breaking reason per file).
- Migration note draft helper: `python scripts/generate_cli_contract_migration_note.py`.
- CI contract artifact: `python scripts/generate_cli_contract_report.py` outputs `artifacts/cli-contract-report.json` and uploads `cli-contract-report`.
- Contract report now includes semantic summaries/hints for `error-kind-map.json` and `governance-index.json` when changed.
- CI review-note artifact: `python scripts/generate_cli_contract_review_note.py --report artifacts/cli-contract-report.json --output artifacts/cli-contract-review-note.md` outputs reviewer/release markdown and uploads `cli-contract-review-note`.
- CI capabilities artifact: `python scripts/generate_cli_capabilities_report.py` outputs `artifacts/cli-capabilities-report.json` and uploads `cli-capabilities-report`.
- Capabilities vs governance index alignment: `python scripts/check_cli_capabilities_alignment.py --report artifacts/cli-capabilities-report.json` (ensures `contract_surface=both` commands are explicitly marked porcelain-stable in `governance-index.json`).
- Offline coverage governance audit: `python scripts/check_cli_offline_coverage_alignment.py --report artifacts/cli-capabilities-report.json` (ensures offline-safe script-facing commands map to owned coverage rows in `governance-index.json`).
- Capability-driven test coverage audit: `python scripts/check_cli_capability_test_coverage.py --report artifacts/cli-capabilities-report.json` (ensures `governance-index.json` capability scope maps to concrete Phase A JSON/porcelain regression rows).
- Governance exemptions are structured and temporary: each `offline_coverage_exempt` / `capability_test_exempt` entry must include `reason`, `owner`, `due` (`YYYY-MM-DD`), and `exit_criteria`.
- Contract gate script unit tests: `python -m unittest scripts/test_cli_contract_gate.py`.
- Metrics phase-event schema contract: `cargo test -p envr-cli --test metrics_contract` validates `schemas/cli/metrics-event.json`.
- Metrics dictionary consistency: `cargo test -p envr-cli --test metrics_dictionary_contract` keeps `output-contract.md` field table aligned with `schemas/cli/metrics-event.json`.
- Porcelain / shared flags / i18n structure: `crates/envr-cli/tests/automation_matrix.rs`, `help_i18n_structure.rs`.
- JSON line with `RUST_LOG`: `json_stdout_with_rust_log.rs`.

## Phase A coverage map (command → tests)

Offline-safe unless noted. Schema validation strips UTF-8 BOM in `assert_valid` (`json_schema_contract.rs`).

| Command / area | JSON ok (schema where applicable) | JSON err | Porcelain |
|----------------|-----------------------------------|----------|-----------|
| `list` | `list_json_matches_schemas`, `list_json_contract` | `list_unknown_runtime_json_failure_matches_envelope_schema`, `validation_error_json_has_code` | `porcelain_list_all_runtimes_tab_lines_when_no_runtime_filter`, `cli_harness::list_node_porcelain_one_line_per_version` |
| `current` | `current_json_matches_schemas`, `current_json_contract` | `current_unknown_runtime_json_failure_matches_envelope_schema`, `current_invalid_runtime_json_validation_envelope` | `porcelain_current_all_runtimes_tab_lines` |
| `remote` | `remote_json_matches_schemas_when_success` (skips on network failure) | — | — |
| `doctor` | `doctor_json_data_matches_schema`, `doctor_json_contract`, `doctor_json_flag_matches_doctor_format_json` | `doctor_issues_json_matches_schema_when_runtime_root_missing` | — |
| `status` | `project_status_json_matches_schemas`, `status_json_contract` | `check_failure_project_check_failed_matches_schema` (via `check`) | — |
| `resolve` / `which` | `resolve_json_matches_schemas_with_project_pin`, `which_json_matches_schemas_with_project_pin` | — | `porcelain_resolve_single_line_runtime_home`, `porcelain_which_single_line_executable_path` |
| `config` | `config_path_json_matches_schemas`, `config_keys_get_show_set_json_match_schemas_under_envr_root` (uses `ENVR_ROOT` temp), `config_edit_json_matches_schemas`, `config_edit_json_contract` | — | — |
| `deactivate` / `hook prompt` / `template` / `update` | `deactivate_hint_json_matches_schemas`, `hook_prompt_json_matches_schemas`, `template_rendered_json_matches_schemas`, `update_info_json_matches_schemas` (+ `json_envelope` counterparts) | — | — |
| `run` / `exec` | `run_child_completed_json_matches_schemas`, `exec_child_completed_json_matches_schemas`, `exec_dry_run_json_matches_schemas`; `run_json_child_includes_install_metadata`, `exec_dry_run_json_envelope_message_dry_run` | — | — |
| `use` / `uninstall` | `use_sets_current_json_matches_schemas`, `uninstall_dry_run_json_matches_envelope_and_data_shape` | — | — |
| `cache` | `cache_clean_dry_run_prune_json_matches_schemas`, `cache_index_status_json_matches_schemas`, `cache_index_sync_json_matches_schemas_when_success` (skips on failure); `e2e_flows` text/json cache | — | — |
| `shim` | `shim_sync_json_matches_schemas`, `shim_sync_json_contract` | — | — |
| `rust` | `rust_install_managed_json_matches_schemas`, `rust_install_managed_json_contract` | — | — |
| `bundle` | `bundle_created_and_applied_json_match_schemas` | — | — |
| `project` / `prune` | `project_add_json_matches_schemas`, `project_sync_json_matches_schemas`, `project_validated_json_matches_schemas`, `project_sync_json_contract`, `prune_dry_run_json_matches_schemas` | — | — |
| `--quiet` + JSON | `quiet_validation_json_message_is_bracket_tag_only` | — | — |

When you add a command that scripts might call with `--format json` or `--porcelain`, **update this matrix** and add or extend a test.
