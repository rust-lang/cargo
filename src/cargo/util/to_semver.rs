use crate::util::errors::CargoResult;
use semver::Version;

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
        match Version::parse(self.trim()) {
            Ok(v) => Ok(v),
            Err(..) => Err(anyhow::format_err!(
                "cannot parse '{}' as a SemVer version",
                self
            )),
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
