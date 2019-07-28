use std::fmt;
use std::rc::Rc;
use std::str::FromStr;

use log::trace;
use semver::ReqParseError;
use semver::VersionReq;
use serde::ser;
use serde::Serialize;
use url::Url;

use crate::core::interning::InternedString;
use crate::core::{PackageId, SourceId, Summary};
use crate::util::errors::{CargoResult, CargoResultExt};
use crate::util::{Cfg, CfgExpr, Config};

/// Information about a dependency requested by a Cargo manifest.
/// Cheap to copy.
#[derive(PartialEq, Eq, Hash, Ord, PartialOrd, Clone, Debug)]
pub struct Dependency {
    inner: Rc<Inner>,
}

/// The data underlying a `Dependency`.
#[derive(PartialEq, Eq, Hash, Ord, PartialOrd, Clone, Debug)]
struct Inner {
    name: InternedString,
    source_id: SourceId,
    /// Source ID for the registry as specified in the manifest.
    ///
    /// This will be None if it is not specified (crates.io dependency).
    /// This is different from `source_id` for example when both a `path` and
    /// `registry` is specified. Or in the case of a crates.io dependency,
    /// `source_id` will be crates.io and this will be None.
    registry_id: Option<SourceId>,
    req: VersionReq,
    specified_req: bool,
    kind: Kind,
    only_match_name: bool,
    explicit_name_in_toml: Option<InternedString>,

    optional: bool,
    public: bool,
    default_features: bool,
    features: Vec<InternedString>,

    // This dependency should be used only for this platform.
    // `None` means *all platforms*.
    platform: Option<Platform>,
}

#[derive(Eq, PartialEq, Hash, Ord, PartialOrd, Clone, Debug)]
pub enum Platform {
    Name(String),
    Cfg(CfgExpr),
}

#[derive(Serialize)]
struct SerializedDependency<'a> {
    name: &'a str,
    source: SourceId,
    req: String,
    kind: Kind,
    rename: Option<&'a str>,

    optional: bool,
    uses_default_features: bool,
    features: &'a [InternedString],
    target: Option<&'a Platform>,
    /// The registry URL this dependency is from.
    /// If None, then it comes from the default registry (crates.io).
    registry: Option<Url>,
}

impl ser::Serialize for Dependency {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        SerializedDependency {
            name: &*self.package_name(),
            source: self.source_id(),
            req: self.version_req().to_string(),
            kind: self.kind(),
            optional: self.is_optional(),
            uses_default_features: self.uses_default_features(),
            features: self.features(),
            target: self.platform(),
            rename: self.explicit_name_in_toml().map(|s| s.as_str()),
            registry: self.registry_id().map(|sid| sid.url().clone()),
        }
        .serialize(s)
    }
}

#[derive(PartialEq, Eq, Hash, Ord, PartialOrd, Clone, Debug, Copy)]
pub enum Kind {
    Normal,
    Development,
    Build,
}

fn parse_req_with_deprecated(
    name: &str,
    req: &str,
    extra: Option<(PackageId, &Config)>,
) -> CargoResult<VersionReq> {
    match VersionReq::parse(req) {
        Err(ReqParseError::DeprecatedVersionRequirement(requirement)) => {
            let (inside, config) = match extra {
                Some(pair) => pair,
                None => return Err(ReqParseError::DeprecatedVersionRequirement(requirement).into()),
            };
            let msg = format!(
                "\
parsed version requirement `{}` is no longer valid

Previous versions of Cargo accepted this malformed requirement,
but it is being deprecated. This was found when parsing the manifest
of {} {}, and the correct version requirement is `{}`.

This will soon become a hard error, so it's either recommended to
update to a fixed version or contact the upstream maintainer about
this warning.
",
                req,
                inside.name(),
                inside.version(),
                requirement
            );
            config.shell().warn(&msg)?;

            Ok(requirement)
        }
        Err(e) => {
            let err: CargoResult<VersionReq> = Err(e.into());
            let v: VersionReq = err.chain_err(|| {
                format!(
                    "failed to parse the version requirement `{}` for dependency `{}`",
                    req, name
                )
            })?;
            Ok(v)
        }
        Ok(v) => Ok(v),
    }
}

impl ser::Serialize for Kind {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        match *self {
            Kind::Normal => None,
            Kind::Development => Some("dev"),
            Kind::Build => Some("build"),
        }
        .serialize(s)
    }
}

