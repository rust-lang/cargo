use std::collections::HashSet;
use std::fmt;
use std::ptr;
use std::sync::Mutex;

use semver::{Version, VersionReq};
use semver::ReqParseError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::util::errors::CargoResult;

lazy_static::lazy_static! {
    static ref SEM_VERSION_CACHE: Mutex<HashSet<&'static Version>> =
        Mutex::new(HashSet::new());
    static ref SEM_VERSION_REQ_CACHE: Mutex<HashSet<&'static VersionReq>> =
        Mutex::new(HashSet::new());
}

#[derive(Clone, Copy, Eq, Hash, PartialOrd, Ord)]
pub struct SemVersion {
    inner: &'static Version,
}

impl SemVersion {
    pub fn new(version: Version) -> SemVersion {
        let mut cache = SEM_VERSION_CACHE.lock().unwrap();
        let version = cache.get(&version).cloned().unwrap_or_else(|| {
            let version = Box::leak(Box::new(version));
            cache.insert(version);
            version
        });
        SemVersion { inner: version }
    }

    pub fn value(&self) -> &'static Version {
        self.inner
    }
}

impl PartialEq for SemVersion {
    fn eq(&self, other: &SemVersion) -> bool {
        ptr::eq(self.inner, other.inner)
    }
}

impl Serialize for SemVersion {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error> where S: Serializer {
        self.inner.serialize(ser)
    }
}

impl<'de> Deserialize<'de> for SemVersion {
    fn deserialize<D>(de: D) -> Result<SemVersion, D::Error> where D: Deserializer<'de> {
        Ok(SemVersion::new(<Version>::deserialize(de)?))
    }
}

impl fmt::Debug for SemVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.inner, f)
    }
}

impl fmt::Display for SemVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.inner, f)
    }
}


pub trait ToSemver {
    fn to_semver(self) -> CargoResult<SemVersion>;
}

impl ToSemver for SemVersion {
    fn to_semver(self) -> CargoResult<SemVersion> {
        Ok(self)
    }
}

impl<'a> ToSemver for &'a str {
    fn to_semver(self) -> CargoResult<SemVersion> {
        match Version::parse(self) {
            Ok(v) => Ok(SemVersion::new(v)),
            Err(..) => Err(failure::format_err!("cannot parse '{}' as a semver", self)),
        }
    }
}

impl<'a> ToSemver for &'a String {
    fn to_semver(self) -> CargoResult<SemVersion> {
        (**self).to_semver()
    }
}

impl<'a> ToSemver for &'a SemVersion {
    fn to_semver(self) -> CargoResult<SemVersion> {
        Ok(self.clone())
    }
}


#[derive(Clone, Copy, Eq, Hash, PartialOrd, Ord)]
pub struct SemVersionReq {
    inner: &'static VersionReq,
}

impl SemVersionReq {
    pub fn new(req: VersionReq) -> SemVersionReq {
        let mut cache = SEM_VERSION_REQ_CACHE.lock().unwrap();
        let req = cache.get(&req).cloned().unwrap_or_else(|| {
            let req = Box::leak(Box::new(req));
            cache.insert(req);
            req
        });
        SemVersionReq { inner: req }
    }

    pub fn any() -> SemVersionReq {
        SemVersionReq::new(VersionReq::any())
    }

    pub fn value(&self) -> &'static VersionReq {
        self.inner
    }
}

impl PartialEq for SemVersionReq {
    fn eq(&self, other: &SemVersionReq) -> bool {
        ptr::eq(self.inner, other.inner)
    }
}

impl Serialize for SemVersionReq {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error> where S: Serializer {
        self.inner.serialize(ser)
    }
}

impl<'de> Deserialize<'de> for SemVersionReq {
    fn deserialize<D>(de: D) -> Result<SemVersionReq, D::Error> where D: Deserializer<'de> {
        Ok(SemVersionReq::new(<VersionReq>::deserialize(de)?))
    }
}

impl fmt::Debug for SemVersionReq {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.inner, f)
    }
}

impl fmt::Display for SemVersionReq {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.inner, f)
    }
}


pub trait ToSemverReq {
    fn to_semver_req(self) -> Result<SemVersionReq, ReqParseError>;
}

impl<'a> ToSemverReq for &'a str {
    fn to_semver_req(self) -> Result<SemVersionReq, ReqParseError> {
        Ok(SemVersionReq::new(VersionReq::parse(self)?))
    }
}


pub trait ToSemverReqExact {
    fn to_semver_req_exact(self) -> SemVersionReq;
}

impl<'a> ToSemverReqExact for &'a Version {
    fn to_semver_req_exact(self) -> SemVersionReq {
        SemVersionReq::new(VersionReq::exact(self))
    }
}
