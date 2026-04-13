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

1. Add or update a stable entry in [`Command::trace_name`](../../crates/envr-cli/src/cli.rs) and keep `cli::command_trace_tests` passing.
2. Add a row (or extend notes) in this matrix under **Matrix** / **Phase A coverage map**.
3. For each new success `emit_ok` / `write_envelope` **`message`** id, add `schemas/cli/data/<message>.json` (enforced by `json_emit_ok_message_schema_files`).
4. Extend or add an integration test in `json_schema_contract`, `json_envelope`, and/or `automation_matrix` for the JSON and porcelain paths you claim.
5. Document porcelain line shapes or JSON `data` fields in [output-contract.md](./output-contract.md) if they are stability targets.

**JSON parsing:** rely on `success`, `schema_version`, `code` (failures), success `message` as a stable id, and typed **`data`** per schema — not on localized failure `message` text (see [output-contract § JSON envelope](./output-contract.md#json-envelope-contract)).

## Parse errors (clap)

If the user invokes `envr` with **invalid flags or missing required positional arguments**, **clap** prints a usage/error message to **stderr** and a non-zero exit code. That path is **not** guaranteed to emit the JSON envelope. Automation should pass valid argv; integration tests focus on **post-parse** dispatch paths.

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
| `bundle` | yes | yes | — |
| `project` / `prune` / … | yes | yes | — |

**Note:** `install` JSON **success** is not in the offline regression suite (needs download or a large fixture); **`use`**, **`uninstall --dry-run`**, and **`project add` / `project validate`** cover the surrounding JSON paths.

## Regression tests

- **Command result pipeline (Phase C):** [`CommandOutcome`](../../crates/envr-cli/src/command_outcome.rs) on all `EnvrResult<i32>` handlers; optional one-liner [`finish_cli_cmd`](../../crates/envr-cli/src/command_outcome.rs) is re-exported as [`envr_cli::finish_cli_cmd`](../../crates/envr-cli/src/lib.rs) for embedders.
- Envelope shape and selected payloads: `crates/envr-cli/tests/json_envelope.rs`, `json_schema_contract.rs`.
- **Every `emit_ok` / success `write_envelope` message** has `schemas/cli/data/<message>.json`: `crates/envr-cli/tests/json_emit_ok_message_schema_files.rs`.
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
| `config` | `config_path_json_matches_schemas`, `config_keys_get_show_set_json_match_schemas_under_envr_root` (uses `ENVR_ROOT` temp) | — | — |
| `deactivate` / `hook prompt` / `template` / `update` | `deactivate_hint_json_matches_schemas`, `hook_prompt_json_matches_schemas`, `template_rendered_json_matches_schemas`, `update_info_json_matches_schemas` (+ `json_envelope` counterparts) | — | — |
| `run` / `exec` | `run_child_completed_json_matches_schemas`, `exec_child_completed_json_matches_schemas`, `exec_dry_run_json_matches_schemas`; `run_json_child_includes_install_metadata`, `exec_dry_run_json_envelope_message_dry_run` | — | — |
| `use` / `uninstall` | `use_sets_current_json_matches_schemas`, `uninstall_dry_run_json_matches_envelope_and_data_shape` | — | — |
| `cache` | `cache_clean_dry_run_prune_json_matches_schemas`, `cache_index_status_json_matches_schemas`, `cache_index_sync_json_matches_schemas_when_success` (skips on failure); `e2e_flows` text/json cache | — | — |
| `bundle` | `bundle_created_and_applied_json_match_schemas` | — | — |
| `project` / `prune` | `project_add_json_matches_schemas`, `project_validated_json_matches_schemas`, `prune_dry_run_json_matches_schemas` | — | — |
| `--quiet` + JSON | `quiet_validation_json_message_is_bracket_tag_only` | — | — |

When you add a command that scripts might call with `--format json` or `--porcelain`, **update this matrix** and add or extend a test.
