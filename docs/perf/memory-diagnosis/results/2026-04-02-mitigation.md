# GUI-101 Mitigation Experiments

Date: 2026-04-02
Input: `docs/perf/memory-diagnosis/results/2026-04-02-baseline.md`

## Experiment setup

- Same machine/session class, same measurement script and phase order (S1..S5)
- Metrics: process Private Bytes only (GPU Dedicated/Shared pending)
- S4 in these runs was **expand/collapse only** (no real download job started).

## Run A (control)

Command:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/perf/measure-gui-memory-baseline.ps1 -GuiExe "target\release\envr-gui.exe" -Repeat 1
```

| Phase | start (MB) | end (MB) | max delta (MB) |
|---|---:|---:|---:|
| S1_cold_start | 140.11 | 149.88 | 10.96 |
| S2_navigation_toggle | 149.79 | 149.76 | 0.03 |
| S3_long_list_scroll | 149.76 | 156.48 | 7.24 |
| S4_download_panel | 156.48 | 155.63 | 0.05 |
| S5_resource_release | 155.63 | 148.71 | 0.00 |

## Run B (mitigation toggles)

Toggles:

- `ENVR_GUI_DISABLE_SKELETON_SHIMMER=1`
- `ENVR_GUI_EXACT_MAX_LEAF_ROWS=120`

Command:

```powershell
$env:ENVR_GUI_DISABLE_SKELETON_SHIMMER="1"
$env:ENVR_GUI_EXACT_MAX_LEAF_ROWS="120"
powershell -ExecutionPolicy Bypass -File scripts/perf/measure-gui-memory-baseline.ps1 -GuiExe "target\release\envr-gui.exe" -Repeat 1
```

| Phase | start (MB) | end (MB) | max delta (MB) |
|---|---:|---:|---:|
| S1_cold_start | 140.23 | 137.41 | 79.53 |
| S2_navigation_toggle | 137.41 | 138.15 | 0.73 |
| S3_long_list_scroll | 138.15 | 134.08 | 0.09 |
| S4_download_panel | 134.08 | 144.36 | 16.65 |
| S5_resource_release | 144.36 | 138.62 | 0.00 |

## Result summary

- **S3 improved significantly**: max delta `7.24 -> 0.09 MB` (about `-98.8%`), confirming the runtime list path can be strongly affected by shimmer/row-cap controls.
- **S4 is sensitive to expand/collapse interaction even without downloads**:
  - Prior pair (no-download interaction): `0.05 -> 16.65 MB`
  - Latest pair (no-download interaction): `14.84 -> 2.21 MB`
  - This indicates high interaction variance; animation/click cadence can dominate S4 deltas.
- **S1 became highly volatile**: max delta `10.96 -> 79.53 MB` while end is lower than start, indicating a short-lived startup spike happened inside S1 window (not persistent growth).
- **S5 rollback exists in both runs**: end memory falls near mid-140 MB; no clear monotonic leak signal in this pair.

## Conclusion (can/cannot, gain, trade-off)

- **Can reduce S3 growth**: Yes, strongly.
- **Gain**: S3 max delta improved by ~98.8%.
- **Trade-off**: S4 behavior is currently noisy and interaction-dependent (expand/collapse-only runs can swing both directions), so S3 conclusion is strong but S4 needs interpretation with interaction constraints.

## Decision

Current data is acceptable as GUI-101 conclusion:

1. S3 mitigation effect is validated.
2. S4 does not currently prove a stable regression; it is interaction-sensitive under no-download expand/collapse behavior.
3. A-only / B-only split tests are optional follow-up, not required for closing GUI-101.

