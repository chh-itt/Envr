//! CLI **UX policy** and **presenter** helpers: one place for how globals map to stdout/stderr behavior.
//!
//! # Policy (automation vs human)
//!
//! | Dimension | Rule |
//! |-----------|------|
//! | **Machine JSON** | [`CliUxPolicy::is_json`] — stdout is one JSON envelope line per emission; no ANSI. |
//! | **Human text** | [`CliUxPolicy::is_text`] — free-form lines; may use ANSI when [`CliUxPolicy::use_ansi_stdout`]. |
//! | **Quiet** | Suppresses *non-error* human text ([`CliUxPolicy::human_text_primary`]); errors still emit (bracket-only in text, trimmed envelope in JSON). |
//! | **Porcelain** | Script-oriented plain text ([`CliUxPolicy::wants_porcelain_lines`]); incompatible with JSON; also turns off *decorative* output below. |
//! | **Primary human** | [`CliUxPolicy::human_text_primary`] — text mode, not quiet: ordinary hints, tables, success lines (no requirement for ANSI). |
//! | **Decorated human** | [`CliUxPolicy::human_text_decorated`] — primary human **and** not porcelain: install/dry-run headlines, colored doctor sections, highlighted diff keys. |
//! | **Verbose stderr** | [`CliUxPolicy::verbose_stderr`] — `--verbose` traces when not `--quiet` (even in JSON; unchanged legacy behavior). |
//! | **Colors** | Only when not `--no-color` and stdout is a TTY ([`CliUxPolicy::use_ansi_stdout`]). Rich layouts also skip ANSI when porcelain ([`CliUxPolicy::use_rich_text_styles`]). |
//!
//! Commands should prefer [`CliPresenter`] for repeated `if !g.quiet && text` patterns instead of
//! re-deriving flags from [`crate::cli::GlobalArgs`].

use crate::cli::{GlobalArgs, OutputFormat};

/// High-level output persona for future response shaping.
///
/// Phase 3 starts by introducing this stable model without changing existing
/// command behavior. Current output remains identical until persona-specific
/// shaping is enabled command-by-command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CliPersona {
    Automation,
    Operator,
    Onboarding,
}

impl CliPersona {
    pub const ENV_KEY: &'static str = "ENVR_CLI_PERSONA";

    #[inline]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "automation" | "auto" | "machine" => Some(Self::Automation),
            "operator" | "ops" | "default" => Some(Self::Operator),
            "onboarding" | "guide" | "newcomer" => Some(Self::Onboarding),
            _ => None,
        }
    }

    #[inline]
    pub fn from_env() -> Self {
        std::env::var(Self::ENV_KEY)
            .ok()
            .as_deref()
            .and_then(Self::parse)
            .unwrap_or(Self::Operator)
    }

    #[inline]
    pub fn token(self) -> &'static str {
        match self {
            Self::Automation => "automation",
            Self::Operator => "operator",
            Self::Onboarding => "onboarding",
        }
    }
}

/// Snapshot of global output flags used for presentation decisions ([`CliUxPolicy::from_global`]).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CliUxPolicy {
    pub format: OutputFormat,
    pub quiet: bool,
    pub porcelain: bool,
    pub no_color: bool,
    pub persona: CliPersona,
}

impl CliUxPolicy {
    #[inline]
    pub fn from_global(g: &GlobalArgs) -> Self {
        Self {
            format: g.effective_output_format(),
            quiet: g.quiet,
            porcelain: g.porcelain,
            no_color: g.no_color,
            persona: CliPersona::from_env(),
        }
    }

    #[inline]
    pub fn is_json(self) -> bool {
        matches!(self.format, OutputFormat::Json)
    }

    #[inline]
    pub fn is_text(self) -> bool {
        matches!(self.format, OutputFormat::Text)
    }

    /// Human-oriented informational lines on stdout (println) are allowed.
    #[inline]
    pub fn human_text_primary(self) -> bool {
        self.is_text() && !self.quiet
    }

    /// Colored headings, install/dry-run progress lines, doctor styled sections — not `--porcelain`.
    #[inline]
    pub fn human_text_decorated(self) -> bool {
        self.human_text_primary() && !self.porcelain
    }

    /// `--verbose` lines on stderr; independent of text/json (matches prior `verbose && !quiet`).
    #[inline]
    pub fn verbose_stderr(self, verbose_flag: bool) -> bool {
        verbose_flag && !self.quiet
    }

    /// Failure JSON envelopes use bracket-only `message` and drop heavy `data`/`diagnostics` when quiet.
    #[inline]
    pub fn quiet_json_failure_trim(self) -> bool {
        self.quiet && self.is_json()
    }

    /// Tab-separated / single-path style output for scripts (`--porcelain`), not JSON.
    #[inline]
    pub fn wants_porcelain_lines(self) -> bool {
        self.porcelain && !self.is_json()
    }

