use cargo_platform::Platform;
use semver::VersionReq;
use serde::ser;
use serde::Serialize;
use std::borrow::Cow;
use std::fmt;
use std::path::PathBuf;
use std::rc::Rc;
use tracing::trace;

use crate::core::compiler::{CompileKind, CompileTarget};
use crate::core::{PackageId, SourceId, Summary};
use crate::util::errors::CargoResult;
use crate::util::interning::InternedString;
use crate::util::OptVersionReq;

/// Information about a dependency requested by a Cargo manifest.
/// Cheap to copy.
#[derive(PartialEq, Eq, Hash, Clone, Debug)]
pub struct Dependency {
    inner: Rc<Inner>,
}

/// The data underlying a `Dependency`.
#[derive(PartialEq, Eq, Hash, Clone, Debug)]
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
    req: OptVersionReq,
    specified_req: bool,
    kind: DepKind,
    only_match_name: bool,
    explicit_name_in_toml: Option<InternedString>,

    optional: bool,
    public: bool,
    default_features: bool,
    features: Vec<InternedString>,
    // The presence of this information turns a dependency into an artifact dependency.
    artifact: Option<Artifact>,

    // This dependency should be used only for this platform.
    // `None` means *all platforms*.
    platform: Option<Platform>,
}

#[derive(Serialize)]
struct SerializedDependency<'a> {
    name: &'a str,
    source: SourceId,
    req: String,
    kind: DepKind,
    rename: Option<&'a str>,

    optional: bool,
    uses_default_features: bool,
    features: &'a [InternedString],
    #[serde(skip_serializing_if = "Option::is_none")]
    artifact: Option<&'a Artifact>,
    target: Option<&'a Platform>,
    /// The registry URL this dependency is from.
    /// If None, then it comes from the default registry (crates.io).
    registry: Option<&'a str>,

    /// The file system path for a local path dependency.
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<PathBuf>,
}

impl ser::Serialize for Dependency {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        let registry_id = self.registry_id();
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
            registry: registry_id.as_ref().map(|sid| sid.url().as_str()),
            path: self.source_id().local_path(),
            artifact: self.artifact(),
        }
        .serialize(s)
    }
}

#[derive(PartialEq, Eq, Hash, Ord, PartialOrd, Clone, Debug, Copy)]
pub enum DepKind {
    Normal,
    Development,
    Build,
}

impl DepKind {
    pub fn kind_table(&self) -> &'static str {
        match self {
            DepKind::Normal => "dependencies",
            DepKind::Development => "dev-dependencies",
            DepKind::Build => "build-dependencies",
        }
    }
}

impl ser::Serialize for DepKind {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        match *self {
            DepKind::Normal => None,
            DepKind::Development => Some("dev"),
            DepKind::Build => Some("build"),
        }
        .serialize(s)
    }
}

