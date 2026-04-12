# CLI scripting integration guide

This guide focuses on automation-first usage of `envr` in CI/CD, remote hosts, and local scripts.

For machine-parseable guarantees, see `docs/cli/output-contract.md`.
For declarative settings automation, see `docs/cli/config.md`.
For offline usage and portable bundles, see `docs/cli/offline.md` and `docs/cli/bundle.md`.

## Output modes for scripts

Use one of these modes in scripts:

- `--porcelain` (alias: `--plain`): plain text, no labels/decorations.
- `--format json`: stable JSON envelope with `schema_version`, `success`, `code`, `message`, `data`, `diagnostics` (see `docs/schemas/` and `docs/cli/output-contract.md`).

Recommended rule:

- For single-value shell substitutions, prefer `--porcelain`.
- For robust control flow and diagnostics in CI pipelines, prefer `--format json`.

## Shell completion

`envr completion <shell>` prints a completion script to **stdout** (clap-generated). Typical use:

- Bash: `source <(envr completion bash)` or save the output to a file sourced from `.bashrc`.
- Zsh: `source <(envr completion zsh)` (often placed under `fpath` as `_envr`).
- Fish: `envr completion fish | source`.
- PowerShell: evaluate the printed script (see `envr completion powershell --help`).

Place global flags before `completion` as usual; the subcommand only prints the shell script on stdout (never JSON).

## Download and install progress vs scripting flags

Runtime installs and special installers (for example **managed rustup** via `envr rust install-managed`) can report **byte progress** while fetching archives:

- **Live stderr meter** (spinner / percentage): only when **default text output**, **`--quiet` is off**, **`--porcelain` / `--plain` is off**, and **stderr is a TTY**. This avoids flooding CI logs with `\r` updates.
- **`--format json`**: no live meter; success and failure are still expressed in the JSON envelope on stdout (see `docs/cli/output-contract.md`).
- **`--quiet`**: suppresses normal human-oriented messages; errors still go through the usual error path (stderr in text mode, envelope in JSON mode).
- **`--porcelain`**: plain, script-friendly text when not in JSON mode; no decorative labels; **no** live download meter (same gate as above).

The GUI uses the same progress atomics in the download panel for runtime installs and for **Install stable** (managed rustup), including cancel from the panel.

## Command shape (runtime-targeted verbs)

Runtime operations follow a consistent shape:

- `envr install <runtime> <version>`
- `envr use <runtime> <version>`
- `envr uninstall <runtime> <version>`
- `envr list [runtime]`
- `envr current [runtime]`
- `envr remote [runtime] [--prefix <prefix>]`

Rust-specific (when **no** system `rustup` is installed; see `docs/runtime/rust.md`):

- `envr rust install-managed` — downloads `rustup-init` and installs envr-managed rustup with a **stable** default toolchain (honours `runtime.rust.download_source` / mirror env). Fails if a system rustup is already present.

Advanced scope-wide commands keep their flag style:

- `envr exec --lang <runtime> ...`
- `envr run ...`
- `envr env ...`

## Short commands and `er`

Top-level subcommands have **visible aliases** (shown in `envr --help`), for example:

- `envr i …` → `install`
- `envr u …` → `uninstall`
- `envr ls …` → `list`
- `envr sw …` → `use` (switch)
- `envr cur …` → `current`
- `envr doc` → `doctor`
- `envr cfg …` → `config`
- `envr sh …` → `shim`
- `envr c …` → `cache`

Nested shortcuts (expanded before parsing):

- `envr diag …` / `envr dx …` → `diagnostics export …`
- `envr ci …` → `cache index sync …`
- `envr cis …` → `cache index status …`

