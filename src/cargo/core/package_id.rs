use semver;
use url;
use url::Url;
use std::fmt;
use std::fmt::{Show,Formatter};
use serialize::{
    Encodable,
    Encoder,
    Decodable,
    Decoder
};

use util::{CargoResult, CargoError};
use core::source::Location;

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
        url::from_str(self)
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

#[deriving(Clone,PartialEq)]
pub struct PackageId {
    name: String,
    version: semver::Version,
    namespace: Location,
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

impl PackageId {
    pub fn new<T: ToVersion>(name: &str, version: T,
                             ns: &Location) -> CargoResult<PackageId> {
        let v = try!(version.to_version().map_err(InvalidVersion));
        Ok(PackageId {
            name: name.to_str(),
            version: v,
            namespace: ns.clone()
        })
    }

    pub fn get_name<'a>(&'a self) -> &'a str {
        self.name.as_slice()
    }

    pub fn get_version<'a>(&'a self) -> &'a semver::Version {
        &self.version
    }

    pub fn get_namespace<'a>(&'a self) -> &'a Location {
        &self.namespace
    }
}

static central_repo: &'static str = "http://rust-lang.org/central-repo";

impl Show for PackageId {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        try!(write!(f, "{} v{}", self.name, self.version));

        if self.namespace.to_str().as_slice() != central_repo {
            try!(write!(f, " ({})", self.namespace));
        }

        Ok(())
    }
}

impl<D: Decoder<Box<CargoError + Send>>>
    Decodable<D,Box<CargoError + Send>>
    for PackageId
{
    fn decode(d: &mut D) -> Result<PackageId, Box<CargoError + Send>> {
        let vector: Vec<String> = try!(Decodable::decode(d));

        PackageId::new(
            vector.get(0).as_slice(),
            vector.get(1).as_slice(),
            &try!(Location::parse(vector.get(2).as_slice())))
    }
}

impl<E, S: Encoder<E>> Encodable<S,E> for PackageId {
    fn encode(&self, e: &mut S) -> Result<(), E> {
        (vec!(self.name.clone(), self.version.to_str()),
              self.namespace.to_str()).encode(e)
    }
}

#[cfg(test)]
mod tests {
    use super::{PackageId, central_repo};
    use core::source::Location;

    #[test]
    fn invalid_version_handled_nicely() {
        let repo = Location::parse(central_repo).unwrap();
        assert!(PackageId::new("foo", "1.0", &repo).is_err());
        assert!(PackageId::new("foo", "1", &repo).is_err());
        assert!(PackageId::new("foo", "bar", &repo).is_err());
        assert!(PackageId::new("foo", "", &repo).is_err());
    }
}
