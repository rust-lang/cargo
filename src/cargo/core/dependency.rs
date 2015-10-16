use semver::VersionReq;

use core::{SourceId, Summary, PackageId};
use std::rc::Rc;
use util::CargoResult;

/// The data underlying a Dependency.
#[derive(PartialEq,Clone,Debug)]
pub struct DependencyInner {
    name: String,
    source_id: SourceId,
    req: VersionReq,
    specified_req: Option<String>,
    kind: Kind,
    only_match_name: bool,

    optional: bool,
    default_features: bool,
    features: Vec<String>,

    // This dependency should be used only for this platform.
    // `None` means *all platforms*.
    only_for_platform: Option<String>,
}

/// Information about a dependency requested by a Cargo manifest.
/// Cheap to copy.
#[derive(PartialEq,Clone,Debug)]
pub struct Dependency {
    inner: Rc<DependencyInner>,
}

#[derive(PartialEq, Clone, Debug, Copy)]
pub enum Kind {
    Normal,
    Development,
    Build,
}

impl DependencyInner {
    /// Attempt to create a `Dependency` from an entry in the manifest.
    pub fn parse(name: &str,
                 version: Option<&str>,
                 source_id: &SourceId) -> CargoResult<DependencyInner> {
        let version_req = match version {
            Some(v) => try!(VersionReq::parse(v)),
            None => VersionReq::any()
        };

        Ok(DependencyInner {
            only_match_name: false,
            req: version_req,
            specified_req: version.map(|s| s.to_string()),
            .. DependencyInner::new_override(name, source_id)
        })
    }

    pub fn new_override(name: &str, source_id: &SourceId) -> DependencyInner {
        DependencyInner {
            name: name.to_string(),
            source_id: source_id.clone(),
            req: VersionReq::any(),
            kind: Kind::Normal,
            only_match_name: true,
            optional: false,
            features: Vec::new(),
            default_features: true,
            specified_req: None,
            only_for_platform: None,
        }
    }

    pub fn version_req(&self) -> &VersionReq { &self.req }
    pub fn name(&self) -> &str { &self.name }
    pub fn source_id(&self) -> &SourceId { &self.source_id }
    pub fn kind(&self) -> Kind { self.kind }
    pub fn specified_req(&self) -> Option<&str> {
        self.specified_req.as_ref().map(|s| &s[..])
    }

    /// If none, this dependencies must be built for all platforms.
    /// If some, it must only be built for the specified platform.
    pub fn only_for_platform(&self) -> Option<&str> {
        self.only_for_platform.as_ref().map(|s| &s[..])
    }

    pub fn set_kind(mut self, kind: Kind) -> DependencyInner {
        self.kind = kind;
        self
    }

    /// Sets the list of features requested for the package.
    pub fn set_features(mut self, features: Vec<String>) -> DependencyInner {
        self.features = features;
        self
    }

    /// Sets whether the dependency requests default features of the package.
    pub fn set_default_features(mut self, default_features: bool) -> DependencyInner {
        self.default_features = default_features;
        self
    }

    /// Sets whether the dependency is optional.
    pub fn set_optional(mut self, optional: bool) -> DependencyInner {
        self.optional = optional;
        self
    }

    /// Set the source id for this dependency
    pub fn set_source_id(mut self, id: SourceId) -> DependencyInner {
        self.source_id = id;
        self
    }

    /// Set the version requirement for this dependency
    pub fn set_version_req(mut self, req: VersionReq) -> DependencyInner {
        self.req = req;
        self
    }

    pub fn set_only_for_platform(mut self, platform: Option<String>)
                                 -> DependencyInner {
        self.only_for_platform = platform;
        self
    }

    /// Lock this dependency to depending on the specified package id
    pub fn lock_to(self, id: &PackageId) -> DependencyInner {
        assert_eq!(self.source_id, *id.source_id());
        assert!(self.req.matches(id.version()));
        self.set_version_req(VersionReq::exact(id.version()))
            .set_source_id(id.source_id().clone())
    }

