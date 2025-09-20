use std::collections::BTreeMap;
use std::fmt;
use std::{cmp::Ordering, str::FromStr};

use serde::{Deserialize, Serialize, de, ser};
use url::Url;

use crate::core::{GitReference, SourceKind};

/// The `Cargo.lock` structure.
#[derive(Serialize, Deserialize, Debug)]
#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
pub struct TomlLockfile {
    pub version: Option<u32>,
    pub package: Option<Vec<TomlLockfileDependency>>,
    /// `root` is optional to allow backward compatibility.
    pub root: Option<TomlLockfileDependency>,
    pub metadata: Option<TomlLockfileMetadata>,
    #[serde(default, skip_serializing_if = "TomlLockfilePatch::is_empty")]
    pub patch: TomlLockfilePatch,
}

pub type TomlLockfileMetadata = BTreeMap<String, String>;

#[derive(Serialize, Deserialize, Debug, Default)]
#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
pub struct TomlLockfilePatch {
    pub unused: Vec<TomlLockfileDependency>,
}

impl TomlLockfilePatch {
    fn is_empty(&self) -> bool {
        self.unused.is_empty()
    }
}

#[derive(Serialize, Deserialize, Debug, PartialOrd, Ord, PartialEq, Eq)]
#[cfg_attr(feature = "unstable-schema", derive(schemars::JsonSchema))]
pub struct TomlLockfileDependency {
    pub name: String,
    pub version: String,
    pub source: Option<TomlLockfileSourceId>,
    pub checksum: Option<String>,
    pub dependencies: Option<Vec<TomlLockfilePackageId>>,
    pub replace: Option<TomlLockfilePackageId>,
}

#[derive(Debug, Clone)]
#[cfg_attr(
    feature = "unstable-schema",
    derive(schemars::JsonSchema),
    schemars(with = "String")
)]
pub struct TomlLockfileSourceId {
    /// Full string of the source
    source_str: String,
    /// Used for sources ordering
    kind: SourceKind,
    /// Used for sources ordering
    url: Url,
}

impl TomlLockfileSourceId {
    pub fn new(source: String) -> Result<Self, EncodableSourceIdError> {
        let source_str = source.clone();
        let (kind, url) = source.split_once('+').ok_or_else(|| {
            EncodableSourceIdError(EncodableSourceIdErrorKind::InvalidSource(source.clone()).into())
        })?;

        let url = Url::parse(url).map_err(|msg| EncodableSourceIdErrorKind::InvalidUrl {
            url: url.to_string(),
            msg: msg.to_string(),
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
                return Err(EncodableSourceIdErrorKind::UnsupportedSource(kind.to_string()).into());
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
    type Err = EncodablePackageIdError;

    fn from_str(s: &str) -> Result<TomlLockfilePackageId, Self::Err> {
        let mut s = s.splitn(3, ' ');
        let name = s.next().unwrap();
        let version = s.next();
        let source_id = match s.next() {
            Some(s) => {
                if let Some(s) = s.strip_prefix('(').and_then(|s| s.strip_suffix(')')) {
                    Some(TomlLockfileSourceId::new(s.to_string())?)
                } else {
                    return Err(EncodablePackageIdErrorKind::InvalidSerializedPackageId.into());
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
pub struct EncodableSourceIdError(#[from] EncodableSourceIdErrorKind);

#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
enum EncodableSourceIdErrorKind {
    #[error("invalid source `{0}`")]
    InvalidSource(String),

    #[error("invalid url `{url}`: {msg}")]
    InvalidUrl { url: String, msg: String },

    #[error("unsupported source protocol: {0}")]
    UnsupportedSource(String),
}

#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct EncodablePackageIdError(#[from] EncodablePackageIdErrorKind);

impl From<EncodableSourceIdError> for EncodablePackageIdError {
    fn from(value: EncodableSourceIdError) -> Self {
        EncodablePackageIdErrorKind::Source(value).into()
    }
}

#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
enum EncodablePackageIdErrorKind {
    #[error("invalid serialied PackageId")]
    InvalidSerializedPackageId,

    #[error(transparent)]
    Source(#[from] EncodableSourceIdError),
}

#[cfg(feature = "unstable-schema")]
#[test]
fn dump_lockfile_schema() {
    let schema = schemars::schema_for!(crate::lockfile::TomlLockfile);
    let dump = serde_json::to_string_pretty(&schema).unwrap();
    snapbox::assert_data_eq!(dump, snapbox::file!("../lockfile.schema.json").raw());
}
