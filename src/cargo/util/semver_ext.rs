use std::fmt::{self, Display};

use semver::{Comparator, Op, Version, VersionReq};

pub trait VersionExt {
    fn is_prerelease(&self) -> bool;

    fn to_exact_req(&self) -> VersionReq;
}

impl VersionExt for Version {
    fn is_prerelease(&self) -> bool {
        !self.pre.is_empty()
    }

    fn to_exact_req(&self) -> VersionReq {
        VersionReq {
            comparators: vec![Comparator {
                op: Op::Exact,
                major: self.major,
                minor: Some(self.minor),
                patch: Some(self.patch),
                pre: self.pre.clone(),
            }],
        }
    }
}

#[derive(PartialEq, Eq, Hash, Clone, Debug)]
pub enum OptVersionReq {
    Any,
    Req(VersionReq),
    /// The exact locked version and the original version requirement.
    Locked(Version, VersionReq),
    /// The exact requested version and the original version requirement.
    ///
    /// This looks identical to [`OptVersionReq::Locked`] but has a different
    /// meaning, and is used for the `--precise` field of `cargo update`.
    /// See comments in [`OptVersionReq::matches`] for more.
    Precise(Version, VersionReq),
}

impl OptVersionReq {
    pub fn exact(version: &Version) -> Self {
        OptVersionReq::Req(version.to_exact_req())
    }

    // Since some registries have allowed crate versions to differ only by build metadata,
    // A query using OptVersionReq::exact return nondeterministic results.
    // So we `lock_to` the exact version were interested in.
    pub fn lock_to_exact(version: &Version) -> Self {
        OptVersionReq::Locked(version.clone(), version.to_exact_req())
    }

    pub fn is_exact(&self) -> bool {
        match self {
            OptVersionReq::Any => false,
            OptVersionReq::Req(req) | OptVersionReq::Precise(_, req) => {
                req.comparators.len() == 1 && {
                    let cmp = &req.comparators[0];
                    cmp.op == Op::Exact && cmp.minor.is_some() && cmp.patch.is_some()
                }
            }
            OptVersionReq::Locked(..) => true,
        }
    }

    pub fn lock_to(&mut self, version: &Version) {
        assert!(self.matches(version), "cannot lock {} to {}", self, version);
        use OptVersionReq::*;
        let version = version.clone();
        *self = match self {
            Any => Locked(version, VersionReq::STAR),
            Req(req) | Locked(_, req) | Precise(_, req) => Locked(version, req.clone()),
        };
    }

    /// Makes the requirement precise to the requested version.
    ///
    /// This is used for the `--precise` field of `cargo update`.
    pub fn precise_to(&mut self, version: &Version) {
        use OptVersionReq::*;
        let version = version.clone();
        *self = match self {
            Any => Precise(version, VersionReq::STAR),
            Req(req) | Locked(_, req) | Precise(_, req) => Precise(version, req.clone()),
        };
    }

    pub fn is_precise(&self) -> bool {
        matches!(self, OptVersionReq::Precise(..))
    }

    /// Gets the version to which this req is precise to, if any.
    pub fn precise_version(&self) -> Option<&Version> {
        match self {
            OptVersionReq::Precise(version, _) => Some(version),
            _ => None,
        }
    }

    pub fn is_locked(&self) -> bool {
        matches!(self, OptVersionReq::Locked(..))
    }

    /// Gets the version to which this req is locked, if any.
    pub fn locked_version(&self) -> Option<&Version> {
        match self {
            OptVersionReq::Locked(version, _) => Some(version),
            _ => None,
        }
    }

    pub fn matches(&self, version: &Version) -> bool {
        match self {
            OptVersionReq::Any => true,
            OptVersionReq::Req(req) => req.matches(version),
            OptVersionReq::Locked(v, _) => {
                // Generally, cargo is of the opinion that semver metadata should be ignored.
                // If your registry has two versions that only differing metadata you get the bugs you deserve.
                // We also believe that lock files should ensure reproducibility
                // and protect against mutations from the registry.
                // In this circumstance these two goals are in conflict, and we pick reproducibility.
                // If the lock file tells us that there is a version called `1.0.0+bar` then
                // we should not silently use `1.0.0+foo` even though they have the same version.
                v == version
            }
            OptVersionReq::Precise(v, _) => {
                // This is used for the `--precise` field of cargo update.
                //
                // Unfortunately crates.io allowed versions to differ only
                // by build metadata. This shouldn't be allowed, but since
                // it is, this will honor it if requested.
                //
                // In that context we treat a requirement that does not have
                // build metadata as allowing any metadata. But, if a requirement
                // has build metadata, then we only allow it to match the exact
                // metadata.
                v.major == version.major
                    && v.minor == version.minor
                    && v.patch == version.patch
                    && v.pre == version.pre
                    && (v.build == version.build || v.build.is_empty())
            }
        }
    }
}

impl Display for OptVersionReq {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OptVersionReq::Any => f.write_str("*"),
            OptVersionReq::Req(req)
            | OptVersionReq::Locked(_, req)
            | OptVersionReq::Precise(_, req) => Display::fmt(req, f),
        }
    }
}

impl From<VersionReq> for OptVersionReq {
    fn from(req: VersionReq) -> Self {
        OptVersionReq::Req(req)
    }
}
