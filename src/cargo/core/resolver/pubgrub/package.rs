//! The PubGrub "package" encoding for Cargo's resolution problem.
//!
//! PubGrub selects at most one version per package. Cargo needs to (a) allow a
//! crate to appear at several semver-incompatible versions and (b) perform
//! feature unification. We encode both into a richer package identity,
//! [`PubGrubPackage`], adapted from the encoding used by
//! `Eh2406/pubgrub-crates-benchmark` (the `Names` enum) but extended to carry a
//! [`SourceId`] (Cargo resolves across multiple sources) and to own its data.
//!
//! The variants are:
//!
//! * [`PubGrubPackage::Root`] — a synthetic package whose dependencies are the
//!   workspace members being resolved.
//! * [`PubGrubPackage::Bucket`] — a concrete crate, identified by name, source
//!   and [`SemverCompatibility`] bucket. Selecting a version of a bucket is
//!   selecting a concrete crate version. Distinct buckets may coexist, which is
//!   how incompatible majors are allowed.
//! * [`PubGrubPackage::BucketFeatures`] — a virtual package standing for "this
//!   feature (or optional dependency) of the bucket is enabled". Feature
//!   unification falls out of normal version solving over these packages.
//! * [`PubGrubPackage::BucketDefaultFeatures`] — "default features of the bucket
//!   are enabled".
//! * [`PubGrubPackage::Wide`] (+ feature variants) — used when a dependency's
//!   version requirement could span more than one compatibility bucket. The
//!   wide package defers the choice of bucket to a second resolution step.
//! * [`PubGrubPackage::Links`] — enforces the global uniqueness of a `links`
//!   attribute value.

use std::fmt::{self, Display};

use semver::VersionReq;

use crate::core::SourceId;
use crate::util::OptVersionReq;
use crate::util::interning::InternedString;

use super::semver_pubgrub::{SemverCompatibility, SemverPubgrub};

/// Distinguishes the two feature "namespaces" Cargo uses: a real feature name
/// (`Feat`) versus an optional dependency activated via `dep:` (`Dep`).
#[derive(Clone, Copy, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub enum FeatureNamespace {
    /// A named feature (`feature = [..]`) or `crate/feat`.
    Feat(InternedString),
    /// An optional dependency named with `dep:name`.
    Dep(InternedString),
}

impl Display for FeatureNamespace {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FeatureNamespace::Dep(n) => write!(f, "dep:{n}"),
            FeatureNamespace::Feat(n) => write!(f, "{n}"),
        }
    }
}

/// Identity of a concrete crate within a single compatibility bucket.
#[derive(Clone, Eq, PartialEq, Hash)]
pub struct BucketName {
    pub name: InternedString,
    pub source: SourceId,
    pub compat: SemverCompatibility,
}

impl Display for BucketName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{:?}", self.name, self.compat)
    }
}

/// Identity of a "wide" dependency whose requirement may span multiple buckets.
///
/// The requirement and the requesting parent's bucket are part of the identity
/// so that two parents requesting the same crate with different wide
/// requirements remain distinct packages.
#[derive(Clone, Eq, PartialEq, Hash)]
pub struct WideName {
    pub name: InternedString,
    pub source: SourceId,
    pub req: VersionReq,
    pub from: InternedString,
    pub from_compat: SemverCompatibility,
}

impl Display for WideName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}(from {}@{:?}):{}", self.name, self.from, self.from_compat, self.req)
    }
}

/// A PubGrub package: the unit over which PubGrub selects a single version.
#[derive(Clone, Eq, PartialEq, Hash)]
pub enum PubGrubPackage {
    /// Synthetic root; its dependencies are the workspace members.
    Root,
    /// A concrete crate bucket. `member` is true for workspace members being
    /// resolved directly (which also pull in their dev-dependencies).
    /// `all_features` is true when every feature/optional dependency should be
    /// activated (the lock-file resolution pass), as opposed to a specific set
    /// selected through [`PubGrubPackage::BucketFeatures`].
    Bucket { name: BucketName, member: bool, all_features: bool },
    /// "Feature (or optional dep) of the bucket is enabled".
    BucketFeatures { name: BucketName, feature: FeatureNamespace },
    /// "Default features of the bucket are enabled".
    BucketDefaultFeatures { name: BucketName },
    /// A wide dependency spanning multiple buckets.
    Wide { name: WideName },
    /// A wide dependency with a feature enabled.
    WideFeatures { name: WideName, feature: FeatureNamespace },
    /// A wide dependency with default features enabled.
    WideDefaultFeatures { name: WideName },
    /// Enforces global uniqueness of a `links` value.
    Links { links: InternedString },
}

