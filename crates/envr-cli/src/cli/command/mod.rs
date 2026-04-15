//! Clap subcommand tree and [`Command`] metadata dispatch.

mod nested;
mod root;

pub use nested::{
    AliasCmd, BundleCmd, CacheCmd, CacheIndexCmd, ConfigCmd, ConfigValueType, DebugCmd,
    DiagnosticsCmd, EnvShellKind, HelpCmd, HookCmd, ProfileCmd, ProjectCmd, RustCmd, ShimCmd,
};
pub use root::Command;

use super::metadata::{
    CommandCapabilities, CommandKey, CommandMetadata, RuntimeHandlerGroup, metadata_for_key,
};

macro_rules! command_key_arms {
    ($self_expr:expr, $( $pattern:pat => $key:expr ),+ $(,)?) => {
        match $self_expr {
            $( $pattern => $key, )+
        }
    };
}

impl Command {
    #[inline]
    pub(crate) fn key(&self) -> CommandKey {
        command_key_arms!(
            self,
            Command::Install { .. } => CommandKey::Install,
            Command::Use { .. } => CommandKey::Use,
            Command::List { .. } => CommandKey::List,
            Command::Current { .. } => CommandKey::Current,
            Command::Uninstall { .. } => CommandKey::Uninstall,
            Command::Which { .. } => CommandKey::Which,
            Command::Remote { .. } => CommandKey::Remote,
            Command::Rust(RustCmd::InstallManaged) => CommandKey::RustInstallManaged,
            Command::Why { .. } => CommandKey::Why,
            Command::Resolve { .. } => CommandKey::Resolve,
            Command::Exec { .. } => CommandKey::Exec,
            Command::Run { .. } => CommandKey::Run,
            Command::Env { .. } => CommandKey::Env,
            Command::Template { .. } => CommandKey::Template,
            Command::Shell { .. } => CommandKey::Shell,
            Command::Hook(HookCmd::Bash) => CommandKey::HookBash,
            Command::Hook(HookCmd::Zsh) => CommandKey::HookZsh,
            Command::Hook(HookCmd::Keys { .. }) => CommandKey::HookKeys,
            Command::Hook(HookCmd::Prompt { .. }) => CommandKey::HookPrompt,
            Command::Prune { .. } => CommandKey::Prune,
            Command::Init { .. } => CommandKey::Init,
            Command::Check { .. } => CommandKey::Check,
            Command::Status { .. } => CommandKey::Status,
            Command::Project(ProjectCmd::Add { .. }) => CommandKey::ProjectAdd,
            Command::Project(ProjectCmd::Sync { .. }) => CommandKey::ProjectSync,
            Command::Project(ProjectCmd::Validate { .. }) => CommandKey::ProjectValidate,
            Command::Import { .. } => CommandKey::Import,
            Command::Export { .. } => CommandKey::Export,
            Command::Profile(ProfileCmd::List { .. }) => CommandKey::ProfileList,
            Command::Profile(ProfileCmd::Show { .. }) => CommandKey::ProfileShow,
            Command::Config(ConfigCmd::Schema) => CommandKey::ConfigSchema,
            Command::Config(ConfigCmd::Validate) => CommandKey::ConfigValidate,
            Command::Config(ConfigCmd::Edit) => CommandKey::ConfigEdit,
            Command::Config(ConfigCmd::Path) => CommandKey::ConfigPath,
            Command::Config(ConfigCmd::Show) => CommandKey::ConfigShow,
            Command::Config(ConfigCmd::Keys) => CommandKey::ConfigKeys,
            Command::Config(ConfigCmd::Get { .. }) => CommandKey::ConfigGet,
            Command::Config(ConfigCmd::Set { .. }) => CommandKey::ConfigSet,
            Command::Alias(AliasCmd::List) => CommandKey::AliasList,
            Command::Alias(AliasCmd::Add { .. }) => CommandKey::AliasAdd,
            Command::Alias(AliasCmd::Remove { .. }) => CommandKey::AliasRemove,
            Command::Shim(ShimCmd::Sync { .. }) => CommandKey::ShimSync,
            Command::Cache(CacheCmd::Clean { .. }) => CommandKey::CacheClean,
            Command::Cache(CacheCmd::Index(CacheIndexCmd::Sync { .. })) => CommandKey::CacheIndexSync,
            Command::Cache(CacheCmd::Index(CacheIndexCmd::Status { .. })) => CommandKey::CacheIndexStatus,
            Command::Bundle(BundleCmd::Create { .. }) => CommandKey::BundleCreate,
            Command::Bundle(BundleCmd::Apply { .. }) => CommandKey::BundleApply,
            Command::Doctor { .. } => CommandKey::Doctor,
            Command::Deactivate => CommandKey::Deactivate,
            Command::Debug(DebugCmd::Info) => CommandKey::DebugInfo,
            Command::Diagnostics(DiagnosticsCmd::Export { .. }) => CommandKey::DiagnosticsExport,
            Command::Completion { .. } => CommandKey::Completion,
            Command::Help(HelpCmd::Shortcuts) => CommandKey::HelpShortcuts,
            Command::Update { .. } => CommandKey::Update
        )
    }

    #[inline]
    fn metadata(&self) -> CommandMetadata {
        let mut m = metadata_for_key(self.key());
        if let Command::Doctor { json: true, .. } = self {
            m.legacy_json_shorthand = true;
        }
        m
    }

    /// Stable snake_case label for tracing and structured logs (not localized).
    pub fn trace_name(&self) -> &'static str {
        self.metadata().trace_name
    }

    /// Subcommand-local flags that mean **JSON output for this process** (same as global `--format json`).
    ///
    /// **Extension point:** when adding `--json` (or equivalent) on a subcommand, add a match arm
    /// here and wire the handler with [`crate::cli::GlobalArgs::cloned_with_legacy_json`].
    /// [`Cli::resolved_output_format`](crate::cli::Cli::resolved_output_format)
    /// and [`apply_global`](crate::cli::apply_global) must stay in sync via this method only.
    #[inline]
    pub(crate) fn legacy_json_shorthand(&self) -> bool {
        self.metadata().legacy_json_shorthand
    }

    #[inline]
    pub(crate) fn runtime_handler_group(&self) -> Option<RuntimeHandlerGroup> {
        self.metadata().runtime_group
    }

    #[inline]
    pub(crate) fn capabilities(&self) -> CommandCapabilities {
        self.metadata().capabilities
    }
}
