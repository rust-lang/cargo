use std::vec::Vec;
use semver;
use core;

/**
 * Represents a rust library internally to cargo. This will things like where
 * on the local system the code is located, it's remote location, dependencies,
 * etc..
 *
 * This differs from core::Project
 */
#[deriving(Clone,Eq,Show)]
pub struct Package {
    name_ver: core::NameVer,
    deps: Vec<core::Dependency>,
    root: ~str,
    source: ~str,
    target: ~str
}

impl Package {
    pub fn new(name: &core::NameVer, deps: &Vec<core::Dependency>, root: &str, source: &str, target: &str) -> Package {
        Package { name_ver: name.clone(), deps: deps.clone(), root: root.to_owned(), source: source.to_owned(), target: target.to_owned()  }
    }

    pub fn get_name<'a>(&'a self) -> &'a str {
        self.name_ver.get_name()
    }

    pub fn get_version<'a>(&'a self) -> &'a semver::Version {
        self.name_ver.get_version()
    }

    pub fn get_root<'a>(&'a self) -> &'a str {
        self.root.as_slice()
    }

    pub fn get_source<'a>(&'a self) -> &'a str {
        self.source.as_slice()
    }

    pub fn get_target<'a>(&'a self) -> &'a str {
        self.target.as_slice()
    }

    pub fn get_dependencies<'a>(&'a self) -> &'a [core::Dependency] {
        self.deps.as_slice()
    }
}
