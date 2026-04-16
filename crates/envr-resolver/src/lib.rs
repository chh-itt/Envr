//! Project pin grammar (`KIND@SPEC`), `.envr.toml` updates, **managed runtime home resolution**
//! (pin vs global `current`), and planning which pins look missing-but-installable.
//!
//! **PATH merging** and **toolchain env extensions** (Go/Deno/Bun from settings) live here;
//! orchestration (`collect_run_env`, rustup) stays in `envr-cli`. Low-level version directory
//! matching stays in `envr-shim-core`.

mod merge_env;
mod missing_pins;
mod pin_spec;
mod project_file;
mod run_home;

pub use merge_env::{
    dedup_paths, extend_env_with_tooling_settings, go_env_from_settings, path_sep, prepend_path,
    runtime_bin_dirs, version_label_from_runtime_home,
};
pub use missing_pins::{
    RUNTIME_PLAN_ORDER, list_pinned_runtime_specs, plan_missing_installable_pins,
    runtime_error_might_install_fix,
};
pub use pin_spec::{RuntimePinSpec, parse_runtime_pin_spec, runtime_kind_toml_key};
pub use project_file::upsert_runtime_pin;
pub use run_home::{
    resolve_bun_home, resolve_deno_home, resolve_dotnet_home, resolve_exec_lang_home,
    resolve_go_home, resolve_php_home, resolve_run_lang_home,
};
