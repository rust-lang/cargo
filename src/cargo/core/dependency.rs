use semver::Version;

#[deriving(Clone,Eq,Show)]
pub struct Dependency {
    name: ~str,
    version: Vec<VersionReq>
}

#[deriving(Clone,Eq,Show)]
pub struct VersionReq {
    parts: Vec<VersionOrd>
}

type VersionOrd = (Version, Vec<Ordering>);

impl VersionReq {
    pub fn parse(req: &str) -> VersionReq {
    }

    /*
    pub fn new(version: Version, comparison: Vec<Ordering>) -> VersionReq {
        VersionReq { comparison: comparison, version: version }
    }
    */

    pub fn matches(&self, version: &Version) -> bool {
        /*
        let ordering = compare_versions(&self.version, version);
        self.comparison.iter().any(|ord| ordering == *ord)
        */
        false
    }
}

fn compare_versions(a: &Version, b: &Version) -> Ordering {
    if a == b {
        Equal
    } else if a.lt(b) {
        Less
    } else {
        Greater
    }
}

impl Dependency {
    pub fn new(name: &str) -> Dependency {
        Dependency { name: name.to_owned(), version: Vec::new() }
    }

    pub fn get_name<'a>(&'a self) -> &'a str {
        self.name.as_slice()
    }
}

#[cfg(test)]
mod tests {
    use super::VersionReq;
    use semver;
    use semver::Version;
    use hamcrest::{
        assert_that,
        Matcher,
        MatchResult,
        SelfDescribing
    };

    trait VersionReqExt {
        pub fn greater_than(version: Version) -> VersionReq {
            VersionReq::new(version, vec!(Greater))
        }

        pub fn equal_to(version: Version) -> VersionReq {
            VersionReq::new(version, vec!(Equal))
        }
    }

    impl VersionReqExt for VersionReq {}

    #[test]
    fn test_req_matches() {
        let req = VersionReq::new(semver::parse("2.0.0").unwrap(), vec!(Equal));
        //let req = greater_than(semver::parse("2.0.0").unwrap());

        assert_that(req, version_match("2.0.0"));
    }

    struct VersionMatch {
        version: ~str,
    }

    impl SelfDescribing for VersionMatch {
        fn describe(&self) -> ~str {
            format!("Requirement to match {}", self.version)
        }
    }

    impl Matcher<VersionReq> for VersionMatch {
        fn matches(&self, actual: VersionReq) -> MatchResult {
            match semver::parse(self.version) {
                None => Err(~"was not a valid semver version"),
                Some(ref version) => {
                    if actual.matches(version) {
                        Ok(())
                    } else {
                        Err(format!("{} did not match {}", version, actual))
                    }
                }
            }
        }
    }

    fn version_match(str: &str) -> ~VersionMatch {
        ~VersionMatch { version: str.to_owned() }
    }

}
