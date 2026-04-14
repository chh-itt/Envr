# Contributing

## CLI automation (envr)

If you change or add user-facing CLI behavior that scripts might depend on:

1. **`Command::trace_name`** — stable snake_case label for tracing; update [`crates/envr-cli/src/cli/command/mod.rs`](crates/envr-cli/src/cli/command/mod.rs) / [`metadata.rs`](crates/envr-cli/src/cli/metadata.rs) registry and keep `command_trace_tests` green.
2. **Automation matrix** — update [`docs/cli/automation-matrix.md`](docs/cli/automation-matrix.md) (matrix + Phase A map when applicable).
3. **JSON `data` stubs** — new success `message` ids need `schemas/cli/data/<message>.json` (see `json_emit_ok_message_schema_files` in `envr-cli` tests).
4. **Schema index** — regenerate/verify `schemas/cli/index.json` with `python scripts/generate_cli_schema_index.py` (or `--check`) after adding/changing envelope message/code literals.
5. **Tests** — extend [`crates/envr-cli/tests/json_schema_contract.rs`](crates/envr-cli/tests/json_schema_contract.rs), [`json_envelope.rs`](crates/envr-cli/tests/json_envelope.rs), and/or [`automation_matrix.rs`](crates/envr-cli/tests/automation_matrix.rs) for claimed JSON/porcelain behavior.
6. **Contract gate** — run `python scripts/check_cli_contract_gate.py` for schema/index changes; if the change is breaking, include `Migration note` in [`docs/cli/output-contract.md`](docs/cli/output-contract.md) and mention all changed schema ids/codes. Use `--explain` to print the first breaking reason per file.
7. **Migration note draft (optional helper)** — `python scripts/generate_cli_contract_migration_note.py` generates a ready-to-edit `Migration note` draft based on schema/index diffs.
8. **Contract report artifact (CI helper)** — `python scripts/generate_cli_contract_report.py` writes a machine-readable diff report for schema governance and PR review.
9. **Gate script tests** — `python -m unittest scripts/test_cli_contract_gate.py` validates key gate helpers (schema ids, migration-note matching, multi-reason diff).
5. **Contract doc** — normative rules and parser guidance: [`docs/cli/output-contract.md`](docs/cli/output-contract.md).

**Output format:** use [`GlobalArgs::effective_output_format`](crates/envr-cli/src/cli/global.rs). Any subcommand flag equivalent to `--format json` must extend [`Command::legacy_json_shorthand`](crates/envr-cli/src/cli/command/mod.rs), use [`GlobalArgs::cloned_with_legacy_json`](crates/envr-cli/src/cli/global.rs) in the handler, and add a unit test next to `legacy_json_shorthand_centralizes_subcommand_json_flags` in [`mod.rs`](crates/envr-cli/src/cli/mod.rs) ([`Cli::resolved_output_format`](crates/envr-cli/src/cli/mod.rs) / [`apply_global`](crates/envr-cli/src/cli/mod.rs) follow `legacy_json_shorthand` automatically).

**Dispatch boundary:** [`commands::dispatch`](crates/envr-cli/src/commands/mod.rs) returns `(CommandOutcome, GlobalArgs)` so [`cli::run`](crates/envr-cli/src/cli/mod.rs) can call [`CommandOutcome::finish`](crates/envr-cli/src/command_outcome.rs) once without cloning globals. Handlers that are pure `EnvrResult<i32>` bodies should expose `pub(crate) fn run_inner` and use [`CommandOutcome::from_result`](crates/envr-cli/src/command_outcome.rs) in dispatch (see `which`, `resolve_cmd`, `check`, `config_cmd`, `deactivate_cmd`). Handlers that return a bare `i32` (e.g. completion emit) use [`CommandOutcome::from_exit_code`](crates/envr-cli/src/command_outcome.rs). Runtime commands go through [`with_runtime_service`](crates/envr-cli/src/commands/common.rs), which ends in `from_result` (connection errors → `CommandOutcome::Err`). Do not construct [`CommandOutcome::Done`](crates/envr-cli/src/command_outcome.rs) manually outside `command_outcome.rs`.

Run `cargo test -p envr-cli` before submitting CLI-facing changes.