impl Dependency {
    /// Attempt to create a `Dependency` from an entry in the manifest.
    pub fn parse(
        name: impl Into<InternedString>,
        version: Option<&str>,
        source_id: SourceId,
    ) -> CargoResult<Dependency> {
        let name = name.into();
        let (specified_req, version_req) = match version {
            Some(v) => match VersionReq::parse(v) {
                Ok(req) => (true, OptVersionReq::Req(req)),
                Err(err) => {
                    return Err(anyhow::Error::new(err).context(format!(
                        "failed to parse the version requirement `{}` for dependency `{}`",
                        v, name,
                    )))
                }
            },
            None => (false, OptVersionReq::Any),
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

    pub fn new_override(name: InternedString, source_id: SourceId) -> Dependency {
        assert!(!name.is_empty());
        Dependency {
            inner: Rc::new(Inner {
                name,
                source_id,
                registry_id: None,
                req: OptVersionReq::Any,
                kind: DepKind::Normal,
                only_match_name: true,
                optional: false,
                public: false,
                features: Vec::new(),
                default_features: true,
                specified_req: false,
                platform: None,
                explicit_name_in_toml: None,
                artifact: None,
            }),
        }
    }

    pub fn version_req(&self) -> &OptVersionReq {
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

    pub fn kind(&self) -> DepKind {
        self.inner.kind
    }

    pub fn is_public(&self) -> bool {
        self.inner.public
    }

    /// Sets whether the dependency is public.
    pub fn set_public(&mut self, public: bool) -> &mut Dependency {
        if public {
            // Setting 'public' only makes sense for normal dependencies
            assert_eq!(self.kind(), DepKind::Normal);
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

    pub fn set_kind(&mut self, kind: DepKind) -> &mut Dependency {
        if self.is_public() {
            // Setting 'public' only makes sense for normal dependencies
            assert_eq!(kind, DepKind::Normal);
        }
        Rc::make_mut(&mut self.inner).kind = kind;
        self
    }

    /// Sets the list of features requested for the package.
    pub fn set_features(
        &mut self,
        features: impl IntoIterator<Item = impl Into<InternedString>>,
    ) -> &mut Dependency {
        Rc::make_mut(&mut self.inner).features = features.into_iter().map(|s| s.into()).collect();
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
    pub fn set_version_req(&mut self, req: OptVersionReq) -> &mut Dependency {
        Rc::make_mut(&mut self.inner).req = req;
        self
    }

    pub fn set_platform(&mut self, platform: Option<Platform>) -> &mut Dependency {
        Rc::make_mut(&mut self.inner).platform = platform;
        self
    }

    pub fn set_explicit_name_in_toml(
        &mut self,
        name: impl Into<InternedString>,
    ) -> &mut Dependency {
        Rc::make_mut(&mut self.inner).explicit_name_in_toml = Some(name.into());
        self
    }

    /// Locks this dependency to depending on the specified package ID.
    pub fn lock_to(&mut self, id: PackageId) -> &mut Dependency {
        assert_eq!(self.inner.source_id, id.source_id());
        trace!(
            "locking dep from `{}` with `{}` at {} to {}",
            self.package_name(),
            self.version_req(),
            self.source_id(),
            id
        );
        let me = Rc::make_mut(&mut self.inner);
        me.req.lock_to(id.version());

        // Only update the `precise` of this source to preserve other
        // information about dependency's source which may not otherwise be
        // tested during equality/hashing.
        me.source_id = me.source_id.with_precise_from(id.source_id());
        self
    }

    /// Locks this dependency to a specified version.
    ///
    /// Mainly used in dependency patching like `[patch]` or `[replace]`, which
    /// doesn't need to lock the entire dependency to a specific [`PackageId`].
    pub fn lock_version(&mut self, version: &semver::Version) -> &mut Dependency {
        let me = Rc::make_mut(&mut self.inner);
        me.req.lock_to(version);
        self
    }

    /// Returns `true` if this is a "locked" dependency. Basically a locked
    /// dependency has an exact version req, but not vice versa.
    pub fn is_locked(&self) -> bool {
        self.inner.req.is_locked()
    }

    /// Returns `false` if the dependency is only used to build the local package.
    pub fn is_transitive(&self) -> bool {
        match self.inner.kind {
            DepKind::Normal | DepKind::Build => true,
            DepKind::Development => false,
        }
    }

    pub fn is_build(&self) -> bool {
        matches!(self.inner.kind, DepKind::Build)
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
        if self.source_id() == to_replace {
            self.set_source_id(replace_with);
        }
        self
    }

    pub(crate) fn set_artifact(&mut self, artifact: Artifact) {
        Rc::make_mut(&mut self.inner).artifact = Some(artifact);
    }

    pub(crate) fn artifact(&self) -> Option<&Artifact> {
        self.inner.artifact.as_ref()
    }

    /// Dependencies are potential rust libs if they are not artifacts or they are an
    /// artifact which allows to be seen as library.
    /// Previously, every dependency was potentially seen as library.
    pub(crate) fn maybe_lib(&self) -> bool {
        self.artifact().map(|a| a.is_lib).unwrap_or(true)
    }
}

/// The presence of an artifact turns an ordinary dependency into an Artifact dependency.
/// As such, it will build one or more different artifacts of possibly various kinds
/// for making them available at build time for rustc invocations or runtime
/// for build scripts.
///
/// This information represents a requirement in the package this dependency refers to.
#[derive(PartialEq, Eq, Hash, Clone, Debug)]
pub struct Artifact {
    inner: Rc<Vec<ArtifactKind>>,
    is_lib: bool,
    target: Option<ArtifactTarget>,
}

#[derive(Serialize)]
pub struct SerializedArtifact<'a> {
    kinds: &'a [ArtifactKind],
    lib: bool,
    target: Option<&'a str>,
}

impl ser::Serialize for Artifact {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        SerializedArtifact {
            kinds: self.kinds(),
            lib: self.is_lib,
            target: self.target.as_ref().map(ArtifactTarget::as_str),
        }
        .serialize(s)
    }
}

impl Artifact {
    pub(crate) fn parse(
        artifacts: &[impl AsRef<str>],
        is_lib: bool,
        target: Option<&str>,
    ) -> CargoResult<Self> {
        let kinds = ArtifactKind::validate(
            artifacts
                .iter()
                .map(|s| ArtifactKind::parse(s.as_ref()))
                .collect::<Result<Vec<_>, _>>()?,
        )?;
        Ok(Artifact {
            inner: Rc::new(kinds),
            is_lib,
            target: target.map(ArtifactTarget::parse).transpose()?,
        })
    }

    pub(crate) fn kinds(&self) -> &[ArtifactKind] {
        &self.inner
    }

    pub(crate) fn is_lib(&self) -> bool {
        self.is_lib
    }

    pub(crate) fn target(&self) -> Option<ArtifactTarget> {
        self.target
    }
}

#[derive(PartialEq, Eq, Hash, Copy, Clone, Ord, PartialOrd, Debug)]
pub enum ArtifactTarget {
    /// Only applicable to build-dependencies, causing them to be built
    /// for the given target (i.e. via `--target <triple>`) instead of for the host.
    /// Has no effect on non-build dependencies.
    BuildDependencyAssumeTarget,
    /// The name of the platform triple, like `x86_64-apple-darwin`, that this
    /// artifact will always be built for, no matter if it is a build,
    /// normal or dev dependency.
    Force(CompileTarget),
}

impl ArtifactTarget {
    pub fn parse(target: &str) -> CargoResult<ArtifactTarget> {
        Ok(match target {
            "target" => ArtifactTarget::BuildDependencyAssumeTarget,
            name => ArtifactTarget::Force(CompileTarget::new(name)?),
        })
    }

    pub fn as_str(&self) -> &str {
        match self {
            ArtifactTarget::BuildDependencyAssumeTarget => "target",
            ArtifactTarget::Force(target) => target.rustc_target().as_str(),
        }
    }

    pub fn to_compile_kind(&self) -> Option<CompileKind> {
        self.to_compile_target().map(CompileKind::Target)
    }

    pub fn to_compile_target(&self) -> Option<CompileTarget> {
        match self {
            ArtifactTarget::BuildDependencyAssumeTarget => None,
            ArtifactTarget::Force(target) => Some(*target),
        }
    }

    pub(crate) fn to_resolved_compile_kind(
        &self,
        root_unit_compile_kind: CompileKind,
    ) -> CompileKind {
        match self {
            ArtifactTarget::Force(target) => CompileKind::Target(*target),
            ArtifactTarget::BuildDependencyAssumeTarget => root_unit_compile_kind,
        }
    }

    pub(crate) fn to_resolved_compile_target(
        &self,
        root_unit_compile_kind: CompileKind,
    ) -> Option<CompileTarget> {
        match self.to_resolved_compile_kind(root_unit_compile_kind) {
            CompileKind::Host => None,
            CompileKind::Target(target) => Some(target),
        }
    }
}

#[derive(PartialEq, Eq, Hash, Copy, Clone, Ord, PartialOrd, Debug)]
pub enum ArtifactKind {
    /// We represent all binaries in this dependency
    AllBinaries,
    /// We represent a single binary
    SelectedBinary(InternedString),
    Cdylib,
    Staticlib,
}

impl ser::Serialize for ArtifactKind {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        self.as_str().serialize(s)
    }
}

impl fmt::Display for ArtifactKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.as_str())
    }
}