    /// ANSI SGR on stdout (list highlighting, dry-run diff, etc.).
    #[inline]
    pub fn use_ansi_stdout(self) -> bool {
        use std::io::{self, IsTerminal};
        !self.no_color && io::stdout().is_terminal()
    }

    /// Decorative ANSI for multi-column human layouts: off when porcelain or colors disabled.
    #[inline]
    pub fn use_rich_text_styles(self) -> bool {
        self.use_ansi_stdout() && !self.porcelain
    }
}

/// Thin facade over [`GlobalArgs`] + [`CliUxPolicy`] for command bodies.
#[derive(Clone, Copy, Debug)]
pub struct CliPresenter<'a> {
    pub global: &'a GlobalArgs,
    pub policy: CliUxPolicy,
}

impl<'a> CliPresenter<'a> {
    #[inline]
    pub fn new(global: &'a GlobalArgs) -> Self {
        Self {
            global,
            policy: CliUxPolicy::from_global(global),
        }
    }

    /// Run `f` only when human text primary output is allowed (text mode, not quiet).
    #[inline]
    pub fn with_human_text_primary(self, f: impl FnOnce()) {
        if self.policy.human_text_primary() {
            f();
        }
    }

    /// Run `f` only when decorative (non-porcelain) human output is allowed.
    #[inline]
    pub fn with_human_text_decorated(self, f: impl FnOnce()) {
        if self.policy.human_text_decorated() {
            f();
        }
    }

    /// True when stderr may show full error text (same gate as primary human text for errors in text mode).
    #[inline]
    pub fn human_text_errors_full(self) -> bool {
        self.policy.human_text_primary()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::GlobalArgs;

    fn g_text(quiet: bool, porcelain: bool) -> GlobalArgs {
        GlobalArgs {
            output_format: None,
            porcelain,
            quiet,
            no_color: true,
            debug: false,
            verbose: false,
            runtime_root: None,
        }
    }

    #[test]
    fn human_primary_only_when_text_and_not_quiet() {
        let p = CliUxPolicy::from_global(&g_text(false, false));
        assert!(p.human_text_primary());
        assert!(p.human_text_decorated());
        let p = CliUxPolicy::from_global(&g_text(true, false));
        assert!(!p.human_text_primary());
        assert!(!p.human_text_decorated());
        let p = CliUxPolicy::from_global(&g_text(false, true));
        assert!(p.human_text_primary());
        assert!(!p.human_text_decorated());
        let p = CliUxPolicy::from_global(&GlobalArgs {
            output_format: Some(OutputFormat::Json),
            porcelain: false,
            quiet: false,
            no_color: true,
            debug: false,
            verbose: false,
            runtime_root: None,
        });
        assert!(!p.human_text_primary());
    }

    #[test]
    fn porcelain_lines_not_in_json() {
        let p = CliUxPolicy::from_global(&GlobalArgs {
            output_format: Some(OutputFormat::Json),
            porcelain: true,
            quiet: false,
            no_color: true,
            debug: false,
            verbose: false,
            runtime_root: None,
        });
        assert!(!p.wants_porcelain_lines());
    }

    #[test]
    fn presenter_runs_closure_only_when_allowed() {
        let g = g_text(false, false);
        let mut n = 0;
        CliPresenter::new(&g).with_human_text_primary(|| n += 1);
        assert_eq!(n, 1);
        let gq = g_text(true, false);
        CliPresenter::new(&gq).with_human_text_primary(|| n += 1);
        assert_eq!(n, 1);
    }

    #[test]
    fn verbose_stderr_matches_legacy_quiet_gate() {
        let g = g_text(false, false);
        let p = CliUxPolicy::from_global(&g);
        assert!(p.verbose_stderr(true));
        assert!(!p.verbose_stderr(false));
        let gq = g_text(true, false);
        assert!(!CliUxPolicy::from_global(&gq).verbose_stderr(true));
    }

    #[test]
    fn persona_parse_accepts_aliases() {
        assert_eq!(
            CliPersona::parse("automation"),
            Some(CliPersona::Automation)
        );
        assert_eq!(CliPersona::parse("auto"), Some(CliPersona::Automation));
        assert_eq!(CliPersona::parse("ops"), Some(CliPersona::Operator));
        assert_eq!(CliPersona::parse("guide"), Some(CliPersona::Onboarding));
        assert_eq!(CliPersona::parse("unknown"), None);
    }

    #[test]
    fn persona_from_env_defaults_to_operator() {
        // SAFETY: test-only env mutation in single-threaded assertion scope.
        unsafe {
            std::env::remove_var(CliPersona::ENV_KEY);
        }
        assert_eq!(CliPersona::from_env(), CliPersona::Operator);
    }
}
