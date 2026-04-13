//! Session context built once at `run` / `exec` (and related) entry points: shim context, parsed
//! project config, and optional [`RuntimeService`] for install paths.
//!
//! [`RunExecContext::from_cli_project`] obtains the service via [`crate::CliRuntimeSession::connect`]
//! so `exec` / `run` share the same construction path as [`crate::commands::common::with_runtime_service`] (default root).

use envr_config::project_config::{
    ProjectConfig, ProjectConfigLocation, load_project_config_profile,
};
use envr_core::runtime::service::RuntimeService;
use envr_error::{EnvrError, EnvrResult};
use envr_shim_core::ShimContext;
use std::path::PathBuf;

use crate::CliRuntimeSession;
use crate::commands::common;

fn enrich_project_config_load_error(e: EnvrError) -> EnvrError {
    EnvrError::Validation(format!(
        "{}\n{}",
        e,
        envr_core::i18n::tr_key(
            "cli.config.invalid_hint",
            "请检查 `.envr.toml` 键名/值类型（示例：`[runtimes.node] version = \"20\"`）。",
            "Check `.envr.toml` key names/value types (example: `[runtimes.node] version = \"20\"`).",
        )
    ))
}

/// Working directory, runtime root, profile, and merged `.envr.toml` (loaded once).
#[derive(Debug)]
pub struct CliProjectContext {
    pub ctx: ShimContext,
    pub project: Option<(ProjectConfig, ProjectConfigLocation)>,
}

impl CliProjectContext {
    pub fn load(path: PathBuf, profile: Option<String>) -> EnvrResult<Self> {
        let ctx = common::shim_context_for(path, profile)?;
        let project = load_project_config_profile(&ctx.working_dir, ctx.profile.as_deref())?;
        Ok(Self { ctx, project })
    }

    #[inline]
    pub fn project_config(&self) -> Option<&ProjectConfig> {
        self.project.as_ref().map(|(c, _)| c)
    }
}

/// `--path` working-directory hint plus optional `--profile` / `ENVR_PROFILE`, shared by `exec`, `run`, and similar commands.
#[derive(Clone, Debug)]
pub struct CliPathProfile {
    pub path: PathBuf,
    pub profile: Option<String>,
}

impl CliPathProfile {
    pub fn new(path: PathBuf, profile: Option<String>) -> Self {
        Self { path, profile }
    }

    /// [`CliProjectContext::load`] with this path and profile (no [`RuntimeService`]).
    pub fn load_project(self) -> EnvrResult<CliProjectContext> {
        CliProjectContext::load(self.path, self.profile)
    }

    /// Same as [`RunExecContext::load_with_project_hint`]: shim context, merged project config, and service.
    pub fn load_run_exec(self) -> EnvrResult<RunExecContext> {
        RunExecContext::load_with_project_hint(self.path, self.profile)
    }
}

/// [`CliProjectContext`] plus [`RuntimeService`] for commands that may install runtimes (`run` / `exec`).
pub struct RunExecContext {
    pub base: CliProjectContext,
    pub service: RuntimeService,
}

impl RunExecContext {
    /// [`CliProjectContext::load`] + [`RuntimeService`]. For `exec`/`run`, prefer [`Self::load_with_project_hint`].
    pub fn load(path: PathBuf, profile: Option<String>) -> EnvrResult<Self> {
        let base = CliProjectContext::load(path, profile)?;
        Self::from_cli_project(base)
    }

    /// Attach a [`RuntimeService`] after building [`CliProjectContext`] (e.g. when project load uses custom error mapping).
    pub fn from_cli_project(base: CliProjectContext) -> EnvrResult<Self> {
        let session = CliRuntimeSession::connect()?;
        debug_assert_eq!(
            session.runtime_root, base.ctx.runtime_root,
            "shim context and session must agree on resolve_runtime_root()"
        );
        Ok(Self {
            base,
            service: session.into_service(),
        })
    }

    /// Shim context + merged project config + service, with an extra hint when `.envr.toml` fails to load or parse.
    ///
    /// Prefer this for `envr exec` / `envr run`; use [`Self::load`] when raw errors are enough.
    pub fn load_with_project_hint(path: PathBuf, profile: Option<String>) -> EnvrResult<Self> {
        let ctx = common::shim_context_for(path, profile)?;
        let project = load_project_config_profile(&ctx.working_dir, ctx.profile.as_deref())
            .map_err(enrich_project_config_load_error)?;
        Self::from_cli_project(CliProjectContext { ctx, project })
    }

    #[inline]
    pub fn ctx(&self) -> &ShimContext {
        &self.base.ctx
    }

    #[inline]
    pub fn project(&self) -> &Option<(ProjectConfig, ProjectConfigLocation)> {
        &self.base.project
    }

    #[inline]
    pub fn project_config(&self) -> Option<&ProjectConfig> {
        self.base.project_config()
    }

    #[inline]
    pub fn service(&self) -> &RuntimeService {
        &self.service
    }
}

#[cfg(test)]
mod tests {
    use super::CliProjectContext;
    use std::fs;

    #[test]
    fn cli_project_context_load_empty_dir_has_no_project() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let session = CliProjectContext::load(tmp.path().to_path_buf(), None).expect("load");
        assert!(session.project.is_none());
        let expect = fs::canonicalize(tmp.path()).expect("canonicalize tmp");
        assert_eq!(session.ctx.working_dir, expect);
    }

    #[test]
    fn cli_project_context_load_finds_envr_toml_upward() {
        let tmp = tempfile::tempdir().expect("tempdir");
        fs::write(
            tmp.path().join(".envr.toml"),
            r#"
[runtimes.node]
version = "20"
"#,
        )
        .expect("write");
        let nested = tmp.path().join("app");
        fs::create_dir_all(&nested).expect("mkdir");
        let session = CliProjectContext::load(nested, None).expect("load");
        let (cfg, loc) = session.project.as_ref().expect("project");
        assert_eq!(cfg.runtimes.get("node").and_then(|r| r.version.as_deref()), Some("20"));
        let expect_root = fs::canonicalize(tmp.path()).expect("canonicalize root");
        assert_eq!(loc.dir, expect_root);
    }
}
