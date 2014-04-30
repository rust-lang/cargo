use std::vec::Vec;
use semver;
use core;
use core::{NameVer,Dependency};
use core::manifest::{Manifest,LibTarget};

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
    source: LibTarget,
    target: ~str
}

impl Package {
    pub fn new(name: &core::NameVer, deps: &Vec<core::Dependency>, root: &str, source: &LibTarget, target: &str) -> Package {
        Package { name_ver: name.clone(), deps: deps.clone(), root: root.to_owned(), source: source.clone(), target: target.to_owned()  }
    }

    pub fn from_manifest(manifest: &Manifest) -> Package {
        let project = &manifest.project;

        Package {
            name_ver: core::NameVer::new(project.name.as_slice(), project.version.as_slice()),
            deps: manifest.dependencies.clone(),
            root: manifest.root.clone(),
            source: manifest.lib.as_slice().get(0).unwrap().clone(),
            target: manifest.target.clone()
        }
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

    pub fn get_source<'a>(&'a self) -> &'a LibTarget {
        &self.source
    }

    pub fn get_target<'a>(&'a self) -> &'a str {
        self.target.as_slice()
    }

    pub fn get_dependencies<'a>(&'a self) -> &'a [core::Dependency] {
        self.deps.as_slice()
    }
}
