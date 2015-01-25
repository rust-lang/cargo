use std::cmp::Ordering;
use std::error::{Error, FromError};
use std::fmt::{self, Formatter};
use std::hash::{Hash, SipHasher};
use std::hash;
use std::sync::Arc;

use regex::Regex;
use rustc_serialize::{Encodable, Encoder, Decodable, Decoder};
use semver;

use util::{CargoResult, CargoError, short_hash, ToSemver};
use core::source::SourceId;

/// Identifier for a specific version of a package in a specific source.
#[derive(Clone, Debug)]
pub struct PackageId {
    inner: Arc<PackageIdInner>,
}

#[derive(PartialEq, PartialOrd, Eq, Ord, Debug)]
struct PackageIdInner {
    name: String,
    version: semver::Version,
    source_id: SourceId,
}

impl Encodable for PackageId {
    fn encode<S: Encoder>(&self, s: &mut S) -> Result<(), S::Error> {
        let source = self.inner.source_id.to_url();
        let encoded = format!("{} {} ({})", self.inner.name, self.inner.version,
                              source);
        encoded.encode(s)
    }
}

impl Decodable for PackageId {
    fn decode<D: Decoder>(d: &mut D) -> Result<PackageId, D::Error> {
        let string: String = try!(Decodable::decode(d));
        let regex = Regex::new(r"^([^ ]+) ([^ ]+) \(([^\)]+)\)$").unwrap();
        let captures = regex.captures(string.as_slice()).expect("invalid serialized PackageId");

        let name = captures.at(1).unwrap();
        let version = captures.at(2).unwrap();
        let url = captures.at(3).unwrap();
        let version = semver::Version::parse(version).ok().expect("invalid version");
        let source_id = SourceId::from_url(url.to_string());

        Ok(PackageId {
            inner: Arc::new(PackageIdInner {
                name: name.to_string(),
                version: version,
                source_id: source_id,
            }),
        })
    }
}

impl<S: hash::Writer + hash::Hasher> Hash<S> for PackageId {
    fn hash(&self, state: &mut S) {
        self.inner.name.hash(state);
        self.inner.version.to_string().hash(state);
        self.inner.source_id.hash(state);
    }
}

impl PartialEq for PackageId {
    fn eq(&self, other: &PackageId) -> bool {
        (*self.inner).eq(&*other.inner)
    }
}
impl PartialOrd for PackageId {
    fn partial_cmp(&self, other: &PackageId) -> Option<Ordering> {
        (*self.inner).partial_cmp(&*other.inner)
    }
}
impl Eq for PackageId {}
impl Ord for PackageId {
    fn cmp(&self, other: &PackageId) -> Ordering {
        (*self.inner).cmp(&*other.inner)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum PackageIdError {
    InvalidVersion(String),
    InvalidNamespace(String)
}

impl Error for PackageIdError {
    fn description(&self) -> &str { "failed to parse package id" }
}

impl fmt::Display for PackageIdError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            PackageIdError::InvalidVersion(ref v) => {
                write!(f, "invalid version: {}", *v)
            }
            PackageIdError::InvalidNamespace(ref ns) => {
                write!(f, "invalid namespace: {}", *ns)
            }
        }
    }
}

impl CargoError for PackageIdError {
    fn is_human(&self) -> bool { true }
}

impl FromError<PackageIdError> for Box<CargoError> {
    fn from_error(t: PackageIdError) -> Box<CargoError> { Box::new(t) }
}

#[derive(PartialEq, Hash, Clone, RustcEncodable, Debug)]
pub struct Metadata {
    pub metadata: String,
    pub extra_filename: String
}

impl PackageId {
    pub fn new<T: ToSemver>(name: &str, version: T,
                             sid: &SourceId) -> CargoResult<PackageId> {
        let v = try!(version.to_semver().map_err(PackageIdError::InvalidVersion));
        Ok(PackageId {
            inner: Arc::new(PackageIdInner {
                name: name.to_string(),
                version: v,
                source_id: sid.clone(),
            }),
        })
    }

    pub fn get_name(&self) -> &str {
        self.inner.name.as_slice()
    }

    pub fn get_version(&self) -> &semver::Version {
        &self.inner.version
    }

    pub fn get_source_id(&self) -> &SourceId {
        &self.inner.source_id
    }

    pub fn generate_metadata(&self) -> Metadata {
        let metadata = short_hash(
            &(self.inner.name.as_slice(), self.inner.version.to_string(),
              &self.inner.source_id));
        let extra_filename = format!("-{}", metadata);

        Metadata { metadata: metadata, extra_filename: extra_filename }
    }

    pub fn with_precise(&self, precise: Option<String>) -> PackageId {
        PackageId {
            inner: Arc::new(PackageIdInner {
                name: self.inner.name.to_string(),
                version: self.inner.version.clone(),
                source_id: self.inner.source_id.with_precise(precise),
            }),
        }
    }
}

impl Metadata {
    pub fn mix<T: Hash<SipHasher>>(&mut self, t: &T) {
        let new_metadata = short_hash(&(self.metadata.as_slice(), t));
        self.extra_filename = format!("-{}", new_metadata);
        self.metadata = new_metadata;
    }
}

impl fmt::Display for PackageId {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        try!(write!(f, "{} v{}", self.inner.name, self.inner.version));

        if !self.inner.source_id.is_default_registry() {
            try!(write!(f, " ({})", self.inner.source_id));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::PackageId;
    use core::source::SourceId;
    use sources::RegistrySource;
    use util::ToUrl;

    #[test]
    fn invalid_version_handled_nicely() {
        let loc = RegistrySource::default_url().to_url().unwrap();
        let repo = SourceId::for_registry(&loc);

        assert!(PackageId::new("foo", "1.0", &repo).is_err());
        assert!(PackageId::new("foo", "1", &repo).is_err());
        assert!(PackageId::new("foo", "bar", &repo).is_err());
        assert!(PackageId::new("foo", "", &repo).is_err());
    }
}
