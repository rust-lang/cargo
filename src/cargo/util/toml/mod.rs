use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::str;

use anyhow::{anyhow, bail, Context as _};
use cargo_platform::Platform;
use cargo_util::paths;
use log::{debug, trace};
use semver::{self, VersionReq};
use serde::de;
use serde::ser;
use serde::{Deserialize, Serialize};
use toml_edit::easy as toml;
use url::Url;

use crate::core::compiler::{CompileKind, CompileTarget};
use crate::core::dependency::{Artifact, ArtifactTarget, DepKind};
use crate::core::manifest::{ManifestMetadata, TargetSourcePath, Warnings};
use crate::core::resolver::ResolveBehavior;
use crate::core::{Dependency, Manifest, PackageId, Summary, Target};
use crate::core::{Edition, EitherManifest, Feature, Features, VirtualManifest, Workspace};
use crate::core::{GitReference, PackageIdSpec, SourceId, WorkspaceConfig, WorkspaceRootConfig};
use crate::sources::{CRATES_IO_INDEX, CRATES_IO_REGISTRY};
use crate::util::errors::{CargoResult, ManifestError};
use crate::util::interning::InternedString;
use crate::util::{
    self, config::ConfigRelativePath, validate_package_name, Config, IntoUrl, VersionReqExt,
};

mod targets;
use self::targets::targets;

/// Loads a `Cargo.toml` from a file on disk.
///
/// This could result in a real or virtual manifest being returned.
///
/// A list of nested paths is also returned, one for each path dependency
/// within the manifest. For virtual manifests, these paths can only
/// come from patched or replaced dependencies. These paths are not
/// canonicalized.
pub fn read_manifest(
    path: &Path,
    source_id: SourceId,
    config: &Config,
) -> Result<(EitherManifest, Vec<PathBuf>), ManifestError> {
    trace!(
        "read_manifest; path={}; source-id={}",
        path.display(),
        source_id
    );
    let contents = paths::read(path).map_err(|err| ManifestError::new(err, path.into()))?;

    read_manifest_from_str(&contents, path, source_id, config)
        .with_context(|| format!("failed to parse manifest at `{}`", path.display()))
        .map_err(|err| ManifestError::new(err, path.into()))
}

/// Parse an already-loaded `Cargo.toml` as a Cargo manifest.
///
/// This could result in a real or virtual manifest being returned.
///
/// A list of nested paths is also returned, one for each path dependency
/// within the manifest. For virtual manifests, these paths can only
/// come from patched or replaced dependencies. These paths are not
/// canonicalized.
pub fn read_manifest_from_str(
    contents: &str,
    manifest_file: &Path,
    source_id: SourceId,
    config: &Config,
) -> CargoResult<(EitherManifest, Vec<PathBuf>)> {
    let package_root = manifest_file.parent().unwrap();

    let toml = {
        let pretty_filename = manifest_file
            .strip_prefix(config.cwd())
            .unwrap_or(manifest_file);
        parse_document(contents, pretty_filename, config)?
    };

    // Provide a helpful error message for a common user error.
    if let Some(package) = toml.get("package").or_else(|| toml.get("project")) {
        if let Some(feats) = package.get("cargo-features") {
            let mut feats = feats.clone();
            if let Some(value) = feats.as_value_mut() {
                // Only keep formatting inside of the `[]` and not formatting around it
                value.decor_mut().clear();
            }
            bail!(
                "cargo-features = {} was found in the wrong location: it \
                 should be set at the top of Cargo.toml before any tables",
                feats.to_string()
            );
        }
    }

    let mut unused = BTreeSet::new();
    let manifest: TomlManifest = serde_ignored::deserialize(toml, |path| {
        let mut key = String::new();
        stringify(&mut key, &path);
        unused.insert(key);
    })?;
    let add_unused = |warnings: &mut Warnings| {
        for key in unused {
            warnings.add_warning(format!("unused manifest key: {}", key));
            if key == "profiles.debug" {
                warnings.add_warning("use `[profile.dev]` to configure debug builds".to_string());
            }
        }
    };

    let manifest = Rc::new(manifest);
    return if manifest.project.is_some() || manifest.package.is_some() {
        let (mut manifest, paths) =
            TomlManifest::to_real_manifest(&manifest, source_id, package_root, config)?;
        add_unused(manifest.warnings_mut());
        if manifest.targets().iter().all(|t| t.is_custom_build()) {
            bail!(
                "no targets specified in the manifest\n\
                 either src/lib.rs, src/main.rs, a [lib] section, or \
                 [[bin]] section must be present"
            )
        }
        Ok((EitherManifest::Real(manifest), paths))
    } else {
        let (mut m, paths) =
            TomlManifest::to_virtual_manifest(&manifest, source_id, package_root, config)?;
        add_unused(m.warnings_mut());
        Ok((EitherManifest::Virtual(m), paths))
    };

    fn stringify(dst: &mut String, path: &serde_ignored::Path<'_>) {
        use serde_ignored::Path;

        match *path {
            Path::Root => {}
            Path::Seq { parent, index } => {
                stringify(dst, parent);
                if !dst.is_empty() {
                    dst.push('.');
                }
                dst.push_str(&index.to_string());
            }
            Path::Map { parent, ref key } => {
                stringify(dst, parent);
                if !dst.is_empty() {
                    dst.push('.');
                }
                dst.push_str(key);
            }
            Path::Some { parent }
            | Path::NewtypeVariant { parent }
            | Path::NewtypeStruct { parent } => stringify(dst, parent),
        }
    }
}

/// Attempts to parse a string into a [`toml::Value`]. This is not specific to any
/// particular kind of TOML file.
///
/// The purpose of this wrapper is to detect invalid TOML which was previously
/// accepted and display a warning to the user in that case. The `file` and `config`
/// parameters are only used by this fallback path.
pub fn parse(toml: &str, _file: &Path, _config: &Config) -> CargoResult<toml::Value> {
    // At the moment, no compatibility checks are needed.
    toml.parse()
        .map_err(|e| anyhow::Error::from(e).context("could not parse input as TOML"))
}

pub fn parse_document(
    toml: &str,
    _file: &Path,
    _config: &Config,
) -> CargoResult<toml_edit::Document> {
    // At the moment, no compatibility checks are needed.
    toml.parse()
        .map_err(|e| anyhow::Error::from(e).context("could not parse input as TOML"))
}

type TomlLibTarget = TomlTarget;
type TomlBinTarget = TomlTarget;
type TomlExampleTarget = TomlTarget;
type TomlTestTarget = TomlTarget;
type TomlBenchTarget = TomlTarget;

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum TomlDependency<P = String> {
    /// In the simple format, only a version is specified, eg.
    /// `package = "<version>"`
    Simple(String),
    /// The simple format is equivalent to a detailed dependency
    /// specifying only a version, eg.
    /// `package = { version = "<version>" }`
    Detailed(DetailedTomlDependency<P>),
}

impl<'de, P: Deserialize<'de>> de::Deserialize<'de> for TomlDependency<P> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct TomlDependencyVisitor<P>(PhantomData<P>);

        impl<'de, P: Deserialize<'de>> de::Visitor<'de> for TomlDependencyVisitor<P> {
            type Value = TomlDependency<P>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(
                    "a version string like \"0.9.8\" or a \
                     detailed dependency like { version = \"0.9.8\" }",
                )
            }

            fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(TomlDependency::Simple(s.to_owned()))
            }

            fn visit_map<V>(self, map: V) -> Result<Self::Value, V::Error>
            where
                V: de::MapAccess<'de>,
            {
                let mvd = de::value::MapAccessDeserializer::new(map);
                DetailedTomlDependency::deserialize(mvd).map(TomlDependency::Detailed)
            }
        }

        deserializer.deserialize_any(TomlDependencyVisitor(PhantomData))
    }
}

