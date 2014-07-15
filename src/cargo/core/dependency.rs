use core::{VersionReq,SourceId,Summary};
use util::CargoResult;

#[deriving(PartialEq,Clone,Show)]
pub struct Dependency {
    name: String,
    source_id: SourceId,
    req: VersionReq,
    transitive: bool
}

impl Dependency {
    pub fn parse(name: &str, version: Option<&str>,
                 source_id: &SourceId) -> CargoResult<Dependency> {
        let version = match version {
            Some(v) => try!(VersionReq::parse(v)),
            None => VersionReq::any()
        };

        Ok(Dependency {
            name: name.to_string(),
            source_id: source_id.clone(),
            req: version,
            transitive: true
        })
    }

    pub fn get_version_req(&self) -> &VersionReq {
        &self.req
    }

    pub fn get_name(&self) -> &str {
        self.name.as_slice()
    }

    pub fn get_source_id(&self) -> &SourceId {
        &self.source_id
    }

    pub fn as_dev(&self) -> Dependency {
        let mut dep = self.clone();
        dep.transitive = false;
        dep
    }

    pub fn is_transitive(&self) -> bool {
        self.transitive
    }

    pub fn matches(&self, sum: &Summary) -> bool {
        debug!("matches; self={}; summary={}", self, sum);
        debug!("         a={}; b={}", self.source_id, sum.get_source_id());

        self.name.as_slice() == sum.get_name() &&
            self.req.matches(sum.get_version()) &&
            &self.source_id == sum.get_source_id()
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
            name: dep.get_name().to_string(),
            req: dep.get_version_req().to_string()
        }
    }
}
