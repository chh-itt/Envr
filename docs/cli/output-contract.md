# CLI output contract (stable for scripts)

This document defines script-facing output guarantees for `envr`.

## Scope

These guarantees are for automation usage only:

- `--porcelain` (alias: `--plain`)
- `--format json`

Human-readable default text output is not a stability target for parsers.

## Precedence

If both flags are provided, JSON mode wins:

- `--format json` > `--porcelain`

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

All JSON responses use a single-line envelope:

```json
{
  "schema_version": 2,
  "success": true,
  "code": null,
  "message": "some_message",
  "data": {},
  "diagnostics": []
}
```

### Fields

- `schema_version`: integer (currently **2**). v2 renames list/current/remote `data` keys (see below). Scripts should read this before assuming `data` layout; it will increment on breaking contract changes.
- `success`: boolean
- `code`: nullable string (error code token on failure)
- `message`: stable message key / short result id
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

## Compatibility policy

- New fields may be added to JSON `data` objects.
- Existing envelope top-level keys will not be removed without a **`schema_version`** bump.
- Breaking changes to documented `data` shapes require a **`schema_version`** bump and updated schemas under `docs/schemas/`.
- Porcelain line formats above are treated as stable and must remain backward compatible.
