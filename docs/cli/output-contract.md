# CLI output contract (stable for scripts)

This document defines script-facing output guarantees for `envr`.

## Scope

These guarantees are for automation usage only:

- `--porcelain` (alias: `--plain`)
- `--format json`

Human-readable default text output is not a stability target for parsers.

### CLI argument parse errors (before dispatch)

- **Default behavior:** if the process **exits while parsing argv** (unknown flag, missing required argument, invalid value type, etc.), **clap** prints a **human-oriented** help or error message to **stderr** and exits with a **non-zero** status.
- **When JSON is explicitly requested:** if `--format json` is present (or a supported legacy JSON shorthand such as `doctor --json`), `envr` emits a **single JSON envelope line on stdout** with `success: false` and stable `code: "argv_parse_error"`.
  - `data` shape: `{ "source": "clap", "kind": "<clap ErrorKind>", "error": "<rendered parse error>", "exit_code": <int> }`.

## Precedence

If both flags are provided, JSON mode wins:

- `--format json` > `--porcelain`

### Effective output format (implementation note)

- Global default is **text** when `--format` is omitted ([`GlobalArgs::effective_output_format`](../../crates/envr-cli/src/cli/global.rs)).
- Subcommand-local JSON shorthands (today: **`doctor --json`**) are listed in **`Command::legacy_json_shorthand`** in [`command/mod.rs`](../../crates/envr-cli/src/cli/command/mod.rs); [`Cli::resolved_output_format`](../../crates/envr-cli/src/cli/mod.rs) and [`apply_global`](../../crates/envr-cli/src/cli/mod.rs) use it so **`ENVR_OUTPUT_FORMAT`** and handler output stay aligned. If both `--format text` and `doctor --json` appear, the legacy `--json` shorthand wins for resolution.

## Output mode matrix (automation)

| Mode | stdout | stderr on success | stderr on error |
|------|--------|-------------------|-----------------|
| **Text** (default) | Human-oriented lines from each command | Optional progress / hints (`envr: …`) | `envr: [E_<CODE>] <message>`; network-class errors may append a mirror hint line |
| **`--porcelain`** | Plain lines only (per-command contract below); no envelope | Same as text unless a command documents otherwise | Same bracket line as text (`[E_<CODE>]`) |
| **`--format json`** | Single JSON object per logical response (see envelope) | Tracing to **stderr** when `RUST_LOG` / filters apply (never mixed into stdout) | Same envelope with `success: false`; see failure `data` below |
| **`--quiet`** | Suppressed where documented (many commands still emit JSON stdout when `json`) | Reduced or empty | Text: bracket-only line; JSON: `message` is bracket tag, `diagnostics` omitted, failure `data` omitted |

`--quiet` does not disable `--format json` success lines; it trims human-oriented fields inside the envelope where implemented.

## Observability (tracing)

- The `envr` binary attaches a `tracing` console layer to **stderr** and a rolling file under `ENVR_LOG_DIR` or the platform default log dir (for example `%APPDATA%\envr\logs` on Windows).
- **`RUST_LOG`** (e.g. `info`, `debug`) therefore only affects **stderr**, so **stdout** stays a single JSON line (or porcelain lines) for automation. Regression: `crates/envr-cli/tests/json_stdout_with_rust_log.rs`.
- Each dispatched subcommand runs under span **`envr.cli.command`** with field **`command`** = stable snake_case (see [`Command::trace_name`](../../crates/envr-cli/src/cli/command/mod.rs) / `cli::command_trace_tests`).
- CLI metrics on target **`envr_cli_metrics`** include phase-level events:
  - `phase=parse`: fields include `output_mode`, `quiet`, `success`, `exit_code`, `error_code`. Parse metrics are buffered during argv parsing and flushed after logging init; parse-failure early exits do a best-effort logging init + flush before process exit.
  - `phase=dispatch`: fields include `command`, `output_mode`, `success`, `exit_code`, `error_code`, `elapsed_ms`.
  - `phase=finish`: fields include `output_mode`, `success`, `exit_code`, `error_code`.
- Machine-readable schema: [`schemas/cli/metrics-event.json`](../../schemas/cli/metrics-event.json). Regression test: `crates/envr-cli/tests/metrics_contract.rs`.
- Metrics payload policy:
  - `output_mode` uses stable lowercase tokens: `text` / `json`.
  - `error_code` is a stable snake_case token on failures; on success it is an empty string (`""`), not `null`.
  - For command paths that finish as `Done(non-zero)` without a typed `EnvrError`, metrics use fallback token `nonzero_exit`.
