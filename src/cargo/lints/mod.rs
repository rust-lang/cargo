use std::path::Path;

use cargo_util_schemas::manifest::RustVersion;
use cargo_util_schemas::manifest::TomlToolLints;
use cargo_util_terminal::report::AnnotationKind;
use cargo_util_terminal::report::Group;
use cargo_util_terminal::report::Level;
use cargo_util_terminal::report::Snippet;

use crate::core::Workspace;
use crate::core::{Edition, Feature, Features, MaybePackage, Package};
use crate::{CargoResult, GlobalContext};

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

pub fn missing_lints_features(
    manifest: ManifestFor<'_>,
    manifest_path: &Path,
    cargo_lints: &TomlToolLints,
    error_count: &mut usize,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    let manifest_path = rel_cwd_manifest_path(manifest_path, gctx);
    for lint_name in cargo_lints.keys().map(|name| name) {
        let Some((name, default_level, feature_gate)) = rules::find_lint_or_group(lint_name) else {
            continue;
        };

        let (_, source, _) = lint::level_priority(name, *default_level, cargo_lints);

        // Only run analysis on user-specified lints
        if !source.is_user_specified() {
            continue;
        }

        // Only run this on lints that are gated by a feature
        if let Some(feature_gate) = feature_gate
            && !manifest.unstable_features().is_enabled(feature_gate)
        {
            report_feature_not_enabled(
                name,
                feature_gate,
                &manifest,
                &manifest_path,
                error_count,
                gctx,
            )?;
        }
    }

    Ok(())
}

fn report_feature_not_enabled(
    lint_name: &str,
    feature_gate: &Feature,
    manifest: &ManifestFor<'_>,
    manifest_path: &str,
    error_count: &mut usize,
    gctx: &GlobalContext,
) -> CargoResult<()> {
    let dash_feature_name = feature_gate.name().replace("_", "-");

    let mut error = Group::with_title(
        Level::ERROR.primary_title(format!("use of unstable lint `{lint_name}`")),
    );

    if let Some(document) = manifest.document()
        && let Some(contents) = manifest.contents()
    {
        let key_path = match manifest {
            ManifestFor::Package(_) => &["lints", "cargo", lint_name][..],
            ManifestFor::Workspace { .. } => &["workspace", "lints", "cargo", lint_name][..],
        };
        let Some(span) = get_key_value_span(document, key_path) else {
            // This lint is handled by either package or workspace lint.
            return Ok(());
        };

        error = error.element(Snippet::source(contents).path(manifest_path).annotation(
            AnnotationKind::Primary.span(span.key).label(format!(
                "this is behind `{dash_feature_name}`, which is not enabled"
            )),
        ))
    }

    let report = [error.element(Level::HELP.message(format!(
        "consider adding `cargo-features = [\"{dash_feature_name}\"]` to the top of the manifest"
    )))];

    *error_count += 1;
    gctx.shell().print_report(&report, true)?;

    Ok(())
}
