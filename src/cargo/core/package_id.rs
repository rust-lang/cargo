use std::cmp::Ordering;
use std::collections::HashSet;
use std::fmt::{self, Formatter};
use std::hash;
use std::hash::Hash;
use std::path::Path;
use std::ptr;
use std::sync::Mutex;
use std::sync::OnceLock;

use serde::de;
use serde::ser;

use crate::core::ActivationKey;
use crate::core::PackageIdSpec;
use crate::core::SourceId;
use crate::util::interning::InternedString;
use crate::util::CargoResult;

static PACKAGE_ID_CACHE: OnceLock<Mutex<HashSet<&'static PackageIdInner>>> = OnceLock::new();

/// Identifier for a specific version of a package in a specific source.
#[derive(Clone, Copy, Eq, PartialOrd, Ord)]
pub struct PackageId {
    inner: &'static PackageIdInner,
}

struct PackageIdInner {
    name: InternedString,
    version: semver::Version,
    source_id: SourceId,
    // This field is used as a cache to improve the resolver speed,
    // and is not included in the `Eq`, `Hash` and `Ord` impls.
    activation_key: ActivationKey,
}

impl Ord for PackageIdInner {
    fn cmp(&self, other: &Self) -> Ordering {
        let self_key = (self.name, &self.version, self.source_id);
        let other_key = (other.name, &other.version, other.source_id);
        self_key.cmp(&other_key)
    }
}

impl PartialOrd for PackageIdInner {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(&other))
    }
}

impl Eq for PackageIdInner {}

// Custom equality that uses full equality of SourceId, rather than its custom equality.
//
// The `build` part of the version is usually ignored (like a "comment").
// However, there are some cases where it is important. The download path from
// a registry includes the build metadata, and Cargo uses PackageIds for
// creating download paths. Including it here prevents the PackageId interner
// from getting poisoned with PackageIds where that build metadata is missing.
impl PartialEq for PackageIdInner {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.version == other.version
            && self.source_id.full_eq(other.source_id)
    }
}

// Custom hash that is coherent with the custom equality above.
impl Hash for PackageIdInner {
    fn hash<S: hash::Hasher>(&self, into: &mut S) {
        self.name.hash(into);
        self.version.hash(into);
        self.source_id.full_hash(into);
    }
}

impl ser::Serialize for PackageId {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        s.collect_str(&format_args!(
            "{} {} ({})",
            self.inner.name,
            self.inner.version,
            self.inner.source_id.as_url()
        ))
    }
}

impl<'de> de::Deserialize<'de> for PackageId {
    fn deserialize<D>(d: D) -> Result<PackageId, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let string = String::deserialize(d)?;

        let (field, rest) = string
            .split_once(' ')
            .ok_or_else(|| de::Error::custom("invalid serialized PackageId"))?;
        let name = InternedString::new(field);

        let (field, rest) = rest
            .split_once(' ')
            .ok_or_else(|| de::Error::custom("invalid serialized PackageId"))?;
        let version = field.parse().map_err(de::Error::custom)?;

        let url =
            strip_parens(rest).ok_or_else(|| de::Error::custom("invalid serialized PackageId"))?;
        let source_id = SourceId::from_url(url).map_err(de::Error::custom)?;

        Ok(PackageId::new(name, version, source_id))
    }
}

fn strip_parens(value: &str) -> Option<&str> {
    let value = value.strip_prefix('(')?;
    let value = value.strip_suffix(')')?;
    Some(value)
}

impl PartialEq for PackageId {
    fn eq(&self, other: &PackageId) -> bool {
        if ptr::eq(self.inner, other.inner) {
            return true;
        }
        // This is here so that PackageId uses SourceId's and Version's idea
        // of equality. PackageIdInner uses a more exact notion of equality.
        self.inner.name == other.inner.name
            && self.inner.version == other.inner.version
            && self.inner.source_id == other.inner.source_id
    }
}

impl Hash for PackageId {
    fn hash<S: hash::Hasher>(&self, state: &mut S) {
        // This is here (instead of derived) so that PackageId uses SourceId's
        // and Version's idea of equality. PackageIdInner uses a more exact
        // notion of hashing.
        self.inner.name.hash(state);
        self.inner.version.hash(state);
        self.inner.source_id.hash(state);
    }
}

impl PackageId {
    pub fn try_new(
        name: impl Into<InternedString>,
        version: &str,
        sid: SourceId,
    ) -> CargoResult<PackageId> {
        let v = version.parse()?;
        Ok(PackageId::new(name.into(), v, sid))
    }

