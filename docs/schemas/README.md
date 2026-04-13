# JSON Schemas (`--format json`)

These files describe **machine-readable** CLI output when `envr` is run with `--format json`, with **`ENVR_OUTPUT_FORMAT=json`** set to match (including when **`doctor --json`** is used instead of the global flag).

## Envelope

| File | Purpose |
|------|---------|
| [cli-envelope-v2.schema.json](./cli-envelope-v2.schema.json) | Current envelope (`schema_version` **2**) |
| [cli-envelope-v1.schema.json](./cli-envelope-v1.schema.json) | Legacy envelope (`schema_version` **1**); kept for reference |

`schema_version` is **2** for current `envr` builds. v2 renames list/current/remote `data` keys to `installed_runtimes`, `active_versions`, and `remote_runtimes` (see [../cli/output-contract.md](../cli/output-contract.md)).

### Per-`message` data files (`data-*-vN`)

The trailing **`vN`** is the **payload schema revision** for that `message` value. Bump **`N`** when you make a **breaking** change to the `data` object for that command; bump the top-level **`schema_version`** only when the **outer envelope** contract changes. See [§ Envelope schema_version vs per-command data](../cli/output-contract.md#envelope-schema_version-vs-per-command-data-schemas) in the output contract.

## `schemas/cli/data` (repository mirror)

Machine-checkable JSON Schema files live under **`schemas/cli/data/<message>.json`** (one file per success `message` emitted via `emit_ok` or `write_envelope(..., true, ...)`). The integration test `crates/envr-cli/tests/json_emit_ok_message_schema_files.rs` fails if the Rust sources introduce a new message without adding that file.

**Source of truth (drift policy):** The **filenames** under `schemas/cli/data/*.json` are enforced by tests (`json_emit_ok_message_schema_files`, `json_schema_contract`). The Markdown tables below are a **human index** into `docs/schemas/data-*-vN.schema.json`; when they disagree with code or `schemas/cli/data/`, update the schemas and tables together. Prefer extending `crates/envr-cli/tests/json_schema_contract.rs` when you tighten a stable success or failure payload.

The tables below map logical names to the **`docs/schemas/data-*-v1.schema.json`** copies used for human-readable versioning; keep them aligned when tightening contracts.

## `data` by `message` (success)

| `message` | Schema |
|-----------|--------|
| `list_installed` | [data-list-installed-v1.schema.json](./data-list-installed-v1.schema.json) |
| `list_remote` | [data-list-remote-v1.schema.json](./data-list-remote-v1.schema.json) |
| `show_current` | [data-show-current-v1.schema.json](./data-show-current-v1.schema.json) |
| `doctor_ok`, `doctor_issues` | [data-doctor-v2.schema.json](./data-doctor-v2.schema.json) |
| `deactivate_hint` | [data-deactivate-hint-v1.schema.json](./data-deactivate-hint-v1.schema.json) |
| `project_status` | [data-project-status-v1.schema.json](./data-project-status-v1.schema.json) |
| `hook_prompt` | [data-hook-prompt-v1.schema.json](./data-hook-prompt-v1.schema.json) |
| `template_rendered` | [data-template-rendered-v1.schema.json](./data-template-rendered-v1.schema.json) |
| `installed`, `uninstalled`, `current_runtime_set` | [data-runtime-kind-version-v1.schema.json](./data-runtime-kind-version-v1.schema.json) |
| `child_completed`, `child_exit` | [data-child-process-v1.schema.json](./data-child-process-v1.schema.json) |
| `project_config_ok` | [data-project-config-ok-v1.schema.json](./data-project-config-ok-v1.schema.json) |
| `project_pin_added`, `project_synced`, `project_validated`, `project_sync_pending`, `project_validate_failed` | [data-project-op-v1.schema.json](./data-project-op-v1.schema.json) |
| `runtime_resolved` | [data-runtime-resolved-v1.schema.json](./data-runtime-resolved-v1.schema.json) |
| `resolved_executable` | [data-resolved-executable-v1.schema.json](./data-resolved-executable-v1.schema.json) |
| `config_path` | [data-config-path-v1.schema.json](./data-config-path-v1.schema.json) |
| `config_keys` | [data-config-keys-v1.schema.json](./data-config-keys-v1.schema.json) |
| `config_get` | [data-config-get-v1.schema.json](./data-config-get-v1.schema.json) |
| `config_set` | [data-config-set-v1.schema.json](./data-config-set-v1.schema.json) |
| `config_show` | [data-config-show-v1.schema.json](./data-config-show-v1.schema.json) |
| `bundle_created` | [data-bundle-created-v1.schema.json](./data-bundle-created-v1.schema.json) |
| `bundle_applied` | [data-bundle-applied-v1.schema.json](./data-bundle-applied-v1.schema.json) |
| `prune_dry_run` | [data-prune-dry-run-v1.schema.json](./data-prune-dry-run-v1.schema.json) |
| `prune_executed` | [data-prune-executed-v1.schema.json](./data-prune-executed-v1.schema.json) |
| `alias_list` | [data-alias-list-v1.schema.json](./data-alias-list-v1.schema.json) |
| `alias_added` | [data-alias-added-v1.schema.json](./data-alias-added-v1.schema.json) |
| `alias_removed` | [data-alias-removed-v1.schema.json](./data-alias-removed-v1.schema.json) |
| `profiles_list` | [data-profiles-list-v1.schema.json](./data-profiles-list-v1.schema.json) |
| `profile_show` | [data-profile-show-v1.schema.json](./data-profile-show-v1.schema.json) |
| `project_config_init` | [data-project-config-init-v1.schema.json](./data-project-config-init-v1.schema.json) |
| `project_env` | [data-project-env-v1.schema.json](./data-project-env-v1.schema.json) |
| `update_info` | [data-update-info-v1.schema.json](./data-update-info-v1.schema.json) |
| `cache_cleaned` | [data-cache-cleaned-v1.schema.json](./data-cache-cleaned-v1.schema.json) |
| `cache_index_synced` | [data-cache-index-synced-v1.schema.json](./data-cache-index-synced-v1.schema.json) |
| `cache_index_status` | [data-cache-index-status-v1.schema.json](./data-cache-index-status-v1.schema.json) |
| `config_imported` | [data-config-imported-v1.schema.json](./data-config-imported-v1.schema.json) |
| `config_exported` | [data-config-exported-v1.schema.json](./data-config-exported-v1.schema.json) |
| `shims_synced` | [data-shims-synced-v1.schema.json](./data-shims-synced-v1.schema.json) |
| `diagnostics_export_ok` | [data-diagnostics-export-ok-v1.schema.json](./data-diagnostics-export-ok-v1.schema.json) |

## `data` on failure (non-null)

| Condition | Schema |
|-----------|--------|
| Most errors (`data` is `null`) | [data-null-v1.schema.json](./data-null-v1.schema.json) |
| `code` = `project_check_failed` | [data-project-check-failed-v1.schema.json](./data-project-check-failed-v1.schema.json); repo mirror: [`schemas/cli/data/failure_project_check_failed.json`](../../schemas/cli/data/failure_project_check_failed.json) (see `json_schema_contract` test) |
| `code` = `diagnostics_export_failed` | [data-diagnostics-export-failed-v1.schema.json](./data-diagnostics-export-failed-v1.schema.json) |
| `uninstall --dry-run` refusal (`code` = `validation`) | [data-uninstall-dry-run-v1.schema.json](./data-uninstall-dry-run-v1.schema.json) |
| `code` = `doctor_issues` | Same payload as `doctor_ok`: [data-doctor-v2.schema.json](./data-doctor-v2.schema.json); repo mirrors: `schemas/cli/data/doctor_ok.json` and `doctor_issues.json` |

## Exceptions

- **`uninstall --dry-run` success** uses `emit_ok` with a **localized** `message` string (not a stable token). Parse `success` + `data` shape instead of `message` for that case; `data` matches [data-uninstall-dry-run-v1.schema.json](./data-uninstall-dry-run-v1.schema.json).

## Validation

### Automated script (recommended)

From the repo root (needs Python 3.9+ and `pip install jsonschema referencing`):

```bash
pip install jsonschema referencing
python scripts/validate_cli_json_contract.py
```

The script runs `envr` with a temp `ENVR_RUNTIME_ROOT`, captures JSON lines, validates the envelope, then `data` using the table above.

### Manual (`jsonschema` CLI)

```bash
python -m jsonschema -i response.json docs/schemas/cli-envelope-v2.schema.json
```

## Runtime listing layout

For **list**, **remote**, and **current**, `data` uses **`installed_runtimes`**, **`remote_runtimes`**, and **`active_versions`** respectively (arrays of per-runtime rows). See [../cli/output-contract.md](../cli/output-contract.md).

Repository copies under `schemas/cli/data/` (plus integration tests) mirror many `message` ids; permissive stubs document presence—tighten fields in `docs/schemas/data-*.schema.json` as contracts evolve.
