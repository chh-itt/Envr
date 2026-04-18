//! Single source of truth for CLI command identity: tracing, runtime routing hints, capabilities,
//! `--help` path (see `help_registry/table.inc`), and **success** JSON `message` tokens (`emit_ok` /
//! `write_envelope` / `emit_doctor` success path) aligned with `schemas/cli/index.json`.

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(crate) enum RuntimeHandlerGroup {
    Installation,
    Project,
    Misc,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(crate) enum ContractSurface {
    None,
    Json,
    Both,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(crate) struct CommandCapabilities {
    pub(crate) may_network: bool,
    pub(crate) offline_safe: bool,
    pub(crate) contract_surface: ContractSurface,
}

impl CommandCapabilities {
    pub(crate) const fn new(
        may_network: bool,
        offline_safe: bool,
        contract_surface: ContractSurface,
    ) -> Self {
        Self {
            may_network,
            offline_safe,
            contract_surface,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CommandSpec {
    pub(crate) trace_name: &'static str,
    /// Subcommand-local flag that implies JSON output before clap finishes parsing (e.g. `--json`).
    pub(crate) legacy_json_flag: Option<&'static str>,
    pub(crate) legacy_json_shorthand: bool,
    pub(crate) runtime_required: bool,
    pub(crate) runtime_group: Option<RuntimeHandlerGroup>,
    pub(crate) capabilities: CommandCapabilities,
    /// Clap subcommand path for localized `--help` (must match `help_registry/table.inc`).
    pub(crate) help_path: &'static [&'static str],
    /// Success envelope `message` values this command may emit (subset of `schemas/cli/index.json` `data_messages`).
    pub(crate) success_messages: &'static [&'static str],
}

impl CommandSpec {
    const fn new(
        trace_name: &'static str,
        legacy_json_flag: Option<&'static str>,
        legacy_json_shorthand: bool,
        runtime_required: bool,
        runtime_group: Option<RuntimeHandlerGroup>,
        capabilities: CommandCapabilities,
        help_path: &'static [&'static str],
        success_messages: &'static [&'static str],
    ) -> Self {
        Self {
            trace_name,
            legacy_json_flag,
            legacy_json_shorthand,
            runtime_required,
            runtime_group,
            capabilities,
            help_path,
            success_messages,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub(crate) enum CommandKey {
    Install,
    Use,
    List,
    Current,
    Uninstall,
    Which,
    Remote,
    RustInstallManaged,
    Why,
    Resolve,
    Exec,
    Run,
    Env,
    Template,
    Shell,
    HookBash,
    HookZsh,
    HookKeys,
    HookPrompt,
    Prune,
    Init,
    Check,
    Status,
    ProjectAdd,
    ProjectSync,
    ProjectValidate,
    Import,
    Export,
    ProfileList,
    ProfileShow,
    ConfigSchema,
    ConfigValidate,
    ConfigEdit,
    ConfigPath,
    ConfigShow,
    ConfigKeys,
    ConfigGet,
    ConfigSet,
    AliasList,
    AliasAdd,
    AliasRemove,
    ShimSync,
    CacheClean,
    CacheIndexSync,
    CacheIndexStatus,
    CacheRuntimeStatus,
    BundleCreate,
    BundleApply,
    Doctor,
    Deactivate,
    DebugInfo,
    DiagnosticsExport,
    Completion,
    HelpShortcuts,
    Update,
}

const COMMAND_SPEC_REGISTRY: &[(CommandKey, CommandSpec)] = &[
    (
        CommandKey::Install,
        CommandSpec::new(
            "install",
            None,
            false,
            true,
            Some(RuntimeHandlerGroup::Installation),
            CommandCapabilities::new(true, false, ContractSurface::Json),
            &["install"],
            &["installed"],
        ),
    ),
    (
        CommandKey::Use,
        CommandSpec::new(
            "use",
            None,
            false,
            true,
            Some(RuntimeHandlerGroup::Installation),
            CommandCapabilities::new(true, false, ContractSurface::Json),
            &["use"],
            &["current_runtime_set"],
        ),
    ),
    (
        CommandKey::List,
        CommandSpec::new(
            "list",
            None,
            false,
            true,
            Some(RuntimeHandlerGroup::Installation),
            CommandCapabilities::new(false, true, ContractSurface::Both),
            &["list"],
            &["list_installed"],
        ),
    ),
    (
        CommandKey::Current,
        CommandSpec::new(
            "current",
            None,
            false,
            true,
            Some(RuntimeHandlerGroup::Installation),
            CommandCapabilities::new(false, true, ContractSurface::Both),
            &["current"],
            &["show_current"],
        ),
    ),
    (
        CommandKey::Uninstall,
        CommandSpec::new(
            "uninstall",
            None,
            false,
            true,
            Some(RuntimeHandlerGroup::Installation),
            CommandCapabilities::new(false, true, ContractSurface::Json),
            &["uninstall"],
            &["uninstalled"],
        ),
    ),
    (
        CommandKey::Which,
        CommandSpec::new(
            "which",
            None,
            false,
            false,
            None,
            CommandCapabilities::new(false, true, ContractSurface::Both),
            &["which"],
            &["resolved_executable"],
        ),
    ),
    (
        CommandKey::Remote,
        CommandSpec::new(
            "remote",
            None,
            false,
            true,
            Some(RuntimeHandlerGroup::Misc),
            CommandCapabilities::new(true, false, ContractSurface::Json),
            &["remote"],
            &["list_remote"],
        ),
    ),
    (
        CommandKey::RustInstallManaged,
        CommandSpec::new(
            "rust_install_managed",
            None,
            false,
            false,
            None,
            CommandCapabilities::new(true, false, ContractSurface::Json),
            &["rust", "install-managed"],
            &["rust_managed_installed"],
        ),
    ),
    (
        CommandKey::Why,
        CommandSpec::new(
            "why",
            None,
            false,
            false,
            None,
            CommandCapabilities::new(false, true, ContractSurface::Json),
            &["why"],
            &["why_runtime"],
        ),
    ),
    (
        CommandKey::Resolve,
        CommandSpec::new(
            "resolve",
            None,
            false,
            false,
            None,
            CommandCapabilities::new(false, true, ContractSurface::Both),
            &["resolve"],
            &["runtime_resolved"],
        ),
    ),
    (
        CommandKey::Exec,
        CommandSpec::new(
            "exec",
            None,
            false,
            false,
            None,
            CommandCapabilities::new(false, true, ContractSurface::Json),
            &["exec"],
            &["child_completed", "dry_run", "dry_run_diff"],
        ),
    ),
    (
        CommandKey::Run,
        CommandSpec::new(
            "run",
            None,
            false,
            false,
            None,
            CommandCapabilities::new(false, true, ContractSurface::Json),
            &["run"],
            &["child_completed", "dry_run", "dry_run_diff"],
        ),
    ),
    (
        CommandKey::Env,
        CommandSpec::new(
            "env",
            None,
            false,
            false,
            None,
            CommandCapabilities::new(false, true, ContractSurface::Json),
            &["env"],
            &["project_env"],
        ),
    ),
    (
        CommandKey::Template,
        CommandSpec::new(
            "template",
            None,
            false,
            false,
            None,
            CommandCapabilities::new(false, true, ContractSurface::Json),
            &["template"],
            &["template_rendered"],
        ),
    ),
    (
        CommandKey::Shell,
        CommandSpec::new(
            "shell",
            None,
            false,
            false,
            None,
            CommandCapabilities::new(false, true, ContractSurface::Json),
            &["shell"],
            &["shell_exited"],
        ),
    ),
    (
        CommandKey::HookBash,
        CommandSpec::new(
            "hook_bash",
            None,
            false,
            false,
            None,
            CommandCapabilities::new(false, true, ContractSurface::Json),
            &["hook", "bash"],
            &["shell_hook"],
        ),
    ),
    (
        CommandKey::HookZsh,
        CommandSpec::new(
            "hook_zsh",
            None,
            false,
            false,
            None,
            CommandCapabilities::new(false, true, ContractSurface::Json),
            &["hook", "zsh"],
            &["shell_hook"],
        ),
    ),
    (
        CommandKey::HookKeys,
        CommandSpec::new(
            "hook_keys",
            None,
            false,
            false,
            None,
            CommandCapabilities::new(false, true, ContractSurface::Json),
            &["hook", "keys"],
            &["hook_keys"],
        ),
    ),
    (
        CommandKey::HookPrompt,
        CommandSpec::new(
            "hook_prompt",
            None,
            false,
            false,
            None,
            CommandCapabilities::new(false, true, ContractSurface::Json),
            &["hook", "prompt"],
            &["hook_prompt"],
        ),
    ),
    (
        CommandKey::Prune,
        CommandSpec::new(
            "prune",
            None,
            false,
            true,
            Some(RuntimeHandlerGroup::Project),
            CommandCapabilities::new(false, true, ContractSurface::Json),
            &["prune"],
            &["prune_dry_run", "prune_executed"],
        ),
    ),
    (
        CommandKey::Init,
        CommandSpec::new(
            "init",
            None,
            false,
            false,
            None,
            CommandCapabilities::new(false, true, ContractSurface::Json),
            &["init"],
            &["project_config_init"],
        ),
    ),
    (
        CommandKey::Check,
        CommandSpec::new(
            "check",
            None,
            false,
            false,
            None,
            CommandCapabilities::new(false, true, ContractSurface::Json),
            &["check"],
            &["project_config_ok"],
        ),
    ),
    (
        CommandKey::Status,
        CommandSpec::new(
            "status",
            None,
            false,
            false,
            None,
            CommandCapabilities::new(false, true, ContractSurface::Json),
            &["status"],
            &["project_status"],
        ),
    ),
    (
        CommandKey::ProjectAdd,
        CommandSpec::new(
            "project_add",
            None,
            false,
            true,
            Some(RuntimeHandlerGroup::Project),
            CommandCapabilities::new(false, true, ContractSurface::Json),
            &["project", "add"],
            &["project_pin_added"],
        ),
    ),
    (
        CommandKey::ProjectSync,
        CommandSpec::new(
            "project_sync",
            None,
            false,
            true,
            Some(RuntimeHandlerGroup::Project),
            CommandCapabilities::new(false, true, ContractSurface::Json),
            &["project", "sync"],
            &["project_synced"],
        ),
    ),
    (
        CommandKey::ProjectValidate,
        CommandSpec::new(
            "project_validate",
            None,
            false,
            true,
            Some(RuntimeHandlerGroup::Project),
            CommandCapabilities::new(true, false, ContractSurface::Json),
            &["project", "validate"],
            &["project_validated"],
        ),
    ),
    (
        CommandKey::Import,
        CommandSpec::new(
            "import",
            None,
            false,
            false,
            None,
            CommandCapabilities::new(false, true, ContractSurface::Json),
            &["import"],
            &["config_imported"],
        ),
    ),
    (
        CommandKey::Export,
        CommandSpec::new(
            "export",
            None,
            false,
            false,
            None,
            CommandCapabilities::new(false, true, ContractSurface::Json),
            &["export"],
            &["config_exported"],
        ),
    ),
    (
        CommandKey::ProfileList,
        CommandSpec::new(
            "profile_list",
            None,
            false,
            false,
            None,
            CommandCapabilities::new(false, true, ContractSurface::Json),
            &["profile", "list"],
            &["profiles_list"],
        ),
    ),
    (
        CommandKey::ProfileShow,
        CommandSpec::new(
            "profile_show",
            None,
            false,
            false,
            None,
            CommandCapabilities::new(false, true, ContractSurface::Json),
            &["profile", "show"],
            &["profile_show"],
        ),
    ),
    (
        CommandKey::ConfigSchema,
        CommandSpec::new(
            "config_schema",
            None,
            false,
            false,
            None,
            CommandCapabilities::new(false, true, ContractSurface::Json),
            &["config", "schema"],
            &["config_schema"],
        ),
    ),
    (
        CommandKey::ConfigValidate,
        CommandSpec::new(
            "config_validate",
            None,
            false,
            false,
            None,
            CommandCapabilities::new(false, true, ContractSurface::Json),
            &["config", "validate"],
            &["config_validate_ok"],
        ),
    ),
    (
        CommandKey::ConfigEdit,
        CommandSpec::new(
            "config_edit",
            None,
            false,
            false,
            None,
            CommandCapabilities::new(false, true, ContractSurface::Json),
            &["config", "edit"],
            &["config_edit_ok"],
        ),
    ),
    (
        CommandKey::ConfigPath,
        CommandSpec::new(
            "config_path",
            None,
            false,
            false,
            None,
            CommandCapabilities::new(false, true, ContractSurface::Json),
            &["config", "path"],
            &["config_path"],
        ),
    ),
    (
        CommandKey::ConfigShow,
        CommandSpec::new(
            "config_show",
            None,
            false,
            false,
            None,
            CommandCapabilities::new(false, true, ContractSurface::Json),
            &["config", "show"],
            &["config_show"],
        ),
    ),
    (
        CommandKey::ConfigKeys,
        CommandSpec::new(
            "config_keys",
            None,
            false,
            false,
            None,
            CommandCapabilities::new(false, true, ContractSurface::Json),
            &["config", "keys"],
            &["config_keys"],
        ),
    ),
    (
        CommandKey::ConfigGet,
        CommandSpec::new(
            "config_get",
            None,
            false,
            false,
            None,
            CommandCapabilities::new(false, true, ContractSurface::Json),
            &["config", "get"],
            &["config_get"],
        ),
    ),
    (
        CommandKey::ConfigSet,
        CommandSpec::new(
            "config_set",
            None,
            false,
            false,
            None,
            CommandCapabilities::new(false, true, ContractSurface::Json),
            &["config", "set"],
            &["config_set"],
        ),
    ),
    (
        CommandKey::AliasList,
        CommandSpec::new(
            "alias_list",
            None,
            false,
            false,
            None,
            CommandCapabilities::new(false, true, ContractSurface::Json),
            &["alias", "list"],
            &["alias_list"],
        ),
    ),
    (
        CommandKey::AliasAdd,
        CommandSpec::new(
            "alias_add",
            None,
            false,
            false,
            None,
            CommandCapabilities::new(false, true, ContractSurface::Json),
            &["alias", "add"],
            &["alias_added"],
        ),
    ),
    (
        CommandKey::AliasRemove,
        CommandSpec::new(
            "alias_remove",
            None,
            false,
            false,
            None,
            CommandCapabilities::new(false, true, ContractSurface::Json),
            &["alias", "remove"],
            &["alias_removed"],
        ),
    ),
    (
        CommandKey::ShimSync,
        CommandSpec::new(
            "shim_sync",
            None,
            false,
            false,
            None,
            CommandCapabilities::new(false, true, ContractSurface::Json),
            &["shim", "sync"],
            &["shims_synced"],
        ),
    ),
    (
        CommandKey::CacheClean,
        CommandSpec::new(
            "cache_clean",
            None,
            false,
            false,
            None,
            CommandCapabilities::new(false, true, ContractSurface::Json),
            &["cache", "clean"],
            &["cache_cleaned"],
        ),
    ),
    (
        CommandKey::CacheIndexSync,
        CommandSpec::new(
            "cache_index_sync",
            None,
            false,
            false,
            None,
            CommandCapabilities::new(true, false, ContractSurface::Json),
            &["cache", "index", "sync"],
            &["cache_index_synced"],
        ),
    ),
    (
        CommandKey::CacheIndexStatus,
        CommandSpec::new(
            "cache_index_status",
            None,
            false,
            false,
            None,
            CommandCapabilities::new(false, true, ContractSurface::Json),
            &["cache", "index", "status"],
            &["cache_index_status"],
        ),
    ),
    (
        CommandKey::CacheRuntimeStatus,
        CommandSpec::new(
            "cache_runtime_status",
            None,
            false,
            false,
            None,
            CommandCapabilities::new(false, true, ContractSurface::Json),
            &["cache", "runtime", "status"],
            &["cache_runtime_status"],
        ),
    ),
    (
        CommandKey::BundleCreate,
        CommandSpec::new(
            "bundle_create",
            None,
            false,
            false,
            None,
            CommandCapabilities::new(false, true, ContractSurface::Json),
            &["bundle", "create"],
            &["bundle_created"],
        ),
    ),
    (
        CommandKey::BundleApply,
        CommandSpec::new(
            "bundle_apply",
            None,
            false,
            false,
            None,
            CommandCapabilities::new(false, true, ContractSurface::Json),
            &["bundle", "apply"],
            &["bundle_applied"],
        ),
    ),
    (
        CommandKey::Doctor,
        CommandSpec::new(
            "doctor",
            Some("--json"),
            false,
            true,
            Some(RuntimeHandlerGroup::Misc),
            CommandCapabilities::new(false, true, ContractSurface::Json),
            &["doctor"],
            &["doctor_ok", "doctor_issues"],
        ),
    ),
    (
        CommandKey::Deactivate,
        CommandSpec::new(
            "deactivate",
            None,
            false,
            false,
            None,
            CommandCapabilities::new(false, true, ContractSurface::Json),
            &["deactivate"],
            &["deactivate_hint"],
        ),
    ),
    (
        CommandKey::DebugInfo,
        CommandSpec::new(
            "debug_info",
            None,
            false,
            false,
            None,
            CommandCapabilities::new(false, true, ContractSurface::Json),
            &["debug", "info"],
            &["debug_info"],
        ),
    ),
    (
        CommandKey::DiagnosticsExport,
        CommandSpec::new(
            "diagnostics_export",
            None,
            false,
            true,
            Some(RuntimeHandlerGroup::Misc),
            CommandCapabilities::new(false, true, ContractSurface::Json),
            &["diagnostics", "export"],
            &["diagnostics_export_ok"],
        ),
    ),
    (
        CommandKey::Completion,
        CommandSpec::new(
            "completion",
            None,
            false,
            false,
            None,
            CommandCapabilities::new(false, true, ContractSurface::None),
            &["completion"],
            &[],
        ),
    ),
    (
        CommandKey::HelpShortcuts,
        CommandSpec::new(
            "help_shortcuts",
            None,
            false,
            false,
            None,
            CommandCapabilities::new(false, true, ContractSurface::None),
            &["help", "shortcuts"],
            &["help_shortcuts"],
        ),
    ),
    (
        CommandKey::Update,
        CommandSpec::new(
            "update",
            None,
            false,
            false,
            None,
            CommandCapabilities::new(true, false, ContractSurface::Json),
            &["update"],
            &["update_info"],
        ),
    ),
];

#[cfg(test)]
pub(crate) fn all_command_keys() -> impl Iterator<Item = CommandKey> {
    COMMAND_SPEC_REGISTRY.iter().map(|(key, _)| *key)
}

#[cfg(test)]
pub(crate) fn spec_registry_entries() -> &'static [(CommandKey, CommandSpec)] {
    COMMAND_SPEC_REGISTRY
}

pub(crate) fn spec_for_key(key: CommandKey) -> CommandSpec {
    COMMAND_SPEC_REGISTRY
        .iter()
        .find_map(|(k, m)| if *k == key { Some(*m) } else { None })
        .expect("command spec registry missing key")
}

pub(crate) fn command_specs() -> &'static [(CommandKey, CommandSpec)] {
    COMMAND_SPEC_REGISTRY
}

#[cfg(test)]
mod registry_alignment_tests {
    use super::*;
    use std::collections::{BTreeSet, HashSet};
    use std::path::Path;

    fn all_declared_success_messages() -> BTreeSet<&'static str> {
        let mut s = BTreeSet::new();
        for (_, spec) in COMMAND_SPEC_REGISTRY {
            for m in spec.success_messages {
                s.insert(*m);
            }
        }
        s
    }

    #[test]
    fn registry_command_keys_are_unique() {
        let mut seen = HashSet::new();
        for (k, _) in COMMAND_SPEC_REGISTRY {
            assert!(
                seen.insert(*k),
                "duplicate CommandKey {:?} in COMMAND_SPEC_REGISTRY",
                k
            );
        }
        assert_eq!(seen.len(), COMMAND_SPEC_REGISTRY.len());
    }

    #[test]
    fn registry_trace_names_are_unique() {
        let mut seen = HashSet::new();
        for (_, m) in COMMAND_SPEC_REGISTRY {
            assert!(
                seen.insert(m.trace_name),
                "duplicate trace_name {:?} in COMMAND_SPEC_REGISTRY",
                m.trace_name
            );
        }
    }

    #[test]
    fn registry_help_paths_are_unique() {
        let mut seen = HashSet::new();
        for (_, m) in COMMAND_SPEC_REGISTRY {
            let key = m.help_path.join("/");
            assert!(
                seen.insert(key.clone()),
                "duplicate help_path `{key}` in COMMAND_SPEC_REGISTRY"
            );
        }
    }

    #[test]
    fn spec_for_key_matches_each_static_registry_row() {
        for (k, expected) in COMMAND_SPEC_REGISTRY {
            let got = spec_for_key(*k);
            assert_eq!(
                got, *expected,
                "spec_for_key({k:?}) must equal the static COMMAND_SPEC_REGISTRY row"
            );
        }
    }

    #[test]
    fn declared_success_messages_have_schema_stubs() {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let data_dir = manifest.join("../../schemas/cli/data");
        for msg in all_declared_success_messages() {
            let p = data_dir.join(format!("{msg}.json"));
            assert!(
                p.is_file(),
                "CommandSpec references success message `{msg}` but {}",
                p.display()
            );
        }
    }

    #[test]
    fn declared_success_messages_match_schema_index() {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let index_path = manifest.join("../../schemas/cli/index.json");
        let raw = std::fs::read_to_string(&index_path).expect("read index.json");
        let v: serde_json::Value = serde_json::from_str(&raw).expect("parse index");
        let idx: std::collections::BTreeSet<String> = v["success_codes"]
            .as_array()
            .expect("success_codes")
            .iter()
            .filter_map(|x| x.as_str().map(ToOwned::to_owned))
            .collect();
        let declared: std::collections::BTreeSet<String> = all_declared_success_messages()
            .into_iter()
            .map(String::from)
            .collect();
        assert_eq!(
            idx, declared,
            "schemas/cli/index.json success_codes must equal the union of CommandSpec::success_messages; run `python scripts/generate_cli_schema_index.py`"
        );
    }
}
