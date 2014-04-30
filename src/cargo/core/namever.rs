use semver;
use serialize::{
    Encodable,
    Encoder,
    Decodable,
    Decoder
};

#[deriving(Clone,Eq,Show,Ord)]
pub struct NameVer {
    name: ~str,
    version: semver::Version
}

impl NameVer {
    pub fn new(name: &str, version: &str) -> NameVer {
        NameVer { name: name.to_owned(), version: semver::parse(version.to_owned()).unwrap() }
    }

    pub fn get_name<'a>(&'a self) -> &'a str {
        self.name.as_slice()
    }

    pub fn get_version<'a>(&'a self) -> &'a semver::Version {
        &self.version
    }
}

impl<E, D: Decoder<E>> Decodable<D,E> for NameVer {
    fn decode(d: &mut D) -> Result<NameVer, E> {
        let vector: Vec<~str> = try!(Decodable::decode(d));
        Ok(NameVer { name: vector.get(0).clone(), version: semver::parse(vector.get(1).clone()).unwrap() })
    }
}

impl<E, S: Encoder<E>> Encodable<S,E> for NameVer {
    fn encode(&self, e: &mut S) -> Result<(), E> {
        (vec!(self.name.clone(), self.version.to_str())).encode(e)
    }
}
