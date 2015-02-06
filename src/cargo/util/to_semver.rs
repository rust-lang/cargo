use semver::Version;

pub trait ToSemver {
    fn to_semver(self) -> Result<Version, String>;
}

impl ToSemver for Version {
    fn to_semver(self) -> Result<Version, String> { Ok(self) }
}

impl<'a> ToSemver for &'a str {
    fn to_semver(self) -> Result<Version, String> {
        match Version::parse(self) {
            Ok(v) => Ok(v),
            Err(..) => Err(format!("cannot parse '{}' as a semver", self)),
        }
    }
}

impl<'a> ToSemver for &'a String {
    fn to_semver(self) -> Result<Version, String> {
        (**self).to_semver()
    }
}
