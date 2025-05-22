use std::fmt;
use std::fmt::Display;

use serde_untagged::UntaggedEnumVisitor;

use crate::core::PartialVersion;
use crate::core::PartialVersionError;

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Debug, serde::Serialize)]
#[serde(transparent)]
pub struct RustVersion(PartialVersion);

impl RustVersion {
    pub fn is_compatible_with(&self, rustc: &PartialVersion) -> bool {
        let msrv = self.0.to_caret_req();
        // Remove any pre-release identifiers for easier comparison
        let rustc = semver::Version {
            major: rustc.major,
            minor: rustc.minor.unwrap_or_default(),
            patch: rustc.patch.unwrap_or_default(),
            pre: Default::default(),
            build: Default::default(),
        };
        msrv.matches(&rustc)
    }

    pub fn into_partial(self) -> PartialVersion {
        self.0
    }

    pub fn as_partial(&self) -> &PartialVersion {
        &self.0
    }
}

impl std::str::FromStr for RustVersion {
    type Err = RustVersionError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let partial = value.parse::<PartialVersion>();
        let partial = partial.map_err(RustVersionErrorKind::PartialVersion)?;
        partial.try_into()
    }
}

impl TryFrom<semver::Version> for RustVersion {
    type Error = RustVersionError;

    fn try_from(version: semver::Version) -> Result<Self, Self::Error> {
        let version = PartialVersion::from(version);
        Self::try_from(version)
    }
}

impl TryFrom<PartialVersion> for RustVersion {
    type Error = RustVersionError;

    fn try_from(partial: PartialVersion) -> Result<Self, Self::Error> {
        if partial.pre.is_some() {
            return Err(RustVersionErrorKind::Prerelease.into());
        }
        if partial.build.is_some() {
            return Err(RustVersionErrorKind::BuildMetadata.into());
        }
        Ok(Self(partial))
    }
}

impl<'de> serde::Deserialize<'de> for RustVersion {
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

impl Display for RustVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Error parsing a [`RustVersion`].
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct RustVersionError(#[from] RustVersionErrorKind);

/// Non-public error kind for [`RustVersionError`].
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
enum RustVersionErrorKind {
    #[error("unexpected prerelease field, expected a version like \"1.32\"")]
    Prerelease,

    #[error("unexpected build field, expected a version like \"1.32\"")]
    BuildMetadata,

    #[error(transparent)]
    PartialVersion(#[from] PartialVersionError),
}

#[cfg(test)]
mod test {
    use super::*;
    use snapbox::prelude::*;
    use snapbox::str;

    #[test]
    fn is_compatible_with_rustc() {
        let cases = &[
            ("1", "1.70.0", true),
            ("1.30", "1.70.0", true),
            ("1.30.10", "1.70.0", true),
            ("1.70", "1.70.0", true),
            ("1.70.0", "1.70.0", true),
            ("1.70.1", "1.70.0", false),
            ("1.70", "1.70.0-nightly", true),
            ("1.70.0", "1.70.0-nightly", true),
            ("1.71", "1.70.0", false),
            ("2", "1.70.0", false),
        ];
        let mut passed = true;
        for (msrv, rustc, expected) in cases {
            let msrv: RustVersion = msrv.parse().unwrap();
            let rustc = PartialVersion::from(semver::Version::parse(rustc).unwrap());
            if msrv.is_compatible_with(&rustc) != *expected {
                println!("failed: {msrv} is_compatible_with {rustc} == {expected}");
                passed = false;
            }
        }
        assert!(passed);
    }

    #[test]
    fn is_compatible_with_workspace_msrv() {
        let cases = &[
            ("1", "1", true),
            ("1", "1.70", true),
            ("1", "1.70.0", true),
            ("1.30", "1", false),
            ("1.30", "1.70", true),
            ("1.30", "1.70.0", true),
            ("1.30.10", "1", false),
            ("1.30.10", "1.70", true),
            ("1.30.10", "1.70.0", true),
            ("1.70", "1", false),
            ("1.70", "1.70", true),
            ("1.70", "1.70.0", true),
            ("1.70.0", "1", false),
            ("1.70.0", "1.70", true),
            ("1.70.0", "1.70.0", true),
            ("1.70.1", "1", false),
            ("1.70.1", "1.70", false),
            ("1.70.1", "1.70.0", false),
            ("1.71", "1", false),
            ("1.71", "1.70", false),
            ("1.71", "1.70.0", false),
            ("2", "1.70.0", false),
        ];
        let mut passed = true;
        for (dep_msrv, ws_msrv, expected) in cases {
            let dep_msrv: RustVersion = dep_msrv.parse().unwrap();
            let ws_msrv = ws_msrv.parse::<RustVersion>().unwrap().into_partial();
            if dep_msrv.is_compatible_with(&ws_msrv) != *expected {
                println!("failed: {dep_msrv} is_compatible_with {ws_msrv} == {expected}");
                passed = false;
            }
        }
        assert!(passed);
    }

    #[test]
    fn parse_errors() {
        let cases = &[
            // Disallow caret
            (
                "^1.43",
                str![[r#"unexpected version requirement, expected a version like "1.32""#]],
            ),
            // Valid pre-release
            (
                "1.43.0-beta.1",
                str![[r#"unexpected prerelease field, expected a version like "1.32""#]],
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
            let actual: Result<RustVersion, _> = input.parse();
            let actual = match actual {
                Ok(result) => format!("didn't fail: {result:?}"),
                Err(err) => err.to_string(),
            };
            snapbox::assert_data_eq!(actual, expected.clone().raw());
        }
    }
}