impl PubGrubPackage {
    /// The same bucket package, with default features enabled.
    pub fn with_default_features(&self) -> Self {
        match self {
            PubGrubPackage::Bucket { name, .. }
            | PubGrubPackage::BucketFeatures { name, .. }
            | PubGrubPackage::BucketDefaultFeatures { name } => {
                PubGrubPackage::BucketDefaultFeatures { name: name.clone() }
            }
            PubGrubPackage::Wide { name }
            | PubGrubPackage::WideFeatures { name, .. }
            | PubGrubPackage::WideDefaultFeatures { name } => {
                PubGrubPackage::WideDefaultFeatures { name: name.clone() }
            }
            PubGrubPackage::Root | PubGrubPackage::Links { .. } => {
                panic!("with_default_features on non-crate package")
            }
        }
    }

    /// The same bucket package, with the given feature enabled.
    pub fn with_feature(&self, feature: FeatureNamespace) -> Self {
        match self {
            PubGrubPackage::Bucket { name, .. }
            | PubGrubPackage::BucketFeatures { name, .. }
            | PubGrubPackage::BucketDefaultFeatures { name } => {
                PubGrubPackage::BucketFeatures { name: name.clone(), feature }
            }
            PubGrubPackage::Wide { name }
            | PubGrubPackage::WideFeatures { name, .. }
            | PubGrubPackage::WideDefaultFeatures { name } => {
                PubGrubPackage::WideFeatures { name: name.clone(), feature }
            }
            PubGrubPackage::Root | PubGrubPackage::Links { .. } => {
                panic!("with_feature on non-crate package")
            }
        }
    }
}

impl Display for PubGrubPackage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PubGrubPackage::Root => f.write_str("root"),
            PubGrubPackage::Bucket { name, member, all_features } => {
                write!(
                    f,
                    "{name}{}{}",
                    if *member { " (member)" } else { "" },
                    if *all_features { " (all-features)" } else { "" },
                )
            }
            PubGrubPackage::BucketFeatures { name, feature } => write!(f, "{name}/{feature}"),
            PubGrubPackage::BucketDefaultFeatures { name } => write!(f, "{name}/default"),
            PubGrubPackage::Wide { name } => write!(f, "wide:{name}"),
            PubGrubPackage::WideFeatures { name, feature } => write!(f, "wide:{name}/{feature}"),
            PubGrubPackage::WideDefaultFeatures { name } => write!(f, "wide:{name}/default"),
            PubGrubPackage::Links { links } => write!(f, "links:{links}"),
        }
    }
}

impl fmt::Debug for PubGrubPackage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(self, f)
    }
}

/// Convert a dependency's [`OptVersionReq`] into a PubGrub [`SemverPubgrub`].
///
/// `Locked`/`Precise` reqs pin to an exact version (they carry a concrete
/// [`semver::Version`]); everything else uses the underlying [`VersionReq`].
pub fn opt_version_req_to_pubgrub(req: &OptVersionReq) -> SemverPubgrub {
    match req {
        OptVersionReq::Any => SemverPubgrub::full(),
        OptVersionReq::Req(req) => SemverPubgrub::from(req),
        OptVersionReq::Locked(v, _) | OptVersionReq::Precise(v, _) => {
            SemverPubgrub::singleton(v.clone())
        }
    }
}

/// Extract a [`VersionReq`] from an [`OptVersionReq`] for use in [`WideName`].
pub fn opt_version_req_to_version_req(req: &OptVersionReq) -> VersionReq {
    match req {
        OptVersionReq::Any => VersionReq::STAR,
        OptVersionReq::Req(req)
        | OptVersionReq::Locked(_, req)
        | OptVersionReq::Precise(_, req) => req.clone(),
    }
}
