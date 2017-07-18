use std::fmt;
use std::rc::Rc;
use std::str::FromStr;

use semver::VersionReq;
use semver::ReqParseError;
use serde::ser;

use core::{SourceId, Summary, PackageId};
use util::{Cfg, CfgExpr, Config};
use util::errors::{CargoResult, CargoResultExt, CargoError};

/// Information about a dependency requested by a Cargo manifest.
/// Cheap to copy.
#[derive(PartialEq, Clone, Debug)]
pub struct Dependency {
    inner: Rc<Inner>,
}

/// The data underlying a Dependency.
#[derive(PartialEq, Clone, Debug)]
struct Inner {
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

#[derive(Serialize)]
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

impl ser::Serialize for Dependency {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
        where S: ser::Serializer,
    {
        SerializedDependency {
            name: self.name(),
            source: &self.source_id(),
            req: self.version_req().to_string(),
            kind: self.kind(),
            optional: self.is_optional(),
            uses_default_features: self.uses_default_features(),
            features: self.features(),
            target: self.platform(),
        }.serialize(s)
    }
}

#[derive(PartialEq, Clone, Debug, Copy)]
pub enum Kind {
    Normal,
    Development,
    Build,
}

fn parse_req_with_deprecated(req: &str,
                             extra: Option<(&PackageId, &Config)>)
                             -> CargoResult<VersionReq> {
    match VersionReq::parse(req) {
        Err(e) => {
            let (inside, config) = match extra {
                Some(pair) => pair,
                None => return Err(e.into()),
            };
            match e {
                ReqParseError::DeprecatedVersionRequirement(requirement) => {
                    let msg = format!("\
parsed version requirement `{}` is no longer valid

Previous versions of Cargo accepted this malformed requirement,
but it is being deprecated. This was found when parsing the manifest
of {} {}, and the correct version requirement is `{}`.

This will soon become a hard error, so it's either recommended to
update to a fixed version or contact the upstream maintainer about
this warning.
",
req, inside.name(), inside.version(), requirement);
                    config.shell().warn(&msg)?;

                    Ok(requirement)
                }
                e => Err(e.into()),
            }
        },
        Ok(v) => Ok(v),
    }
}

impl ser::Serialize for Kind {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
        where S: ser::Serializer,
    {
        match *self {
            Kind::Normal => None,
            Kind::Development => Some("dev"),
            Kind::Build => Some("build"),
        }.serialize(s)
    }
}

impl Dependency {
    /// Attempt to create a `Dependency` from an entry in the manifest.
    pub fn parse(name: &str,
                 version: Option<&str>,
                 source_id: &SourceId,
                 inside: &PackageId,
                 config: &Config) -> CargoResult<Dependency> {
        let arg = Some((inside, config));
        let (specified_req, version_req) = match version {
            Some(v) => (true, parse_req_with_deprecated(v, arg)?),
            None => (false, VersionReq::any())
        };

        let mut ret = Dependency::new_override(name, source_id);
        {
            let ptr = Rc::make_mut(&mut ret.inner);
            ptr.only_match_name = false;
            ptr.req = version_req;
            ptr.specified_req = specified_req;
        }
        Ok(ret)
    }

    /// Attempt to create a `Dependency` from an entry in the manifest.
    pub fn parse_no_deprecated(name: &str,
                               version: Option<&str>,
                               source_id: &SourceId) -> CargoResult<Dependency> {
        let (specified_req, version_req) = match version {
            Some(v) => (true, parse_req_with_deprecated(v, None)?),
            None => (false, VersionReq::any())
        };

        let mut ret = Dependency::new_override(name, source_id);
        {
            let ptr = Rc::make_mut(&mut ret.inner);
            ptr.only_match_name = false;
            ptr.req = version_req;
            ptr.specified_req = specified_req;
        }
        Ok(ret)
    }

    pub fn new_override(name: &str, source_id: &SourceId) -> Dependency {
        Dependency {
            inner: Rc::new(Inner {
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
            }),
        }
    }

    pub fn version_req(&self) -> &VersionReq {
        &self.inner.req
    }

    pub fn name(&self) -> &str {
        &self.inner.name
    }

    pub fn source_id(&self) -> &SourceId {
        &self.inner.source_id
    }

    pub fn kind(&self) -> Kind {
        self.inner.kind
    }

    pub fn specified_req(&self) -> bool {
        self.inner.specified_req
    }

