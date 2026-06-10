//! Hard-coded and user-controlled diagnostics
//!
//! Diagnostics are user messages, like warnings and errors.
//! When they are named for setting a user-overridable level,
//! they are called lints.
//!
//! # When should a diagnostic be a lint
//!
//! Lints are generally preferred because of the level of control for users.
//!
//! Use a hard-coded diagnostic when:
//! - Critical errors
//! - There is no associated package or workspace. The diagnostic must still be suppressible
//!   somehow (e.g. a user explicitly opting in to a config field's default value)
//! - The warning message is too important to allow a user to hide (rare)
//!
//! # Adding a diagnostic
//!
//! The mechanics of adding a diagnostic is dependent on the requirements:
//! - TOML syntax or manifest schema: [`passes::emit_parse_diagnostics`], [`rules::PARSE_PASS_RULES`]
//! - Lockfile
//!   - May be overly broad for what dependencies are checked
//! - Pre-build unit graph
//!   - Tailored to a specific configuration (features, targets) but requires users to enumerate every configuration
//! - Post-build unit graph: [`rules::unused_dependencies::lint_build_results`]
//!   - Slow feedback cycle since a build needs to happen
//! - Does not fit into any idea of a pass: directly call [`cargo_util_terminal::Shell::warn`] or [`crate::CargoResult::Err`]
//!
//! When evaluating a diagnostic:
//! - Only evaluate and emit for local packages unless it is for a [future-incompat lint]
//!
//! When generating a diagnostic [report][cargo_util_terminal::report::Report]:
//! - Try to keep the report succinct while ensuring a beginner can understand what is wrong and how to fix.
//!   It is a difficult balance to hit; err on the side of providing extra information.
//! - Messages should generally be a phrase, starting with a lowercase letter.
//!   If multiple sentences are needed, consider if a [message][cargo_util_terminal::report::Message] or sub-diagnostic would be more
//!   appropriate.
//! - Only the first lint for a package should emit the [`lint::Lint::emitted_source`]
//!
//! See also [rustc's Errors and Lints](https://rustc-dev-guide.rust-lang.org/diagnostics.html)
//!
//! # Adding a pass
//!
//! When a diagnostic requires adding a new pass, keep in mind:
//! - Support for `build.warnings`
//! - When errors should block further evaluation within the pass
//! - Providing a summary at the end, like what is provided by [`DiagnosticStats::report_summary`]
//! - Prefer data driven passes to simplify adding rules
//!   - Ensure the pass' lints are in [`rules::LINTS`], e.g. `ensure_parse_passed_in_lints`
//!   - Prefer evaluating the lint level within the pass
//!
//! See [`passes::emit_parse_diagnostics`] as an example.
//!
//! [future-incompat lint]: https://rustc-dev-guide.rust-lang.org/diagnostics.html#future-incompatible-lints

use anyhow::bail;
use cargo_util_schemas::manifest::RustVersion;
use cargo_util_schemas::manifest::TomlToolLints;

use crate::CargoResult;
use crate::core::Workspace;
use crate::core::{Edition, Features, MaybePackage, Package};
use crate::util::GlobalContext;

mod lint;
mod report;

pub mod passes;
pub mod rules;

pub use lint::{Lint, LintGroup, LintLevel, LintLevelProduct, LintLevelSource};
pub use report::{AsIndex, get_key_value, get_key_value_span, rel_cwd_manifest_path};
pub use rules::{LINT_GROUPS, LINTS};

pub struct DiagnosticStats {
    warning_count: usize,
    lint_warning_count: usize,
    error_count: usize,
}

impl DiagnosticStats {
    pub fn new() -> Self {
        Self {
            warning_count: 0,
            lint_warning_count: 0,
            error_count: 0,
        }
    }

    pub fn lint_warning_count(&self) -> usize {
        self.lint_warning_count
    }

    pub fn warning_count(&self) -> usize {
        self.warning_count
    }

    pub fn error_count(&self) -> usize {
        self.error_count
    }

    pub fn record_warning(&mut self) {
        self.warning_count += 1;
    }

    pub fn record_error(&mut self) {
        self.error_count += 1;
    }

