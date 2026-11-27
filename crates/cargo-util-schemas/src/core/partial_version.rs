use std::cmp::Ordering;
use std::fmt::{self, Display};
use std::hash::{Hash, Hasher};

use semver::{Comparator, Prerelease, Version, VersionReq};
use serde_untagged::UntaggedEnumVisitor;

#[derive(Clone, Debug)]
pub struct PartialVersion {
    pub major: u64,
    pub minor: Option<u64>,
    pub patch: Option<u64>,
    pub pre: Option<semver::Prerelease>,
    pub build: Option<semver::BuildMetadata>,
}

impl PartialEq for PartialVersion {
    fn eq(&self, other: &Self) -> bool {
        // An explicitly empty prerelease is semantically the same as no
        // prerelease. Normalize it so equality agrees with ordering.
        self.major == other.major
            && self.minor == other.minor
            && self.patch == other.patch
            && self.pre.as_ref().unwrap_or(&Prerelease::EMPTY)
                == other.pre.as_ref().unwrap_or(&Prerelease::EMPTY)
            && self.build == other.build
    }
}

impl Eq for PartialVersion {}

impl PartialOrd for PartialVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PartialVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        self.major
            .cmp(&other.major)
            .then_with(|| self.minor.cmp(&other.minor))
            .then_with(|| self.patch.cmp(&other.patch))
            .then_with(|| {
                // SemVer sorts a prerelease before its release, while `Option`
                // would sort `None` before `Some`. Normalizing to
                // `Prerelease::EMPTY` gives the SemVer behavior because it
                // sorts after all non-empty prereleases.
                self.pre
                    .as_ref()
                    .unwrap_or(&Prerelease::EMPTY)
                    .cmp(other.pre.as_ref().unwrap_or(&Prerelease::EMPTY))
            })
            .then_with(|| self.build.cmp(&other.build))
    }
}

impl Hash for PartialVersion {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.major.hash(state);
        self.minor.hash(state);
        self.patch.hash(state);
        self.pre.as_ref().unwrap_or(&Prerelease::EMPTY).hash(state);
        self.build.hash(state);
    }
}

impl PartialVersion {
    pub fn to_version(&self) -> Option<Version> {
        Some(Version {
            major: self.major,
            minor: self.minor?,
            patch: self.patch?,
            pre: self.pre.clone().unwrap_or_default(),
            build: self.build.clone().unwrap_or_default(),
        })
    }

    pub fn to_caret_req(&self) -> VersionReq {
        VersionReq {
            comparators: vec![Comparator {
                op: semver::Op::Caret,
                major: self.major,
                minor: self.minor,
                patch: self.patch,
                pre: self.pre.as_ref().cloned().unwrap_or_default(),
            }],
        }
    }

    /// Check if this matches a version, including build metadata
    ///
    /// Build metadata does not affect version precedence but may be necessary for uniquely
    /// identifying a package.
    pub fn matches(&self, version: &Version) -> bool {
        if !version.pre.is_empty() && self.pre.is_none() {
            // Pre-release versions must be explicitly opted into, if for no other reason than to
            // give us room to figure out and define the semantics
            return false;
        }
        self.major == version.major
            && self.minor.map(|f| f == version.minor).unwrap_or(true)
            && self.patch.map(|f| f == version.patch).unwrap_or(true)
            && self.pre.as_ref().map(|f| f == &version.pre).unwrap_or(true)
            && self
                .build
                .as_ref()
                .map(|f| f == &version.build)
                .unwrap_or(true)
    }
}

impl From<semver::Version> for PartialVersion {
    fn from(ver: semver::Version) -> Self {
        let pre = if ver.pre.is_empty() {
            None
        } else {
            Some(ver.pre)
        };
        let build = if ver.build.is_empty() {
            None
        } else {
            Some(ver.build)
        };
        Self {
            major: ver.major,
            minor: Some(ver.minor),
            patch: Some(ver.patch),
            pre,
            build,
        }
    }
}

impl std::str::FromStr for PartialVersion {
    type Err = PartialVersionError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match semver::Version::parse(value) {
            Ok(ver) => Ok(ver.into()),
            Err(_) => {
                // HACK: Leverage `VersionReq` for partial version parsing
                let mut version_req = match semver::VersionReq::parse(value) {
                    Ok(req) => req,
                    Err(_) if value.contains('-') => return Err(ErrorKind::Prerelease.into()),
                    Err(_) if value.contains('+') => return Err(ErrorKind::BuildMetadata.into()),
                    Err(_) => return Err(ErrorKind::Unexpected.into()),
                };
                if version_req.comparators.len() != 1 {
                    return Err(ErrorKind::VersionReq.into());
                }
                let comp = version_req.comparators.pop().unwrap();
                if comp.op != semver::Op::Caret {
                    return Err(ErrorKind::VersionReq.into());
                } else if value.starts_with('^') {
                    // Can't distinguish between `^` present or not
                    return Err(ErrorKind::VersionReq.into());
                }
                let pre = if comp.pre.is_empty() {
                    None
                } else {
                    Some(comp.pre)
                };
                Ok(Self {
                    major: comp.major,
                    minor: comp.minor,
                    patch: comp.patch,
                    pre,
                    build: None,
                })
            }
        }
    }
}

