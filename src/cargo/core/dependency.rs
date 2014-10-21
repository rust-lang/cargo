use core::{SourceId,Summary};
use semver::VersionReq;
use util::CargoResult;

/// Informations about a dependency requested by a Cargo manifest.
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
    /// Attempt to create a `Dependency` from an entry in the manifest.
    ///
    /// ## Example
    ///
    /// ```
    /// use cargo::core::SourceId;
    /// use cargo::core::Dependency;
    ///
    /// Dependency::parse("color", Some("1.*"),
    ///                   &SourceId::for_central().unwrap()).unwrap();
    /// ```
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

    /// Returns the version of the dependency that is being requested.
    pub fn get_version_req(&self) -> &VersionReq {
        &self.req
    }

    pub fn get_name(&self) -> &str {
        self.name.as_slice()
    }

    /// Returns the place where this dependency must be searched for.
    pub fn get_source_id(&self) -> &SourceId {
        &self.source_id
    }

    pub fn transitive(mut self, transitive: bool) -> Dependency {
        self.transitive = transitive;
        self
    }

    /// Sets the list of features requested for the package.
    pub fn features(mut self, features: Vec<String>) -> Dependency {
        self.features = features;
        self
    }

    /// Sets whether the dependency requests default features of the package.
    pub fn default_features(mut self, default_features: bool) -> Dependency {
        self.default_features = default_features;
        self
    }

    /// Sets whether the dependency is optional.
    pub fn optional(mut self, optional: bool) -> Dependency {
        self.optional = optional;
        self
    }

    /// Returns false if the dependency is only used to build the local package.
    pub fn is_transitive(&self) -> bool { self.transitive }
    pub fn is_optional(&self) -> bool { self.optional }
    /// Returns true if the default features of the dependency are requested.
    pub fn uses_default_features(&self) -> bool { self.default_features }
    /// Returns the list of features that are requested by the dependency.
    pub fn get_features(&self) -> &[String] { self.features.as_slice() }

    /// Returns true if the package (`sum`) can fulfill this dependency request.
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