pub trait ResolveToPath {
    fn resolve(&self, config: &Config) -> PathBuf;
}

impl ResolveToPath for String {
    fn resolve(&self, _: &Config) -> PathBuf {
        self.into()
    }
}

impl ResolveToPath for ConfigRelativePath {
    fn resolve(&self, c: &Config) -> PathBuf {
        self.resolve_path(c)
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct DetailedTomlDependency<P = String> {
    version: Option<String>,
    registry: Option<String>,
    /// The URL of the `registry` field.
    /// This is an internal implementation detail. When Cargo creates a
    /// package, it replaces `registry` with `registry-index` so that the
    /// manifest contains the correct URL. All users won't have the same
    /// registry names configured, so Cargo can't rely on just the name for
    /// crates published by other users.
    registry_index: Option<String>,
    // `path` is relative to the file it appears in. If that's a `Cargo.toml`, it'll be relative to
    // that TOML file, and if it's a `.cargo/config` file, it'll be relative to that file.
    path: Option<P>,
    git: Option<String>,
    branch: Option<String>,
    tag: Option<String>,
    rev: Option<String>,
    features: Option<Vec<String>>,
    optional: Option<bool>,
    default_features: Option<bool>,
    #[serde(rename = "default_features")]
    default_features2: Option<bool>,
    package: Option<String>,
    public: Option<bool>,

    /// One ore more of 'bin', 'cdylib', 'staticlib', 'bin:<name>'.
    artifact: Option<StringOrVec>,
    /// If set, the artifact should also be a dependency
    lib: Option<bool>,
    /// A platform name, like `x86_64-apple-darwin`
    target: Option<String>,
}

// Explicit implementation so we avoid pulling in P: Default
impl<P> Default for DetailedTomlDependency<P> {
    fn default() -> Self {
        Self {
            version: Default::default(),
            registry: Default::default(),
            registry_index: Default::default(),
            path: Default::default(),
            git: Default::default(),
            branch: Default::default(),
            tag: Default::default(),
            rev: Default::default(),
            features: Default::default(),
            optional: Default::default(),
            default_features: Default::default(),
            default_features2: Default::default(),
            package: Default::default(),
            public: Default::default(),
            artifact: Default::default(),
            lib: Default::default(),
            target: Default::default(),
        }
    }
}

/// This type is used to deserialize `Cargo.toml` files.
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct TomlManifest {
    cargo_features: Option<Vec<String>>,
    package: Option<Box<TomlProject>>,
    project: Option<Box<TomlProject>>,
    profile: Option<TomlProfiles>,
    lib: Option<TomlLibTarget>,
    bin: Option<Vec<TomlBinTarget>>,
    example: Option<Vec<TomlExampleTarget>>,
    test: Option<Vec<TomlTestTarget>>,
    bench: Option<Vec<TomlTestTarget>>,
    dependencies: Option<BTreeMap<String, TomlDependency>>,
    dev_dependencies: Option<BTreeMap<String, TomlDependency>>,
    #[serde(rename = "dev_dependencies")]
    dev_dependencies2: Option<BTreeMap<String, TomlDependency>>,
    build_dependencies: Option<BTreeMap<String, TomlDependency>>,
    #[serde(rename = "build_dependencies")]
    build_dependencies2: Option<BTreeMap<String, TomlDependency>>,
    features: Option<BTreeMap<InternedString, Vec<InternedString>>>,
    target: Option<BTreeMap<String, TomlPlatform>>,
    replace: Option<BTreeMap<String, TomlDependency>>,
    patch: Option<BTreeMap<String, BTreeMap<String, TomlDependency>>>,
    workspace: Option<TomlWorkspace>,
    badges: Option<BTreeMap<String, BTreeMap<String, String>>>,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default)]
pub struct TomlProfiles(BTreeMap<InternedString, TomlProfile>);

impl TomlProfiles {
    pub fn get_all(&self) -> &BTreeMap<InternedString, TomlProfile> {
        &self.0
    }

    pub fn get(&self, name: &str) -> Option<&TomlProfile> {
        self.0.get(name)
    }

    pub fn validate(&self, features: &Features, warnings: &mut Vec<String>) -> CargoResult<()> {
        for (name, profile) in &self.0 {
            profile.validate(name, features, warnings)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TomlOptLevel(pub String);

impl<'de> de::Deserialize<'de> for TomlOptLevel {
    fn deserialize<D>(d: D) -> Result<TomlOptLevel, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = TomlOptLevel;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an optimization level")
            }

            fn visit_i64<E>(self, value: i64) -> Result<TomlOptLevel, E>
            where
                E: de::Error,
            {
                Ok(TomlOptLevel(value.to_string()))
            }

            fn visit_str<E>(self, value: &str) -> Result<TomlOptLevel, E>
            where
                E: de::Error,
            {
                if value == "s" || value == "z" {
                    Ok(TomlOptLevel(value.to_string()))
                } else {
                    Err(E::custom(format!(
                        "must be `0`, `1`, `2`, `3`, `s` or `z`, \
                         but found the string: \"{}\"",
                        value
                    )))
                }
            }
        }

        d.deserialize_any(Visitor)
    }
}

impl ser::Serialize for TomlOptLevel {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        match self.0.parse::<u32>() {
            Ok(n) => n.serialize(serializer),
            Err(_) => self.0.serialize(serializer),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(untagged, expecting = "expected a boolean or an integer")]
pub enum U32OrBool {
    U32(u32),
    Bool(bool),
}

#[derive(Deserialize, Serialize, Clone, Debug, Default, Eq, PartialEq)]
#[serde(default, rename_all = "kebab-case")]
pub struct TomlProfile {
    pub opt_level: Option<TomlOptLevel>,
    pub lto: Option<StringOrBool>,
    pub codegen_backend: Option<InternedString>,
    pub codegen_units: Option<u32>,
    pub debug: Option<U32OrBool>,
    pub split_debuginfo: Option<String>,
    pub debug_assertions: Option<bool>,
    pub rpath: Option<bool>,
    pub panic: Option<String>,
    pub overflow_checks: Option<bool>,
    pub incremental: Option<bool>,
    pub dir_name: Option<InternedString>,
    pub inherits: Option<InternedString>,
    pub strip: Option<StringOrBool>,
    // Note that `rustflags` is used for the cargo-feature `profile_rustflags`
    pub rustflags: Option<Vec<InternedString>>,
    // These two fields must be last because they are sub-tables, and TOML
    // requires all non-tables to be listed first.
    pub package: Option<BTreeMap<ProfilePackageSpec, TomlProfile>>,
    pub build_override: Option<Box<TomlProfile>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Ord, PartialOrd, Hash)]
pub enum ProfilePackageSpec {
    Spec(PackageIdSpec),
    All,
}

impl ser::Serialize for ProfilePackageSpec {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        self.to_string().serialize(s)
    }
}

impl<'de> de::Deserialize<'de> for ProfilePackageSpec {
    fn deserialize<D>(d: D) -> Result<ProfilePackageSpec, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let string = String::deserialize(d)?;
        if string == "*" {
            Ok(ProfilePackageSpec::All)
        } else {
            PackageIdSpec::parse(&string)
                .map_err(de::Error::custom)
                .map(ProfilePackageSpec::Spec)
        }
    }
}

impl fmt::Display for ProfilePackageSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProfilePackageSpec::Spec(spec) => spec.fmt(f),
            ProfilePackageSpec::All => f.write_str("*"),
        }
    }
}