impl ArtifactKind {
    /// Returns a string of crate type of the artifact being built.
    ///
    /// Note that the name of `SelectedBinary` would be dropped and displayed as `bin`.
    pub fn crate_type(&self) -> &'static str {
        match self {
            ArtifactKind::AllBinaries | ArtifactKind::SelectedBinary(_) => "bin",
            ArtifactKind::Cdylib => "cdylib",
            ArtifactKind::Staticlib => "staticlib",
        }
    }

    pub fn as_str(&self) -> Cow<'static, str> {
        match *self {
            ArtifactKind::SelectedBinary(name) => format!("bin:{}", name.as_str()).into(),
            _ => self.crate_type().into(),
        }
    }

    pub fn parse(kind: &str) -> CargoResult<Self> {
        Ok(match kind {
            "bin" => ArtifactKind::AllBinaries,
            "cdylib" => ArtifactKind::Cdylib,
            "staticlib" => ArtifactKind::Staticlib,
            _ => {
                return kind
                    .strip_prefix("bin:")
                    .map(|bin_name| ArtifactKind::SelectedBinary(InternedString::new(bin_name)))
                    .ok_or_else(|| anyhow::anyhow!("'{}' is not a valid artifact specifier", kind))
            }
        })
    }

    fn validate(kinds: Vec<ArtifactKind>) -> CargoResult<Vec<ArtifactKind>> {
        if kinds.iter().any(|k| matches!(k, ArtifactKind::AllBinaries))
            && kinds
                .iter()
                .any(|k| matches!(k, ArtifactKind::SelectedBinary(_)))
        {
            anyhow::bail!("Cannot specify both 'bin' and 'bin:<name>' binary artifacts, as 'bin' selects all available binaries.");
        }
        let mut kinds_without_dupes = kinds.clone();
        kinds_without_dupes.sort();
        kinds_without_dupes.dedup();
        let num_dupes = kinds.len() - kinds_without_dupes.len();
        if num_dupes != 0 {
            anyhow::bail!(
                "Found {} duplicate binary artifact{}",
                num_dupes,
                (num_dupes > 1).then(|| "s").unwrap_or("")
            );
        }
        Ok(kinds)
    }
}
