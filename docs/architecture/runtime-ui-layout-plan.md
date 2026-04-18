# Runtime UI layout: order + hide (single switch)

## Goals

- **One persisted preference** controls both:
  - **Runtime hub** horizontal tab bar (`runtime_nav_bar`): which kinds appear, and left-to-right order.
  - **Dashboard** “运行时概览”: same order; **hidden** kinds are not removed from data but shown in a **trailing** section (or behind a “显示已隐藏” expander — pick one in implementation).
- **Hide** = not shown in the Runtime hub bar; on the dashboard, still recoverable at the end (or in a collapsed block).
- **Order** = display order only (no impact on installs, PATH, shims, or core behavior).
- **Restore default** = reset order to built-in descriptor order and clear all hidden flags.

## Non-goals

- Per-runtime “pin to OS PATH” or CLI behavior changes.
- Different hide semantics between dashboard and runtime hub (explicitly **not** supported — one `hidden` set).

## Persistence (`settings.toml`)

### Location

Extend `envr_config::settings::GuiSettings` (same section as `downloads_panel`) so GUI-only layout survives restarts and stays out of `runtime.*` provider settings.

### Schema (proposed)

```toml
[gui.runtime_layout]
# Permutation of runtime keys, e.g. ["node","python",...]. Empty = built-in default order.
order = []
# Keys hidden from Runtime hub; on dashboard they appear only in the trailing / “hidden” region.
hidden = []
```

### Wire types (`envr-config`)

- Add `RuntimeLayoutSettings { order: Vec<String>, hidden: Vec<String> }` under `GuiSettings`, both `#[serde(default)]`.
- Keys MUST match `RuntimeDescriptor::key` (`"node"`, `"python"`, …) for stable serde across enum refactors.
- **`Settings::validate`**: optional light validation (no unknown keys; no duplicates; subset of known keys). On invalid file, prefer **normalization** (drop unknown, dedupe, append missing kinds in default order) rather than hard-failing startup — log or surface soft warning in GUI if needed.

### Default resolution (`envr-gui` helper, or `envr-config` if reused by CLI later)

1. Start from `RUNTIME_DESCRIPTORS` iteration order as **canonical default order**.
2. If `order` is empty → use canonical order.
3. If `order` is non-empty → treat as user permutation; **merge** any new kinds not present in file (future-proof) by appending them in canonical order after existing entries.
4. `hidden` → `HashSet<&str>` for membership; only known keys kept.

Exported helpers (suggested module `crates/envr-gui/src/runtime_layout.rs` or `envr-config` if you want CLI parity):

- `fn effective_order(layout: &RuntimeLayoutSettings) -> Vec<RuntimeKind>`
- `fn visible_kinds(layout: &RuntimeLayoutSettings) -> Vec<RuntimeKind>` — `effective_order` minus `hidden`
- `fn dashboard_rows_order(layout, rows: &[RuntimeRow]) -> Vec<RuntimeRow>` — reorder `rows` to match `effective_order`, then **partition** into `[visible | hidden]` where `hidden` membership uses the same `hidden` set (relative order preserved within each partition).

## GUI behavior

### Runtime hub (`view/runtime_nav/mod.rs`)

- Replace `for kind in runtime_kinds_all()` with `for kind in visible_kinds(&layout)`.
- **Active kind becomes hidden** (e.g. after editing in Settings): on next render or layout change, if `env_center.kind` ∉ visible set → set `env_center.kind = visible_kinds[0]` (or first in order) and fire the same side effects as `PickKind` (reuse a small internal `fn switch_runtime_kind(state, k)` to avoid duplicating `handle_env_center`).

### Dashboard (`view/dashboard/panel.rs`)

- Pass **layout snapshot** + optional **edit mode flag** into `dashboard_view` (signature change in `shell/mod.rs`).
- Replace flat `runtime_overview_card` caption list with **per-runtime cards** (reuse `card_container_style` / spacing tokens).
- **Normal mode**
  - Primary grid/list: **visible** runtimes in `dashboard_rows_order` visible partition.
  - **Hidden tail**: same card style but muted / smaller section title, e.g. “已隐藏（仍可在设置中恢复）”, only the hidden partition.
  - Optional compact toggle: “显示已隐藏详情” if you want collapsed-by-default tail — product choice; default ON tail is simpler MVP.
- **Card interactions**
  - **Click card body** → `Task::batch([Navigate(Runtime), PickKind(kind)])` or a single `Message::OpenRuntime(kind)` that sets route + kind in one `update` turn (prefer **one message** to avoid double task ordering issues).
  - **Hide** button → toggle key in `hidden`, save `settings.toml` (same path as other GUI persistence).
  - **⋯ / More** → optional: “在设置中编辑顺序” → `Navigate(Settings)` + scroll/focus later (phase 2); MVP can skip.
- **Edit layout mode** (local UI state, not necessarily persisted — e.g. `DashboardState.layout_editing: bool`)
  - Toggle button near overview title: “编辑布局” / “完成”.
  - While editing: show **↑ / ↓** on each card (visible + hidden sections) to swap positions in **`order`** list (full permutation, including hidden kinds — so hidden retain relative order when un-hidden).
  - **Do not** navigate to Runtime on card press while editing (or use explicit “进入” button) to avoid conflicting with reorder.
  - Exit edit mode: keep `order` changes already written, or batch-write on “完成” — **recommend write on each change** with debounce optional; simplest is write immediately like downloads panel position.

