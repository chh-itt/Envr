//! Application layer: **services / use-cases** invoked by CLI adapters (`commands/*`).
//!
//! This module is intentionally **IO-free** (no stdout/stderr printing). It encapsulates:
//! - Business orchestration around [`envr_core::runtime::service::RuntimeService`]
//! - Project/config loading for commands
//! - Error mapping to `EnvrError` (but not envelope emission)
//!
//! The CLI layer (`commands/*`) remains responsible for:
//! - Clap argv → request adaptation
//! - Interactive prompts / TTY behavior
//! - Presenter/UX policy and output emission

pub mod runtime_installation;
