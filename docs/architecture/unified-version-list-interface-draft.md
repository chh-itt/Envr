# Unified Version List Interface Draft

This draft defines a single, runtime-agnostic version-list pipeline for GUI rendering.

## Goals

- One rendering model for Node/Python/Java/Go/Ruby/Elixir/Erlang/PHP/Deno/Bun/.NET.
- Lazy loading by default:
  - Do not load list data before entering Runtime page.
  - Do not load child rows before expanding one major row.
- Stale-while-revalidate UX:
  - Render cache first.
  - Refresh in background.
  - Merge new rows without blanking existing rows.
- Keep only installable rows (host/platform compatible assets).

## Data Model

```rust
pub struct MajorRowVm {
    pub runtime: RuntimeKind,
    pub major_key: String,            // "25", "3", "27", "1.23"
    pub latest_installable: Option<String>,
    pub installed_versions: Vec<String>,
    pub current_version: Option<String>,
    pub expandable: bool,
    pub expanded: bool,
    pub children_loaded: bool,
}

pub struct ChildRowVm {
    pub runtime: RuntimeKind,
    pub major_key: String,
    pub version: String,              // full version, e.g. "25.9.0", "27.3.4.10"
    pub installed: bool,
    pub current: bool,
}
```

## Runtime Adapter Contract

```rust
pub trait VersionListAdapter {
    fn kind(&self) -> RuntimeKind;

    // Main row (collapsed list): one row per major line.
    fn load_major_rows_cached(&self) -> EnvrResult<Vec<MajorVersionRecord>>;
    fn refresh_major_rows_remote(&self) -> EnvrResult<Vec<MajorVersionRecord>>;

    // Child rows (expanded list): full versions under one major.
    fn load_children_cached(&self, major_key: &str) -> EnvrResult<Vec<VersionRecord>>;
    fn refresh_children_remote(&self, major_key: &str) -> EnvrResult<Vec<VersionRecord>>;

    // Installability filter to avoid "shown but cannot install" rows.
    fn is_installable_on_host(&self, version: &VersionRecord) -> bool;
}

pub struct MajorVersionRecord {
    pub major_key: String,
    pub latest_installable: Option<String>,
}

pub struct VersionRecord {
    pub version: String,
}
```

## Cache Contract

- Cache only fields needed for rendering:
  - `major_key`, `latest_installable`, `version`.
- Suggested TTL:
  - Major rows: 10-30 minutes.
  - Child rows: 5-15 minutes.
- Keep env override for pagination coverage (for tag/list based upstreams).

## GUI Flow

1. Enter Runtime page:
   - Read major cache, render immediately.
   - Start background refresh for majors.
2. Refresh result:
   - Merge by `major_key`.
   - Keep existing rows on transient error.
3. Expand major row:
   - If no child cache: fetch remote children.
   - If child cache exists: render first, then refresh in background.
4. Leave Runtime page:
   - Drop render-only VM state.
   - Keep cache.

## Merge Rules

- Never clear visible list before replacement data is ready.
- Preserve expansion state if major key still exists.
- If current version exists but installed scan is temporarily stale, inject current into VM as fallback.

## Error Isolation

- Major refresh failure:
  - Keep current major rows.
  - Show inline non-fatal warning.
- Child refresh failure:
  - Affect only that expanded major row.
  - Keep other rows interactive.

## Instrumentation Hooks

Add metrics for:

- time to first major row render
- time to major refresh complete
- time to child row first render after expand
- visible major rows count
- visible child rows count
- cache hit ratio for major/child loads
