# Contributing

## CLI automation (envr)

If you change or add user-facing CLI behavior that scripts might depend on:

### Single checklist for adding a new CLI command

Use this one-pass checklist whenever you add a new command/subcommand that may be used by automation:

1. **Command identity (spec/trace/dispatch hints)** — add a row in [`crates/envr-cli/src/cli/command_spec.rs`](crates/envr-cli/src/cli/command_spec.rs), then keep command key mapping in [`crates/envr-cli/src/cli/command/mod.rs`](crates/envr-cli/src/cli/command/mod.rs) aligned.
2. **Dispatch + help alignment** — wire command routing under `crates/envr-cli/src/commands/dispatch_*.rs` and add/align help entry in [`crates/envr-cli/src/cli/help_registry/table.inc`](crates/envr-cli/src/cli/help_registry/table.inc).
3. **JSON contract surface** — add/adjust `schemas/cli/data/<message>.json`, then regenerate/verify `schemas/cli/index.json` (`python scripts/generate_cli_schema_index.py --check`).
4. **Tests** — extend command coverage in [`crates/envr-cli/tests/json_schema_contract.rs`](crates/envr-cli/tests/json_schema_contract.rs) and related envelope/automation tests for any claimed contract.
5. **Governance index + exemptions** — update `schemas/cli/governance-index.json` rows; use exemption entries only as temporary gaps with `reason/owner/due/exit_criteria`.
6. **Run one-shot governance gate** — `python scripts/check_cli_governance_all.py` (or `--quick`) before pushing.
7. **Breaking contract handling** — for breaking schema/index changes, run `python scripts/check_cli_contract_gate.py` and add `Migration note` in [`docs/cli/output-contract.md`](docs/cli/output-contract.md).

### Additional CLI governance tools

- `python scripts/generate_cli_contract_migration_note.py`: draft a `Migration note` from schema/index diffs.
- `python scripts/generate_cli_contract_report.py`: generate machine-readable contract diff report.
- `python -m unittest scripts/test_cli_contract_gate.py`: validate contract gate helper logic.
- Normative contract docs: [`docs/cli/output-contract.md`](docs/cli/output-contract.md).

**Output format:** use [`GlobalArgs::effective_output_format`](crates/envr-cli/src/cli/global.rs). Any subcommand flag equivalent to `--format json` must extend [`Command::legacy_json_shorthand`](crates/envr-cli/src/cli/command/mod.rs), use [`GlobalArgs::cloned_with_legacy_json`](crates/envr-cli/src/cli/global.rs) in the handler, and add a unit test next to `legacy_json_shorthand_centralizes_subcommand_json_flags` in [`mod.rs`](crates/envr-cli/src/cli/mod.rs) ([`Cli::resolved_output_format`](crates/envr-cli/src/cli/mod.rs) / [`apply_global`](crates/envr-cli/src/cli/mod.rs) follow `legacy_json_shorthand` automatically).

**Dispatch boundary:** [`commands::dispatch`](crates/envr-cli/src/commands/mod.rs) returns `(CommandOutcome, GlobalArgs)` so [`cli::run`](crates/envr-cli/src/cli/mod.rs) can call [`CommandOutcome::finish`](crates/envr-cli/src/command_outcome.rs) once without cloning globals. Handlers that are pure `EnvrResult<i32>` bodies should expose `pub(crate) fn run_inner` and use [`CommandOutcome::from_result`](crates/envr-cli/src/command_outcome.rs) in dispatch (see `which`, `resolve_cmd`, `check`, `config_cmd`, `deactivate_cmd`). Handlers that return a bare `i32` (e.g. completion emit) use [`CommandOutcome::from_exit_code`](crates/envr-cli/src/command_outcome.rs). Runtime commands go through [`with_runtime_service`](crates/envr-cli/src/commands/common.rs), which ends in `from_result` (connection errors → `CommandOutcome::Err`). Do not construct [`CommandOutcome::Done`](crates/envr-cli/src/command_outcome.rs) manually outside `command_outcome.rs`.

Run `cargo test -p envr-cli` before submitting CLI-facing changes.

## Runtime provider split policy (CQRS migration)

- Runtime runtime-query logic should go through `RuntimeIndex`; runtime-mutation logic should go through `RuntimeInstaller`.
- During migration, `RuntimeProvider` remains as a compatibility surface; do not add new read-path coupling to write APIs.
- For CLI read-oriented commands (`current`, `list`, `remote`, `bundle create`), prefer `RuntimeService::index_port` over direct read calls on `RuntimeService`.
- Run `python scripts/check_runtime_trait_split.py` before submitting runtime architecture changes.
- Compatibility retirement criteria:
  - All `envr-runtime-*` providers expose explicit split ports (`index_port`/`installer_port`) and pass split tests.
  - CLI/GUI read paths no longer call legacy read methods through `RuntimeService`.
  - CI keeps `check_runtime_trait_split.py` green for one full release cycle.
