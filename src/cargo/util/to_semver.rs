use crate::util::errors::CargoResult;
use semver::{Version, VersionReq};
use semver::ReqParseError;

pub trait ToSemver {
    fn to_semver(self) -> CargoResult<Version>;
}

impl ToSemver for Version {
    fn to_semver(self) -> CargoResult<Version> {
        Ok(self)
    }
}

impl<'a> ToSemver for &'a str {
    fn to_semver(self) -> CargoResult<Version> {
        match Version::parse(self) {
            Ok(v) => Ok(v),
            Err(..) => Err(failure::format_err!("cannot parse '{}' as a semver", self)),
        }
    }
}

impl<'a> ToSemver for &'a String {
    fn to_semver(self) -> CargoResult<Version> {
        (**self).to_semver()
    }
}

impl<'a> ToSemver for &'a Version {
    fn to_semver(self) -> CargoResult<Version> {
        Ok(self.clone())
    }
}


pub trait ToSemverReq {
    fn to_semver_req(self) -> Result<VersionReq, ReqParseError>;
}

impl<'a> ToSemverReq for &'a str {
    fn to_semver_req(self) -> Result<VersionReq, ReqParseError> {
        VersionReq::parse(self)
    }
}


pub trait ToSemverReqExact {
    fn to_semver_req_exact(self) -> VersionReq;
}

impl<'a> ToSemverReqExact for &'a Version {
    fn to_semver_req_exact(self) -> VersionReq {
        VersionReq::exact(self)
    }
}
