use semver;
use url::Url;
use std::fmt;
use std::fmt::{Show,Formatter};
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

trait ToUrl {
    fn to_url(self) -> Result<Url, String>;
}

impl<'a> ToUrl for &'a str {
    fn to_url(self) -> Result<Url, String> {
        Url::parse(self)
    }
}

impl ToUrl for Url {
    fn to_url(self) -> Result<Url, String> {
        Ok(self)
    }
}

impl<'a> ToUrl for &'a Url {
    fn to_url(self) -> Result<Url, String> {
        Ok(self.clone())
    }
}

#[deriving(Clone, PartialEq)]
pub struct PackageId {
    name: String,
    version: semver::Version,
    source_id: SourceId,
}

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

    pub fn get_name<'a>(&'a self) -> &'a str {
        self.name.as_slice()
    }

    pub fn get_version<'a>(&'a self) -> &'a semver::Version {
        &self.version
    }

    pub fn get_source_id<'a>(&'a self) -> &'a SourceId {
        &self.source_id
    }

    pub fn generate_metadata(&self) -> Metadata {
        let metadata = format!("{}:-:{}:-:{}", self.name, self.version, self.source_id);
        let extra_filename = short_hash(
            &(self.name.as_slice(), self.version.to_string(), &self.source_id));

        Metadata { metadata: metadata, extra_filename: extra_filename }
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

impl<D: Decoder<Box<CargoError + Send>>>
    Decodable<D,Box<CargoError + Send>>
    for PackageId
{
    fn decode(d: &mut D) -> CargoResult<PackageId> {
        let (name, version, source_id): (String, String, SourceId) = try!(Decodable::decode(d));

        PackageId::new(name.as_slice(), version.as_slice(), &source_id)
    }
}

impl<E, S: Encoder<E>> Encodable<S,E> for PackageId {
    fn encode(&self, e: &mut S) -> Result<(), E> {
        (self.name.clone(), self.version.to_string(), self.source_id.clone()).encode(e)
    }
}

#[cfg(test)]
mod tests {
    use super::{PackageId, central_repo};
    use core::source::{Location, RegistryKind, SourceId};

    #[test]
    fn invalid_version_handled_nicely() {
        let loc = Location::parse(central_repo).unwrap();
        let repo = SourceId::new(RegistryKind, loc);

        assert!(PackageId::new("foo", "1.0", &repo).is_err());
        assert!(PackageId::new("foo", "1", &repo).is_err());
        assert!(PackageId::new("foo", "bar", &repo).is_err());
        assert!(PackageId::new("foo", "", &repo).is_err());
    }
}
