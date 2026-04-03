# GUI-102 Dioxus WGPU/Native Result

Date: 2026-04-02
Renderer route: Dioxus Native (`dioxus-native`)

## Build / run status

- POC path: `experiments/dioxus-poc`
- Binary: `dioxus-poc-wgpu`
- Build: `cargo check` and `cargo build` passed
- Scenario parity: empty page, nav switch, list 5k/10k, panel toggle simulation

## Metrics table (same header as iced baseline)

| Phase | Private Bytes start (MB) | Private Bytes end (MB) | Private Bytes max delta (MB) | GPU Dedicated/Shared (start/end) | Notes |
|-------|-----------------------------|--------------------------|---------------------------------|------------------------------------|-------|
| S1_cold_start | 422.71 | 427.96 | 5.50 | TBD | |
| S2_navigation_toggle | 427.96 | 427.51 | 0.00 | TBD | |
| S3_long_list_scroll | 427.51 | 427.29 | 0.00 | TBD | |
| S4_download_panel | 427.29 | 427.46 | 0.18 | TBD | no real download, toggle simulation only |
| S5_resource_release | 427.46 | 0.00 | 0.00 | TBD | process exited before phase end sample |

## Notes

- This run used the shared script in timed phases; no manual S2/S3/S4 interaction was performed.
- This run used **process-tree Private Bytes** metric.
- Startup memory is still much higher than WebView route under the same metric scope.
