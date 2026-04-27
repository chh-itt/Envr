# envr workspace

This repository is organized as a Rust workspace for the envr refactor.

## Layout

- `Cargo.toml`: workspace root manifest
- `crates/`: all workspace crates (libraries and binaries)
- `refactor docs/`: architecture and implementation docs
- `docs/cli/`: CLI product docs (command spectrum, JSON contract, scripting, config, offline, bundle)
- `tasks.md`: phased task checklist and implementation records

## Contributing

- **CLI automation checklist** (trace names, matrix, schemas, tests): [CONTRIBUTING.md](CONTRIBUTING.md)

## CLI documentation

- **Command spectrum (L1/L2/L3 + aliases):** [docs/cli/commands.md](docs/cli/commands.md)
- **Task-oriented flows (init, CI, offline, diagnostics):** [docs/cli/recipes.md](docs/cli/recipes.md)
- **Automation output matrix (JSON / porcelain checklist):** [docs/cli/automation-matrix.md](docs/cli/automation-matrix.md)
- **JSON `data` schemas:** [docs/schemas/README.md](docs/schemas/README.md) (mirrored under `schemas/cli/data/`; `envr-cli` tests enforce one file per `emit_ok` message id)
- Run `envr --help` for grouped commands and tier legend; see also `refactor docs/02-cli-设计.md` §2.

## License

This workspace is licensed under either of Apache License, Version 2.0 or MIT license at your option.
See [LICENSE](LICENSE) for details.
