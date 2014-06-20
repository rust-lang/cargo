use semver::Version;
use core::{VersionReq,SourceId};
use util::CargoResult;

#[deriving(PartialEq,Clone,Show)]
pub struct Dependency {
    name: String,
    namespace: SourceId,
    req: VersionReq
}

impl Dependency {
    pub fn new(name: &str, req: &VersionReq,
               namespace: &SourceId) -> Dependency {
        Dependency {
            name: name.to_str(),
            namespace: namespace.clone(),
            req: req.clone()
        }
    }

    pub fn parse(name: &str, version: &str,
                 namespace: &SourceId) -> CargoResult<Dependency> {
        Ok(Dependency {
            name: name.to_str(),
            namespace: namespace.clone(),
            req: try!(VersionReq::parse(version)),
        })
    }

    pub fn exact(name: &str, version: &Version,
                 namespace: &SourceId) -> Dependency {
        Dependency {
            name: name.to_str(),
            namespace: namespace.clone(),
            req: VersionReq::exact(version)
        }
    }

    pub fn get_version_req<'a>(&'a self) -> &'a VersionReq {
        &self.req
    }

    pub fn get_name<'a>(&'a self) -> &'a str {
        self.name.as_slice()
    }

    pub fn get_namespace<'a>(&'a self) -> &'a SourceId {
        &self.namespace
    }
}

#[deriving(PartialEq,Clone,Encodable)]
pub struct SerializedDependency {
    name: String,
    req: String
}

impl SerializedDependency {
    pub fn from_dependency(dep: &Dependency) -> SerializedDependency {
        SerializedDependency {
            name: dep.get_name().to_str(),
            req: dep.get_version_req().to_str()
        }
    }
}
