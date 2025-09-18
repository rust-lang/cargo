use std::collections::BTreeMap;
use std::fmt;
use std::{cmp::Ordering, str::FromStr};

use serde::{Deserialize, Serialize, de, ser};
use url::Url;

use crate::core::{GitReference, SourceKind};

/// The `Cargo.lock` structure.
#[derive(Serialize, Deserialize, Debug)]
pub struct EncodableResolve {
    pub version: Option<u32>,
    pub package: Option<Vec<EncodableDependency>>,
    /// `root` is optional to allow backward compatibility.
    pub root: Option<EncodableDependency>,
    pub metadata: Option<Metadata>,
    #[serde(default, skip_serializing_if = "Patch::is_empty")]
    pub patch: Patch,
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Patch {
    pub unused: Vec<EncodableDependency>,
}

pub type Metadata = BTreeMap<String, String>;

impl Patch {
    fn is_empty(&self) -> bool {
        self.unused.is_empty()
    }
}

#[derive(Serialize, Deserialize, Debug, PartialOrd, Ord, PartialEq, Eq)]
pub struct EncodableDependency {
    pub name: String,
    pub version: String,
    pub source: Option<EncodableSourceId>,
    pub checksum: Option<String>,
    pub dependencies: Option<Vec<EncodablePackageId>>,
    pub replace: Option<EncodablePackageId>,
}

#[derive(Debug, Clone)]
pub struct EncodableSourceId {
    /// Full string of the source
    source_str: String,
    /// Used for sources ordering
    kind: SourceKind,
    /// Used for sources ordering
    url: Url,
}

impl EncodableSourceId {
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

impl ser::Serialize for EncodableSourceId {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        s.collect_str(&self.as_url())
    }
}

impl<'de> de::Deserialize<'de> for EncodableSourceId {
    fn deserialize<D>(d: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let s = String::deserialize(d)?;
        Ok(EncodableSourceId::new(s).map_err(de::Error::custom)?)
    }
}

impl std::hash::Hash for EncodableSourceId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.kind.hash(state);
        self.url.hash(state);
    }
}

impl std::cmp::PartialEq for EncodableSourceId {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind && self.url == other.url
    }
}

impl std::cmp::Eq for EncodableSourceId {}

impl PartialOrd for EncodableSourceId {
    fn partial_cmp(&self, other: &EncodableSourceId) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for EncodableSourceId {
    fn cmp(&self, other: &EncodableSourceId) -> Ordering {
        self.kind
            .cmp(&other.kind)
            .then_with(|| self.url.cmp(&other.url))
    }
}

#[derive(Debug, PartialOrd, Ord, PartialEq, Eq, Hash, Clone)]
pub struct EncodablePackageId {
    pub name: String,
    pub version: Option<String>,
    pub source: Option<EncodableSourceId>,
}

impl fmt::Display for EncodablePackageId {
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

impl FromStr for EncodablePackageId {
    type Err = EncodablePackageIdError;

    fn from_str(s: &str) -> Result<EncodablePackageId, Self::Err> {
        let mut s = s.splitn(3, ' ');
        let name = s.next().unwrap();
        let version = s.next();
        let source_id = match s.next() {
            Some(s) => {
                if let Some(s) = s.strip_prefix('(').and_then(|s| s.strip_suffix(')')) {
                    Some(EncodableSourceId::new(s.to_string())?)
                } else {
                    return Err(EncodablePackageIdErrorKind::InvalidSerializedPackageId.into());
                }
            }
            None => None,
        };

        Ok(EncodablePackageId {
            name: name.to_string(),
            version: version.map(|v| v.to_string()),
            source: source_id,
        })
    }
}

impl ser::Serialize for EncodablePackageId {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        s.collect_str(self)
    }
}

impl<'de> de::Deserialize<'de> for EncodablePackageId {
    fn deserialize<D>(d: D) -> Result<EncodablePackageId, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        String::deserialize(d).and_then(|string| {
            string
                .parse::<EncodablePackageId>()
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
