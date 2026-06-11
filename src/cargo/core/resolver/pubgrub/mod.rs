//! An alternative dependency resolver built on the [`pubgrub`] crate.
//!
//! This is an experimental, side-by-side implementation of Cargo's dependency
//! resolver gated behind the `-Zpubgrub-resolver` unstable flag. The default
//! resolver in the parent [`super`] module is the hand-rolled backtracking
//! solver; this module instead encodes Cargo's resolution problem into the
//! PubGrub algorithm.
//!
//! # Encoding
//!
//! PubGrub natively allows only a single version of each "package" to be
//! selected. Cargo, however, allows the same crate to appear multiple times in
//! the graph at semver-incompatible versions, and performs feature
//! unification. To bridge this gap we use a richer notion of a "package", see
//! [`package::PubGrubPackage`]:
//!
//! * the real crate, bucketed by its semver-compatibility range, so that
//!   semver-incompatible versions are distinct PubGrub packages and may
//!   coexist;
//! * a virtual package per crate feature, so that feature unification falls out
//!   of normal version solving;
//! * a synthetic `root` package representing the set of workspace members being
//!   resolved.
//!
//! See the individual submodules for the details of each piece.

use pubgrub::PubGrubError;
use pubgrub::{DefaultStringReporter, Reporter};

use crate::core::resolver::Resolve;
use crate::core::resolver::ResolveVersion;
use crate::core::resolver::VersionPreferences;
use crate::core::resolver::dep_cache::RegistryQueryer;
use crate::core::resolver::features::{CliFeatures, RequestedFeatures};
use crate::core::resolver::types::ResolveOpts;
use crate::core::{Dependency, PackageIdSpec, Registry, Summary};
use crate::util::context::GlobalContext;
use crate::util::errors::CargoResult;
use crate::util::interning::InternedString;

mod package;
mod provider;
mod semver_pubgrub;
mod solution;

use self::package::PubGrubPackage;
use self::provider::{Provider, Root, root_version};

/// Resolve the dependency graph using the PubGrub algorithm.
///
/// This mirrors the signature of [`super::resolve`] so the two resolvers are
/// drop-in interchangeable at the call site in `ops::resolve`.
pub(super) fn resolve(
    summaries: &[(Summary, ResolveOpts)],
    replacements: &[(PackageIdSpec, Dependency)],
    registry: &impl Registry,
    version_prefs: &VersionPreferences,
    resolve_version: ResolveVersion,
    _gctx: Option<&GlobalContext>,
) -> CargoResult<Resolve> {
    let registry = RegistryQueryer::new(registry, replacements, version_prefs);

    let roots = summaries
        .iter()
        .map(|(summary, opts)| root_from_opts(summary.clone(), opts))
        .collect();

    let provider = Provider::new(registry, version_prefs, roots);

    match pubgrub::resolve(&provider, PubGrubPackage::Root, root_version()) {
        Ok(solution) => solution::into_resolve(&provider, &solution, resolve_version),
        Err(err) => {
            // A real (e.g. network) error stashed during a callback takes
            // precedence over PubGrub's own error.
            if let Some(err) = provider.take_error() {
                return Err(err);
            }
            Err(report_error(err))
        }
    }
}

/// Build a [`Root`] describing how a workspace member's features were requested.
fn root_from_opts(summary: Summary, opts: &ResolveOpts) -> Root {
    let (all_features, default_features, features) = match &opts.features {
        RequestedFeatures::CliFeatures(CliFeatures {
            features,
            all_features,
            uses_default_features,
        }) => {
            let names = features
                .iter()
                .filter_map(|fv| match fv {
                    crate::core::summary::FeatureValue::Feature(f) => Some(*f),
                    // `dep:`/`dep/feat` CLI features are uncommon for workspace
                    // members; treat their base name as a requested feature.
                    crate::core::summary::FeatureValue::Dep { dep_name } => Some(*dep_name),
                    crate::core::summary::FeatureValue::DepFeature { dep_feature, .. } => {
                        Some(*dep_feature)
                    }
                })
                .collect();
            (*all_features, *uses_default_features, names)
        }
        RequestedFeatures::DepFeatures {
            features,
            uses_default_features,
        } => {
            let names: Vec<InternedString> = features.iter().copied().collect();
            (false, *uses_default_features, names)
        }
    };
    Root {
        summary,
        dev_deps: opts.dev_deps,
        all_features,
        default_features,
        features,
    }
}

/// Turn a PubGrub error into a Cargo error.
///
/// This is currently a thin wrapper; richer reporting from the derivation tree
/// is layered on in a later change.
fn report_error<T: Registry>(err: PubGrubError<Provider<'_, T>>) -> anyhow::Error {
    match err {
        PubGrubError::NoSolution(mut derivation_tree) => {
            derivation_tree.collapse_no_versions();
            anyhow::anyhow!(
                "failed to select a version for the requirement\n{}",
                DefaultStringReporter::report(&derivation_tree)
            )
        }
        other => anyhow::anyhow!("pubgrub resolution failed: {other}"),
    }
}