impl TomlProfile {
    pub fn validate(
        &self,
        name: &str,
        features: &Features,
        warnings: &mut Vec<String>,
    ) -> CargoResult<()> {
        self.validate_profile(name, features)?;
        if let Some(ref profile) = self.build_override {
            profile.validate_override("build-override")?;
            profile.validate_profile(&format!("{name}.build-override"), features)?;
        }
        if let Some(ref packages) = self.package {
            for (override_name, profile) in packages {
                profile.validate_override("package")?;
                profile.validate_profile(&format!("{name}.package.{override_name}"), features)?;
            }
        }

        // Profile name validation
        Self::validate_name(name)?;

        if let Some(dir_name) = self.dir_name {
            // This is disabled for now, as we would like to stabilize named
            // profiles without this, and then decide in the future if it is
            // needed. This helps simplify the UI a little.
            bail!(
                "dir-name=\"{}\" in profile `{}` is not currently allowed, \
                 directory names are tied to the profile name for custom profiles",
                dir_name,
                name
            );
        }

        // `inherits` validation
        if matches!(self.inherits.map(|s| s.as_str()), Some("debug")) {
            bail!(
                "profile.{}.inherits=\"debug\" should be profile.{}.inherits=\"dev\"",
                name,
                name
            );
        }

        match name {
            "doc" => {
                warnings.push("profile `doc` is deprecated and has no effect".to_string());
            }
            "test" | "bench" => {
                if self.panic.is_some() {
                    warnings.push(format!("`panic` setting is ignored for `{}` profile", name))
                }
            }
            _ => {}
        }

        if let Some(panic) = &self.panic {
            if panic != "unwind" && panic != "abort" {
                bail!(
                    "`panic` setting of `{}` is not a valid setting, \
                     must be `unwind` or `abort`",
                    panic
                );
            }
        }

        Ok(())
    }

    /// Validate dir-names and profile names according to RFC 2678.
    pub fn validate_name(name: &str) -> CargoResult<()> {
        if let Some(ch) = name
            .chars()
            .find(|ch| !ch.is_alphanumeric() && *ch != '_' && *ch != '-')
        {
            bail!(
                "invalid character `{}` in profile name `{}`\n\
                Allowed characters are letters, numbers, underscore, and hyphen.",
                ch,
                name
            );
        }

        const SEE_DOCS: &str = "See https://doc.rust-lang.org/cargo/reference/profiles.html \
            for more on configuring profiles.";

        let lower_name = name.to_lowercase();
        if lower_name == "debug" {
            bail!(
                "profile name `{}` is reserved\n\
                 To configure the default development profile, use the name `dev` \
                 as in [profile.dev]\n\
                {}",
                name,
                SEE_DOCS
            );
        }
        if lower_name == "build-override" {
            bail!(
                "profile name `{}` is reserved\n\
                 To configure build dependency settings, use [profile.dev.build-override] \
                 and [profile.release.build-override]\n\
                 {}",
                name,
                SEE_DOCS
            );
        }

        // These are some arbitrary reservations. We have no plans to use
        // these, but it seems safer to reserve a few just in case we want to
        // add more built-in profiles in the future. We can also uses special
        // syntax like cargo:foo if needed. But it is unlikely these will ever
        // be used.
        if matches!(
            lower_name.as_str(),
            "build"
                | "check"
                | "clean"
                | "config"
                | "fetch"
                | "fix"
                | "install"
                | "metadata"
                | "package"
                | "publish"
                | "report"
                | "root"
                | "run"
                | "rust"
                | "rustc"
                | "rustdoc"
                | "target"
                | "tmp"
                | "uninstall"
        ) || lower_name.starts_with("cargo")
        {
            bail!(
                "profile name `{}` is reserved\n\
                 Please choose a different name.\n\
                 {}",
                name,
                SEE_DOCS
            );
        }

        Ok(())
    }

    /// Validates a profile.
    ///
    /// This is a shallow check, which is reused for the profile itself and any overrides.
    fn validate_profile(&self, name: &str, features: &Features) -> CargoResult<()> {
        if let Some(codegen_backend) = &self.codegen_backend {
            features.require(Feature::codegen_backend())?;
            if codegen_backend.contains(|c: char| !c.is_ascii_alphanumeric() && c != '_') {
                bail!(
                    "`profile.{}.codegen-backend` setting of `{}` is not a valid backend name.",
                    name,
                    codegen_backend,
                );
            }
        }
        if self.rustflags.is_some() {
            features.require(Feature::profile_rustflags())?;
        }
        Ok(())
    }

    /// Validation that is specific to an override.
    fn validate_override(&self, which: &str) -> CargoResult<()> {
        if self.package.is_some() {
            bail!("package-specific profiles cannot be nested");
        }
        if self.build_override.is_some() {
            bail!("build-override profiles cannot be nested");
        }
        if self.panic.is_some() {
            bail!("`panic` may not be specified in a `{}` profile", which)
        }
        if self.lto.is_some() {
            bail!("`lto` may not be specified in a `{}` profile", which)
        }
        if self.rpath.is_some() {
            bail!("`rpath` may not be specified in a `{}` profile", which)
        }
        Ok(())
    }

    /// Overwrite self's values with the given profile.
    pub fn merge(&mut self, profile: &TomlProfile) {
        if let Some(v) = &profile.opt_level {
            self.opt_level = Some(v.clone());
        }

        if let Some(v) = &profile.lto {
            self.lto = Some(v.clone());
        }

        if let Some(v) = profile.codegen_backend {
            self.codegen_backend = Some(v);
        }

        if let Some(v) = profile.codegen_units {
            self.codegen_units = Some(v);
        }

        if let Some(v) = &profile.debug {
            self.debug = Some(v.clone());
        }

        if let Some(v) = profile.debug_assertions {
            self.debug_assertions = Some(v);
        }

        if let Some(v) = &profile.split_debuginfo {
            self.split_debuginfo = Some(v.clone());
        }

        if let Some(v) = profile.rpath {
            self.rpath = Some(v);
        }

        if let Some(v) = &profile.panic {
            self.panic = Some(v.clone());
        }

        if let Some(v) = profile.overflow_checks {
            self.overflow_checks = Some(v);
        }

        if let Some(v) = profile.incremental {
            self.incremental = Some(v);
        }

        if let Some(v) = &profile.rustflags {
            self.rustflags = Some(v.clone());
        }

        if let Some(other_package) = &profile.package {
            match &mut self.package {
                Some(self_package) => {
                    for (spec, other_pkg_profile) in other_package {
                        match self_package.get_mut(spec) {
                            Some(p) => p.merge(other_pkg_profile),
                            None => {
                                self_package.insert(spec.clone(), other_pkg_profile.clone());
                            }
                        }
                    }
                }
                None => self.package = Some(other_package.clone()),
            }
        }

        if let Some(other_bo) = &profile.build_override {
            match &mut self.build_override {
                Some(self_bo) => self_bo.merge(other_bo),
                None => self.build_override = Some(other_bo.clone()),
            }
        }

        if let Some(v) = &profile.inherits {
            self.inherits = Some(*v);
        }

        if let Some(v) = &profile.dir_name {
            self.dir_name = Some(*v);
        }

        if let Some(v) = &profile.strip {
            self.strip = Some(v.clone());
        }
    }
}

/// A StringOrVec can be parsed from either a TOML string or array,
/// but is always stored as a vector.
#[derive(Clone, Debug, Serialize, Eq, PartialEq, PartialOrd, Ord)]
pub struct StringOrVec(Vec<String>);

