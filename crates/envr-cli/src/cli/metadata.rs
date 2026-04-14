//! Command metadata registry (trace names, runtime grouping, capabilities).

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
pub(crate) struct CommandMetadata {
    pub(crate) trace_name: &'static str,
    pub(crate) legacy_json_shorthand: bool,
    pub(crate) runtime_required: bool,
    pub(crate) runtime_group: Option<RuntimeHandlerGroup>,
    pub(crate) capabilities: CommandCapabilities,
}

impl CommandMetadata {
    const fn new(
        trace_name: &'static str,
        legacy_json_shorthand: bool,
        runtime_required: bool,
        runtime_group: Option<RuntimeHandlerGroup>,
        capabilities: CommandCapabilities,
    ) -> Self {
        Self {
            trace_name,
            legacy_json_shorthand,
            runtime_required,
            runtime_group,
            capabilities,
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

const COMMAND_METADATA_REGISTRY: &[(CommandKey, CommandMetadata)] = &[
    (CommandKey::Install, CommandMetadata::new("install", false, true, Some(RuntimeHandlerGroup::Installation), CommandCapabilities::new(true, false, ContractSurface::Json))),
    (CommandKey::Use, CommandMetadata::new("use", false, true, Some(RuntimeHandlerGroup::Installation), CommandCapabilities::new(true, false, ContractSurface::Json))),
    (CommandKey::List, CommandMetadata::new("list", false, true, Some(RuntimeHandlerGroup::Installation), CommandCapabilities::new(false, true, ContractSurface::Both))),
    (CommandKey::Current, CommandMetadata::new("current", false, true, Some(RuntimeHandlerGroup::Installation), CommandCapabilities::new(false, true, ContractSurface::Both))),
    (CommandKey::Uninstall, CommandMetadata::new("uninstall", false, true, Some(RuntimeHandlerGroup::Installation), CommandCapabilities::new(false, true, ContractSurface::Json))),
    (CommandKey::Which, CommandMetadata::new("which", false, false, None, CommandCapabilities::new(false, true, ContractSurface::Both))),
    (CommandKey::Remote, CommandMetadata::new("remote", false, true, Some(RuntimeHandlerGroup::Misc), CommandCapabilities::new(true, false, ContractSurface::Json))),
    (CommandKey::RustInstallManaged, CommandMetadata::new("rust_install_managed", false, false, None, CommandCapabilities::new(true, false, ContractSurface::Json))),
    (CommandKey::Why, CommandMetadata::new("why", false, false, None, CommandCapabilities::new(false, true, ContractSurface::Json))),
    (CommandKey::Resolve, CommandMetadata::new("resolve", false, false, None, CommandCapabilities::new(false, true, ContractSurface::Both))),
    (CommandKey::Exec, CommandMetadata::new("exec", false, false, None, CommandCapabilities::new(false, true, ContractSurface::Json))),
    (CommandKey::Run, CommandMetadata::new("run", false, false, None, CommandCapabilities::new(false, true, ContractSurface::Json))),
    (CommandKey::Env, CommandMetadata::new("env", false, false, None, CommandCapabilities::new(false, true, ContractSurface::Json))),
    (CommandKey::Template, CommandMetadata::new("template", false, false, None, CommandCapabilities::new(false, true, ContractSurface::Json))),
    (CommandKey::Shell, CommandMetadata::new("shell", false, false, None, CommandCapabilities::new(false, true, ContractSurface::Json))),
    (CommandKey::HookBash, CommandMetadata::new("hook_bash", false, false, None, CommandCapabilities::new(false, true, ContractSurface::Json))),
    (CommandKey::HookZsh, CommandMetadata::new("hook_zsh", false, false, None, CommandCapabilities::new(false, true, ContractSurface::Json))),
    (CommandKey::HookKeys, CommandMetadata::new("hook_keys", false, false, None, CommandCapabilities::new(false, true, ContractSurface::Json))),
    (CommandKey::HookPrompt, CommandMetadata::new("hook_prompt", false, false, None, CommandCapabilities::new(false, true, ContractSurface::Json))),
    (CommandKey::Prune, CommandMetadata::new("prune", false, true, Some(RuntimeHandlerGroup::Project), CommandCapabilities::new(false, true, ContractSurface::Json))),
    (CommandKey::Init, CommandMetadata::new("init", false, false, None, CommandCapabilities::new(false, true, ContractSurface::Json))),
    (CommandKey::Check, CommandMetadata::new("check", false, false, None, CommandCapabilities::new(false, true, ContractSurface::Json))),
    (CommandKey::Status, CommandMetadata::new("status", false, false, None, CommandCapabilities::new(false, true, ContractSurface::Json))),
    (CommandKey::ProjectAdd, CommandMetadata::new("project_add", false, true, Some(RuntimeHandlerGroup::Project), CommandCapabilities::new(false, true, ContractSurface::Json))),
    (CommandKey::ProjectSync, CommandMetadata::new("project_sync", false, true, Some(RuntimeHandlerGroup::Project), CommandCapabilities::new(false, true, ContractSurface::Json))),
    (CommandKey::ProjectValidate, CommandMetadata::new("project_validate", false, true, Some(RuntimeHandlerGroup::Project), CommandCapabilities::new(true, false, ContractSurface::Json))),
    (CommandKey::Import, CommandMetadata::new("import", false, false, None, CommandCapabilities::new(false, true, ContractSurface::Json))),
    (CommandKey::Export, CommandMetadata::new("export", false, false, None, CommandCapabilities::new(false, true, ContractSurface::Json))),
    (CommandKey::ProfileList, CommandMetadata::new("profile_list", false, false, None, CommandCapabilities::new(false, true, ContractSurface::Json))),
    (CommandKey::ProfileShow, CommandMetadata::new("profile_show", false, false, None, CommandCapabilities::new(false, true, ContractSurface::Json))),
    (CommandKey::ConfigSchema, CommandMetadata::new("config_schema", false, false, None, CommandCapabilities::new(false, true, ContractSurface::Json))),
    (CommandKey::ConfigValidate, CommandMetadata::new("config_validate", false, false, None, CommandCapabilities::new(false, true, ContractSurface::Json))),
    (CommandKey::ConfigEdit, CommandMetadata::new("config_edit", false, false, None, CommandCapabilities::new(false, true, ContractSurface::Json))),
    (CommandKey::ConfigPath, CommandMetadata::new("config_path", false, false, None, CommandCapabilities::new(false, true, ContractSurface::Json))),
    (CommandKey::ConfigShow, CommandMetadata::new("config_show", false, false, None, CommandCapabilities::new(false, true, ContractSurface::Json))),
    (CommandKey::ConfigKeys, CommandMetadata::new("config_keys", false, false, None, CommandCapabilities::new(false, true, ContractSurface::Json))),
    (CommandKey::ConfigGet, CommandMetadata::new("config_get", false, false, None, CommandCapabilities::new(false, true, ContractSurface::Json))),
    (CommandKey::ConfigSet, CommandMetadata::new("config_set", false, false, None, CommandCapabilities::new(false, true, ContractSurface::Json))),
    (CommandKey::AliasList, CommandMetadata::new("alias_list", false, false, None, CommandCapabilities::new(false, true, ContractSurface::Json))),
    (CommandKey::AliasAdd, CommandMetadata::new("alias_add", false, false, None, CommandCapabilities::new(false, true, ContractSurface::Json))),
    (CommandKey::AliasRemove, CommandMetadata::new("alias_remove", false, false, None, CommandCapabilities::new(false, true, ContractSurface::Json))),
    (CommandKey::ShimSync, CommandMetadata::new("shim_sync", false, false, None, CommandCapabilities::new(false, true, ContractSurface::Json))),
    (CommandKey::CacheClean, CommandMetadata::new("cache_clean", false, false, None, CommandCapabilities::new(false, true, ContractSurface::Json))),
    (CommandKey::CacheIndexSync, CommandMetadata::new("cache_index_sync", false, false, None, CommandCapabilities::new(true, false, ContractSurface::Json))),
    (CommandKey::CacheIndexStatus, CommandMetadata::new("cache_index_status", false, false, None, CommandCapabilities::new(false, true, ContractSurface::Json))),
    (CommandKey::BundleCreate, CommandMetadata::new("bundle_create", false, false, None, CommandCapabilities::new(false, true, ContractSurface::Json))),
    (CommandKey::BundleApply, CommandMetadata::new("bundle_apply", false, false, None, CommandCapabilities::new(false, true, ContractSurface::Json))),
    (CommandKey::Doctor, CommandMetadata::new("doctor", false, true, Some(RuntimeHandlerGroup::Misc), CommandCapabilities::new(false, true, ContractSurface::Json))),
    (CommandKey::Deactivate, CommandMetadata::new("deactivate", false, false, None, CommandCapabilities::new(false, true, ContractSurface::Json))),
    (CommandKey::DebugInfo, CommandMetadata::new("debug_info", false, false, None, CommandCapabilities::new(false, true, ContractSurface::Json))),
    (CommandKey::DiagnosticsExport, CommandMetadata::new("diagnostics_export", false, true, Some(RuntimeHandlerGroup::Misc), CommandCapabilities::new(false, true, ContractSurface::Json))),
    (CommandKey::Completion, CommandMetadata::new("completion", false, false, None, CommandCapabilities::new(false, true, ContractSurface::None))),
    (CommandKey::HelpShortcuts, CommandMetadata::new("help_shortcuts", false, false, None, CommandCapabilities::new(false, true, ContractSurface::None))),
    (CommandKey::Update, CommandMetadata::new("update", false, false, None, CommandCapabilities::new(true, false, ContractSurface::Json))),
];

#[cfg(test)]
pub(crate) fn all_command_keys() -> impl Iterator<Item = CommandKey> {
    COMMAND_METADATA_REGISTRY.iter().map(|(key, _)| *key)
}

#[cfg(test)]
pub(crate) fn metadata_registry_entries() -> &'static [(CommandKey, CommandMetadata)] {
    COMMAND_METADATA_REGISTRY
}

pub(crate) fn metadata_for_key(key: CommandKey) -> CommandMetadata {
    COMMAND_METADATA_REGISTRY
        .iter()
        .find_map(|(k, m)| if *k == key { Some(*m) } else { None })
        .expect("command metadata registry missing key")
}

#[cfg(test)]
mod registry_alignment_tests {
    use super::*;
    use std::collections::HashSet;

    // Registry row count vs `CommandKey` cardinality is asserted in
    // `crate::cli::command_trace_tests::command_key_mapping_round_trips_against_registry`
    // (argv sample table length vs `metadata_registry_entries()`), not via a literal here.

    #[test]
    fn registry_command_keys_are_unique() {
        let mut seen = HashSet::new();
        for (k, _) in COMMAND_METADATA_REGISTRY {
            assert!(
                seen.insert(*k),
                "duplicate CommandKey {:?} in COMMAND_METADATA_REGISTRY",
                k
            );
        }
        assert_eq!(seen.len(), COMMAND_METADATA_REGISTRY.len());
    }

    #[test]
    fn registry_trace_names_are_unique() {
        let mut seen = HashSet::new();
        for (_, m) in COMMAND_METADATA_REGISTRY {
            assert!(
                seen.insert(m.trace_name),
                "duplicate trace_name {:?} in COMMAND_METADATA_REGISTRY",
                m.trace_name
            );
        }
    }

    #[test]
    fn metadata_for_key_matches_each_static_registry_row() {
        for (k, expected) in COMMAND_METADATA_REGISTRY {
            let got = metadata_for_key(*k);
            assert_eq!(
                got, *expected,
                "metadata_for_key({k:?}) must equal the static COMMAND_METADATA_REGISTRY row"
            );
        }
    }
}
