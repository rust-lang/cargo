use semver;
use std::hash::Hash;
use std::fmt::{mod, Show, Formatter};
use collections::hash;
use serialize::{
    Encodable,
    Encoder,
    Decodable,
    Decoder
};

use util::{CargoResult, CargoError, short_hash};
use core::source::SourceId;

trait ToVersion {
    fn to_version(self) -> Result<semver::Version, String>;
}

impl ToVersion for semver::Version {
    fn to_version(self) -> Result<semver::Version, String> {
        Ok(self)
    }
}

impl<'a> ToVersion for &'a str {
    fn to_version(self) -> Result<semver::Version, String> {
        match semver::parse(self) {
            Some(v) => Ok(v),
            None => Err(format!("cannot parse '{}' as a semver", self)),
        }
    }
}

#[deriving(Clone, PartialEq, PartialOrd, Ord)]
pub struct PackageId {
    name: String,
    version: semver::Version,
    source_id: SourceId,
}

impl<E, S: Encoder<E>> Encodable<S, E> for PackageId {
    fn encode(&self, s: &mut S) -> Result<(), E> {
        let source = self.source_id.to_url();
        let encoded = format!("{} {} ({})", self.name, self.version, source);
        encoded.encode(s)
    }
}

impl<E, D: Decoder<E>> Decodable<D, E> for PackageId {
    fn decode(d: &mut D) -> Result<PackageId, E> {
        let string: String = raw_try!(Decodable::decode(d));
        let regex = regex!(r"^([^ ]+) ([^ ]+) \(([^\)]+)\)$");
        let captures = regex.captures(string.as_slice()).expect("invalid serialized PackageId");

        let name = captures.at(1);
        let version = semver::parse(captures.at(2)).expect("invalid version");
        let source_id = SourceId::from_url(captures.at(3).to_string());

        Ok(PackageId {
            name: name.to_string(),
            version: version,
            source_id: source_id
        })
    }
}

impl<S: hash::Writer> Hash<S> for PackageId {
    fn hash(&self, state: &mut S) {
        self.name.hash(state);
        self.version.to_string().hash(state);
        self.source_id.hash(state);
    }
}

impl Eq for PackageId {}

#[deriving(Clone, Show, PartialEq)]
pub enum PackageIdError {
    InvalidVersion(String),
    InvalidNamespace(String)
}

impl CargoError for PackageIdError {
    fn description(&self) -> String {
        match *self {
            InvalidVersion(ref v) => format!("invalid version: {}", *v),
            InvalidNamespace(ref ns) => format!("invalid namespace: {}", *ns),
        }
    }
    fn is_human(&self) -> bool { true }
}

#[deriving(PartialEq, Hash, Clone, Encodable)]
pub struct Metadata {
    pub metadata: String,
    pub extra_filename: String
}

impl PackageId {
    pub fn new<T: ToVersion>(name: &str, version: T,
                             sid: &SourceId) -> CargoResult<PackageId> {
        let v = try!(version.to_version().map_err(InvalidVersion));
        Ok(PackageId {
            name: name.to_string(),
            version: v,
            source_id: sid.clone()
        })
    }

    pub fn get_name(&self) -> &str {
        self.name.as_slice()
    }

    pub fn get_version(&self) -> &semver::Version {
        &self.version
    }

    pub fn get_source_id(&self) -> &SourceId {
        &self.source_id
    }

    pub fn generate_metadata(&self) -> Metadata {
        let metadata = short_hash(
            &(self.name.as_slice(), self.version.to_string(), &self.source_id));
        let extra_filename = format!("-{}", metadata);

        Metadata { metadata: metadata, extra_filename: extra_filename }
    }
}

impl Metadata {
    pub fn mix<T: Hash>(&mut self, t: &T) {
        let new_metadata = short_hash(&(self.metadata.as_slice(), t));
        self.extra_filename = format!("-{}", new_metadata);
        self.metadata = new_metadata;
    }
}

static central_repo: &'static str = "http://rust-lang.org/central-repo";

impl Show for PackageId {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        try!(write!(f, "{} v{}", self.name, self.version));

        if self.source_id.to_string().as_slice() != central_repo {
            try!(write!(f, " ({})", self.source_id));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{PackageId, central_repo};
    use core::source::{RegistryKind, SourceId};
    use util::ToUrl;

    #[test]
    fn invalid_version_handled_nicely() {
        let loc = central_repo.to_url().unwrap();
        let repo = SourceId::new(RegistryKind, loc);

        assert!(PackageId::new("foo", "1.0", &repo).is_err());
        assert!(PackageId::new("foo", "1", &repo).is_err());
        assert!(PackageId::new("foo", "bar", &repo).is_err());
        assert!(PackageId::new("foo", "", &repo).is_err());
    }
}
