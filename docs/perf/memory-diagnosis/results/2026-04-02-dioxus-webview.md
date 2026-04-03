# GUI-102 Dioxus WebView Result

Date: 2026-04-02
Renderer route: Dioxus Desktop (WebView)

## Build / run status

- POC path: `experiments/dioxus-poc`
- Binary: `dioxus-poc-webview`
- Build: `cargo check` and `cargo build` passed
- Scenario parity: empty page, nav switch, list 5k/10k, panel toggle simulation

## Metrics table (same header as iced baseline)

| Phase | Private Bytes start (MB) | Private Bytes end (MB) | Private Bytes max delta (MB) | GPU Dedicated/Shared (start/end) | Notes |
|-------|-----------------------------|--------------------------|---------------------------------|------------------------------------|-------|
| S1_cold_start | 166.27 | 166.63 | 2.77 | TBD | |
| S2_navigation_toggle | 167.17 | 166.41 | 0.00 | TBD | |
| S3_long_list_scroll | 166.36 | 166.01 | 0.00 | TBD | |
| S4_download_panel | 166.01 | 165.14 | 0.62 | TBD | no real download, toggle simulation only |
| S5_resource_release | 165.45 | 165.03 | 0.00 | TBD | |

## Notes

- This run used the shared script in timed phases; no manual S2/S3/S4 interaction was performed.
- This run used **process-tree Private Bytes** metric; values now include child processes (fairer for WebView architectures).