impl<'de> de::Deserialize<'de> for StringOrVec {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = StringOrVec;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("string or list of strings")
            }

            fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(StringOrVec(vec![s.to_string()]))
            }

            fn visit_seq<V>(self, v: V) -> Result<Self::Value, V::Error>
            where
                V: de::SeqAccess<'de>,
            {
                let seq = de::value::SeqAccessDeserializer::new(v);
                Vec::deserialize(seq).map(StringOrVec)
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}

impl StringOrVec {
    pub fn iter<'a>(&'a self) -> std::slice::Iter<'a, String> {
        self.0.iter()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(untagged, expecting = "expected a boolean or a string")]
pub enum StringOrBool {
    String(String),
    Bool(bool),
}

#[derive(PartialEq, Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum VecStringOrBool {
    VecString(Vec<String>),
    Bool(bool),
}

impl<'de> de::Deserialize<'de> for VecStringOrBool {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = VecStringOrBool;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a boolean or vector of strings")
            }

            fn visit_seq<V>(self, v: V) -> Result<Self::Value, V::Error>
            where
                V: de::SeqAccess<'de>,
            {
                let seq = de::value::SeqAccessDeserializer::new(v);
                Vec::deserialize(seq).map(VecStringOrBool::VecString)
            }

            fn visit_bool<E>(self, b: bool) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(VecStringOrBool::Bool(b))
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}

fn version_trim_whitespace<'de, D>(deserializer: D) -> Result<semver::Version, D::Error>
where
    D: de::Deserializer<'de>,
{
    struct Visitor;

    impl<'de> de::Visitor<'de> for Visitor {
        type Value = semver::Version;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("SemVer version")
        }

        fn visit_str<E>(self, string: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            string.trim().parse().map_err(de::Error::custom)
        }
    }

    deserializer.deserialize_str(Visitor)
}

/// Represents the `package`/`project` sections of a `Cargo.toml`.
///
/// Note that the order of the fields matters, since this is the order they
/// are serialized to a TOML file. For example, you cannot have values after
/// the field `metadata`, since it is a table and values cannot appear after
/// tables.
#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
pub struct TomlProject {
    edition: Option<String>,
    rust_version: Option<String>,
    name: InternedString,
    #[serde(deserialize_with = "version_trim_whitespace")]
    version: semver::Version,
    authors: Option<Vec<String>>,
    build: Option<StringOrBool>,
    metabuild: Option<StringOrVec>,
    #[serde(rename = "default-target")]
    default_target: Option<String>,
    #[serde(rename = "forced-target")]
    forced_target: Option<String>,
    links: Option<String>,
    exclude: Option<Vec<String>>,
    include: Option<Vec<String>>,
    publish: Option<VecStringOrBool>,
    workspace: Option<String>,
    im_a_teapot: Option<bool>,
    autobins: Option<bool>,
    autoexamples: Option<bool>,
    autotests: Option<bool>,
    autobenches: Option<bool>,
    default_run: Option<String>,

    // Package metadata.
    description: Option<String>,
    homepage: Option<String>,
    documentation: Option<String>,
    readme: Option<StringOrBool>,
    keywords: Option<Vec<String>>,
    categories: Option<Vec<String>>,
    license: Option<String>,
    license_file: Option<String>,
    repository: Option<String>,
    resolver: Option<String>,

    // Note that this field must come last due to the way toml serialization
    // works which requires tables to be emitted after all values.
    metadata: Option<toml::Value>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TomlWorkspace {
    members: Option<Vec<String>>,
    #[serde(rename = "default-members")]
    default_members: Option<Vec<String>>,
    exclude: Option<Vec<String>>,
    resolver: Option<String>,

    // Note that this field must come last due to the way toml serialization
    // works which requires tables to be emitted after all values.
    metadata: Option<toml::Value>,
}

impl TomlProject {
    pub fn to_package_id(&self, source_id: SourceId) -> CargoResult<PackageId> {
        PackageId::new(self.name, self.version.clone(), source_id)
    }
}

struct Context<'a, 'b> {
    deps: &'a mut Vec<Dependency>,
    source_id: SourceId,
    nested_paths: &'a mut Vec<PathBuf>,
    config: &'b Config,
    warnings: &'a mut Vec<String>,
    platform: Option<Platform>,
    root: &'a Path,
    features: &'a Features,
}

impl TomlManifest {
    /// Prepares the manifest for publishing.
    // - Path and git components of dependency specifications are removed.
    // - License path is updated to point within the package.
    pub fn prepare_for_publish(
        &self,
        ws: &Workspace<'_>,
        package_root: &Path,
    ) -> CargoResult<TomlManifest> {
        let config = ws.config();
        let mut package = self
            .package
            .as_ref()
            .or_else(|| self.project.as_ref())
            .unwrap()
            .clone();
        package.workspace = None;
        package.resolver = ws.resolve_behavior().to_manifest();
        if let Some(license_file) = &package.license_file {
            let license_path = Path::new(&license_file);
            let abs_license_path = paths::normalize_path(&package_root.join(license_path));
            if abs_license_path.strip_prefix(package_root).is_err() {
                // This path points outside of the package root. `cargo package`
                // will copy it into the root, so adjust the path to this location.
                package.license_file = Some(
                    license_path
                        .file_name()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .to_string(),
                );
            }
        }
        let all = |_d: &TomlDependency| true;
        return Ok(TomlManifest {
            package: Some(package),
            project: None,
            profile: self.profile.clone(),
            lib: self.lib.clone(),
            bin: self.bin.clone(),
            example: self.example.clone(),
            test: self.test.clone(),
            bench: self.bench.clone(),
            dependencies: map_deps(config, self.dependencies.as_ref(), all)?,
            dev_dependencies: map_deps(
                config,
                self.dev_dependencies
                    .as_ref()
                    .or_else(|| self.dev_dependencies2.as_ref()),
                TomlDependency::is_version_specified,
            )?,
            dev_dependencies2: None,
            build_dependencies: map_deps(
                config,
                self.build_dependencies
                    .as_ref()
                    .or_else(|| self.build_dependencies2.as_ref()),
                all,
            )?,
            build_dependencies2: None,
            features: self.features.clone(),
            target: match self.target.as_ref().map(|target_map| {
                target_map
                    .iter()
                    .map(|(k, v)| {
                        Ok((
                            k.clone(),
                            TomlPlatform {
                                dependencies: map_deps(config, v.dependencies.as_ref(), all)?,
                                dev_dependencies: map_deps(
                                    config,
                                    v.dev_dependencies
                                        .as_ref()
                                        .or_else(|| v.dev_dependencies2.as_ref()),
                                    TomlDependency::is_version_specified,
                                )?,
                                dev_dependencies2: None,
                                build_dependencies: map_deps(
                                    config,
                                    v.build_dependencies
                                        .as_ref()
                                        .or_else(|| v.build_dependencies2.as_ref()),
                                    all,
                                )?,
                                build_dependencies2: None,
                            },
                        ))
                    })
                    .collect()
            }) {
                Some(Ok(v)) => Some(v),
                Some(Err(e)) => return Err(e),
                None => None,
            },
            replace: None,
            patch: None,
            workspace: None,
            badges: self.badges.clone(),
            cargo_features: self.cargo_features.clone(),
        });

        fn map_deps(
            config: &Config,
            deps: Option<&BTreeMap<String, TomlDependency>>,
            filter: impl Fn(&TomlDependency) -> bool,
        ) -> CargoResult<Option<BTreeMap<String, TomlDependency>>> {
            let deps = match deps {
                Some(deps) => deps,
                None => return Ok(None),
            };
            let deps = deps
                .iter()
                .filter(|(_k, v)| filter(v))
                .map(|(k, v)| Ok((k.clone(), map_dependency(config, v)?)))
                .collect::<CargoResult<BTreeMap<_, _>>>()?;
            Ok(Some(deps))
        }

        fn map_dependency(config: &Config, dep: &TomlDependency) -> CargoResult<TomlDependency> {
            match dep {
                TomlDependency::Detailed(d) => {
                    let mut d = d.clone();
                    // Path dependencies become crates.io deps.
                    d.path.take();
                    // Same with git dependencies.
                    d.git.take();
                    d.branch.take();
                    d.tag.take();
                    d.rev.take();
                    // registry specifications are elaborated to the index URL
                    if let Some(registry) = d.registry.take() {
                        let src = SourceId::alt_registry(config, &registry)?;
                        d.registry_index = Some(src.url().to_string());
                    }
                    Ok(TomlDependency::Detailed(d))
                }
                TomlDependency::Simple(s) => Ok(TomlDependency::Detailed(DetailedTomlDependency {
                    version: Some(s.clone()),
                    ..Default::default()
                })),
            }
        }
    }