    /// If none, this dependencies must be built for all platforms.
    /// If some, it must only be built for the specified platform.
    pub fn platform(&self) -> Option<&Platform> {
        self.inner.platform.as_ref()
    }

    pub fn set_kind(&mut self, kind: Kind) -> &mut Dependency {
        Rc::make_mut(&mut self.inner).kind = kind;
        self
    }

    /// Sets the list of features requested for the package.
    pub fn set_features(&mut self, features: Vec<String>) -> &mut Dependency {
        Rc::make_mut(&mut self.inner).features = features;
        self
    }

    /// Sets whether the dependency requests default features of the package.
    pub fn set_default_features(&mut self, default_features: bool) -> &mut Dependency {
        Rc::make_mut(&mut self.inner).default_features = default_features;
        self
    }

    /// Sets whether the dependency is optional.
    pub fn set_optional(&mut self, optional: bool) -> &mut Dependency {
        Rc::make_mut(&mut self.inner).optional = optional;
        self
    }

    /// Set the source id for this dependency
    pub fn set_source_id(&mut self, id: SourceId) -> &mut Dependency {
        Rc::make_mut(&mut self.inner).source_id = id;
        self
    }

    /// Set the version requirement for this dependency
    pub fn set_version_req(&mut self, req: VersionReq) -> &mut Dependency {
        Rc::make_mut(&mut self.inner).req = req;
        self
    }

    pub fn set_platform(&mut self, platform: Option<Platform>) -> &mut Dependency {
        Rc::make_mut(&mut self.inner).platform = platform;
        self
    }

    /// Lock this dependency to depending on the specified package id
    pub fn lock_to(&mut self, id: &PackageId) -> &mut Dependency {
        assert_eq!(self.inner.source_id, *id.source_id());
        assert!(self.inner.req.matches(id.version()));
        self.set_version_req(VersionReq::exact(id.version()))
            .set_source_id(id.source_id().clone())
    }

    /// Returns whether this is a "locked" dependency, basically whether it has
    /// an exact version req.
    pub fn is_locked(&self) -> bool {
        // Kind of a hack to figure this out, but it works!
        self.inner.req.to_string().starts_with("=")
    }

    /// Returns false if the dependency is only used to build the local package.
    pub fn is_transitive(&self) -> bool {
        match self.inner.kind {
            Kind::Normal | Kind::Build => true,
            Kind::Development => false,
        }
    }

    pub fn is_build(&self) -> bool {
        match self.inner.kind {
            Kind::Build => true,
            _ => false,
        }
    }

    pub fn is_optional(&self) -> bool {
        self.inner.optional
    }

    /// Returns true if the default features of the dependency are requested.
    pub fn uses_default_features(&self) -> bool {
        self.inner.default_features
    }
    /// Returns the list of features that are requested by the dependency.
    pub fn features(&self) -> &[String] {
        &self.inner.features
    }

    /// Returns true if the package (`sum`) can fulfill this dependency request.
    pub fn matches(&self, sum: &Summary) -> bool {
        self.matches_id(sum.package_id())
    }

    /// Returns true if the package (`sum`) can fulfill this dependency request.
    pub fn matches_ignoring_source(&self, sum: &Summary) -> bool {
        self.name() == sum.package_id().name() &&
            self.version_req().matches(sum.package_id().version())
    }

    /// Returns true if the package (`id`) can fulfill this dependency request.
    pub fn matches_id(&self, id: &PackageId) -> bool {
        self.inner.name == id.name() &&
            (self.inner.only_match_name || (self.inner.req.matches(id.version()) &&
                                      &self.inner.source_id == id.source_id()))
    }

    pub fn map_source(mut self, to_replace: &SourceId, replace_with: &SourceId)
                      -> Dependency {
        if self.source_id() != to_replace {
            self
        } else {
            self.set_source_id(replace_with.clone());
            self
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

impl ser::Serialize for Platform {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
        where S: ser::Serializer,
    {
        self.to_string().serialize(s)
    }
}

impl FromStr for Platform {
    type Err = CargoError;

    fn from_str(s: &str) -> CargoResult<Platform> {
        if s.starts_with("cfg(") && s.ends_with(")") {
            let s = &s[4..s.len()-1];
            s.parse().map(Platform::Cfg).chain_err(|| {
                format!("failed to parse `{}` as a cfg expression", s)
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
