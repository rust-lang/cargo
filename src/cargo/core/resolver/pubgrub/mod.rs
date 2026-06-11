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

use crate::core::resolver::Resolve;
use crate::core::resolver::ResolveVersion;
use crate::core::resolver::VersionPreferences;
use crate::core::resolver::types::ResolveOpts;
use crate::core::{Dependency, PackageIdSpec, Registry, Summary};
use crate::util::context::GlobalContext;
use crate::util::errors::CargoResult;

mod package;
mod semver_pubgrub;

/// Resolve the dependency graph using the PubGrub algorithm.
///
/// This mirrors the signature of [`super::resolve`] so the two resolvers are
/// drop-in interchangeable at the call site in `ops::resolve`.
pub(super) fn resolve(
    _summaries: &[(Summary, ResolveOpts)],
    _replacements: &[(PackageIdSpec, Dependency)],
    _registry: &impl Registry,
    _version_prefs: &VersionPreferences,
    _resolve_version: ResolveVersion,
    _gctx: Option<&GlobalContext>,
) -> CargoResult<Resolve> {
    anyhow::bail!("the `-Zpubgrub-resolver` resolver is not yet implemented");
}