    pub fn to_real_manifest(
        me: &Rc<TomlManifest>,
        source_id: SourceId,
        package_root: &Path,
        config: &Config,
    ) -> CargoResult<(Manifest, Vec<PathBuf>)> {
        let mut nested_paths = vec![];
        let mut warnings = vec![];
        let mut errors = vec![];

        // Parse features first so they will be available when parsing other parts of the TOML.
        let empty = Vec::new();
        let cargo_features = me.cargo_features.as_ref().unwrap_or(&empty);
        let features = Features::new(cargo_features, config, &mut warnings, source_id.is_path())?;

        let project = me.project.as_ref().or_else(|| me.package.as_ref());
        let project = project.ok_or_else(|| anyhow!("no `package` section found"))?;

        let package_name = project.name.trim();
        if package_name.is_empty() {
            bail!("package name cannot be an empty string")
        }

        validate_package_name(package_name, "package name", "")?;

        let pkgid = project.to_package_id(source_id)?;

        let edition = if let Some(ref edition) = project.edition {
            edition
                .parse()
                .with_context(|| "failed to parse the `edition` key")?
        } else {
            Edition::Edition2015
        };
        // Add these lines if start a new unstable edition.
        // ```
        // if edition == Edition::Edition20xx {
        //     features.require(Feature::edition20xx))?;
        // }
        // ```
        if !edition.is_stable() {
            // Guard in case someone forgets to add .require()
            return Err(util::errors::internal(format!(
                "edition {} should be gated",
                edition
            )));
        }

        let rust_version = if let Some(rust_version) = &project.rust_version {
            let req = match semver::VersionReq::parse(rust_version) {
                // Exclude semver operators like `^` and pre-release identifiers
                Ok(req) if rust_version.chars().all(|c| c.is_ascii_digit() || c == '.') => req,
                _ => bail!("`rust-version` must be a value like \"1.32\""),
            };
            if let Some(first_version) = edition.first_version() {
                let unsupported =
                    semver::Version::new(first_version.major, first_version.minor - 1, 9999);
                if req.matches(&unsupported) {
                    bail!(
                        "rust-version {} is older than first version ({}) required by \
                            the specified edition ({})",
                        rust_version,
                        first_version,
                        edition,
                    )
                }
            }
            Some(rust_version.clone())
        } else {
            None
        };

        if project.metabuild.is_some() {
            features.require(Feature::metabuild())?;
        }

        let resolve_behavior = match (
            project.resolver.as_ref(),
            me.workspace.as_ref().and_then(|ws| ws.resolver.as_ref()),
        ) {
            (None, None) => None,
            (Some(s), None) | (None, Some(s)) => Some(ResolveBehavior::from_manifest(s)?),
            (Some(_), Some(_)) => {
                bail!("cannot specify `resolver` field in both `[workspace]` and `[package]`")
            }
        };

        // If we have no lib at all, use the inferred lib, if available.
        // If we have a lib with a path, we're done.
        // If we have a lib with no path, use the inferred lib or else the package name.
        let targets = targets(
            &features,
            me,
            package_name,
            package_root,
            edition,
            &project.build,
            &project.metabuild,
            &mut warnings,
            &mut errors,
        )?;

        if targets.is_empty() {
            debug!("manifest has no build targets");
        }

        if let Err(e) = unique_build_targets(&targets, package_root) {
            warnings.push(format!(
                "file found to be present in multiple \
                 build targets: {}",
                e
            ));
        }

        if let Some(links) = &project.links {
            if !targets.iter().any(|t| t.is_custom_build()) {
                bail!(
                    "package `{}` specifies that it links to `{}` but does not \
                     have a custom build script",
                    pkgid,
                    links
                )
            }
        }

        let mut deps = Vec::new();
        let replace;
        let patch;

        {
            let mut cx = Context {
                deps: &mut deps,
                source_id,
                nested_paths: &mut nested_paths,
                config,
                warnings: &mut warnings,
                features: &features,
                platform: None,
                root: package_root,
            };

            fn process_dependencies(
                cx: &mut Context<'_, '_>,
                new_deps: Option<&BTreeMap<String, TomlDependency>>,
                kind: Option<DepKind>,
            ) -> CargoResult<()> {
                let dependencies = match new_deps {
                    Some(dependencies) => dependencies,
                    None => return Ok(()),
                };
                for (n, v) in dependencies.iter() {
                    let dep = v.to_dependency(n, cx, kind)?;
                    validate_package_name(dep.name_in_toml().as_str(), "dependency name", "")?;
                    cx.deps.push(dep);
                }

                Ok(())
            }

            // Collect the dependencies.
            process_dependencies(&mut cx, me.dependencies.as_ref(), None)?;
            let dev_deps = me
                .dev_dependencies
                .as_ref()
                .or_else(|| me.dev_dependencies2.as_ref());
            process_dependencies(&mut cx, dev_deps, Some(DepKind::Development))?;
            let build_deps = me
                .build_dependencies
                .as_ref()
                .or_else(|| me.build_dependencies2.as_ref());
            process_dependencies(&mut cx, build_deps, Some(DepKind::Build))?;

            for (name, platform) in me.target.iter().flatten() {
                cx.platform = {
                    let platform: Platform = name.parse()?;
                    platform.check_cfg_attributes(cx.warnings);
                    Some(platform)
                };
                process_dependencies(&mut cx, platform.dependencies.as_ref(), None)?;
                let build_deps = platform
                    .build_dependencies
                    .as_ref()
                    .or_else(|| platform.build_dependencies2.as_ref());
                process_dependencies(&mut cx, build_deps, Some(DepKind::Build))?;
                let dev_deps = platform
                    .dev_dependencies
                    .as_ref()
                    .or_else(|| platform.dev_dependencies2.as_ref());
                process_dependencies(&mut cx, dev_deps, Some(DepKind::Development))?;
            }

            replace = me.replace(&mut cx)?;
            patch = me.patch(&mut cx)?;
        }

        {
            let mut names_sources = BTreeMap::new();
            for dep in &deps {
                let name = dep.name_in_toml();
                let prev = names_sources.insert(name.to_string(), dep.source_id());
                if prev.is_some() && prev != Some(dep.source_id()) {
                    bail!(
                        "Dependency '{}' has different source paths depending on the build \
                         target. Each dependency must have a single canonical source path \
                         irrespective of build target.",
                        name
                    );
                }
            }
        }

        let exclude = project.exclude.clone().unwrap_or_default();
        let include = project.include.clone().unwrap_or_default();
        let empty_features = BTreeMap::new();

        let summary = Summary::new(
            config,
            pkgid,
            deps,
            me.features.as_ref().unwrap_or(&empty_features),
            project.links.as_deref(),
        )?;

        let metadata = ManifestMetadata {
            description: project.description.clone(),
            homepage: project.homepage.clone(),
            documentation: project.documentation.clone(),
            readme: readme_for_project(package_root, project),
            authors: project.authors.clone().unwrap_or_default(),
            license: project.license.clone(),
            license_file: project.license_file.clone(),
            repository: project.repository.clone(),
            keywords: project.keywords.clone().unwrap_or_default(),
            categories: project.categories.clone().unwrap_or_default(),
            badges: me.badges.clone().unwrap_or_default(),
            links: project.links.clone(),
        };

        let workspace_config = match (me.workspace.as_ref(), project.workspace.as_ref()) {
            (Some(config), None) => WorkspaceConfig::Root(WorkspaceRootConfig::new(
                package_root,
                &config.members,
                &config.default_members,
                &config.exclude,
                &config.metadata,
            )),
            (None, root) => WorkspaceConfig::Member {
                root: root.cloned(),
            },
            (Some(..), Some(..)) => bail!(
                "cannot configure both `package.workspace` and \
                 `[workspace]`, only one can be specified"
            ),
        };
        let profiles = me.profile.clone();
        if let Some(profiles) = &profiles {
            profiles.validate(&features, &mut warnings)?;
        }
        let publish = match project.publish {
            Some(VecStringOrBool::VecString(ref vecstring)) => Some(vecstring.clone()),
            Some(VecStringOrBool::Bool(false)) => Some(vec![]),
            None | Some(VecStringOrBool::Bool(true)) => None,
        };

        if summary.features().contains_key("default-features") {
            warnings.push(
                "`default-features = [\"..\"]` was found in [features]. \
                 Did you mean to use `default = [\"..\"]`?"
                    .to_string(),
            )
        }

        if let Some(run) = &project.default_run {
            if !targets
                .iter()
                .filter(|t| t.is_bin())
                .any(|t| t.name() == run)
            {
                let suggestion =
                    util::closest_msg(run, targets.iter().filter(|t| t.is_bin()), |t| t.name());
                bail!("default-run target `{}` not found{}", run, suggestion);
            }
        }

        let default_kind = project
            .default_target
            .as_ref()
            .map(|t| CompileTarget::new(&*t))
            .transpose()?
            .map(CompileKind::Target);
        let forced_kind = project
            .forced_target
            .as_ref()
            .map(|t| CompileTarget::new(&*t))
            .transpose()?
            .map(CompileKind::Target);

        let custom_metadata = project.metadata.clone();
        let mut manifest = Manifest::new(
            summary,
            default_kind,
            forced_kind,
            targets,
            exclude,
            include,
            project.links.clone(),
            metadata,
            custom_metadata,
            profiles,
            publish,
            replace,
            patch,
            workspace_config,
            features,
            edition,
            rust_version,
            project.im_a_teapot,
            project.default_run.clone(),
            Rc::clone(me),
            project.metabuild.clone().map(|sov| sov.0),
            resolve_behavior,
        );
        if project.license_file.is_some() && project.license.is_some() {
            manifest.warnings_mut().add_warning(
                "only one of `license` or `license-file` is necessary\n\
                 `license` should be used if the package license can be expressed \
                 with a standard SPDX expression.\n\
                 `license-file` should be used if the package uses a non-standard license.\n\
                 See https://doc.rust-lang.org/cargo/reference/manifest.html#the-license-and-license-file-fields \
                 for more information."
                    .to_string(),
            );
        }
        for warning in warnings {
            manifest.warnings_mut().add_warning(warning);
        }
        for error in errors {
            manifest.warnings_mut().add_critical_warning(error);
        }

        manifest.feature_gate()?;

        Ok((manifest, nested_paths))
    }