User-defined aliases in `config/aliases.toml` (see `envr alias add`) are expanded **first** (only the first token after `envr`, up to 8 chained lookups). The first token is chosen **after** known global flags (`--format`, `--porcelain` / `--plain`, `--quiet`, `--no-color`, `--runtime-root`, and `=` forms), and after a bare `--` (POSIX “end of options”) if you use it. Built-in argv shorthands (`diag`, `ci`, `cis`) follow the same rule. A user alias with the same name as a built-in shorthand therefore **replaces** that shorthand for your `ENVR_ROOT`. Example: `envr alias add ci doctor` makes `envr ci …` run `doctor` instead of `cache index sync`.

The crate also installs a **`er`** binary: same arguments as `envr`, but two letters to type. It prefers `envr` in the **same directory** as `er` (typical `cargo install` / release folder); otherwise it falls back to `envr` on `PATH`.

## Porcelain examples

### Resolve executable path

```bash
NODE_PATH="$(envr --porcelain which node)"
echo "$NODE_PATH"
```

### Read current version

```bash
CURRENT_NODE="$(envr --porcelain current node)"
echo "$CURRENT_NODE"
```

### List installed versions

```bash
envr --porcelain list node
```

### Resolve runtime home (for tool wiring)

```bash
NODE_HOME="$(envr --porcelain resolve node --path .)"
echo "$NODE_HOME"
```

## JSON envelope examples

### Validation-aware flow

```bash
OUT="$(envr --format json list not-a-runtime || true)"
echo "$OUT"
# parse `code=validation` / `diagnostics` in your JSON parser
```

### CI-friendly install result

```bash
envr --format json install node 24
```

## JSON parsing pattern (pipelines & CI)

1. **Read `schema_version`** on the envelope (currently `2`). If it changes, re-read `docs/cli/output-contract.md` and `docs/schemas/README.md`.
2. **Branch on `success`**, then on `code` when `success` is false (`validation`, `runtime`, `download`, …).
3. **Branch on `message`** to pick the shape of `data` (stable English tokens such as `list_installed`, `child_completed`, `update_info`). Schemas live under [`docs/schemas/`](../schemas/README.md).
4. When `data` is JSON `null`, there is no structured payload; use `message`, `code`, and `diagnostics`.

### Extract the JSON line (logs may mix stderr / tracing)

Prefer **last** line that parses as JSON, or the **first** line starting with `{` (see `crates/envr-cli/tests/json_envelope.rs`).

### POSIX: gate CI on `check` + JSON

```bash
set -euo pipefail
export ENVR_RUNTIME_ROOT="${ENVR_RUNTIME_ROOT:?set a writable runtime root}"
envr --format json check --path . | tail -n1 | python3 -c "
import json, sys
line = sys.stdin.read().strip().splitlines()[-1]
o = json.loads(line)
assert o.get('schema_version') == 2
sys.exit(0 if o.get('success') else 1)
"
```

### PowerShell: read `exit_code` from `exec` / `run`

```powershell
$env:ENVR_RUNTIME_ROOT = "D:\path\to\envr-data"
$line = envr --format json run -- cmd /c echo ok | Select-Object -Last 1
$o = $line | ConvertFrom-Json
if ($o.schema_version -ne 2) { throw "unexpected schema_version" }
if (-not $o.success) { throw $o.message }
$o.data.exit_code
$o.data.auto_installed   # runtimes installed via --install-if-missing (may be @())
```

### `exec` / `run` JSON metadata

For `child_completed` and `child_exit`, `data` always includes:

- `install_if_missing` (bool) and `auto_installed` (array of `{ "kind", "version" }`) for traceability in CI.
- `exec` also sets `data.lang` (`--lang`). `run` omits `lang`.

### Optional: validate against schemas locally

```bash
cargo build -p envr-cli -q
pip install jsonschema referencing
python scripts/validate_cli_json_contract.py
```

## PATH diagnostics and self-healing hints

`envr doctor` now includes shell-aware PATH suggestions when `shims` is not on PATH.
It prints a direct command for detected shell kind (`powershell`, `cmd`, or `posix`) so users can copy-paste immediately.

## Notes

- `--porcelain` is ignored when `--format json` is set (JSON mode wins).
- Keep scripts explicit about `runtime` and `version`; avoid interactive assumptions.
