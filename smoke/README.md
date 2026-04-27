# Windows smoke suite (PowerShell 7)

This folder contains **manual / operator-run smoke tests** for `envr` on **Windows**.

Goals:

- **Command surface coverage**: verify `envr` / `er` commands parse and `--help` works across the full tree.
- **Runtime real smoke**: verify Windows runtimes are **installable**, **switchable**, and **actually runnable** (standard library / toolchain paths), not only `--version`.

> Execution note: these scripts are intended to be executed by a human operator in a real network environment (proxy allowed). They are not wired into CI in phase 1.

## Prerequisites

- PowerShell 7 (`pwsh`)
- A built `envr` / `er` (recommended): from repo root run:

```powershell
cargo build --release
```

The suite will prefer `.\target\release\envr.exe` / `.\target\release\er.exe` and fall back to `envr.exe` / `er.exe` on `PATH`.

## Isolation model (default)

The suite runs against an isolated root directory under:

- `smoke/.state/root/`

It sets:

- `ENVR_ROOT`
- `ENVR_RUNTIME_ROOT`

so installs/caches/logs do not pollute your real envr installation.

## How to run (recommended order)

From repo root:

```powershell
pwsh -NoProfile -ExecutionPolicy Bypass -File .\smoke\pwsh\00_bootstrap.ps1
pwsh -NoProfile -ExecutionPolicy Bypass -File .\smoke\pwsh\10_commands_all.ps1
pwsh -NoProfile -ExecutionPolicy Bypass -File .\smoke\pwsh\20_runtimes_lifecycle.ps1
pwsh -NoProfile -ExecutionPolicy Bypass -File .\smoke\pwsh\30_runtime_fixtures_run.ps1
```

Each script produces structured logs under `smoke/.state/`.

`20_runtimes_lifecycle.ps1` keeps installed/current versions by default so fixtures can run.
If you want cleanup in the same pass, use:

```powershell
pwsh -NoProfile -ExecutionPolicy Bypass -File .\smoke\pwsh\20_runtimes_lifecycle.ps1 -UninstallAfter
```

Lifecycle supports step timeout too:

```powershell
pwsh -NoProfile -ExecutionPolicy Bypass -File .\smoke\pwsh\20_runtimes_lifecycle.ps1 -StepTimeoutSec 900
```

`30_runtime_fixtures_run.ps1` supports runtime slicing and per-step timeout:

```powershell
# only selected runtimes
pwsh -NoProfile -ExecutionPolicy Bypass -File .\smoke\pwsh\30_runtime_fixtures_run.ps1 -Only node,python,dotnet

# resume from a runtime name
pwsh -NoProfile -ExecutionPolicy Bypass -File .\smoke\pwsh\30_runtime_fixtures_run.ps1 -From kotlin

# set step timeout (seconds)
pwsh -NoProfile -ExecutionPolicy Bypass -File .\smoke\pwsh\30_runtime_fixtures_run.ps1 -StepTimeoutSec 900
```

`luau` is now included in both lifecycle smoke and fixture smoke:

- lifecycle probe: `luau -e "print(1+1)"`
- fixture: table iteration + numeric accumulation, expected output `6`

## Reports & logs

- `smoke/.state/report.json` — machine-readable summary (append-only sections)
- `smoke/.state/summary.md` — human-readable summary
- `smoke/.state/logs/*.log` — raw command output per step

## Expected baseline (Windows)

Use this as a quick pass/fail reference for full runs:

- `00_bootstrap.ps1`: prints resolved `envr` / `er` paths and isolated `ENVR_ROOT`.
- `10_commands_all.ps1`: completes with `OK: command help coverage complete. Paths=<N>`.
- `20_runtimes_lifecycle.ps1`: completes with `OK: runtime lifecycle smoke finished (installed/current kept for fixture run).`
- `30_runtime_fixtures_run.ps1`: expected shape is:
  - most runtimes `OK`
  - `flutter` may be `SKIP` on hosts missing required system deps
  - `fail=0`

Example of a healthy full fixture summary:

- `OK: fixtures run complete. ok=38 fail=0`
- plus one `SKIP` entry for `flutter` on constrained hosts

## Runtime fixtures

Fixtures live under `smoke/runtime-fixtures/<runtime>/` and must:

- execute runtime-provided tooling (compiler/interpreter/package manager) rather than only `--version`
- use standard library or toolchain behavior and assert deterministic output

## Full isolated test commands

From repo root, to ensure the smoke suite never pollutes your global envr state, explicitly set both `ENVR_ROOT` and `ENVR_RUNTIME_ROOT` to `smoke/.state/root/` for the current PowerShell session before running the suite:

```powershell
$env:ENVR_ROOT = (Resolve-Path .\smoke\.state\root).Path
$env:ENVR_RUNTIME_ROOT = $env:ENVR_ROOT

pwsh -NoProfile -ExecutionPolicy Bypass -File .\smoke\pwsh\00_bootstrap.ps1
pwsh -NoProfile -ExecutionPolicy Bypass -File .\smoke\pwsh\10_commands_all.ps1
pwsh -NoProfile -ExecutionPolicy Bypass -File .\smoke\pwsh\20_runtimes_lifecycle.ps1 -StepTimeoutSec 900
pwsh -NoProfile -ExecutionPolicy Bypass -File .\smoke\pwsh\30_runtime_fixtures_run.ps1 -StepTimeoutSec 900
```

If `smoke/.state/root/` does not exist yet in a fresh checkout, create it first:

```powershell
New-Item -ItemType Directory -Force -Path .\smoke\.state\root | Out-Null
$env:ENVR_ROOT = (Resolve-Path .\smoke\.state\root).Path
$env:ENVR_RUNTIME_ROOT = $env:ENVR_ROOT
```

Useful partial commands:

```powershell
# lifecycle + fixtures for luau only
$env:ENVR_ROOT = (Resolve-Path .\smoke\.state\root).Path
$env:ENVR_RUNTIME_ROOT = $env:ENVR_ROOT
pwsh -NoProfile -ExecutionPolicy Bypass -File .\smoke\pwsh\20_runtimes_lifecycle.ps1 -StepTimeoutSec 900
pwsh -NoProfile -ExecutionPolicy Bypass -File .\smoke\pwsh\30_runtime_fixtures_run.ps1 -Only luau -StepTimeoutSec 900
```

## Platform support reference

See runtime support matrix:

- `docs/runtime/platform-support-matrix.md`

This suite is **Windows-first** in phase 1.