    fn to_virtual_manifest(
        me: &Rc<TomlManifest>,
        source_id: SourceId,
        root: &Path,
        config: &Config,
    ) -> CargoResult<(VirtualManifest, Vec<PathBuf>)> {
        if me.project.is_some() {
            bail!("this virtual manifest specifies a [project] section, which is not allowed");
        }
        if me.package.is_some() {
            bail!("this virtual manifest specifies a [package] section, which is not allowed");
        }
        if me.lib.is_some() {
            bail!("this virtual manifest specifies a [lib] section, which is not allowed");
        }
        if me.bin.is_some() {
            bail!("this virtual manifest specifies a [[bin]] section, which is not allowed");
        }
        if me.example.is_some() {
            bail!("this virtual manifest specifies a [[example]] section, which is not allowed");
        }
        if me.test.is_some() {
            bail!("this virtual manifest specifies a [[test]] section, which is not allowed");
        }
        if me.bench.is_some() {
            bail!("this virtual manifest specifies a [[bench]] section, which is not allowed");
        }
        if me.dependencies.is_some() {
            bail!("this virtual manifest specifies a [dependencies] section, which is not allowed");
        }
        if me.dev_dependencies.is_some() || me.dev_dependencies2.is_some() {
            bail!("this virtual manifest specifies a [dev-dependencies] section, which is not allowed");
        }
        if me.build_dependencies.is_some() || me.build_dependencies2.is_some() {
            bail!("this virtual manifest specifies a [build-dependencies] section, which is not allowed");
        }
        if me.features.is_some() {
            bail!("this virtual manifest specifies a [features] section, which is not allowed");
        }
        if me.target.is_some() {
            bail!("this virtual manifest specifies a [target] section, which is not allowed");
        }
        if me.badges.is_some() {
            bail!("this virtual manifest specifies a [badges] section, which is not allowed");
        }

        let mut nested_paths = Vec::new();
        let mut warnings = Vec::new();
        let mut deps = Vec::new();
        let empty = Vec::new();
        let cargo_features = me.cargo_features.as_ref().unwrap_or(&empty);
        let features = Features::new(cargo_features, config, &mut warnings, source_id.is_path())?;

        let (replace, patch) = {
            let mut cx = Context {
                deps: &mut deps,
                source_id,
                nested_paths: &mut nested_paths,
                config,
                warnings: &mut warnings,
                platform: None,
                features: &features,
                root,
            };
            (me.replace(&mut cx)?, me.patch(&mut cx)?)
        };
        let profiles = me.profile.clone();
        if let Some(profiles) = &profiles {
            profiles.validate(&features, &mut warnings)?;
        }
        let resolve_behavior = me
            .workspace
            .as_ref()
            .and_then(|ws| ws.resolver.as_deref())
            .map(|r| ResolveBehavior::from_manifest(r))
            .transpose()?;
        let workspace_config = match me.workspace {
            Some(ref config) => WorkspaceConfig::Root(WorkspaceRootConfig::new(
                root,
                &config.members,
                &config.default_members,
                &config.exclude,
                &config.metadata,
            )),
            None => {
                bail!("virtual manifests must be configured with [workspace]");
            }
        };
        Ok((
            VirtualManifest::new(
                replace,
                patch,
                workspace_config,
                profiles,
                features,
                resolve_behavior,
            ),
            nested_paths,
        ))
    }

    fn replace(&self, cx: &mut Context<'_, '_>) -> CargoResult<Vec<(PackageIdSpec, Dependency)>> {
        if self.patch.is_some() && self.replace.is_some() {
            bail!("cannot specify both [replace] and [patch]");
        }
        let mut replace = Vec::new();
        for (spec, replacement) in self.replace.iter().flatten() {
            let mut spec = PackageIdSpec::parse(spec).with_context(|| {
                format!(
                    "replacements must specify a valid semver \
                     version to replace, but `{}` does not",
                    spec
                )
            })?;
            if spec.url().is_none() {
                spec.set_url(CRATES_IO_INDEX.parse().unwrap());
            }

            if replacement.is_version_specified() {
                bail!(
                    "replacements cannot specify a version \
                     requirement, but found one for `{}`",
                    spec
                );
            }

            let mut dep = replacement.to_dependency(spec.name().as_str(), cx, None)?;
            let version = spec.version().ok_or_else(|| {
                anyhow!(
                    "replacements must specify a version \
                     to replace, but `{}` does not",
                    spec
                )
            })?;
            dep.set_version_req(VersionReq::exact(version))
                .lock_version(version);
            replace.push((spec, dep));
        }
        Ok(replace)
    }

