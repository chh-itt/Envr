# Runtime Registry Phase 1 Design

## Goals

- Decouple `envr-core` from the full `envr-runtime-*` dependency list.
- Keep behavior stable: default build still registers all runtimes.
- Introduce explicit feature-gated runtime registration for controllable builds.
- Avoid runtime auto-registration mechanisms (`inventory`/`ctor`) in this phase.

## Scope

- In scope:
  - Add `envr-runtime-registry` crate to own default provider construction.
  - Move provider list assembly from `envr-core` into registry.
  - Add per-runtime Cargo features and pass-through feature mapping in `envr-core`.
- Out of scope:
  - Dynamic runtime discovery at startup.
  - `inventory`/`ctor` based self-registration.
  - Refactor unrelated provider APIs.

## Module Topology

```text
envr-cli / envr-gui
        |
        v
    envr-core  -----> envr-runtime-php (direct helper API still used)
        |
        v
envr-runtime-registry (feature-gated provider factory list)
        |
        v
  envr-runtime-* crates (optional dependencies)
```

## Feature Model

- `envr-runtime-registry`
  - `default` enables all runtime features.
  - each feature maps to one optional runtime dependency:
    - `runtime-node` -> `dep:envr-runtime-node`
    - `runtime-python` -> `dep:envr-runtime-python`
    - ... (same pattern for all supported runtimes)
- `envr-core`
  - `default` enables all runtime features.
  - each feature pass-through maps to registry:
    - `runtime-node` -> `envr-runtime-registry/runtime-node`
    - `runtime-python` -> `envr-runtime-registry/runtime-python`
    - ...
  - `envr-runtime-registry` is consumed with `default-features = false` so `envr-core` owns the final runtime selection.

## Runtime Registration Flow

1. Caller builds `RuntimeService` via `with_defaults()` or `with_runtime_root(...)`.
2. `envr-core` calls `envr_runtime_registry::default_provider_boxes(...)`.
3. `envr-runtime-registry` appends providers conditionally with `#[cfg(feature = "...")]`.
4. `RuntimeService::new(...)` validates duplicates and exposes normal query/install ports.

This keeps runtime assembly deterministic and fully compile-time controlled.

## Migration Steps (Phase 1)

1. Create `envr-runtime-registry` crate.
2. Move hardcoded provider list from `envr-core::runtime::service` to registry.
3. Replace `envr-core` provider construction with registry call.
4. Remove direct `envr-runtime-*` deps from `envr-core` that were only used for default list assembly.
5. Add feature pass-through in `envr-core`.
6. Run formatting, checks, and selective feature build verification.

## Regression Checklist

- Baseline checks:
  - `cargo check -p envr-runtime-registry`
  - `cargo check -p envr-core`
- Feature slicing check:
  - `cargo check -p envr-core --no-default-features --features runtime-node`
- Behavior invariants:
  - Default build exposes full runtime set.
  - Runtime root override remains effective for provider initialization.
  - RuntimeService duplicate-provider guard remains unchanged.

## Why Stop at Phase 1

- Phase 1 already gives stable decoupling and feature-controlled compilation.
- It keeps startup/runtime semantics simple and debuggable.
- It avoids introducing global constructor ordering and link-time collection complexity too early.
