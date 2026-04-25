# Download Control Plane Blueprint

## Goals

- Centralize download-side transport, scheduling, and throttling in `envr-download`.
- Make runtime providers declare *what to fetch*, not *how to construct clients or regulate concurrency*.
- Keep compatibility with existing runtime behavior while upgrading performance and stability.

## Target Architecture

### 1) Control Plane (`envr-download`)

- **HTTP client pool**
  - keyed by client profile (`user_agent`, timeout, protocol hints)
  - returns shared `reqwest::Client` / `reqwest::blocking::Client` instances
  - removes repeated TCP/TLS setup cost in batch operations

- **Global concurrency guard**
  - process-wide max in-flight downloads
  - backpressure for large batch installs (`project sync --install`)
  - avoid network saturation and disk contention spikes

- **Global rate limiter**
  - process-wide bytes/sec budget (already exists)
  - applied uniformly in blocking + async pipelines

- **Retry/backoff policy**
  - shared policy primitives in `envr-download::task`
  - runtime providers can override per-task policy only when needed

### 2) Runtime Provider Contract

- Providers should use pooled client constructors from `envr-download`.
- Providers should route artifact transfers through `download_url_to_path_resumable*`.
- Providers should avoid local ad-hoc concurrency controls for transport.

## Module Layout (target)

- `envr-download/src/blocking.rs`
  - blocking client pooling
  - blocking transfer path + global limiter hooks
- `envr-download/src/engine.rs`
  - async transfer path + global limiter hooks
- `envr-download/src/global_limit.rs`
  - bandwidth limiter + concurrency limiter
- `envr-download/src/task.rs`
  - retry policy and task state machine

## Migration Plan

### Phase A (now)

- add blocking client pooling
- add process-wide blocking download concurrency guard
- wire CLI/GUI startup settings to both bandwidth + concurrency limits
- keep runtime APIs unchanged

### Phase B

- unify async `DownloadEngine::default_client` with pooled async clients
- apply global concurrency guard to async transfer path
- normalize runtime-specific client construction to profile-based pool usage

### Phase B Status (implemented)

- `envr-download::engine::DownloadEngine::default_client` now uses a process-wide async client pool.
- `envr-download::engine::DownloadEngine::download_to_file` now acquires a global async concurrency permit (when configured via `set_global_download_concurrency_limit`).
- `set_global_download_concurrency_limit` now configures both:
  - blocking path limiter (`GlobalDownloadConcurrencyLimiter`)
  - async path semaphore (`tokio::sync::Semaphore`)

### Phase C

- add queue priority classes (`index`, `artifact`, `prefetch`)
- add instrumentation counters (`pool hit`, queue wait, retry count, throughput)
- expose diagnostics in `doctor` / telemetry output

### Phase C Status (in progress)

- Added process-wide control-plane counters in `envr-download`:
  - pool hit/miss (blocking + async)
  - queue wait events/total wait time (blocking + async)
  - retry scheduled count
  - in-flight current/peak (blocking + async)
- Added public snapshot API: `snapshot_download_control_plane_stats()`.
- Exposed minimal diagnostics in CLI `envr debug info` output (`download_control_plane`).
- Added queue priority classes in global download concurrency control:
  - `DownloadPriority::Index`
  - `DownloadPriority::Artifact` (default)
  - `DownloadPriority::Prefetch`
- Priority scheduling policy is strict-preference under contention (`index > artifact > prefetch`) across both blocking and async download paths.
- Migrated unified remote index refresh paths to `Index` priority for:
  - Node, Go, PHP, Deno, Bun, Elixir, Erlang
  - (artifact installation downloads remain `Artifact` by default)
- Tagged GUI demo/background HTTP job path as `Prefetch` priority.
- Added scheduler tests in `envr-download`:
  - priority gating logic (`can_acquire_respects_priority_under_contention`)
  - async/blocking permit roundtrip (`acquire_async_and_blocking_roundtrip`)

## Expected User-Visible Effect

- faster warm-path index fetches in batch operations (connection reuse)
- smoother multi-runtime installs (bounded concurrency)
- fewer burst failures under weak network conditions (global shaping)
- more predictable completion time instead of spiky bandwidth behavior

## Risk Notes

- profile key design must avoid accidental over-sharing of incompatible client configs
- concurrency defaults must be conservative enough for laptop/CI environments
- queueing should not starve metadata/index requests behind large artifact downloads