    fn patch(&self, cx: &mut Context<'_, '_>) -> CargoResult<HashMap<Url, Vec<Dependency>>> {
        let mut patch = HashMap::new();
        for (url, deps) in self.patch.iter().flatten() {
            let url = match &url[..] {
                CRATES_IO_REGISTRY => CRATES_IO_INDEX.parse().unwrap(),
                _ => cx
                    .config
                    .get_registry_index(url)
                    .or_else(|_| url.into_url())
                    .with_context(|| {
                        format!("[patch] entry `{}` should be a URL or registry name", url)
                    })?,
            };
            patch.insert(
                url,
                deps.iter()
                    .map(|(name, dep)| dep.to_dependency(name, cx, None))
                    .collect::<CargoResult<Vec<_>>>()?,
            );
        }
        Ok(patch)
    }

    /// Returns the path to the build script if one exists for this crate.
    fn maybe_custom_build(
        &self,
        build: &Option<StringOrBool>,
        package_root: &Path,
    ) -> Option<PathBuf> {
        let build_rs = package_root.join("build.rs");
        match *build {
            // Explicitly no build script.
            Some(StringOrBool::Bool(false)) => None,
            Some(StringOrBool::Bool(true)) => Some(build_rs),
            Some(StringOrBool::String(ref s)) => Some(PathBuf::from(s)),
            None => {
                // If there is a `build.rs` file next to the `Cargo.toml`, assume it is
                // a build script.
                if build_rs.is_file() {
                    Some(build_rs)
                } else {
                    None
                }
            }
        }
    }

    pub fn has_profiles(&self) -> bool {
        self.profile.is_some()
    }

    pub fn features(&self) -> Option<&BTreeMap<InternedString, Vec<InternedString>>> {
        self.features.as_ref()
    }
}

/// Returns the name of the README file for a `TomlProject`.
fn readme_for_project(package_root: &Path, project: &TomlProject) -> Option<String> {
    match &project.readme {
        None => default_readme_from_package_root(package_root),
        Some(value) => match value {
            StringOrBool::Bool(false) => None,
            StringOrBool::Bool(true) => Some("README.md".to_string()),
            StringOrBool::String(v) => Some(v.clone()),
        },
    }
}

const DEFAULT_README_FILES: [&str; 3] = ["README.md", "README.txt", "README"];

/// Checks if a file with any of the default README file names exists in the package root.
/// If so, returns a `String` representing that name.
fn default_readme_from_package_root(package_root: &Path) -> Option<String> {
    for &readme_filename in DEFAULT_README_FILES.iter() {
        if package_root.join(readme_filename).is_file() {
            return Some(readme_filename.to_string());
        }
    }

    None
}

/// Checks a list of build targets, and ensures the target names are unique within a vector.
/// If not, the name of the offending build target is returned.
fn unique_build_targets(targets: &[Target], package_root: &Path) -> Result<(), String> {
    let mut seen = HashSet::new();
    for target in targets {
        if let TargetSourcePath::Path(path) = target.src_path() {
            let full = package_root.join(path);
            if !seen.insert(full.clone()) {
                return Err(full.display().to_string());
            }
        }
    }
    Ok(())
}

impl<P: ResolveToPath> TomlDependency<P> {
    pub(crate) fn to_dependency_split(
        &self,
        name: &str,
        source_id: SourceId,
        nested_paths: &mut Vec<PathBuf>,
        config: &Config,
        warnings: &mut Vec<String>,
        platform: Option<Platform>,
        root: &Path,
        features: &Features,
        kind: Option<DepKind>,
    ) -> CargoResult<Dependency> {
        self.to_dependency(
            name,
            &mut Context {
                deps: &mut Vec::new(),
                source_id,
                nested_paths,
                config,
                warnings,
                platform,
                root,
                features,
            },
            kind,
        )
    }

    fn to_dependency(
        &self,
        name: &str,
        cx: &mut Context<'_, '_>,
        kind: Option<DepKind>,
    ) -> CargoResult<Dependency> {
        match *self {
            TomlDependency::Simple(ref version) => DetailedTomlDependency::<P> {
                version: Some(version.clone()),
                ..Default::default()
            }
            .to_dependency(name, cx, kind),
            TomlDependency::Detailed(ref details) => details.to_dependency(name, cx, kind),
        }
    }