- Metrics field dictionary (auditable):
<!-- METRICS_FIELDS_TABLE_START -->
| field | type | required | phases | allowed values / null policy |
|-------|------|----------|--------|-------------------------------|
| `phase` | string | yes | all | `parse` \| `dispatch` \| `finish` |
| `output_mode` | string | yes | all | `text` \| `json` |
| `success` | boolean | yes | all | `true` \| `false` |
| `exit_code` | integer | yes | all | process exit code |
| `error_code` | string | yes | all | snake_case token on failure; success uses empty string `""` |
| `quiet` | boolean | yes | parse | omitted on non-`parse` phases |
| `command` | string | yes | dispatch | snake_case `Command::trace_name`; omitted on non-`dispatch` phases |
| `elapsed_ms` | integer >= 0 | yes | dispatch | non-negative elapsed milliseconds; omitted on non-`dispatch` phases |
<!-- METRICS_FIELDS_TABLE_END -->
- User-visible CLI failures routed through `emit_envr_error` also emit a **`tracing::error!`** event on target **`envr_cli`** with structured fields `cli_error_kind`, `cli_error_exit_code`, `cli_error_diagnostics_len` (message text stays on stderr / JSON envelope as before).

## HTTP timeouts (reqwest-based downloads)

- [`envr_download::DownloadEngine::default_client`](../../crates/envr-download/src/engine.rs) sets a **TCP connect timeout** of **30s** by default ([`DEFAULT_HTTP_CONNECT_TIMEOUT`](../../crates/envr-download/src/engine.rs)). Each transfer still uses [`DownloadOptions::timeout`](../../crates/envr-download/src/engine.rs) (default **60s** for the request body) on top of that.
- **`ENVR_HTTP_CONNECT_TIMEOUT_SECS`**: optional override in seconds, range **1–600**; invalid values fall back to the default.

## Maintainer workflow: changing JSON `data`

1. Change the `serde_json` value passed to `output::emit_ok` / `write_envelope` / failure paths.
2. If the shape is documented or schema-checked, update the matching file under [`docs/schemas/`](../schemas/README.md).
3. Run `cargo test -p envr-cli` (includes JSON envelope and schema contract tests).
4. If the change is breaking for scripts, bump `CLI_JSON_SCHEMA_VERSION` in `crates/envr-cli/src/output.rs` and document the migration in this file.

New optional fields inside existing documented objects are generally non-breaking. Renaming or removing fields requires a schema version bump.

## Envelope schema_version vs per-command data schemas

- **`schema_version`** (top-level integer on every JSON line, e.g. `2`): bump only when the **envelope shape** changes in a breaking way (`success`, `code`, `message`, `data`, `diagnostics` semantics or required keys).
- **Per-command `data`**: discriminated by the envelope’s **`message`** string (`list_installed`, `config_path`, `child_completed`, …). Each documented shape has a JSON Schema file under [`docs/schemas/`](../schemas/README.md) named `data-*-vN.schema.json`. Bump **`N`** when **`data` for that message** changes incompatibly (rename/remove fields). You do **not** have to bump the envelope `schema_version` if the envelope itself is unchanged.
- Optional new keys inside an existing `data` object are usually non-breaking; document them and extend the corresponding `data-*` schema.
- Maintainer checklist for automation coverage: [automation-matrix.md](./automation-matrix.md).

## Porcelain contract

`--porcelain` outputs plain lines with no labels/decorations.

### `envr --porcelain which <tool>`

- Success: exactly one line, absolute executable path.
- No extra env lines are printed.

### `envr --porcelain resolve <runtime> [--path ...] [--spec ...]`

- Success: exactly one line, resolved runtime home path.

### `envr --porcelain current [runtime]`

- With `runtime`:
  - Success + current exists: one line `<version>`.
  - Success + no current: empty stdout.
- Without `runtime`:
  - One line per runtime in format: `<runtime>\t<version-or-empty>`.

### `envr --porcelain list [runtime]`

- With `runtime`:
  - One line per installed version: `<version>`.
- Without `runtime`:
  - One line per installed version across runtimes:
    - `<runtime>\t<version>`

### Parsing notes

- Line separator: `\n`
- Field separator for multi-runtime lines: TAB (`\t`)
- Runtime tokens are lowercase keys (e.g. `node`, `python`, `deno`, `bun`).

