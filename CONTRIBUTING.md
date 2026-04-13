# Contributing

## CLI automation (envr)

If you change or add user-facing CLI behavior that scripts might depend on:

1. **`Command::trace_name`** — stable snake_case label for tracing; update [`crates/envr-cli/src/cli.rs`](crates/envr-cli/src/cli.rs) and keep `command_trace_tests` green.
2. **Automation matrix** — update [`docs/cli/automation-matrix.md`](docs/cli/automation-matrix.md) (matrix + Phase A map when applicable).
3. **JSON `data` stubs** — new success `message` ids need `schemas/cli/data/<message>.json` (see `json_emit_ok_message_schema_files` in `envr-cli` tests).
4. **Tests** — extend [`crates/envr-cli/tests/json_schema_contract.rs`](crates/envr-cli/tests/json_schema_contract.rs), [`json_envelope.rs`](crates/envr-cli/tests/json_envelope.rs), and/or [`automation_matrix.rs`](crates/envr-cli/tests/automation_matrix.rs) for claimed JSON/porcelain behavior.
5. **Contract doc** — normative rules and parser guidance: [`docs/cli/output-contract.md`](docs/cli/output-contract.md).

**Output format:** use [`GlobalArgs::effective_output_format`](crates/envr-cli/src/cli.rs); subcommand `--json` shorthands must be reflected in [`Cli::resolved_output_format`](crates/envr-cli/src/cli.rs) / [`apply_global`](crates/envr-cli/src/cli.rs) if they imply JSON for the process.

**Dispatch boundary:** [`commands::dispatch`](crates/envr-cli/src/commands/mod.rs) returns [`CommandOutcome`](crates/envr-cli/src/command_outcome.rs) for every route; process exit is applied only in [`cli::run`](crates/envr-cli/src/cli.rs) via [`CommandOutcome::finish`](crates/envr-cli/src/command_outcome.rs). New subcommands should return `CommandOutcome::Done(code)` or go through [`with_runtime_service`](crates/envr-cli/src/commands/common.rs) (connection errors → `CommandOutcome::Err`).

Run `cargo test -p envr-cli` before submitting CLI-facing changes.
