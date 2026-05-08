use cargo_util_schemas::manifest::RustVersion;
use cargo_util_schemas::manifest::TomlToolLints;

use crate::core::Workspace;
use crate::core::{Edition, Features, MaybePackage, Package};

mod lint;
mod report;

pub mod rules;

pub use lint::{Lint, LintGroup, LintLevel, LintLevelSource};
pub use report::{AsIndex, get_key_value, get_key_value_span, rel_cwd_manifest_path};
pub use rules::{LINT_GROUPS, LINTS};

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