## JSON envelope contract

### What scripts should rely on

For automation, treat these as the **stable contract**:

- **`success`**, **`schema_version`**, and **`code`** (both success and failure; stable snake_case token)
- **`code`** on **success** paths: a stable snake_case **discriminator** for the payload shape (e.g. `list_installed`, `child_completed`), used with the matching **`data`** schema
- **Typed fields inside `data`** as documented per `message` and JSON Schema under [`docs/schemas/`](../schemas/README.md)

Do **not** build logic on:

- **Natural-language strings** in the envelope **`message`** (it is intended for humans and may be localized); use **`code`**, **`schema_version`**, structured **`data`** when present, and **`diagnostics`** only as human hints
- **Ad-hoc wording** in **`diagnostics`** or stderr mirror hints

Success **`code`** values are programmatic ids, not translated sentences. The envelope **`message`** is human-facing text and may be localized; parsers must rely on **`code`** + `data` schemas.

All JSON responses use a single-line envelope:

```json
{
  "schema_version": 3,
  "success": true,
  "code": "some_code",
  "message": "some human-readable message",
  "data": {},
  "diagnostics": []
}
```

### Fields

- `schema_version`: integer (currently **3**). v3 makes envelope `code` required and stable for both success and failure; scripts must not rely on `message` for control flow.
- `success`: boolean
- `code`: string (stable snake_case token; on success it also acts as the `data` discriminator)
- `message`: human-facing text (may be localized); use `code` + `data` for logic
- `data`: command-specific payload
- `diagnostics`: string array (error chain / hints)

### JSON Schema

Machine-readable schemas live under [`docs/schemas/`](../schemas/README.md) (envelope + per-`message` `data` shapes where documented).

Regression-style checks: build the `envr` binary, then run [`scripts/validate_cli_json_contract.py`](../scripts/validate_cli_json_contract.py) (Python 3 + `jsonschema`, see script header).

`envr doctor --format json` and `doctor.json` inside diagnostics zip use the same `data` object as `DoctorReport`; each `kinds[]` entry exposes **`current_version`** (string or `null`), not `current`, so it stays distinct from envelope fields and other “current” wording.

#### `envr doctor --format json` and `--fix`

- **`fixes_applied`** (optional): present only when `--fix` is passed. Array of human-readable strings describing attempted fixes (success or failure), e.g. refreshed shims or set `current` to a chosen installed version. Scripts should treat unknown strings as opaque; entries are not a stable API beyond “something was attempted”.
- When `--fix` is **not** used, **`fixes_applied` must be omitted** (do not rely on `null` vs missing without checking `schema_version`).

### `exec` / `run` dry-run (`message`: `dry_run`)

When `--dry-run` is set, the process exits **0** after printing the preview (no child process).

- **`message`**: always `dry_run` (stable key for scripts).
- **`data`**:
  - **`command`**: string (first token / program name as passed to the CLI).
  - **`args`**: string array (remaining arguments).
  - **`env`**: object whose keys are environment variable names and values are strings (full merged environment that would be passed to the child, including `PATH`).

No `exit_code` field is present on success (there is no child). Porcelain / `--porcelain` is unchanged for these commands unless explicitly documented later.

### Runtime list commands (`data` arrays, v2)

For **list**, **remote**, and **current**, `data` includes one of these arrays (same row shapes as before; only the property names changed in v2):

- **`list`** (`message`: `list_installed`): **`installed_runtimes`** — each row has `kind` and `versions` (array of objects with at least `version`, `current`, `lts`, `lts_codename`, `npm`). With `--outdated`, each version object may also include **`remote_latest`** (string or `null`) and **`outdated`** (boolean): `outdated` is `true` when a newer patch exists on the remote index for the same major line (best-effort; requires `list_remote_latest_per_major` to succeed for that runtime). When remote metadata is unavailable, `remote_latest` may be `null` and `outdated` is `false`.
- **`remote`** (`message`: `list_remote`): **`remote_runtimes`** — each row has `kind` and `versions` (array of `{ "version": "<label>" }` or richer Node rows).
- **`current`** (`message`: `show_current`): **`active_versions`** — each row has `kind`, `version` (string or `null`), and `hint` (string or `null`).

### `envr why --format json` (`message`: `why_runtime`)

