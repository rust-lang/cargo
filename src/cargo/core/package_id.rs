use std::collections::HashSet;
use std::fmt::{self, Formatter};
use std::hash;
use std::hash::Hash;
use std::path::Path;
use std::ptr;
use std::sync::Mutex;

use semver;
use serde::de;
use serde::ser;

use crate::core::interning::InternedString;
use crate::core::source::SourceId;
use crate::util::{CargoResult, ToSemver};

lazy_static::lazy_static! {
    static ref PACKAGE_ID_CACHE: Mutex<HashSet<&'static PackageIdInner>> =
        Mutex::new(HashSet::new());
}

/// Identifier for a specific version of a package in a specific source.
#[derive(Clone, Copy, Eq, PartialOrd, Ord)]
pub struct PackageId {
    inner: &'static PackageIdInner,
}

#[derive(PartialOrd, Eq, Ord)]
struct PackageIdInner {
    name: InternedString,
    version: semver::Version,
    source_id: SourceId,
}

// Custom equality that uses full equality of SourceId, rather than its custom equality.
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
            self.inner.source_id.to_url()
        ))
    }
}

impl<'de> de::Deserialize<'de> for PackageId {
    fn deserialize<D>(d: D) -> Result<PackageId, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let string = String::deserialize(d)?;
        let mut s = string.splitn(3, ' ');
        let name = s.next().unwrap();
        let name = InternedString::new(name);
        let version = match s.next() {
            Some(s) => s,
            None => return Err(de::Error::custom("invalid serialized PackageId")),
        };
        let version = version.to_semver().map_err(de::Error::custom)?;
        let url = match s.next() {
            Some(s) => s,
            None => return Err(de::Error::custom("invalid serialized PackageId")),
        };
        let url = if url.starts_with('(') && url.ends_with(')') {
            &url[1..url.len() - 1]
        } else {
            return Err(de::Error::custom("invalid serialized PackageId"));
        };
        let source_id = SourceId::from_url(url).map_err(de::Error::custom)?;

        Ok(PackageId::pure(name, version, source_id))
    }
}

impl PartialEq for PackageId {
    fn eq(&self, other: &PackageId) -> bool {
        if ptr::eq(self.inner, other.inner) {
            return true;
        }
        self.inner.name == other.inner.name
            && self.inner.version == other.inner.version
            && self.inner.source_id == other.inner.source_id
    }
}

impl<'a> Hash for PackageId {
    fn hash<S: hash::Hasher>(&self, state: &mut S) {
        self.inner.name.hash(state);
        self.inner.version.hash(state);
        self.inner.source_id.hash(state);
    }
}

impl PackageId {
    pub fn new<T: ToSemver>(name: &str, version: T, sid: SourceId) -> CargoResult<PackageId> {
        let v = version.to_semver()?;
        Ok(PackageId::pure(InternedString::new(name), v, sid))
    }

    pub fn pure(name: InternedString, version: semver::Version, source_id: SourceId) -> PackageId {
        let inner = PackageIdInner {
            name,
            version,
            source_id,
        };
        let mut cache = PACKAGE_ID_CACHE.lock().unwrap();
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

    pub fn with_precise(self, precise: Option<String>) -> PackageId {
        PackageId::pure(
            self.inner.name,
            self.inner.version.clone(),
            self.inner.source_id.with_precise(precise),
        )
    }

    pub fn with_source_id(self, source: SourceId) -> PackageId {
        PackageId::pure(self.inner.name, self.inner.version.clone(), source)
    }

    pub fn map_source(self, to_replace: SourceId, replace_with: SourceId) -> Self {
        if self.source_id() == to_replace {
            self.with_source_id(replace_with)
        } else {
            self
        }
    }

    pub fn stable_hash(self, workspace: &Path) -> PackageIdStableHash<'_> {
        PackageIdStableHash(self, workspace)
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

        if !self.inner.source_id.is_default_registry() {
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

#[cfg(test)]
mod tests {
    use super::PackageId;
    use crate::core::source::SourceId;
    use crate::sources::CRATES_IO_INDEX;
    use crate::util::ToUrl;

    #[test]
    fn invalid_version_handled_nicely() {
        let loc = CRATES_IO_INDEX.to_url().unwrap();
        let repo = SourceId::for_registry(&loc).unwrap();

        assert!(PackageId::new("foo", "1.0", repo).is_err());
        assert!(PackageId::new("foo", "1", repo).is_err());
        assert!(PackageId::new("foo", "bar", repo).is_err());
        assert!(PackageId::new("foo", "", repo).is_err());
    }

    #[test]
    fn debug() {
        let loc = CRATES_IO_INDEX.to_url().unwrap();
        let pkg_id = PackageId::new("foo", "1.0.0", SourceId::for_registry(&loc).unwrap()).unwrap();
        assert_eq!(r#"PackageId { name: "foo", version: "1.0.0", source: "registry `https://github.com/rust-lang/crates.io-index`" }"#, format!("{:?}", pkg_id));

        let pretty = r#"
PackageId {
    name: "foo",
    version: "1.0.0",
    source: "registry `https://github.com/rust-lang/crates.io-index`"
}
"#
        .trim();
        assert_eq!(pretty, format!("{:#?}", pkg_id));
    }

    #[test]
    fn display() {
        let loc = CRATES_IO_INDEX.to_url().unwrap();
        let pkg_id = PackageId::new("foo", "1.0.0", SourceId::for_registry(&loc).unwrap()).unwrap();
        assert_eq!("foo v1.0.0", pkg_id.to_string());
    }
}
