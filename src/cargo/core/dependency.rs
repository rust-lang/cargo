use std::fmt;
use std::rc::Rc;
use std::str::FromStr;

use semver::VersionReq;
use semver::ReqParseError;
use rustc_serialize::{Encoder, Encodable};

use core::{SourceId, Summary, PackageId};
use util::{CargoError, CargoResult, Cfg, CfgExpr, ChainError, human, Config};

/// Information about a dependency requested by a Cargo manifest.
/// Cheap to copy.
#[derive(PartialEq, Clone ,Debug)]
pub struct Dependency {
    inner: Rc<DependencyInner>,
}

/// The data underlying a Dependency.
#[derive(PartialEq, Clone, Debug)]
pub struct DependencyInner {
    name: String,
    source_id: SourceId,
    req: VersionReq,
    specified_req: bool,
    kind: Kind,
    only_match_name: bool,

    optional: bool,
    default_features: bool,
    features: Vec<String>,

    // This dependency should be used only for this platform.
    // `None` means *all platforms*.
    platform: Option<Platform>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Platform {
    Name(String),
    Cfg(CfgExpr),
}

#[derive(RustcEncodable)]
struct SerializedDependency<'a> {
    name: &'a str,
    source: &'a SourceId,
    req: String,
    kind: Kind,

    optional: bool,
    uses_default_features: bool,
    features: &'a [String],
    target: Option<&'a Platform>,
}

impl Encodable for Dependency {
    fn encode<S: Encoder>(&self, s: &mut S) -> Result<(), S::Error> {
        SerializedDependency {
            name: self.name(),
            source: &self.source_id(),
            req: self.version_req().to_string(),
            kind: self.kind(),
            optional: self.is_optional(),
            uses_default_features: self.uses_default_features(),
            features: self.features(),
            target: self.platform(),
        }.encode(s)
    }
}

#[derive(PartialEq, Clone, Debug, Copy)]
pub enum Kind {
    Normal,
    Development,
    Build,
}

impl Encodable for Kind {
    fn encode<S: Encoder>(&self, s: &mut S) -> Result<(), S::Error> {
        match *self {
            Kind::Normal => None,
            Kind::Development => Some("dev"),
            Kind::Build => Some("build"),
        }.encode(s)
    }
}

impl DependencyInner {
    /// Attempt to create a `Dependency` from an entry in the manifest.
    pub fn parse(name: &str,
                 version: Option<&str>,
                 source_id: &SourceId,
                 _config: &Config) -> CargoResult<DependencyInner> {
        let (specified_req, version_req) = match version {
            Some(v) => (true, try!(DependencyInner::parse_with_deprecated(v))),
            None => (false, VersionReq::any())
        };

        Ok(DependencyInner {
            only_match_name: false,
            req: version_req,
            specified_req: specified_req,
            .. DependencyInner::new_override(name, source_id)
        })
    }

    fn parse_with_deprecated(req: &str) -> Result<VersionReq, ReqParseError> {
        match VersionReq::parse(req) {
            Err(e) => {
                match e {
                    ReqParseError::DeprecatedVersionRequirement(requirement) => {
                        // warn here
                        
                        Ok(requirement)
                    }
                    e => Err(e),
                }
            },
            Ok(v) => Ok(v),
        }
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
            specified_req: false,
            platform: None,
        }
    }

    pub fn version_req(&self) -> &VersionReq { &self.req }
    pub fn name(&self) -> &str { &self.name }
    pub fn source_id(&self) -> &SourceId { &self.source_id }
    pub fn kind(&self) -> Kind { self.kind }
    pub fn specified_req(&self) -> bool { self.specified_req }

    /// If none, this dependency must be built for all platforms.
    /// If some, it must only be built for matching platforms.
    pub fn platform(&self) -> Option<&Platform> {
        self.platform.as_ref()
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

    pub fn set_platform(mut self, platform: Option<Platform>)
                        -> DependencyInner {
        self.platform = platform;
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
                 source_id: &SourceId,
                 config: &Config) -> CargoResult<Dependency> {
        DependencyInner::parse(name, version, source_id, config).map(|di| {
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
    pub fn specified_req(&self) -> bool { self.inner.specified_req() }

    /// If none, this dependencies must be built for all platforms.
    /// If some, it must only be built for the specified platform.
    pub fn platform(&self) -> Option<&Platform> {
        self.inner.platform()
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

    pub fn map_source(self, to_replace: &SourceId, replace_with: &SourceId)
                      -> Dependency {
        if self.source_id() != to_replace {
            self
        } else {
            Rc::try_unwrap(self.inner).unwrap_or_else(|r| (*r).clone())
               .set_source_id(replace_with.clone())
               .into_dependency()
        }
    }
}

impl Platform {
    pub fn matches(&self, name: &str, cfg: Option<&[Cfg]>) -> bool {
        match *self {
            Platform::Name(ref p) => p == name,
            Platform::Cfg(ref p) => {
                match cfg {
                    Some(cfg) => p.matches(cfg),
                    None => false,
                }
            }
        }
    }
}

impl Encodable for Platform {
    fn encode<S: Encoder>(&self, s: &mut S) -> Result<(), S::Error> {
        self.to_string().encode(s)
    }
}

impl FromStr for Platform {
    type Err = Box<CargoError>;

    fn from_str(s: &str) -> CargoResult<Platform> {
        if s.starts_with("cfg(") && s.ends_with(")") {
            let s = &s[4..s.len()-1];
            s.parse().map(Platform::Cfg).chain_error(|| {
                human(format!("failed to parse `{}` as a cfg expression", s))
            })
        } else {
            Ok(Platform::Name(s.to_string()))
        }
    }
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Platform::Name(ref n) => n.fmt(f),
            Platform::Cfg(ref e) => write!(f, "cfg({})", e),
        }
    }
}
