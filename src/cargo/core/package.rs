use std::vec::Vec;
use semver;
use semver::{Version,parse};
use core;
use serialize::{Encodable,Encoder,Decodable,Decoder};

#[deriving(Clone,Eq,Show,Ord)]
pub struct NameVer {
    name: ~str,
    version: Version
}

impl NameVer {
    pub fn new(name: &str, version: &str) -> NameVer {
        println!("version: {}", version);
        NameVer { name: name.to_owned(), version: semver::parse(version.to_owned()).unwrap() }
    }

    pub fn get_name<'a>(&'a self) -> &'a str {
        self.name.as_slice()
    }
}

impl<E, D: Decoder<E>> Decodable<D,E> for NameVer {
    fn decode(d: &mut D) -> Result<NameVer, E> {
        let vector: Vec<~str> = try!(Decodable::decode(d));
        Ok(NameVer { name: vector.get(0).clone(), version: parse(vector.get(1).clone()).unwrap() })
    }
}

impl<E, S: Encoder<E>> Encodable<S,E> for NameVer {
    fn encode(&self, e: &mut S) -> Result<(), E> {
        (vec!(self.name.clone(), self.version.to_str())).encode(e)
    }
}

/**
 * Represents a rust library internally to cargo. This will things like where
 * on the local system the code is located, it's remote location, dependencies,
 * etc..
 *
 * This differs from core::Project
 */
#[deriving(Clone,Eq,Show)]
pub struct Package {
    name: ~str,
    deps: Vec<core::Dependency>
}

impl Package {
    pub fn new(name: &str, deps: &Vec<core::Dependency>) -> Package {
        Package { name: name.to_owned(), deps: deps.clone() }
    }

    pub fn get_name<'a>(&'a self) -> &'a str {
        self.name.as_slice()
    }

    pub fn get_dependencies<'a>(&'a self) -> &'a Vec<core::Dependency> {
            &self.deps
    }
}
