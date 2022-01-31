use semver::{Comparator, Op, Version, VersionReq};
use std::fmt::{self, Display};

#[derive(PartialEq, Eq, Hash, Clone, Debug)]
pub enum OptVersionReq {
    Any,
    Req(VersionReq),
    /// The exact locked version and the original version requirement.
    Locked(Version, VersionReq),
}

pub trait VersionExt {
    fn is_prerelease(&self) -> bool;
}

pub trait VersionReqExt {
    fn exact(version: &Version) -> Self;
}

impl VersionExt for Version {
    fn is_prerelease(&self) -> bool {
        !self.pre.is_empty()
    }
}

impl VersionReqExt for VersionReq {
    fn exact(version: &Version) -> Self {
        VersionReq {
            comparators: vec![Comparator {
                op: Op::Exact,
                major: version.major,
                minor: Some(version.minor),
                patch: Some(version.patch),
                pre: version.pre.clone(),
            }],
        }
    }
}

impl OptVersionReq {
    pub fn exact(version: &Version) -> Self {
        OptVersionReq::Req(VersionReq::exact(version))
    }

    pub fn is_exact(&self) -> bool {
        match self {
            OptVersionReq::Any => false,
            OptVersionReq::Req(req) => {
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
            Req(req) => Locked(version, req.clone()),
            Locked(_, req) => Locked(version, req.clone()),
        };
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
                v.major == version.major
                    && v.minor == version.minor
                    && v.patch == version.patch
                    && v.pre == version.pre
            }
        }
    }
}

impl Display for OptVersionReq {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OptVersionReq::Any => f.write_str("*"),
            OptVersionReq::Req(req) => Display::fmt(req, f),
            OptVersionReq::Locked(_, req) => Display::fmt(req, f),
        }
    }
}

impl From<VersionReq> for OptVersionReq {
    fn from(req: VersionReq) -> Self {
        OptVersionReq::Req(req)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locked_has_the_same_with_exact() {
        fn test_versions(target_ver: &str, vers: &[&str]) {
            let ver = Version::parse(target_ver).unwrap();
            let exact = OptVersionReq::exact(&ver);
            let mut locked = exact.clone();
            locked.lock_to(&ver);
            for v in vers {
                let v = Version::parse(v).unwrap();
                assert_eq!(exact.matches(&v), locked.matches(&v));
            }
        }

        test_versions(
            "1.0.0",
            &["1.0.0", "1.0.1", "0.9.9", "0.10.0", "0.1.0", "1.0.0-pre"],
        );
        test_versions("0.9.0", &["0.9.0", "0.9.1", "1.9.0", "0.0.9", "0.9.0-pre"]);
        test_versions("0.0.2", &["0.0.2", "0.0.1", "0.0.3", "0.0.2-pre"]);
        test_versions(
            "0.1.0-beta2.a",
            &[
                "0.1.0-beta2.a",
                "0.9.1",
                "0.1.0",
                "0.1.1-beta2.a",
                "0.1.0-beta2",
            ],
        );
        test_versions("0.1.0+meta", &["0.1.0", "0.1.0+meta", "0.1.0+any"]);
    }
}