    pub fn new(name: InternedString, version: semver::Version, source_id: SourceId) -> PackageId {
        let inner = PackageIdInner {
            activation_key: (name, source_id, (&version).into()).into(),
            name,
            version,
            source_id,
        };
        let mut cache = PACKAGE_ID_CACHE
            .get_or_init(|| Default::default())
            .lock()
            .unwrap();
        let inner = cache.get(&inner).cloned().unwrap_or_else(|| {
            let inner = Box::leak(Box::new(inner));
            cache.insert(inner);
            inner
        });
        PackageId { inner }
    }

    pub fn name(self) -> InternedString {
        self.inner.name
    }
    pub fn version(self) -> &'static semver::Version {
        &self.inner.version
    }
    pub fn source_id(self) -> SourceId {
        self.inner.source_id
    }
    pub fn activation_key(self) -> ActivationKey {
        self.inner.activation_key
    }

    pub fn with_source_id(self, source: SourceId) -> PackageId {
        PackageId::new(self.inner.name, self.inner.version.clone(), source)
    }

    pub fn map_source(self, to_replace: SourceId, replace_with: SourceId) -> Self {
        if self.source_id() == to_replace {
            self.with_source_id(replace_with)
        } else {
            self
        }
    }

    /// Returns a value that implements a "stable" hashable value.
    ///
    /// Stable hashing removes the path prefix of the workspace from path
    /// packages. This helps with reproducible builds, since this hash is part
    /// of the symbol metadata, and we don't want the absolute path where the
    /// build is performed to affect the binary output.
    pub fn stable_hash(self, workspace: &Path) -> PackageIdStableHash<'_> {
        PackageIdStableHash(self, workspace)
    }

    /// Filename of the `.crate` tarball, e.g., `once_cell-1.18.0.crate`.
    pub fn tarball_name(&self) -> String {
        format!("{}-{}.crate", self.name(), self.version())
    }

    /// Convert a `PackageId` to a `PackageIdSpec`, which will have both the `PartialVersion` and `Url`
    /// fields filled in.
    pub fn to_spec(&self) -> PackageIdSpec {
        PackageIdSpec::new(String::from(self.name().as_str()))
            .with_version(self.version().clone().into())
            .with_url(self.source_id().url().clone())
            .with_kind(self.source_id().kind().clone())
    }
}

pub struct PackageIdStableHash<'a>(PackageId, &'a Path);

impl<'a> Hash for PackageIdStableHash<'a> {
    fn hash<S: hash::Hasher>(&self, state: &mut S) {
        self.0.inner.name.hash(state);
        self.0.inner.version.hash(state);
        self.0.inner.source_id.stable_hash(self.1, state);
    }
}

impl fmt::Display for PackageId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{} v{}", self.inner.name, self.inner.version)?;

        if !self.inner.source_id.is_crates_io() {
            write!(f, " ({})", self.inner.source_id)?;
        }

        Ok(())
    }
}

impl fmt::Debug for PackageId {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("PackageId")
            .field("name", &self.inner.name)
            .field("version", &self.inner.version.to_string())
            .field("source", &self.inner.source_id.to_string())
            .finish()
    }
}

impl fmt::Debug for PackageIdInner {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("PackageIdInner")
            .field("name", &self.name)
            .field("version", &self.version.to_string())
            .field("source", &self.source_id.to_string())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::PackageId;
    use crate::core::SourceId;
    use crate::sources::CRATES_IO_INDEX;
    use crate::util::IntoUrl;

    #[test]
    fn invalid_version_handled_nicely() {
        let loc = CRATES_IO_INDEX.into_url().unwrap();
        let repo = SourceId::for_registry(&loc).unwrap();

        assert!(PackageId::try_new("foo", "1.0", repo).is_err());
        assert!(PackageId::try_new("foo", "1", repo).is_err());
        assert!(PackageId::try_new("foo", "bar", repo).is_err());
        assert!(PackageId::try_new("foo", "", repo).is_err());
    }

    #[test]
    fn display() {
        let loc = CRATES_IO_INDEX.into_url().unwrap();
        let pkg_id =
            PackageId::try_new("foo", "1.0.0", SourceId::for_registry(&loc).unwrap()).unwrap();
        assert_eq!("foo v1.0.0", pkg_id.to_string());
    }

    #[test]
    fn unequal_build_metadata() {
        let loc = CRATES_IO_INDEX.into_url().unwrap();
        let repo = SourceId::for_registry(&loc).unwrap();
        let first = PackageId::try_new("foo", "0.0.1+first", repo).unwrap();
        let second = PackageId::try_new("foo", "0.0.1+second", repo).unwrap();
        assert_ne!(first, second);
        assert_ne!(first.inner, second.inner);
    }
}
