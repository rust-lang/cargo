//! `Cargo.lock` / Lockfile schema definition

use std::collections::BTreeMap;
use std::fmt;
use std::{cmp::Ordering, str::FromStr};

use serde::{Deserialize, Serialize, de, ser};
use url::Url;

use crate::core::{GitReference, SourceKind};

/// Serialization of `Cargo.lock`
#[derive(Serialize, Deserialize, Debug)]
#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
pub struct TomlLockfile {
    /// The lockfile format version (`version =` field).
    ///
    /// This field is optional for backward compatibility. Older lockfiles, i.e. V1 and V2, does
    /// not have the version field serialized.
    pub version: Option<u32>,
    /// The list of `[[package]]` entries describing each resolved dependency.
    pub package: Option<Vec<TomlLockfileDependency>>,
    /// The `[root]` table describing the root package.
    ///
    /// This field is optional for backward compatibility. Older lockfiles have the root package
    /// separated, whereas newer lockfiles have the root package as part of `[[package]]`.
    pub root: Option<TomlLockfileDependency>,
    /// The `[metadata]` table
    ///
    ///
    /// In older lockfile versions, dependency checksums were stored here instead of alongside each
    /// package entry.
    pub metadata: Option<TomlLockfileMetadata>,
    /// The `[patch]` table describing unused patches.
    ///
    /// The lockfile stores them as a list of `[[patch.unused]]` entries.
    #[serde(default, skip_serializing_if = "TomlLockfilePatch::is_empty")]
    pub patch: TomlLockfilePatch,
}

/// Serialization of lockfiles metadata
///
/// Older versions of lockfiles have their dependencies' checksums on this `[metadata]` table.
pub type TomlLockfileMetadata = BTreeMap<String, String>;

/// Serialization of unused patches
///
/// Cargo stores patches that were declared but not used during resolution.
#[derive(Serialize, Deserialize, Debug, Default)]
#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
pub struct TomlLockfilePatch {
    /// The list of unused dependency patches.
    pub unused: Vec<TomlLockfileDependency>,
}

impl TomlLockfilePatch {
    fn is_empty(&self) -> bool {
        self.unused.is_empty()
    }
}

/// Serialization of lockfiles dependencies
#[derive(Serialize, Deserialize, Debug, PartialOrd, Ord, PartialEq, Eq)]
#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
pub struct TomlLockfileDependency {
    /// The name of the dependency.
    pub name: String,
    /// The version of the dependency.
    pub version: String,
    /// The source of the dependency.
    ///
    /// Cargo does not serialize path dependencies.
    pub source: Option<TomlLockfileSourceId>,
    /// The checksum of the dependency.
    ///
    /// In older lockfiles, checksums were not stored here and instead on a separate `[metadata]`
    /// table (see [`TomlLockfileMetadata`]).
    pub checksum: Option<String>,
    /// The transitive dependencies used by this dependency.
    pub dependencies: Option<Vec<TomlLockfilePackageId>>,
    /// The replace of the dependency.
    pub replace: Option<TomlLockfilePackageId>,
}

/// Serialization of dependency's source
#[derive(Debug, Clone)]
#[cfg_attr(
    feature = "unstable-schema",
    derive(schemars::JsonSchema),
    schemars(with = "String")
)]
pub struct TomlLockfileSourceId {
    /// The string representation of the source as it appears in the lockfile.
    source_str: String,
    /// The parsed source type, e.g. `git`, `registry`.
    ///
    /// Used for sources ordering.
    kind: SourceKind,
    /// The parsed URL of the source.
    ///
    /// Used for sources ordering.
    url: Url,
}

