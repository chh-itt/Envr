# Performance baseline and regression handling (T903)

## Scope

| Area | What we measure | Primary signal |
|------|-----------------|----------------|
| CLI | Wall time for cheap commands (`--help`, `doctor --format json`) | P95 vs stored baseline |
| GUI | Startup, frame time, CPU while idle (manual or future harness) | Subjective + future automation |

GUI specifics follow `refactor docs/03-gui-设计.md` (smooth interaction, bounded resource use).

## CLI baseline file

- Path: `scripts/perf/baseline.json`
- Fields (milliseconds, upper bound for **local dev machine class**; relax in CI if needed):
  - `help_ms_max` — `envr --help`
  - `doctor_json_ms_max` — `envr doctor --format json` (includes runtime service work)

The script `scripts/perf/measure-cli-smoke.ps1` runs each command several times, drops the first run (warm JIT/cache), takes the median, and **exits non-zero** if any median exceeds the JSON cap.

## Regression workflow

1. Run `pwsh -File scripts/perf/measure-cli-smoke.ps1` from the repo root (with `envr` on `PATH` or pass `-EnvrPath`).
2. If it fails, capture: OS, CPU model, `envr --version`, and the script output.
3. If the change is intentional (e.g. new doctor checks), update `baseline.json` in the same PR with a short justification in the commit message.
4. For large regressions (>2× baseline), file an issue and treat as release-blocking unless waived.

## Future work

- Optional GitHub `workflow_dispatch` job calling the same thresholds on a fixed runner pool.
- GUI: trace-based or screenshot-diff harness once stable.