impl Dependency {
    /// Attempt to create a `Dependency` from an entry in the manifest.
    pub fn parse(
        name: &str,
        version: Option<&str>,
        source_id: SourceId,
        inside: PackageId,
        config: &Config,
    ) -> CargoResult<Dependency> {
        let arg = Some((inside, config));
        let (specified_req, version_req) = match version {
            Some(v) => (true, parse_req_with_deprecated(name, v, arg)?),
            None => (false, VersionReq::any()),
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
    pub fn parse_no_deprecated(
        name: &str,
        version: Option<&str>,
        source_id: SourceId,
    ) -> CargoResult<Dependency> {
        let (specified_req, version_req) = match version {
            Some(v) => (true, parse_req_with_deprecated(name, v, None)?),
            None => (false, VersionReq::any()),
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

    pub fn new_override(name: &str, source_id: SourceId) -> Dependency {
        assert!(!name.is_empty());
        Dependency {
            inner: Rc::new(Inner {
                name: InternedString::new(name),
                source_id,
                registry_id: None,
                req: VersionReq::any(),
                kind: Kind::Normal,
                only_match_name: true,
                optional: false,
                public: false,
                features: Vec::new(),
                default_features: true,
                specified_req: false,
                platform: None,
                explicit_name_in_toml: None,
            }),
        }
    }

    pub fn version_req(&self) -> &VersionReq {
        &self.inner.req
    }

    /// This is the name of this `Dependency` as listed in `Cargo.toml`.
    ///
    /// Or in other words, this is what shows up in the `[dependencies]` section
    /// on the left hand side. This is *not* the name of the package that's
    /// being depended on as the dependency can be renamed. For that use
    /// `package_name` below.
    ///
    /// Both of the dependencies below return `foo` for `name_in_toml`:
    ///
    /// ```toml
    /// [dependencies]
    /// foo = "0.1"
    /// ```
    ///
    /// and ...
    ///
    /// ```toml
    /// [dependencies]
    /// foo = { version = "0.1", package = 'bar' }
    /// ```
    pub fn name_in_toml(&self) -> InternedString {
        self.explicit_name_in_toml().unwrap_or(self.inner.name)
    }

    /// The name of the package that this `Dependency` depends on.
    ///
    /// Usually this is what's written on the left hand side of a dependencies
    /// section, but it can also be renamed via the `package` key.
    ///
    /// Both of the dependencies below return `foo` for `package_name`:
    ///
    /// ```toml
    /// [dependencies]
    /// foo = "0.1"
    /// ```
    ///
    /// and ...
    ///
    /// ```toml
    /// [dependencies]
    /// bar = { version = "0.1", package = 'foo' }
    /// ```
    pub fn package_name(&self) -> InternedString {
        self.inner.name
    }

    pub fn source_id(&self) -> SourceId {
        self.inner.source_id
    }

    pub fn registry_id(&self) -> Option<SourceId> {
        self.inner.registry_id
    }

    pub fn set_registry_id(&mut self, registry_id: SourceId) -> &mut Dependency {
        Rc::make_mut(&mut self.inner).registry_id = Some(registry_id);
        self
    }

    pub fn kind(&self) -> Kind {
        self.inner.kind
    }

    pub fn is_public(&self) -> bool {
        self.inner.public
    }

    /// Sets whether the dependency is public.
    pub fn set_public(&mut self, public: bool) -> &mut Dependency {
        if public {
            // Setting 'public' only makes sense for normal dependencies
            assert_eq!(self.kind(), Kind::Normal);
        }
        Rc::make_mut(&mut self.inner).public = public;
        self
    }

    pub fn specified_req(&self) -> bool {
        self.inner.specified_req
    }

    /// If none, this dependencies must be built for all platforms.
    /// If some, it must only be built for the specified platform.
    pub fn platform(&self) -> Option<&Platform> {
        self.inner.platform.as_ref()
    }

    /// The renamed name of this dependency, if any.
    ///
    /// If the `package` key is used in `Cargo.toml` then this returns the same
    /// value as `name_in_toml`.
    pub fn explicit_name_in_toml(&self) -> Option<InternedString> {
        self.inner.explicit_name_in_toml
    }

    pub fn set_kind(&mut self, kind: Kind) -> &mut Dependency {
        if self.is_public() {
            // Setting 'public' only makes sense for normal dependencies
            assert_eq!(kind, Kind::Normal);
        }
        Rc::make_mut(&mut self.inner).kind = kind;
        self
    }

    /// Sets the list of features requested for the package.
    pub fn set_features(
        &mut self,
        features: impl IntoIterator<Item = impl AsRef<str>>,
    ) -> &mut Dependency {
        Rc::make_mut(&mut self.inner).features = features
            .into_iter()
            .map(|s| InternedString::new(s.as_ref()))
            .collect();
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

    /// Sets the source ID for this dependency.
    pub fn set_source_id(&mut self, id: SourceId) -> &mut Dependency {
        Rc::make_mut(&mut self.inner).source_id = id;
        self
    }

    /// Sets the version requirement for this dependency.
    pub fn set_version_req(&mut self, req: VersionReq) -> &mut Dependency {
        Rc::make_mut(&mut self.inner).req = req;
        self
    }

    pub fn set_platform(&mut self, platform: Option<Platform>) -> &mut Dependency {
        Rc::make_mut(&mut self.inner).platform = platform;
        self
    }

    pub fn set_explicit_name_in_toml(&mut self, name: &str) -> &mut Dependency {
        Rc::make_mut(&mut self.inner).explicit_name_in_toml = Some(InternedString::new(name));
        self
    }

    /// Locks this dependency to depending on the specified package ID.
    pub fn lock_to(&mut self, id: PackageId) -> &mut Dependency {
        assert_eq!(self.inner.source_id, id.source_id());
        assert!(self.inner.req.matches(id.version()));
        trace!(
            "locking dep from `{}` with `{}` at {} to {}",
            self.package_name(),
            self.version_req(),
            self.source_id(),
            id
        );
        self.set_version_req(VersionReq::exact(id.version()))
            .set_source_id(id.source_id())
    }

    /// Returns `true` if this is a "locked" dependency, basically whether it has
    /// an exact version req.
    pub fn is_locked(&self) -> bool {
        // Kind of a hack to figure this out, but it works!
        self.inner.req.to_string().starts_with('=')
    }

    /// Returns `false` if the dependency is only used to build the local package.
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

    /// Returns `true` if the default features of the dependency are requested.
    pub fn uses_default_features(&self) -> bool {
        self.inner.default_features
    }
    /// Returns the list of features that are requested by the dependency.
    pub fn features(&self) -> &[InternedString] {
        &self.inner.features
    }

    /// Returns `true` if the package (`sum`) can fulfill this dependency request.
    pub fn matches(&self, sum: &Summary) -> bool {
        self.matches_id(sum.package_id())
    }

    /// Returns `true` if the package (`id`) can fulfill this dependency request.
    pub fn matches_ignoring_source(&self, id: PackageId) -> bool {
        self.package_name() == id.name() && self.version_req().matches(id.version())
    }

    /// Returns `true` if the package (`id`) can fulfill this dependency request.
    pub fn matches_id(&self, id: PackageId) -> bool {
        self.inner.name == id.name()
            && (self.inner.only_match_name
                || (self.inner.req.matches(id.version()) && self.inner.source_id == id.source_id()))
    }

    pub fn map_source(mut self, to_replace: SourceId, replace_with: SourceId) -> Dependency {
        if self.source_id() != to_replace {
            self
        } else {
            self.set_source_id(replace_with);
            self
        }
    }
}

impl Platform {
    pub fn matches(&self, name: &str, cfg: &[Cfg]) -> bool {
        match *self {
            Platform::Name(ref p) => p == name,
            Platform::Cfg(ref p) => p.matches(cfg),
        }
    }
}

impl ser::Serialize for Platform {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        self.to_string().serialize(s)
    }
}

impl FromStr for Platform {
    type Err = failure::Error;

    fn from_str(s: &str) -> CargoResult<Platform> {
        if s.starts_with("cfg(") && s.ends_with(')') {
            let s = &s[4..s.len() - 1];
            let p = s.parse().map(Platform::Cfg).chain_err(|| {
                failure::format_err!("failed to parse `{}` as a cfg expression", s)
            })?;
            Ok(p)
        } else {
            Ok(Platform::Name(s.to_string()))
        }
    }
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Platform::Name(ref n) => n.fmt(f),
            Platform::Cfg(ref e) => write!(f, "cfg({})", e),
        }
    }
}
