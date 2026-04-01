# Performance baselines (T903)

This folder documents **CLI cold-start style checks** and how to detect regressions. GUI FPS/CPU/memory probes are described in `baseline.md` and align with goals in `refactor docs/03-gui-设计.md` (responsiveness and resource use).

- **`baseline.md`** — metric definitions, thresholds philosophy, and what to do when a run fails.
- **Repository script** — `scripts/perf/measure-cli-smoke.ps1` (Windows); adapt timing for POSIX with `time` / `hyperfine` if needed.

CI does not block merges on perf yet; local or scheduled runs compare against `scripts/perf/baseline.json`.
