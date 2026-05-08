use anyhow::bail;
use cargo_util_schemas::manifest::RustVersion;
use cargo_util_schemas::manifest::TomlToolLints;

use crate::CargoResult;
use crate::core::Workspace;
use crate::core::{Edition, Features, MaybePackage, Package};
use crate::util::GlobalContext;

mod lint;
mod report;

pub mod rules;

pub use lint::{Lint, LintGroup, LintLevel, LintLevelSource};
pub use report::{AsIndex, get_key_value, get_key_value_span, rel_cwd_manifest_path};
pub use rules::{LINT_GROUPS, LINTS};

pub struct DiagnosticStats {
    warning_count: usize,
    error_count: usize,
}

impl DiagnosticStats {
    pub fn new() -> Self {
        Self {
            warning_count: 0,
            error_count: 0,
        }
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
    fn lint_level(&self, pkg_lints: &TomlToolLints, lint: &Lint) -> (LintLevel, LintLevelSource) {
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