### Settings page (authoritative editor)

- New subsection under existing GUI or a dedicated “运行时显示” block:
  - List all runtimes with visibility toggles + up/down (mirrors dashboard edit mode).
  - **恢复默认** button → clear `order` + `hidden` in draft, save.
- Reuse the same `RuntimeLayoutSettings` read/write path as dashboard (single source of truth).

### Saving to disk

- Mirror the pattern used for downloads panel: read `Settings` from `settings_path()`, mutate `st.gui.runtime_layout`, `save_to`.
- Ensure `SettingsViewState` cache stays in sync when saving from Dashboard (either call shared `persist_gui_runtime_layout` that updates `state.settings.cache` if already loaded, or reload from disk after save — pick the smallest consistent approach with existing `gui_ops` / `handle_settings` flows).

## Messages / update loop

- Prefer **`Message::RuntimeLayout(RuntimeLayoutMsg)`** (new enum) over overloading `DashboardMsg` / `SettingsMsg`, so both dashboard and settings can emit the same handler:
  - `ToggleHidden(RuntimeKind)` or `SetHidden { kind, hidden: bool }`
  - `MoveKind { kind, dir: Up | Down }` (operates on full `order` permutation)
  - `ResetToDefault`
  - `SetDashboardLayoutEditing(bool)` if edit mode lives in `DashboardState`
- Handler responsibilities:
  1. Mutate `state.settings.cache.snapshot_mut()` or rebuild draft — align with how `gui.downloads_panel` mutations work (`app.rs` ~1206).
  2. `save` to `settings.toml` (blocking task already used elsewhere).
  3. If on Runtime route, **re-filter** nav; fix `env_center.kind` if now hidden.

## i18n

- Add keys under `gui.runtime_layout.*` / `gui.dashboard.*` in `locales/zh-CN.toml` and English counterpart for: edit mode, hide, show, hidden section title, reset default, tooltips.

## Testing

- **Unit tests** (`envr-config`): serde default; validate + normalize weird `order`/`hidden` inputs.
- **Unit tests** (`envr-gui` or `envr-config`): `effective_order` / `visible_kinds` / partition logic with a fake layout.
- **Manual**: hide active runtime → first visible tab selected; dashboard tail shows hidden; restart app → persisted.

## Implementation phases (recommended)

1. **Config + resolution helpers** — structs, merge, validation; no UI change yet (or wire into nav only behind default layout).
2. **Runtime hub** — consume `visible_kinds`; fix selection when hidden.
3. **Dashboard cards** — layout + click-through; hide toggle + persist.
4. **Dashboard edit mode** — up/down reorder.
5. **Settings** — mirror editor + restore default.
6. **Polish** — optional “⋯”, collapsed hidden section, debounced save.

## Files likely touched

| Area | Files |
|------|--------|
| Config | `crates/envr-config/src/settings.rs`, `templates/settings.schema.zh.toml` |
| GUI state / routing | `crates/envr-gui/src/app.rs`, `crates/envr-gui/src/view/shell/mod.rs` |
| Dashboard | `crates/envr-gui/src/view/dashboard/panel.rs`, `state.rs` |
| Runtime nav | `crates/envr-gui/src/view/runtime_nav/mod.rs` |
| Settings UI | `crates/envr-gui/src/view/settings/*.rs` |
| Persistence helper | `crates/envr-gui/src/gui_ops.rs` (new `save_runtime_layout` task) |
| i18n | `locales/zh-CN.toml`, `locales/en-US.toml` (if present) |

## Open choices (decide during implementation)

1. **Hidden section on dashboard**: always visible tail vs collapsed “显示已隐藏”.
2. **Save strategy**: immediate write on each change vs explicit “保存” on Settings (dashboard can still immediate-write to match downloads panel UX).
3. **Drag-and-drop**: not in v1 unless an existing iced extension is already in the repo; **↑↓** is enough for MVP.

## UI / visual direction (2026)

- **Cards**: elevated surface (`card_container_style` elevation 1–2), generous padding, **subtle hairline border** + soft shadow; **rounded corners** from tokens; **muted secondary line** for “installed count · current version”.
- **Hierarchy**: clear title (runtime display name), one-line summary, optional **left accent** (3–4 px primary strip) so scan speed stays high in long lists.
- **Interaction**: whole card is a **large hit target** with pointer cursor; **hover** feedback via ghost/secondary button styling or container-adjacent affordance (chevron / “进入”).
- **Edit layout**: entering edit mode reveals **icon-only ↑ / ↓** (or grip + arrows) with **44px-class targets** where possible; **drag-with-mouse** is optional follow-up (would need extra widget crate or custom drag layer) — ship polished **press-to-reorder** first.
- **Density**: avoid cramped caption-only list; prefer **one card per runtime**, breathing room between cards (`spacing` from tokens).
- **Hidden block**: visually **de-emphasized** (muted border/text); optional **disclosure** control to collapse the hidden stack so the dashboard stays calm.
- **Motion**: respect `reduce_motion` / zero motion tokens for any future transitions.
