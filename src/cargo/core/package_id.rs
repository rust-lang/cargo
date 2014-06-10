use semver;
use std::fmt;
use std::fmt::{Show,Formatter};
use serialize::{
    Encodable,
    Encoder,
    Decodable,
    Decoder
};

trait ToVersion {
    fn to_version(self) -> Option<semver::Version>;
}

impl ToVersion for semver::Version {
    fn to_version(self) -> Option<semver::Version> {
        Some(self)
    }
}

impl<'a> ToVersion for &'a str {
    fn to_version(self) -> Option<semver::Version> {
        semver::parse(self)
    }
}

#[deriving(Clone,PartialEq,PartialOrd)]
pub struct PackageId {
    name: String,
    version: semver::Version
}

impl PackageId {
    pub fn new<T: ToVersion>(name: &str, version: T) -> PackageId {
        PackageId {
            name: name.to_str(),
            version: version.to_version().unwrap()
        }
    }

    pub fn get_name<'a>(&'a self) -> &'a str {
        self.name.as_slice()
    }

    pub fn get_version<'a>(&'a self) -> &'a semver::Version {
        &self.version
    }
}

impl Show for PackageId {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{} v{}", self.name, self.version)
    }
}

impl<E, D: Decoder<E>> Decodable<D,E> for PackageId {
    fn decode(d: &mut D) -> Result<PackageId, E> {
        let vector: Vec<String> = try!(Decodable::decode(d));

        Ok(PackageId::new(
            vector.get(0).as_slice(),
            semver::parse(vector.get(1).as_slice()).unwrap()))
    }
}

impl<E, S: Encoder<E>> Encodable<S,E> for PackageId {
    fn encode(&self, e: &mut S) -> Result<(), E> {
        (vec!(self.name.clone(), self.version.to_str())).encode(e)
    }
}