impl TomlLockfileSourceId {
    pub fn new(source: String) -> Result<Self, TomlLockfileSourceIdError> {
        let source_str = source.clone();
        let (kind, url) = source
            .split_once('+')
            .ok_or_else(|| TomlLockfileSourceIdErrorKind::InvalidSource(source.clone()))?;

        // Sparse URLs store the kind prefix (sparse+) in the URL. Therefore, for sparse kinds, we
        // want to use the raw `source` instead of the splitted `url`.
        let url = Url::parse(if kind == "sparse" { &source } else { url }).map_err(|msg| {
            TomlLockfileSourceIdErrorKind::InvalidUrl {
                url: url.to_string(),
                msg: msg.to_string(),
            }
        })?;

        let kind = match kind {
            "git" => {
                let reference = GitReference::from_query(url.query_pairs());
                SourceKind::Git(reference)
            }
            "registry" => SourceKind::Registry,
            "sparse" => SourceKind::SparseRegistry,
            "path" => SourceKind::Path,
            kind => {
                return Err(
                    TomlLockfileSourceIdErrorKind::UnsupportedSource(kind.to_string()).into(),
                );
            }
        };

        Ok(Self {
            source_str,
            kind,
            url,
        })
    }

    pub fn kind(&self) -> &SourceKind {
        &self.kind
    }

    pub fn url(&self) -> &Url {
        &self.url
    }

    pub fn source_str(&self) -> &String {
        &self.source_str
    }

    pub fn as_url(&self) -> impl fmt::Display + '_ {
        self.source_str.clone()
    }
}

impl ser::Serialize for TomlLockfileSourceId {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        s.collect_str(&self.as_url())
    }
}

impl<'de> de::Deserialize<'de> for TomlLockfileSourceId {
    fn deserialize<D>(d: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let s = String::deserialize(d)?;
        Ok(TomlLockfileSourceId::new(s).map_err(de::Error::custom)?)
    }
}

impl std::hash::Hash for TomlLockfileSourceId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.kind.hash(state);
        self.url.hash(state);
    }
}

impl std::cmp::PartialEq for TomlLockfileSourceId {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind && self.url == other.url
    }
}

impl std::cmp::Eq for TomlLockfileSourceId {}

impl PartialOrd for TomlLockfileSourceId {
    fn partial_cmp(&self, other: &TomlLockfileSourceId) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TomlLockfileSourceId {
    fn cmp(&self, other: &TomlLockfileSourceId) -> Ordering {
        self.kind
            .cmp(&other.kind)
            .then_with(|| self.url.cmp(&other.url))
    }
}

/// Serialization of package IDs.
///
/// The version and source are only included when necessary to disambiguate between packages:
/// - If multiple packages share the same name, the version is included.
/// - If multiple packages share the same name and version, the source is included.
#[derive(Debug, PartialOrd, Ord, PartialEq, Eq, Hash, Clone)]
#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
pub struct TomlLockfilePackageId {
    pub name: String,
    pub version: Option<String>,
    pub source: Option<TomlLockfileSourceId>,
}

impl fmt::Display for TomlLockfilePackageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)?;
        if let Some(s) = &self.version {
            write!(f, " {}", s)?;
        }
        if let Some(s) = &self.source {
            write!(f, " ({})", s.as_url())?;
        }
        Ok(())
    }
}

impl FromStr for TomlLockfilePackageId {
    type Err = TomlLockfilePackageIdError;

    fn from_str(s: &str) -> Result<TomlLockfilePackageId, Self::Err> {
        let mut s = s.splitn(3, ' ');
        let name = s.next().unwrap();
        let version = s.next();
        let source_id = match s.next() {
            Some(s) => {
                if let Some(s) = s.strip_prefix('(').and_then(|s| s.strip_suffix(')')) {
                    Some(TomlLockfileSourceId::new(s.to_string())?)
                } else {
                    return Err(TomlLockfilePackageIdErrorKind::InvalidSerializedPackageId.into());
                }
            }
            None => None,
        };

        Ok(TomlLockfilePackageId {
            name: name.to_string(),
            version: version.map(|v| v.to_string()),
            source: source_id,
        })
    }
}

impl ser::Serialize for TomlLockfilePackageId {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        s.collect_str(self)
    }
}