    pub fn record_lint(&mut self, lint: LintLevel) {
        match lint {
            LintLevel::Forbid | LintLevel::Deny => {
                self.record_error();
            }
            LintLevel::Warn => {
                self.lint_warning_count += 1;
                self.record_warning();
            }
            LintLevel::Allow => {}
        }
    }

    pub fn report_summary(
        &self,
        action: &str,
        name: Option<&str>,
        gctx: &GlobalContext,
    ) -> CargoResult<()> {
        if 0 < self.warning_count {
            let plural = if self.warning_count == 1 { "" } else { "s" };
            let name = name
                .map(|n| format!("`{n}`"))
                .unwrap_or_else(|| "workspace".to_owned());
            gctx.shell().warn(format!(
                "{name} (manifest) generated {} warning{plural}",
                self.warning_count
            ))?;
        }

        if 0 < self.error_count {
            let plural = if self.error_count == 1 { "" } else { "s" };
            let name = name
                .map(|n| format!("`{n}`"))
                .unwrap_or_else(|| "workspace".to_owned());
            bail!(
                "could not {action} {name} (manifest) due to {} previous error{plural}",
                self.error_count
            )
        }

        Ok(())
    }
}

impl std::ops::Add for DiagnosticStats {
    type Output = DiagnosticStats;

    fn add(mut self, rhs: Self) -> Self::Output {
        self += rhs;
        self
    }
}

impl std::ops::AddAssign for DiagnosticStats {
    fn add_assign(&mut self, rhs: Self) {
        let DiagnosticStats {
            warning_count,
            lint_warning_count,
            error_count,
        } = rhs;
        self.warning_count += warning_count;
        self.lint_warning_count += lint_warning_count;
        self.error_count += error_count;
    }
}

/// Scope at which a lint runs: package-level or workspace-level.
pub enum ManifestFor<'a> {
    /// Lint runs for a specific package.
    Package(&'a Package),
    /// Lint runs for workspace-level config.
    Workspace {
        ws: &'a Workspace<'a>,
        maybe_pkg: &'a MaybePackage,
    },
}

impl ManifestFor<'_> {
    fn lint_level(&self, pkg_lints: &TomlToolLints, lint: &Lint) -> LintLevelProduct {
        lint.level(pkg_lints, self.rust_version(), self.unstable_features())
    }

    pub fn rust_version(&self) -> Option<&RustVersion> {
        match self {
            ManifestFor::Package(p) => p.rust_version(),
            ManifestFor::Workspace { ws, maybe_pkg: _ } => ws.lowest_rust_version(),
        }
    }

    pub fn contents(&self) -> Option<&str> {
        match self {
            ManifestFor::Package(p) => p.manifest().contents(),
            ManifestFor::Workspace { ws: _, maybe_pkg } => maybe_pkg.contents(),
        }
    }

    pub fn document(&self) -> Option<&toml::Spanned<toml::de::DeTable<'static>>> {
        match self {
            ManifestFor::Package(p) => p.manifest().document(),
            ManifestFor::Workspace { ws: _, maybe_pkg } => maybe_pkg.document(),
        }
    }

    pub fn edition(&self) -> Edition {
        match self {
            ManifestFor::Package(p) => p.manifest().edition(),
            ManifestFor::Workspace { ws: _, maybe_pkg } => maybe_pkg.edition(),
        }
    }

    pub fn unstable_features(&self) -> &Features {
        match self {
            ManifestFor::Package(p) => p.manifest().unstable_features(),
            ManifestFor::Workspace { ws: _, maybe_pkg } => maybe_pkg.unstable_features(),
        }
    }
}

impl<'a> From<&'a Package> for ManifestFor<'a> {
    fn from(value: &'a Package) -> ManifestFor<'a> {
        ManifestFor::Package(value)
    }
}

impl<'a> From<(&'a Workspace<'a>, &'a MaybePackage)> for ManifestFor<'a> {
    fn from((ws, maybe_pkg): (&'a Workspace<'a>, &'a MaybePackage)) -> ManifestFor<'a> {
        ManifestFor::Workspace { ws, maybe_pkg }
    }
}