impl Display for PartialVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let major = self.major;
        write!(f, "{major}")?;
        if let Some(minor) = self.minor {
            write!(f, ".{minor}")?;
        }
        if let Some(patch) = self.patch {
            write!(f, ".{patch}")?;
        }
        if let Some(pre) = self.pre.as_ref().filter(|pre| !pre.is_empty()) {
            write!(f, "-{pre}")?;
        }
        if let Some(build) = self.build.as_ref() {
            write!(f, "+{build}")?;
        }
        Ok(())
    }
}

impl serde::Serialize for PartialVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_str(self)
    }
}

impl<'de> serde::Deserialize<'de> for PartialVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        UntaggedEnumVisitor::new()
            .expecting("SemVer version")
            .string(|value| value.parse().map_err(serde::de::Error::custom))
            .deserialize(deserializer)
    }
}

/// Error parsing a [`PartialVersion`].
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct PartialVersionError(#[from] ErrorKind);

/// Non-public error kind for [`PartialVersionError`].
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
enum ErrorKind {
    #[error("unexpected version requirement, expected a version like \"1.32\"")]
    VersionReq,

    #[error("unexpected prerelease field, expected a version like \"1.32\"")]
    Prerelease,

    #[error("unexpected build field, expected a version like \"1.32\"")]
    BuildMetadata,

    #[error("expected a version like \"1.32\"")]
    Unexpected,
}

#[cfg(test)]
mod test {
    use super::*;
    use snapbox::prelude::*;
    use snapbox::str;

    #[test]
    fn parse_success() {
        let cases = &[
            // Valid pre-release
            ("1.43.0-beta.1", str!["1.43.0-beta.1"]),
            // Valid pre-release with wildcard
            ("1.43.0-beta.1.x", str!["1.43.0-beta.1.x"]),
        ];
        for (input, expected) in cases {
            let actual: Result<PartialVersion, _> = input.parse();
            let actual = match actual {
                Ok(result) => result.to_string(),
                Err(err) => format!("didn't pass: {err}"),
            };
            snapbox::assert_data_eq!(actual, expected.clone().raw());
        }
    }

    #[test]
    fn prerelease_ordering() {
        let versions = [
            "1.0.0-alpha",
            "1.0.0-alpha.1",
            "1.0.0-alpha.beta",
            "1.0.0-beta",
            "1.0.0-beta.2",
            "1.0.0-beta.11",
            "1.0.0-rc.1",
            "1.0.0",
        ];
        for pair in versions.windows(2) {
            let lhs: PartialVersion = pair[0].parse().unwrap();
            let rhs: PartialVersion = pair[1].parse().unwrap();
            assert!(lhs < rhs, "expected {lhs} to sort before {rhs}");
        }
    }

    #[test]
    fn empty_prerelease_sorts_like_release() {
        let stable: PartialVersion = "1.0.0".parse().unwrap();
        let mut empty = stable.clone();
        empty.pre = Some(semver::Prerelease::EMPTY);

        assert_eq!(empty, stable);
        assert_eq!(empty.cmp(&stable), std::cmp::Ordering::Equal);
        assert_eq!(empty.to_string(), stable.to_string());

        let mut empty_hash = std::hash::DefaultHasher::new();
        empty.hash(&mut empty_hash);
        let mut stable_hash = std::hash::DefaultHasher::new();
        stable.hash(&mut stable_hash);
        assert_eq!(empty_hash.finish(), stable_hash.finish());
    }

    #[test]
    fn parse_errors() {
        let cases = &[
            // Disallow caret
            (
                "^1.43",
                str![[r#"unexpected version requirement, expected a version like "1.32""#]],
            ),
            // Bad pre-release
            (
                "1.43-beta.1",
                str![[r#"unexpected prerelease field, expected a version like "1.32""#]],
            ),
            // Weird wildcard
            (
                "x",
                str![[r#"unexpected version requirement, expected a version like "1.32""#]],
            ),
            (
                "1.x",
                str![[r#"unexpected version requirement, expected a version like "1.32""#]],
            ),
            (
                "1.1.x",
                str![[r#"unexpected version requirement, expected a version like "1.32""#]],
            ),
            // Non-sense
            ("foodaddle", str![[r#"expected a version like "1.32""#]]),
        ];
        for (input, expected) in cases {
            let actual: Result<PartialVersion, _> = input.parse();
            let actual = match actual {
                Ok(result) => format!("didn't fail: {result:?}"),
                Err(err) => err.to_string(),
            };
            snapbox::assert_data_eq!(actual, expected.clone().raw());
        }
    }
}