    fn is_version_specified(&self) -> bool {
        match self {
            TomlDependency::Detailed(d) => d.version.is_some(),
            TomlDependency::Simple(..) => true,
        }
    }
}

impl<P: ResolveToPath> DetailedTomlDependency<P> {
    fn to_dependency(
        &self,
        name_in_toml: &str,
        cx: &mut Context<'_, '_>,
        kind: Option<DepKind>,
    ) -> CargoResult<Dependency> {
        if self.version.is_none() && self.path.is_none() && self.git.is_none() {
            let msg = format!(
                "dependency ({}) specified without \
                 providing a local path, Git repository, or \
                 version to use. This will be considered an \
                 error in future versions",
                name_in_toml
            );
            cx.warnings.push(msg);
        }

        if let Some(version) = &self.version {
            if version.contains('+') {
                cx.warnings.push(format!(
                    "version requirement `{}` for dependency `{}` \
                     includes semver metadata which will be ignored, removing the \
                     metadata is recommended to avoid confusion",
                    version, name_in_toml
                ));
            }
        }

        if self.git.is_none() {
            let git_only_keys = [
                (&self.branch, "branch"),
                (&self.tag, "tag"),
                (&self.rev, "rev"),
            ];

            for &(key, key_name) in &git_only_keys {
                if key.is_some() {
                    bail!(
                        "key `{}` is ignored for dependency ({}).",
                        key_name,
                        name_in_toml
                    );
                }
            }
        }

        // Early detection of potentially misused feature syntax
        // instead of generating a "feature not found" error.
        if let Some(features) = &self.features {
            for feature in features {
                if feature.contains('/') {
                    bail!(
                        "feature `{}` in dependency `{}` is not allowed to contain slashes\n\
                         If you want to enable features of a transitive dependency, \
                         the direct dependency needs to re-export those features from \
                         the `[features]` table.",
                        feature,
                        name_in_toml
                    );
                }
                if feature.starts_with("dep:") {
                    bail!(
                        "feature `{}` in dependency `{}` is not allowed to use explicit \
                        `dep:` syntax\n\
                         If you want to enable an optional dependency, specify the name \
                         of the optional dependency without the `dep:` prefix, or specify \
                         a feature from the dependency's `[features]` table that enables \
                         the optional dependency.",
                        feature,
                        name_in_toml
                    );
                }
            }
        }

        let new_source_id = match (
            self.git.as_ref(),
            self.path.as_ref(),
            self.registry.as_ref(),
            self.registry_index.as_ref(),
        ) {
            (Some(_), _, Some(_), _) | (Some(_), _, _, Some(_)) => bail!(
                "dependency ({}) specification is ambiguous. \
                 Only one of `git` or `registry` is allowed.",
                name_in_toml
            ),
            (_, _, Some(_), Some(_)) => bail!(
                "dependency ({}) specification is ambiguous. \
                 Only one of `registry` or `registry-index` is allowed.",
                name_in_toml
            ),
            (Some(git), maybe_path, _, _) => {
                if maybe_path.is_some() {
                    bail!(
                        "dependency ({}) specification is ambiguous. \
                         Only one of `git` or `path` is allowed.",
                        name_in_toml
                    );
                }

                let n_details = [&self.branch, &self.tag, &self.rev]
                    .iter()
                    .filter(|d| d.is_some())
                    .count();

                if n_details > 1 {
                    bail!(
                        "dependency ({}) specification is ambiguous. \
                         Only one of `branch`, `tag` or `rev` is allowed.",
                        name_in_toml
                    );
                }

                let reference = self
                    .branch
                    .clone()
                    .map(GitReference::Branch)
                    .or_else(|| self.tag.clone().map(GitReference::Tag))
                    .or_else(|| self.rev.clone().map(GitReference::Rev))
                    .unwrap_or(GitReference::DefaultBranch);
                let loc = git.into_url()?;

                if let Some(fragment) = loc.fragment() {
                    let msg = format!(
                        "URL fragment `#{}` in git URL is ignored for dependency ({}). \
                        If you were trying to specify a specific git revision, \
                        use `rev = \"{}\"` in the dependency declaration.",
                        fragment, name_in_toml, fragment
                    );
                    cx.warnings.push(msg)
                }

                SourceId::for_git(&loc, reference)?
            }
            (None, Some(path), _, _) => {
                let path = path.resolve(cx.config);
                cx.nested_paths.push(path.clone());
                // If the source ID for the package we're parsing is a path
                // source, then we normalize the path here to get rid of
                // components like `..`.
                //
                // The purpose of this is to get a canonical ID for the package
                // that we're depending on to ensure that builds of this package
                // always end up hashing to the same value no matter where it's
                // built from.
                if cx.source_id.is_path() {
                    let path = cx.root.join(path);
                    let path = paths::normalize_path(&path);
                    SourceId::for_path(&path)?
                } else {
                    cx.source_id
                }
            }
            (None, None, Some(registry), None) => SourceId::alt_registry(cx.config, registry)?,
            (None, None, None, Some(registry_index)) => {
                let url = registry_index.into_url()?;
                SourceId::for_registry(&url)?
            }
            (None, None, None, None) => SourceId::crates_io(cx.config)?,
        };

        let (pkg_name, explicit_name_in_toml) = match self.package {
            Some(ref s) => (&s[..], Some(name_in_toml)),
            None => (name_in_toml, None),
        };

        let version = self.version.as_deref();
        let mut dep = Dependency::parse(pkg_name, version, new_source_id)?;
        dep.set_features(self.features.iter().flatten())
            .set_default_features(
                self.default_features
                    .or(self.default_features2)
                    .unwrap_or(true),
            )
            .set_optional(self.optional.unwrap_or(false))
            .set_platform(cx.platform.clone());
        if let Some(registry) = &self.registry {
            let registry_id = SourceId::alt_registry(cx.config, registry)?;
            dep.set_registry_id(registry_id);
        }
        if let Some(registry_index) = &self.registry_index {
            let url = registry_index.into_url()?;
            let registry_id = SourceId::for_registry(&url)?;
            dep.set_registry_id(registry_id);
        }

        if let Some(kind) = kind {
            dep.set_kind(kind);
        }
        if let Some(name_in_toml) = explicit_name_in_toml {
            dep.set_explicit_name_in_toml(name_in_toml);
        }

        if let Some(p) = self.public {
            cx.features.require(Feature::public_dependency())?;

            if dep.kind() != DepKind::Normal {
                bail!("'public' specifier can only be used on regular dependencies, not {:?} dependencies", dep.kind());
            }

            dep.set_public(p);
        }

        if let (Some(artifact), is_lib, target) = (
            self.artifact.as_ref(),
            self.lib.unwrap_or(false),
            self.target.as_deref(),
        ) {
            if cx.config.cli_unstable().bindeps {
                let artifact = Artifact::parse(artifact, is_lib, target)?;
                if dep.kind() != DepKind::Build
                    && artifact.target() == Some(ArtifactTarget::BuildDependencyAssumeTarget)
                {
                    bail!(
                        r#"`target = "target"` in normal- or dev-dependencies has no effect ({})"#,
                        name_in_toml
                    );
                }
                dep.set_artifact(artifact)
            } else {
                bail!("`artifact = …` requires `-Z bindeps` ({})", name_in_toml);
            }
        } else if self.lib.is_some() || self.target.is_some() {
            for (is_set, specifier) in [
                (self.lib.is_some(), "lib"),
                (self.target.is_some(), "target"),
            ] {
                if !is_set {
                    continue;
                }
                bail!(
                    "'{}' specifier cannot be used without an 'artifact = …' value ({})",
                    specifier,
                    name_in_toml
                )
            }
        }
        Ok(dep)
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
struct TomlTarget {
    name: Option<String>,

    // The intention was to only accept `crate-type` here but historical
    // versions of Cargo also accepted `crate_type`, so look for both.
    #[serde(rename = "crate-type")]
    crate_type: Option<Vec<String>>,
    #[serde(rename = "crate_type")]
    crate_type2: Option<Vec<String>>,

    path: Option<PathValue>,
    // Note that `filename` is used for the cargo-feature `different_binary_name`
    filename: Option<String>,
    test: Option<bool>,
    doctest: Option<bool>,
    bench: Option<bool>,
    doc: Option<bool>,
    plugin: Option<bool>,
    #[serde(rename = "proc-macro")]
    proc_macro_raw: Option<bool>,
    #[serde(rename = "proc_macro")]
    proc_macro_raw2: Option<bool>,
    harness: Option<bool>,
    #[serde(rename = "required-features")]
    required_features: Option<Vec<String>>,
    edition: Option<String>,
}

#[derive(Clone)]
struct PathValue(PathBuf);

impl<'de> de::Deserialize<'de> for PathValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        Ok(PathValue(String::deserialize(deserializer)?.into()))
    }
}

impl ser::Serialize for PathValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        self.0.serialize(serializer)
    }
}

/// Corresponds to a `target` entry, but `TomlTarget` is already used.
#[derive(Serialize, Deserialize, Debug)]
struct TomlPlatform {
    dependencies: Option<BTreeMap<String, TomlDependency>>,
    #[serde(rename = "build-dependencies")]
    build_dependencies: Option<BTreeMap<String, TomlDependency>>,
    #[serde(rename = "build_dependencies")]
    build_dependencies2: Option<BTreeMap<String, TomlDependency>>,
    #[serde(rename = "dev-dependencies")]
    dev_dependencies: Option<BTreeMap<String, TomlDependency>>,
    #[serde(rename = "dev_dependencies")]
    dev_dependencies2: Option<BTreeMap<String, TomlDependency>>,
}

impl TomlTarget {
    fn new() -> TomlTarget {
        TomlTarget::default()
    }

    fn name(&self) -> String {
        match self.name {
            Some(ref name) => name.clone(),
            None => panic!("target name is required"),
        }
    }

    fn proc_macro(&self) -> Option<bool> {
        self.proc_macro_raw.or(self.proc_macro_raw2).or_else(|| {
            if let Some(types) = self.crate_types() {
                if types.contains(&"proc-macro".to_string()) {
                    return Some(true);
                }
            }
            None
        })
    }

    fn crate_types(&self) -> Option<&Vec<String>> {
        self.crate_type
            .as_ref()
            .or_else(|| self.crate_type2.as_ref())
    }
}

impl fmt::Debug for PathValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