- **`data`** includes at least:
  - **`lang`**: runtime key (e.g. `node`).
  - **`working_dir`**, **`profile`**: strings / `null`.
  - **`spec_override`**: string or `null` — same semantics as `resolve --spec` when `--spec` is passed; omitted or `null` when not used.
  - **`project`**: `null` or an object with `config_dir`, optional `base_file` / `local_file`, and `pin` (string or `null`).
  - **`resolution`**: `spec_override` | `project_pin` | `global_current` — which rule determined the resolution (`spec_override` when `--spec` is non-empty after trim; otherwise project pin if present, else global `current`).
  - **`resolved_home`**: absolute or canonical path string.

### `envr status` / `envr st` (`--format json`, `message`: `project_status`)

Human **text** mode is labeled lines (working directory, project root or “no project”, optional profile, then one line per known runtime). It is **not** a porcelain-stable line format.

JSON **`data`**:

- **`working_dir`**: string — directory used to search upward for `.envr.toml`.
- **`profile`**: string or `null` — effective profile (`--profile` or `ENVR_PROFILE`).
- **`project`**: `null` or `{ "dir": "<path>" }` — directory containing the merged project config when found.
- **`runtimes`**: array of rows:
  - **`kind`**: string (e.g. `node`, `python`, `java`, …).
  - **`pin`**: string or `null` — `[runtimes.<kind>].version` when set in merged config.
  - **`active_version`**: string — version directory name used for resolution, or a placeholder when resolution fails.
  - **`source`**: `project_pin` | `global_current` | `path_proxy_bypass` — same semantics as `envr which` / shims.
  - **`ok`**: boolean — `false` when resolution failed for that row.
  - **`detail`**: string or `null` — error detail when `ok` is `false`.

Schema: [`data-project-status-v1.schema.json`](../schemas/data-project-status-v1.schema.json).

### `envr hook prompt` (`--format json`, `message`: `hook_prompt`)

- **Default (text)**: **stdout only** — one line, no envelope. Either empty, or a short bracketed fragment such as `[node:20.10.0 python:3.12.1] ` (note trailing space), built from the same resolution rules as `envr status`. Intended for prompts, e.g. after `eval "$(envr hook bash)"` / `eval "$(envr hook zsh)"`: `PS1='[$(_envr_prompt_segment)]\u@\h:\w\$ '` (bash) or `PS1='[$(_envr_prompt_segment)]%m:%~%# '` (zsh).
- **JSON**: **`data.segment`** — string, same content as plain stdout.

Schema: [`data-hook-prompt-v1.schema.json`](../schemas/data-hook-prompt-v1.schema.json).

### `envr-shim` (core `node`): `package.json` `engines.node` stderr hint

Not part of the CLI JSON envelope; documented here because it affects automation UX.

- When **`node`** is invoked through the envr shim, a **`package.json`** is found walking up from the current working directory, and **`engines.node`** is present and parses as a **semver `VersionReq`** that **does not** match the **active** envr-resolved Node version, the shim may print **one line to stderr** before `exec` (does **not** block execution or change the child exit code).
- **Locale**: the line uses the same **`settings.toml` `[i18n]`** rules as the CLI (`FollowSystem` / `zh_cn` / `en_us`), via `envr_core::i18n` initialized once per shim process. Message key: **`cli.shim.hint.node_engines`** in `locales/*.toml` (placeholders `{spec}`, `{active}`).
- **Opt out**: set environment variable **`ENVR_NO_NODE_ENGINES_HINT`** to any non-empty value.
- **Throttle**: at most once per **2 hours** per distinct `(canonical package.json path, engines string, active version label)`; state file under `{runtime_root}/cache` (see implementation).

### Failure behavior

- Validation and runtime errors are still emitted as JSON envelopes when `--format json` is set.
- In **text** mode, errors on stderr use a single unified line: `envr: [E_<CODE>] <message>`, where `<CODE>` is the JSON `code` token in uppercase with underscores (for example `validation` → `[E_VALIDATION]`). This matches searchable codes in logs while JSON output keeps the original snake_case `code` field.
- Process exit code remains non-zero on failure.

#### Exit code mapping (stable policy)

`envr` maps error classes to process exit codes with a centralized table in [`output::exit_code_for_error_code`](../../crates/envr-cli/src/output.rs):

| Error class (`code`) | Exit code |
|----------------------|-----------|
| `io`, `download`, `mirror` | `2` |
| `unknown`, `config`, `validation`, `runtime`, `platform` | `1` |

This table applies to failures routed through [`emit_envr_error`](../../crates/envr-cli/src/output.rs) and is regression-tested in `output.rs`.