impl<'de> de::Deserialize<'de> for TomlLockfilePackageId {
    fn deserialize<D>(d: D) -> Result<TomlLockfilePackageId, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        String::deserialize(d).and_then(|string| {
            string
                .parse::<TomlLockfilePackageId>()
                .map_err(de::Error::custom)
        })
    }
}

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct TomlLockfileSourceIdError(#[from] TomlLockfileSourceIdErrorKind);

#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
enum TomlLockfileSourceIdErrorKind {
    #[error("invalid source `{0}`")]
    InvalidSource(String),

    #[error("invalid url `{url}`: {msg}")]
    InvalidUrl { url: String, msg: String },

    #[error("unsupported source protocol: {0}")]
    UnsupportedSource(String),
}

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct TomlLockfilePackageIdError(#[from] TomlLockfilePackageIdErrorKind);

impl From<TomlLockfileSourceIdError> for TomlLockfilePackageIdError {
    fn from(value: TomlLockfileSourceIdError) -> Self {
        TomlLockfilePackageIdErrorKind::Source(value).into()
    }
}

#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
enum TomlLockfilePackageIdErrorKind {
    #[error("invalid serialied PackageId")]
    InvalidSerializedPackageId,

    #[error(transparent)]
    Source(#[from] TomlLockfileSourceIdError),
}

#[cfg(feature = "unstable-schema")]
#[test]
fn dump_lockfile_schema() {
    let schema = schemars::schema_for!(crate::lockfile::TomlLockfile);
    let dump = serde_json::to_string_pretty(&schema).unwrap();
    snapbox::assert_data_eq!(dump, snapbox::file!("../lockfile.schema.json").raw());
}

#[cfg(test)]
mod tests {
    use crate::core::{GitReference, SourceKind};
    use crate::lockfile::{TomlLockfileSourceId, TomlLockfileSourceIdErrorKind};

    #[track_caller]
    fn ok(source_str: &str, source_kind: SourceKind, url: &str) {
        let source_str = source_str.to_owned();
        let source_id = TomlLockfileSourceId::new(source_str).unwrap();
        assert_eq!(source_id.kind, source_kind);
        assert_eq!(source_id.url().to_string(), url);
    }

    macro_rules! err {
        ($src:expr, $expected:pat) => {
            let kind = TomlLockfileSourceId::new($src.to_owned()).unwrap_err().0;
            assert!(
                matches!(kind, $expected),
                "`{}` parse error mismatch, got {kind:?}",
                $src,
            );
        };
    }

    #[test]
    fn good_sources() {
        ok(
            "sparse+https://my-crates.io",
            SourceKind::SparseRegistry,
            "sparse+https://my-crates.io",
        );
        ok(
            "registry+https://github.com/rust-lang/crates.io-index",
            SourceKind::Registry,
            "https://github.com/rust-lang/crates.io-index",
        );
        ok(
            "git+https://github.com/rust-lang/cargo",
            SourceKind::Git(GitReference::DefaultBranch),
            "https://github.com/rust-lang/cargo",
        );
        ok(
            "git+https://github.com/rust-lang/cargo?branch=dev",
            SourceKind::Git(GitReference::Branch("dev".to_owned())),
            "https://github.com/rust-lang/cargo?branch=dev",
        );
        ok(
            "git+https://github.com/rust-lang/cargo?tag=v1.0",
            SourceKind::Git(GitReference::Tag("v1.0".to_owned())),
            "https://github.com/rust-lang/cargo?tag=v1.0",
        );
        ok(
            "git+https://github.com/rust-lang/cargo?rev=refs/pull/493/head",
            SourceKind::Git(GitReference::Rev("refs/pull/493/head".to_owned())),
            "https://github.com/rust-lang/cargo?rev=refs/pull/493/head",
        );
        ok(
            "path+file:///path/to/root",
            SourceKind::Path,
            "file:///path/to/root",
        );
    }

    #[test]
    fn bad_sources() {
        err!(
            "unknown+https://my-crates.io",
            TomlLockfileSourceIdErrorKind::UnsupportedSource(..)
        );
        err!(
            "registry+https//github.com/rust-lang/crates.io-index",
            TomlLockfileSourceIdErrorKind::InvalidUrl { .. }
        );
        err!(
            "https//github.com/rust-lang/crates.io-index",
            TomlLockfileSourceIdErrorKind::InvalidSource(..)
        );
    }
}
