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

trait ToUrl {
    fn to_url(self) -> Option<Url>;
}

impl<'a> ToUrl for &'a str {
    fn to_url(self) -> Option<Url> {
        url::from_str(self).ok()
    }
}

impl ToUrl for Url {
    fn to_url(self) -> Option<Url> {
        Some(self)
    }
}

impl<'a> ToUrl for &'a Url {
    fn to_url(self) -> Option<Url> {
        Some(self.clone())
    }
}

#[deriving(Clone,PartialEq)]
pub struct PackageId {
    name: String,
    version: semver::Version,
    namespace: Url
}

impl PackageId {
    pub fn new<T: ToVersion, U: ToUrl>(name: &str, version: T,
                                       namespace: U) -> PackageId {
        PackageId {
            name: name.to_str(),
            version: version.to_version().unwrap(),
            namespace: namespace.to_url().unwrap()
        }
    }

    pub fn get_name<'a>(&'a self) -> &'a str {
        self.name.as_slice()
    }

    pub fn get_version<'a>(&'a self) -> &'a semver::Version {
        &self.version
    }

    pub fn get_namespace<'a>(&'a self) -> &'a Url {
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

impl<E, D: Decoder<E>> Decodable<D,E> for PackageId {
    fn decode(d: &mut D) -> Result<PackageId, E> {
        let vector: Vec<String> = try!(Decodable::decode(d));

        Ok(PackageId::new(
            vector.get(0).as_slice(),
            vector.get(1).as_slice(),
            vector.get(2).as_slice()))
    }
}

impl<E, S: Encoder<E>> Encodable<S,E> for PackageId {
    fn encode(&self, e: &mut S) -> Result<(), E> {
        (vec!(self.name.clone(), self.version.to_str()),
              self.namespace.to_str()).encode(e)
    }
}