#### Selected non-null failure `data` shapes (`code` → schema)

These use the same envelope as other failures (`success: false`, string `code`). Repository mirrors for machine checks live under `schemas/cli/data/` where noted.

#### Failure `data` tier policy (P33)

To keep automation stable while allowing forward evolution, failure codes are grouped into tiers:

- **Tier0 (strongly typed)**: `data` is a non-null object with a stable minimal required field set (schema has non-empty `required`).
- **Tier1 (loose object)**: `data` is an object but fields may be best-effort / additive (schema may have empty `required`).
- **Tier2 (nullable)**: `data` may be `null` (schema `type` includes `"null"`); scripts must not rely on `data` being present.

Regression: `crates/envr-cli/tests/failure_data_tiers_contract.rs`.
Machine-readable source of truth: `schemas/cli/governance-index.json` (`failure_tiers`).
Index schema: `schemas/cli/governance-index.schema.json`.
Capability-driven Phase A coverage scope is also indexed in `schemas/cli/governance-index.json` (`capability_test_rows`).

<!-- FAILURE_DATA_TIERS_START -->
Tier0 (strongly typed): `argv_parse_error`, `child_exit`, `project_check_failed`, `project_validate_failed`, `diagnostics_export_failed`

Tier1 (loose object): `project_sync_pending`, `shell_exit`, `aborted`

Tier2 (nullable): `validation`
<!-- FAILURE_DATA_TIERS_END -->

Shared fragment for structured error object (used by selected Tier0 failures):
`schemas/cli/fragments/error_object.json` (`data.error` with `code`, optional `kind`, `message`, `diagnostics_len`, optional `source_chain[]`).
When present, `kind` uses stable coarse categories: `validation` / `runtime` / `io` / `network` / `config` / `platform` / `unknown`.
Single source for `code -> kind` mapping: `schemas/cli/error-kind-map.json` (consumed by Rust output layer and schema fragment gate).
Mapping schema: `schemas/cli/error-kind-map.schema.json`.
Consistency check: `python scripts/check_cli_schema_fragments.py`.
Tier0 note: `argv_parse_error` is an explicit exception because `data.error` is already used as clap-rendered string payload.

| `code` | When | `data` schema (repo mirror or doc) |
|--------|------|-------------------------------------|
| `project_check_failed` | `envr check` finds pin / resolution problems | [`failure_project_check_failed.json`](../../schemas/cli/data/failure_project_check_failed.json) (mirrors [`data-project-check-failed-v1.schema.json`](../schemas/data-project-check-failed-v1.schema.json)) |
| `diagnostics_export_failed` | `envr diagnostics export` cannot write the zip | [`data-diagnostics-export-failed-v1.schema.json`](../schemas/data-diagnostics-export-failed-v1.schema.json) |
| `child_exit` | `exec` / `run` child non-zero (and similar) | Command-specific object; see command docs / tests |
| `doctor_issues` | `envr doctor` when `issues` is non-empty | Same object as success `doctor_ok` — [`data-doctor-v2.schema.json`](../schemas/data-doctor-v2.schema.json) |

Other failures usually keep `data` as `null` unless documented above or under network hints.

#### Structured hints on failure (`data`)

When `--format json` is set and **`--quiet` is not set**, some failures include a non-null **`data`** object (in addition to string `diagnostics` from the error source chain):

- **Network / download class**: if the error is classified as download, mirror, or I/O that looks network-related (e.g. connection, TLS, timeout, DNS), `data` may be:

  ```json
  { "hints": [ "<localized mirror / network suggestion>" ] }
  ```

  The same suggestion is appended to the **text** mode stderr message as a second line. With `--quiet`, JSON failure responses use a null `data` field and omit `diagnostics` as today.

- Other failures keep `data` as `null` unless a specific command documents otherwise.

Scripts should treat unknown keys inside `data` as forward-compatible.

## Compatibility policy

- New fields may be added to JSON `data` objects.
- Existing envelope top-level keys will not be removed without a **`schema_version`** bump.
- Breaking changes to a **specific** success `message`’s `data` shape: bump that command’s **`data-*-vN`** schema revision (see [schemas README](../schemas/README.md)); bump the envelope **`schema_version`** only if the outer envelope contract changes.
- Porcelain line formats above are treated as stable and must remain backward compatible.

## Migration notes

- Add a line prefixed with `Migration note:` whenever a PR introduces a breaking JSON schema contract change.
