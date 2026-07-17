use super::semver_eval_ext;
use semver::{Comparator, Op, Version, VersionReq};
use std::fmt::{self, Display};

pub trait VersionExt {
    fn is_prerelease(&self) -> bool;

    fn to_req(&self, op: Op) -> VersionReq;

    fn to_exact_req(&self) -> VersionReq {
        self.to_req(Op::Exact)
    }

    fn to_caret_req(&self) -> VersionReq {
        self.to_req(Op::Caret)
    }
}

impl VersionExt for Version {
    fn is_prerelease(&self) -> bool {
        !self.pre.is_empty()
    }

    fn to_req(&self, op: Op) -> VersionReq {
        VersionReq {
            comparators: vec![Comparator {
                op,
                major: self.major,
                minor: Some(self.minor),
                patch: Some(self.patch),
                pre: self.pre.clone(),
            }],
        }
    }
}

pub trait VersionReqExt {
    fn matches_prerelease(&self, version: &Version) -> bool;
}

impl VersionReqExt for VersionReq {
    fn matches_prerelease(&self, version: &Version) -> bool {
        semver_eval_ext::matches_prerelease(self, version)
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

    /// Allows to match pre-release in SemVer-Compatible way.
    /// See [`semver_eval_ext`] for `matches_prerelease` semantics.
    pub fn matches_prerelease(&self, version: &Version) -> bool {
        if let OptVersionReq::Req(req) = self {
            return req.matches_prerelease(version);
        } else {
            return self.matches(version);
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

#[cfg(test)]
mod matches_prerelease {
    use semver::VersionReq;

    use super::OptVersionReq;
    use super::Version;

    #[test]
    fn prerelease() {
        // As of the writing, this test is not the final semantic of pre-release
        // semver matching. Part of the behavior is buggy. This test just tracks
        // the current behavior of the unstable `--precise <prerelease>`.
        //
        // The below transformation proposed in the RFC is hard to implement
        // outside the semver crate.
        //
        // ```
        // >=1.2.3, <2.0.0 -> >=1.2.3, <2.0.0-0
        // ```
        //
        // The upper bound semantic is also not resolved. So, at least two
        // outstanding issues are required to be fixed before the stabilization:
        //
        // * Bug 1: `x.y.z-pre.0` shouldn't match `x.y.z`.
        // * Upper bound: Whether `>=x.y.z-0, <x.y.z` should match `x.y.z-0`.
        //
        // See the RFC 3493 for the unresolved upper bound issue:
        // https://rust-lang.github.io/rfcs/3493-precise-pre-release-cargo-update.html#version-ranges-with-pre-release-upper-bounds
        let cases = [
            //
            ("1.2.3", "1.2.3-0", false),
            ("1.2.3", "1.2.3-1", false),
            ("1.2.3", "1.2.4-0", true),
            //
            (">=1.2.3", "1.2.3-0", false),
            (">=1.2.3", "1.2.3-1", false),
            (">=1.2.3", "1.2.4-0", true),
            //
            (">1.2.3", "1.2.3-0", false),
            (">1.2.3", "1.2.3-1", false),
            (">1.2.3", "1.2.4-0", true),
            //
            (">1.2.3, <1.2.4", "1.2.3-0", false),
            (">1.2.3, <1.2.4", "1.2.3-1", false),
            (">1.2.3, <1.2.4", "1.2.4-0", false), // upper bound semantic
            //
            (">=1.2.3, <1.2.4", "1.2.3-0", false),
            (">=1.2.3, <1.2.4", "1.2.3-1", false),
            (">=1.2.3, <1.2.4", "1.2.4-0", false), // upper bound semantic
            //
            (">1.2.3, <=1.2.4", "1.2.3-0", false),
            (">1.2.3, <=1.2.4", "1.2.3-1", false),
            (">1.2.3, <=1.2.4", "1.2.4-0", true),
            //
            (">=1.2.3-0, <1.2.3", "1.2.3-0", true), // upper bound semantic
            (">=1.2.3-0, <1.2.3", "1.2.3-1", true), // upper bound semantic
            (">=1.2.3-0, <1.2.3", "1.2.4-0", false),
            //
            ("1.2.3", "2.0.0-0", false), // upper bound semantics
            ("=1.2.3-0", "1.2.3", false),
            ("=1.2.3-0", "1.2.3-0", true),
            ("=1.2.3-0", "1.2.4", false),
            (">=1.2.3-2, <1.2.3-4", "1.2.3-0", false),
            (">=1.2.3-2, <1.2.3-4", "1.2.3-3", true),
            (">=1.2.3-2, <1.2.3-4", "1.2.3-5", false), // upper bound semantics
        ];
        for (req, ver, expected) in cases {
            let version_req = req.parse().unwrap();
            let version = ver.parse().unwrap();
            let matched = OptVersionReq::Req(version_req).matches_prerelease(&version);
            assert_eq!(expected, matched, "req: {req}; ver: {ver}");
        }
    }

    #[test]
    fn opt_version_req_matches_prerelease() {
        let req_ver: VersionReq = "^1.2.3-rc.0".parse().unwrap();
        let to_ver: Version = "1.2.3-rc.0".parse().unwrap();

        let req = OptVersionReq::Req(req_ver.clone());
        assert!(req.matches_prerelease(&to_ver));

        let req = OptVersionReq::Locked(to_ver.clone(), req_ver.clone());
        assert!(req.matches_prerelease(&to_ver));

        let req = OptVersionReq::Precise(to_ver.clone(), req_ver.clone());
        assert!(req.matches_prerelease(&to_ver));
    }
}
