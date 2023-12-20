use std::fmt::{self, Display};

use semver::{Comparator, Version, VersionReq};
use serde_untagged::UntaggedEnumVisitor;

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Debug)]
pub struct PartialVersion {
    pub major: u64,
    pub minor: Option<u64>,
    pub patch: Option<u64>,
    pub pre: Option<semver::Prerelease>,
    pub build: Option<semver::BuildMetadata>,
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
        if is_req(value) {
            return Err(ErrorKind::VersionReq.into());
        }
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
                assert_eq!(version_req.comparators.len(), 1, "guaranteed by is_req");
                let comp = version_req.comparators.pop().unwrap();
                assert_eq!(comp.op, semver::Op::Caret, "guaranteed by is_req");
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
        if let Some(pre) = self.pre.as_ref() {
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

fn is_req(value: &str) -> bool {
    let Some(first) = value.chars().next() else {
        return false;
    };
    "<>=^~".contains(first) || value.contains('*') || value.contains(',')
}
