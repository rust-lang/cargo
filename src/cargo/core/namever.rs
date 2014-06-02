use semver;
use std::fmt;
use std::fmt::{Show,Formatter};
use serialize::{
    Encodable,
    Encoder,
    Decodable,
    Decoder
};

#[deriving(Clone,PartialEq,PartialOrd)]
pub struct NameVer {
    name: String,
    version: semver::Version
}

impl NameVer {
    pub fn new(name: &str, version: &str) -> NameVer {
        NameVer { name: name.to_str(), version: semver::parse(version.as_slice()).unwrap() }
    }

    pub fn get_name<'a>(&'a self) -> &'a str {
        self.name.as_slice()
    }

    pub fn get_version<'a>(&'a self) -> &'a semver::Version {
        &self.version
    }
}

impl Show for NameVer {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{} v{}", self.name, self.version)
    }
}

impl<E, D: Decoder<E>> Decodable<D,E> for NameVer {
    fn decode(d: &mut D) -> Result<NameVer, E> {
        let vector: Vec<String> = try!(Decodable::decode(d));
        Ok(NameVer { name: vector.get(0).clone(), version: semver::parse(vector.get(1).as_slice()).unwrap() })
    }
}

impl<E, S: Encoder<E>> Encodable<S,E> for NameVer {
    fn encode(&self, e: &mut S) -> Result<(), E> {
        (vec!(self.name.clone(), self.version.to_str())).encode(e)
    }
}
