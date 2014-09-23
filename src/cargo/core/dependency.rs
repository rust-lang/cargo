use core::{SourceId,Summary};
use semver::VersionReq;
use util::CargoResult;

#[deriving(PartialEq,Clone,Show)]
pub struct Dependency {
    name: String,
    source_id: SourceId,
    req: VersionReq,
    transitive: bool,
    only_match_name: bool,

    optional: bool,
    default_features: bool,
    features: Vec<String>,
}

impl Dependency {
    pub fn parse(name: &str,
                 version: Option<&str>,
                 source_id: &SourceId) -> CargoResult<Dependency> {
        let version = match version {
            Some(v) => try!(VersionReq::parse(v)),
            None => VersionReq::any()
        };

        Ok(Dependency {
            only_match_name: false,
            req: version,
            .. Dependency::new_override(name, source_id)
        })
    }

    pub fn new_override(name: &str, source_id: &SourceId) -> Dependency {
        Dependency {
            name: name.to_string(),
            source_id: source_id.clone(),
            req: VersionReq::any(),
            transitive: true,
            only_match_name: true,
            optional: false,
            features: Vec::new(),
            default_features: true,
        }
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

    pub fn transitive(mut self, transitive: bool) -> Dependency {
        self.transitive = transitive;
        self
    }

    pub fn features(mut self, features: Vec<String>) -> Dependency {
        self.features = features;
        self
    }

    pub fn default_features(mut self, default_features: bool) -> Dependency {
        self.default_features = default_features;
        self
    }

    pub fn optional(mut self, optional: bool) -> Dependency {
        self.optional = optional;
        self
    }

    pub fn is_transitive(&self) -> bool { self.transitive }
    pub fn is_optional(&self) -> bool { self.optional }
    pub fn uses_default_features(&self) -> bool { self.default_features }
    pub fn get_features(&self) -> &[String] { self.features.as_slice() }

    pub fn matches(&self, sum: &Summary) -> bool {
        debug!("matches; self={}; summary={}", self, sum);
        debug!("         a={}; b={}", self.source_id, sum.get_source_id());

        self.name.as_slice() == sum.get_name() &&
            (self.only_match_name || (self.req.matches(sum.get_version()) &&
                                      &self.source_id == sum.get_source_id()))
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
