# CLI recipes (task-oriented)

Short **end-to-end flows** for common goals. Command details and flags live in `envr <cmd> --help` and [commands.md](./commands.md). The workspace [README.md](../../README.md) links here from the repo root for discoverability.

## New repository / local project

1. `envr init` — create `.envr.toml` (add `--full` for commented examples).
2. Edit pins under `[runtimes]` as needed.
3. `envr check` — verify pins resolve to installed versions.
4. `envr hook bash` (or `zsh`) — install shell integration; follow printed instructions.

## Temporary environment (this shell only)

- **Merged multi-language PATH** (project + pins): `envr shell` or `eval "$(envr env)"` on POSIX.
- **Single language, one shot**: `envr exec --lang node -- npm test`.
- **Shell resiliency**: if merged run-stack validation fails (for example, a JVM-hosted runtime/JDK mismatch), `envr shell` still starts with base OS env + project `[env]` overlay so recovery commands remain usable.

Global default version for new terminals remains `envr use <runtime> <version>` (updates `current` under the runtime root).

## CI / non-interactive

- Prefer `--format json` for machine-readable success/failure; parse **one JSON object per line** on stdout.
- Use `--quiet` to trim human-oriented fields; errors still use a stable `code` where implemented.
- Pre-cache indexes when offline or to avoid flaky networks: `envr cache index sync` (see [offline.md](./offline.md)).

Example:

```bash
envr --format json check
envr --format json run --dry-run myci ./scripts/ci.sh
```

## Portable offline bundle

See [bundle.md](./bundle.md): `envr bundle create` on a connected machine, `envr bundle apply` on the target.

## Diagnostics for bug reports

```bash
envr doctor
envr diagnostics export
```

## Troubleshooting (humans & integrators)

- **JSON automation**: Prefer `--format json` and parse **one JSON object per line** on stdout for commands that completed dispatch. If the process failed **while parsing arguments** (bad flags, missing values), output is **not** the JSON envelope — see [Output contract § CLI argument parse errors](./output-contract.md#cli-argument-parse-errors-before-dispatch).
- **Something is wrong in a project**: Run `envr check` in the repo root (or `envr check --path <dir>`). On failure, JSON mode sets `code` to `project_check_failed` with structured `data.issues`.
- **PATH / shims / installs**: Run `envr doctor --format json`; success uses `message: doctor_ok`, hard failures use `code: doctor_issues` with the same `data` shape as the passing report (see [schemas/data-doctor-v2.schema.json](../schemas/data-doctor-v2.schema.json)).
- **Network / mirror**: See [offline.md](./offline.md) and mirror hints in the output contract for download-class errors.

## Related

- [Output contract & porcelain](./output-contract.md)
- [Automation matrix](./automation-matrix.md)
- [Scripting notes](./scripting.md)
