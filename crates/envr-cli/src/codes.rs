//! Stable CLI envelope/message codes.
//!
//! Prefer these constants over ad-hoc string literals at call sites.

pub mod ok {
    pub const INSTALLED: &str = "installed";
    pub const CURRENT_RUNTIME_SET: &str = "current_runtime_set";
    pub const LIST_INSTALLED: &str = "list_installed";
    pub const SHOW_CURRENT: &str = "show_current";
    pub const UNINSTALLED: &str = "uninstalled";
    pub const RESOLVED_EXECUTABLE: &str = "resolved_executable";
    pub const LIST_REMOTE: &str = "list_remote";
    pub const RUST_MANAGED_INSTALLED: &str = "rust_managed_installed";
    pub const WHY_RUNTIME: &str = "why_runtime";
    pub const RUNTIME_RESOLVED: &str = "runtime_resolved";
    pub const CHILD_COMPLETED: &str = "child_completed";
    pub const DRY_RUN: &str = "dry_run";
    pub const DRY_RUN_DIFF: &str = "dry_run_diff";
    pub const PROJECT_ENV: &str = "project_env";
    pub const TEMPLATE_RENDERED: &str = "template_rendered";
    pub const SHELL_EXITED: &str = "shell_exited";
    pub const SHELL_HOOK: &str = "shell_hook";
    pub const HOOK_KEYS: &str = "hook_keys";
    pub const HOOK_PROMPT: &str = "hook_prompt";
    pub const PRUNE_DRY_RUN: &str = "prune_dry_run";
    pub const PRUNE_EXECUTED: &str = "prune_executed";
    pub const PROJECT_CONFIG_INIT: &str = "project_config_init";
    pub const PROJECT_CONFIG_OK: &str = "project_config_ok";
    pub const PROJECT_STATUS: &str = "project_status";
    pub const PROJECT_PIN_ADDED: &str = "project_pin_added";
    pub const PROJECT_SYNCED: &str = "project_synced";
    pub const PROJECT_VALIDATED: &str = "project_validated";
    pub const CONFIG_IMPORTED: &str = "config_imported";
    pub const CONFIG_EXPORTED: &str = "config_exported";
    pub const PROFILES_LIST: &str = "profiles_list";
    pub const PROFILE_SHOW: &str = "profile_show";
    pub const CONFIG_SCHEMA: &str = "config_schema";
    pub const CONFIG_VALIDATE_OK: &str = "config_validate_ok";
    pub const CONFIG_EDIT_OK: &str = "config_edit_ok";
    pub const CONFIG_PATH: &str = "config_path";
    pub const CONFIG_SHOW: &str = "config_show";
    pub const CONFIG_KEYS: &str = "config_keys";
    pub const CONFIG_GET: &str = "config_get";
    pub const CONFIG_SET: &str = "config_set";
    pub const ALIAS_LIST: &str = "alias_list";
    pub const ALIAS_ADDED: &str = "alias_added";
    pub const ALIAS_REMOVED: &str = "alias_removed";
    pub const SHIMS_SYNCED: &str = "shims_synced";
    pub const CACHE_CLEANED: &str = "cache_cleaned";
    pub const CACHE_INDEX_SYNCED: &str = "cache_index_synced";
    pub const CACHE_INDEX_STATUS: &str = "cache_index_status";
    pub const BUNDLE_CREATED: &str = "bundle_created";
    pub const BUNDLE_APPLIED: &str = "bundle_applied";
    pub const DOCTOR_OK: &str = "doctor_ok";
    pub const DOCTOR_ISSUES: &str = "doctor_issues";
    pub const DEACTIVATE_HINT: &str = "deactivate_hint";
    pub const DEBUG_INFO: &str = "debug_info";
    pub const DIAGNOSTICS_EXPORT_OK: &str = "diagnostics_export_ok";
    pub const HELP_SHORTCUTS: &str = "help_shortcuts";
    pub const UPDATE_INFO: &str = "update_info";
}

pub mod err {
    pub const ABORTED: &str = "aborted";
    pub const ARGV_PARSE_ERROR: &str = "argv_parse_error";
    pub const VALIDATION: &str = "validation";
    pub const CHILD_EXIT: &str = "child_exit";
    pub const DIAGNOSTICS_EXPORT_FAILED: &str = "diagnostics_export_failed";
    pub const PROJECT_CHECK_FAILED: &str = "project_check_failed";
    pub const PROJECT_SYNC_PENDING: &str = "project_sync_pending";
    pub const PROJECT_VALIDATE_FAILED: &str = "project_validate_failed";
    pub const SHELL_EXIT: &str = "shell_exit";
}
