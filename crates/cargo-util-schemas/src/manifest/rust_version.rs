use std::fmt;
use std::fmt::Display;

use serde_untagged::UntaggedEnumVisitor;

use crate::core::PartialVersion;
use crate::core::PartialVersionError;

#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Debug, serde::Serialize)]
#[serde(transparent)]
pub struct RustVersion(PartialVersion);

impl std::ops::Deref for RustVersion {
    type Target = PartialVersion;

    fn deref(&self) -> &Self::Target {
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
