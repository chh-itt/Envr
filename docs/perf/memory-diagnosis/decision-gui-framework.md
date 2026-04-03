# GUI Framework Decision (GUI-103)

Date: 2026-04-02
Decision scope: `iced` continue optimization vs migrate to Dioxus

## Inputs

- GUI-100 baseline: `docs/perf/memory-diagnosis/results/2026-04-02-baseline.md`
- GUI-101 mitigation: `docs/perf/memory-diagnosis/results/2026-04-02-mitigation.md`
- GUI-102 Dioxus POC:
  - `docs/perf/memory-diagnosis/results/2026-04-02-dioxus-webview.md`
  - `docs/perf/memory-diagnosis/results/2026-04-02-dioxus-wgpu.md`

## Gate check against migration thresholds

| Gate | Target | Current evidence | Pass/Fail |
|---|---|---|---|
| Memory | Dioxus route must reduce key-scene memory >=20% and remain stable | With process-tree accounting, WebView route is ~166MB plateau and Native route is ~428MB plateau in this timed run. No consistent >=20% gain over iced is proven. | **Fail** |
| FPS / smoothness | Not worse than iced on long list | No trustworthy side-by-side FPS dataset yet (GUI-102 run was mostly timed/no-manual interaction). | **Fail** |
| Startup | Cold/hot startup not worse | Native route still shows high startup footprint; WebView does not demonstrate a clear startup advantage over iced under fair accounting. | **Fail** |
| Complexity | Migration cost and risks manageable | POC works, but production migration means re-implementing full GUI features/state/persistence and new packaging/runtime risks. | **Fail** |
| Compatibility | Win/macOS/Linux packaging and behavior clear | Only Windows POC validated; cross-platform packaging/ops not established. | **Fail** |

## Why Dioxus WebView looked much lower before fair accounting

The most likely reason is measurement scope mismatch:

1. `Private Bytes` was recorded for the launched host process only.
2. Dioxus WebView uses platform web runtime (e.g. WebView2 / browser process tree), so heavy memory can live in child/side processes.
3. Dioxus Native keeps renderer memory in-process, so the same metric captures more directly.

After switching to process-tree accounting, WebView moved from `~5.4 MB` to `~166 MB`, confirming the earlier low number was a metric-scope artifact.

## Final decision (required Go/No-Go)

**No-Go (do not migrate now).**

Rationale:

- Current data does not prove Dioxus has a stable, significant memory/perf advantage under equivalent accounting.
- Native Dioxus route currently looks worse in startup memory.
- Iced path already has validated mitigation leverage on the main pain point (S3 list growth), with lower migration risk.

## Execution plan after No-Go

1. Keep `iced` as production GUI framework.
2. Productize the validated GUI-101 mitigations gradually:
   - keep list rendering caps/tuning in Exact mode
   - keep skeleton motion controls (and tune defaults)
3. Improve measurement quality:
   - keep process-tree accounting as default for cross-framework comparison
   - add GPU Dedicated/Shared and FPS to all runs
4. Revisit migration only when a new dataset satisfies all gates above.

## Risk list

- **Measurement risk**: even process-tree sampling may miss detached helper processes depending on runtime behavior.
- **Interpretation risk**: timed/no-manual runs are useful for baseline but not full interaction parity.
- **Delivery risk**: framework migration would consume significant capacity before proving a net gain.