    /// Returns false if the dependency is only used to build the local package.
    pub fn is_transitive(&self) -> bool {
        match self.kind {
            Kind::Normal | Kind::Build => true,
            Kind::Development => false,
        }
    }
    pub fn is_build(&self) -> bool {
        match self.kind { Kind::Build => true, _ => false }
    }
    pub fn is_optional(&self) -> bool { self.optional }
    /// Returns true if the default features of the dependency are requested.
    pub fn uses_default_features(&self) -> bool { self.default_features }
    /// Returns the list of features that are requested by the dependency.
    pub fn features(&self) -> &[String] { &self.features }

    /// Returns true if the package (`sum`) can fulfill this dependency request.
    pub fn matches(&self, sum: &Summary) -> bool {
        self.matches_id(sum.package_id())
    }

    /// Returns true if the package (`id`) can fulfill this dependency request.
    pub fn matches_id(&self, id: &PackageId) -> bool {
        self.name == id.name() &&
            (self.only_match_name || (self.req.matches(id.version()) &&
                                      &self.source_id == id.source_id()))
    }

    pub fn into_dependency(self) -> Dependency {
        Dependency {inner: Rc::new(self)}
    }
}

impl Dependency {
    /// Attempt to create a `Dependency` from an entry in the manifest.
    pub fn parse(name: &str,
                 version: Option<&str>,
                 source_id: &SourceId) -> CargoResult<Dependency> {
        DependencyInner::parse(name, version, source_id).map(|di| {
            di.into_dependency()
        })
    }

    pub fn new_override(name: &str, source_id: &SourceId) -> Dependency {
        DependencyInner::new_override(name, source_id).into_dependency()
    }

    pub fn clone_inner(&self) -> DependencyInner { (*self.inner).clone() }

    pub fn version_req(&self) -> &VersionReq { self.inner.version_req() }
    pub fn name(&self) -> &str { self.inner.name() }
    pub fn source_id(&self) -> &SourceId { self.inner.source_id() }
    pub fn kind(&self) -> Kind { self.inner.kind() }
    pub fn specified_req(&self) -> Option<&str> { self.inner.specified_req() }

    /// If none, this dependencies must be built for all platforms.
    /// If some, it must only be built for the specified platform.
    pub fn only_for_platform(&self) -> Option<&str> {
        self.inner.only_for_platform()
    }

    /// Lock this dependency to depending on the specified package id
    pub fn lock_to(self, id: &PackageId) -> Dependency {
        self.clone_inner().lock_to(id).into_dependency()
    }

    /// Returns false if the dependency is only used to build the local package.
    pub fn is_transitive(&self) -> bool { self.inner.is_transitive() }
    pub fn is_build(&self) -> bool { self.inner.is_build() }
    pub fn is_optional(&self) -> bool { self.inner.is_optional() }
    /// Returns true if the default features of the dependency are requested.
    pub fn uses_default_features(&self) -> bool {
        self.inner.uses_default_features()
    }
    /// Returns the list of features that are requested by the dependency.
    pub fn features(&self) -> &[String] { self.inner.features() }

    /// Returns true if the package (`sum`) can fulfill this dependency request.
    pub fn matches(&self, sum: &Summary) -> bool { self.inner.matches(sum) }

    /// Returns true if the package (`id`) can fulfill this dependency request.
    pub fn matches_id(&self, id: &PackageId) -> bool {
        self.inner.matches_id(id)
    }
}

#[derive(PartialEq,Clone,RustcEncodable)]
pub struct SerializedDependency {
    name: String,
    req: String
}

impl SerializedDependency {
    pub fn from_dependency(dep: &Dependency) -> SerializedDependency {
        SerializedDependency {
            name: dep.name().to_string(),
            req: dep.version_req().to_string()
        }
    }
}
