use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt;
use std::marker::PhantomData;
use std::path::{Component, Path, PathBuf};
use std::rc::Rc;
use std::str;

use anyhow::{anyhow, bail, Context as _};
use cargo_platform::Platform;
use cargo_util::paths;
use log::{debug, trace};
use semver::{self, Version, VersionReq};
use serde::de;
use serde::ser;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::core::compiler::{CompileKind, CompileTarget};
use crate::core::dependency::DepKind;
use crate::core::manifest::{IntermediateManifest, ManifestMetadata, TargetSourcePath, Warnings};
use crate::core::resolver::ResolveBehavior;
use crate::core::{
    find_workspace_root, Dependency, InheritableFields, Manifest, PackageId, Summary, Target,
};
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

    do_read_manifest(&contents, path, source_id, config)
        .with_context(|| format!("failed to parse manifest at `{}`", path.display()))
        .map_err(|err| ManifestError::new(err, path.into()))
}

fn do_read_manifest(
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
        parse(contents, pretty_filename, config)?
    };

    // Provide a helpful error message for a common user error.
    if let Some(package) = toml.get("package").or_else(|| toml.get("project")) {
        if let Some(feats) = package.get("cargo-features") {
            bail!(
                "cargo-features = {} was found in the wrong location, it \
                 should be set at the top of Cargo.toml before any tables",
                toml::to_string(feats).unwrap()
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

    if let Some(deps) = manifest
        .workspace
        .as_ref()
        .and_then(|ws| ws.dependencies.as_ref())
    {
        for (name, dep) in deps {
            if dep.is_optional() {
                bail!(
                    "{} is optional, but workspace dependencies cannot be optional",
                    name
                );
            }
        }
    }

    return if manifest.project.is_some() || manifest.package.is_some() {
        let (mut manifest, paths) =
            TomlManifest::to_intermediate(&manifest, source_id, package_root)?;
        add_unused(manifest.warnings_mut());
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
pub fn parse(toml: &str, file: &Path, config: &Config) -> CargoResult<toml::Value> {
    let first_error = match toml.parse() {
        Ok(ret) => return Ok(ret),
        Err(e) => e,
    };

    let mut second_parser = toml::de::Deserializer::new(toml);
    second_parser.set_require_newline_after_table(false);
    if let Ok(ret) = toml::Value::deserialize(&mut second_parser) {
        let msg = format!(
            "\
TOML file found which contains invalid syntax and will soon not parse
at `{}`.

The TOML spec requires newlines after table definitions (e.g., `[a] b = 1` is
invalid), but this file has a table header which does not have a newline after
it. A newline needs to be added and this warning will soon become a hard error
in the future.",
            file.display()
        );
        config.shell().warn(&msg)?;
        return Ok(ret);
    }

    let mut third_parser = toml::de::Deserializer::new(toml);
    third_parser.set_allow_duplicate_after_longer_table(true);
    if let Ok(ret) = toml::Value::deserialize(&mut third_parser) {
        let msg = format!(
            "\
TOML file found which contains invalid syntax and will soon not parse
at `{}`.

The TOML spec requires that each table header is defined at most once, but
historical versions of Cargo have erroneously accepted this file. The table
definitions will need to be merged together with one table header to proceed,
and this will become a hard error in the future.",
            file.display()
        );
        config.shell().warn(&msg)?;
        return Ok(ret);
    }

    let first_error = anyhow::Error::from(first_error);
    Err(first_error.context("could not parse input as TOML"))
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
    Workspace(TomlWorkspaceDependency),
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
                let details: IntermediateDependency<P> = IntermediateDependency::deserialize(mvd)?;
                if let Some(workspace) = details.workspace {
                    if workspace {
                        Ok(TomlDependency::Workspace(TomlWorkspaceDependency {
                            workspace: true,
                            features: details.features,
                            optional: details.optional,
                        }))
                    } else {
                        return Err(de::Error::custom("workspace cannot be false"));
                    }
                } else {
                    Ok(TomlDependency::Detailed(DetailedTomlDependency {
                        version: details.version,
                        registry: details.registry,
                        registry_index: details.registry_index,
                        path: details.path,
                        git: details.git,
                        branch: details.branch,
                        tag: details.tag,
                        rev: details.rev,
                        features: details.features,
                        optional: details.optional,
                        default_features: details.default_features,
                        default_features2: details.default_features2,
                        package: details.package,
                        public: details.public,
                    }))
                }
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
pub struct IntermediateDependency<P = String> {
    workspace: Option<bool>,
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
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct TomlWorkspaceDependency {
    workspace: bool,
    features: Option<Vec<String>>,
    optional: Option<bool>,
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
    #[serde(deserialize_with = "deserialize_workspace_badges", default)]
    badges: Option<MaybeWorkspace<BTreeMap<String, BTreeMap<String, String>>>>,
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
    pub codegen_units: Option<u32>,
    pub debug: Option<U32OrBool>,
    pub split_debuginfo: Option<String>,
    pub debug_assertions: Option<bool>,
    pub rpath: Option<bool>,
    pub panic: Option<String>,
    pub overflow_checks: Option<bool>,
    pub incremental: Option<bool>,
    pub package: Option<BTreeMap<ProfilePackageSpec, TomlProfile>>,
    pub build_override: Option<Box<TomlProfile>>,
    pub dir_name: Option<InternedString>,
    pub inherits: Option<InternedString>,
    pub strip: Option<StringOrBool>,
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
        match *self {
            ProfilePackageSpec::Spec(ref spec) => spec.serialize(s),
            ProfilePackageSpec::All => "*".serialize(s),
        }
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

impl TomlProfile {
    pub fn validate(
        &self,
        name: &str,
        features: &Features,
        warnings: &mut Vec<String>,
    ) -> CargoResult<()> {
        if name == "debug" {
            warnings.push("use `[profile.dev]` to configure debug builds".to_string());
        }

        if let Some(ref profile) = self.build_override {
            features.require(Feature::profile_overrides())?;
            profile.validate_override("build-override")?;
        }
        if let Some(ref packages) = self.package {
            features.require(Feature::profile_overrides())?;
            for profile in packages.values() {
                profile.validate_override("package")?;
            }
        }

        // Feature gate definition of named profiles
        match name {
            "dev" | "release" | "bench" | "test" | "doc" => {}
            _ => {
                features.require(Feature::named_profiles())?;
            }
        }

        // Profile name validation
        Self::validate_name(name, "profile name")?;

        // Feature gate on uses of keys related to named profiles
        if self.inherits.is_some() {
            features.require(Feature::named_profiles())?;
        }

        if self.dir_name.is_some() {
            features.require(Feature::named_profiles())?;
        }

        // `dir-name` validation
        match &self.dir_name {
            None => {}
            Some(dir_name) => {
                Self::validate_name(dir_name, "dir-name")?;
            }
        }

        // `inherits` validation
        match &self.inherits {
            None => {}
            Some(inherits) => {
                Self::validate_name(inherits, "inherits")?;
            }
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

        if self.strip.is_some() {
            features.require(Feature::strip())?;
        }
        Ok(())
    }

    /// Validate dir-names and profile names according to RFC 2678.
    pub fn validate_name(name: &str, what: &str) -> CargoResult<()> {
        if let Some(ch) = name
            .chars()
            .find(|ch| !ch.is_alphanumeric() && *ch != '_' && *ch != '-')
        {
            bail!("Invalid character `{}` in {}: `{}`", ch, what, name);
        }

        match name {
            "package" | "build" => {
                bail!("Invalid {}: `{}`", what, name);
            }
            "debug" if what == "profile" => {
                if what == "profile name" {
                    // Allowed, but will emit warnings
                } else {
                    bail!("Invalid {}: `{}`", what, name);
                }
            }
            "doc" if what == "dir-name" => {
                bail!("Invalid {}: `{}`", what, name);
            }
            _ => {}
        }

        Ok(())
    }

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

#[derive(Clone, Debug, Serialize, Eq, PartialEq)]
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

pub fn map_deps(
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
        TomlDependency::Workspace(_) => unreachable!(),
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(untagged)]
pub enum MaybeWorkspace<T> {
    Workspace(TomlWorkspaceField),
    Defined(T),
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum MaybeWorkspaceBadge {
    Workspace(TomlWorkspaceField),
    Defined(BTreeMap<String, BTreeMap<String, String>>),
}

/// This exists only to provide a nicer error message.
fn deserialize_workspace_badges<'de, D>(
    deserializer: D,
) -> Result<Option<MaybeWorkspace<BTreeMap<String, BTreeMap<String, String>>>>, D::Error>
where
    D: de::Deserializer<'de>,
{
    match Option::deserialize(deserializer) {
        Ok(None) => Ok(None),
        Ok(Some(MaybeWorkspaceBadge::Defined(badges))) => Ok(Some(MaybeWorkspace::Defined(badges))),
        Ok(Some(MaybeWorkspaceBadge::Workspace(ws))) if ws.workspace => {
            Ok(Some(MaybeWorkspace::Workspace(TomlWorkspaceField {
                workspace: true,
            })))
        }
        Ok(Some(MaybeWorkspaceBadge::Workspace(_))) => {
            Err(de::Error::custom("workspace cannot be false"))
        }

        Err(_) => Err(de::Error::custom(
            "expected a table of badges or { workspace = true }",
        )),
    }
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct TomlWorkspaceField {
    workspace: bool,
}

impl<T> MaybeWorkspace<T> {
    fn from_option(value: &Option<MaybeWorkspace<T>>) -> Option<T>
    where
        T: Clone,
    {
        match value {
            Some(MaybeWorkspace::Defined(value)) => Some(value.clone()),
            _ => None,
        }
    }
}

/// Parses an optional field, defaulting to the workspace's value.
fn ws_default<T, F>(
    value: Option<MaybeWorkspace<T>>,
    workspace: &InheritableFields,
    f: F,
    label: &str,
) -> CargoResult<Option<MaybeWorkspace<T>>>
where
    T: std::fmt::Debug + Clone,
    F: FnOnce(&InheritableFields) -> &Option<T>,
{
    match (value, workspace) {
        (None, _) => Ok(None),
        (Some(MaybeWorkspace::Defined(value)), _) => Ok(Some(MaybeWorkspace::Defined(value))),
        (Some(MaybeWorkspace::Workspace(TomlWorkspaceField { workspace: true })), ws) => f(ws)
            .clone()
            .ok_or_else(|| {
                anyhow!(
                    "error reading `{0}` from workspace root manifest's `[workspace.{0}]`",
                    label
                )
            })
            .map(|value| Some(MaybeWorkspace::Defined(value))),
        (Some(MaybeWorkspace::Workspace(TomlWorkspaceField { workspace: false })), _) => Err(
            anyhow!("workspace cannot be false for key `package.{0}`", label),
        ),
    }
}

fn version_trim_whitespace<'de, D>(
    deserializer: D,
) -> Result<MaybeWorkspace<semver::Version>, D::Error>
where
    D: de::Deserializer<'de>,
{
    struct Visitor;

    impl<'de> de::Visitor<'de> for Visitor {
        type Value = MaybeWorkspace<semver::Version>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("SemVer version")
        }

        fn visit_str<E>(self, string: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            match string.trim().parse().map_err(de::Error::custom) {
                Ok(parsed) => Ok(MaybeWorkspace::Defined(parsed)),
                Err(e) => Err(e),
            }
        }

        fn visit_map<V>(self, map: V) -> Result<Self::Value, V::Error>
        where
            V: de::MapAccess<'de>,
        {
            let mvd = de::value::MapAccessDeserializer::new(map);
            TomlWorkspaceField::deserialize(mvd).and_then(|t| {
                if t.workspace {
                    Ok(MaybeWorkspace::Workspace(TomlWorkspaceField {
                        workspace: true,
                    }))
                } else {
                    Err(de::Error::custom("workspace cannot be false"))
                }
            })
        }
    }

    deserializer.deserialize_any(Visitor)
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
    edition: Option<MaybeWorkspace<String>>,
    rust_version: Option<String>,
    name: InternedString,
    #[serde(deserialize_with = "version_trim_whitespace")]
    version: MaybeWorkspace<semver::Version>,
    authors: Option<MaybeWorkspace<Vec<String>>>,
    build: Option<StringOrBool>,
    metabuild: Option<StringOrVec>,
    #[serde(rename = "default-target")]
    default_target: Option<String>,
    #[serde(rename = "forced-target")]
    forced_target: Option<String>,
    links: Option<String>,
    exclude: Option<Vec<String>>,
    include: Option<Vec<String>>,
    publish: Option<MaybeWorkspace<VecStringOrBool>>,
    workspace: Option<String>,
    im_a_teapot: Option<bool>,
    autobins: Option<bool>,
    autoexamples: Option<bool>,
    autotests: Option<bool>,
    autobenches: Option<bool>,
    default_run: Option<String>,

    // Package metadata.
    description: Option<MaybeWorkspace<String>>,
    homepage: Option<MaybeWorkspace<String>>,
    documentation: Option<MaybeWorkspace<String>>,
    readme: Option<MaybeWorkspace<StringOrBool>>,
    keywords: Option<MaybeWorkspace<Vec<String>>>,
    categories: Option<MaybeWorkspace<Vec<String>>>,
    license: Option<MaybeWorkspace<String>>,
    #[serde(rename = "license-file")]
    license_file: Option<MaybeWorkspace<String>>,
    repository: Option<MaybeWorkspace<String>>,
    resolver: Option<String>,

    // Note that this field must come last due to the way toml serialization
    // works which requires tables to be emitted after all values.
    metadata: Option<toml::Value>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TomlWorkspace {
    pub members: Option<Vec<String>>,
    #[serde(rename = "default-members")]
    pub default_members: Option<Vec<String>>,
    pub exclude: Option<Vec<String>>,
    resolver: Option<String>,

    // Properties that can be inherited by members.
    pub dependencies: Option<BTreeMap<String, TomlDependency>>,
    pub version: Option<semver::Version>,
    pub authors: Option<Vec<String>>,
    pub description: Option<String>,
    pub documentation: Option<String>,
    pub readme: Option<StringOrBool>,
    pub homepage: Option<String>,
    pub repository: Option<String>,
    pub license: Option<String>,
    #[serde(rename = "license-file")]
    pub license_file: Option<String>,
    pub keywords: Option<Vec<String>>,
    pub categories: Option<Vec<String>>,
    pub publish: Option<VecStringOrBool>,
    pub edition: Option<String>,
    pub badges: Option<BTreeMap<String, BTreeMap<String, String>>>,

    // Note that this field must come last due to the way toml serialization
    // works which requires tables to be emitted after all values.
    pub metadata: Option<toml::Value>,
}

impl TomlProject {
    pub fn to_package_id(&self, source_id: SourceId, version: Version) -> CargoResult<PackageId> {
        PackageId::new(self.name, version, source_id)
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
    // No need to check for MaybeWorkspace since this should only be called on a package
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
        if let Some(MaybeWorkspace::Defined(license_file)) = &package.license_file {
            let license_path = Path::new(&license_file);
            let abs_license_path = paths::normalize_path(&package_root.join(license_path));
            if abs_license_path.strip_prefix(package_root).is_err() {
                // This path points outside of the package root. `cargo package`
                // will copy it into the root, so adjust the path to this location.
                package.license_file = Some(MaybeWorkspace::Defined(
                    license_path
                        .file_name()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .to_string(),
                ));
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
    }

    pub fn to_intermediate(
        me: &Rc<TomlManifest>,
        source_id: SourceId,
        package_root: &Path,
    ) -> CargoResult<(IntermediateManifest, Vec<PathBuf>)> {
        let project = me.project.as_ref().or_else(|| me.package.as_ref());
        let project = project.ok_or_else(|| anyhow!("no `package` section found"))?;

        let package_name = project.name.trim();
        if package_name.is_empty() {
            bail!("package name cannot be an empty string")
        }

        validate_package_name(package_name, "package name", "")?;

        let workspace_config = match (me.workspace.as_ref(), project.workspace.as_ref()) {
            (Some(toml_workspace), None) => WorkspaceConfig::Root(
                WorkspaceRootConfig::from_toml_workspace(package_root, toml_workspace),
            ),
            (None, root) => WorkspaceConfig::Member {
                root: root.cloned(),
            },
            (Some(..), Some(..)) => bail!(
                "cannot configure both `package.workspace` and \
                 `[workspace]`, only one can be specified"
            ),
        };

        let manifest = IntermediateManifest::new(workspace_config, source_id, Rc::clone(me));

        Ok((manifest, vec![]))
    }

    pub fn to_real_manifest(
        me: &Rc<TomlManifest>,
        source_id: SourceId,
        package_root: &Path,
        config: &Config,
        inherit: &InheritableFields,
    ) -> CargoResult<(Manifest, Vec<PathBuf>)> {
        let mut nested_paths = vec![];
        let mut warnings = vec![];
        let mut errors = vec![];

        // Parse features first so they will be available when parsing other parts of the TOML.
        let empty = Vec::new();
        let cargo_features = me.cargo_features.as_ref().unwrap_or(&empty);
        let features = Features::new(cargo_features, config, &mut warnings, source_id.is_path())?;

        let project = me.project.clone().or_else(|| me.package.clone());
        let project = &mut project.ok_or_else(|| anyhow!("no `package` section found"))?;

        TomlManifest::parse_toml_project(project, inherit)?;

        project.license_file = match (project.license_file.clone(), inherit.license_file.as_ref()) {
            (None, _) => None,
            (Some(MaybeWorkspace::Defined(defined)), _) => Some(MaybeWorkspace::Defined(defined)),
            (Some(MaybeWorkspace::Workspace(_)), None) => {
                bail!("error reading license-file: workspace root does not defined [workspace.license-file]");
            }
            (Some(MaybeWorkspace::Workspace(_)), Some(ws_license_file)) => {
                Some(MaybeWorkspace::Defined(join_relative_path(
                    inherit.ws_path.clone().unwrap().as_path(),
                    package_root,
                    ws_license_file,
                )?))
            }
        };

        project.readme = match (project.readme.clone(), inherit.readme.as_ref()) {
            (None, _) => match default_readme_from_package_root(package_root) {
                None => None,
                Some(readme) => Some(MaybeWorkspace::Defined(StringOrBool::String(readme))),
            },
            (Some(MaybeWorkspace::Defined(defined)), _) => Some(MaybeWorkspace::Defined(defined)),
            (Some(MaybeWorkspace::Workspace(_)), None) => {
                bail!("error reading readme: workspace root does not defined [workspace.readme]")
            }
            (Some(MaybeWorkspace::Workspace(_)), Some(defined)) => match defined {
                StringOrBool::String(file) => Some(MaybeWorkspace::Defined(StringOrBool::String(
                    join_relative_path(
                        inherit.ws_path.clone().unwrap().as_path(),
                        package_root,
                        file,
                    )?,
                ))),
                StringOrBool::Bool(val) => {
                    Some(MaybeWorkspace::Defined(StringOrBool::Bool(val.clone())))
                }
            },
        };

        let package_name = project.name.trim();
        if package_name.is_empty() {
            bail!("package name cannot be an empty string")
        }

        validate_package_name(package_name, "package name", "")?;

        let version = if let MaybeWorkspace::Defined(version) = project.version.clone() {
            version
        } else {
            bail!("no version specified")
        };
        let pkgid = project.to_package_id(source_id, version)?;

        let edition = if let Some(MaybeWorkspace::Defined(ref edition)) = project.edition {
            features
                .require(Feature::edition())
                .with_context(|| "editions are unstable")?;
            edition
                .parse()
                .with_context(|| "failed to parse the `edition` key")?
        } else {
            Edition::Edition2015
        };
        if edition == Edition::Edition2021 {
            features.require(Feature::edition2021())?;
        } else if !edition.is_stable() {
            // Guard in case someone forgets to add .require()
            return Err(util::errors::internal(format!(
                "edition {} should be gated",
                edition
            )));
        }

        let rust_version = if let Some(rust_version) = &project.rust_version {
            if features.require(Feature::rust_version()).is_err() {
                let mut msg =
                    "`rust-version` is not supported on this version of Cargo and will be ignored"
                        .to_string();
                if config.nightly_features_allowed {
                    msg.push_str(
                        "\n\n\
                        consider adding `cargo-features = [\"rust-version\"]` to the manifest",
                    );
                } else {
                    msg.push_str(
                        "\n\n\
                        this Cargo does not support nightly features, but if you\n\
                        switch to nightly channel you can add\n\
                        `cargo-features = [\"rust-version\"]` to enable this feature",
                    );
                }
                warnings.push(msg);
                None
            } else {
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
            }
        } else {
            None
        };

        if project.metabuild.is_some() {
            features.require(Feature::metabuild())?;
        }

        if project.resolver.is_some()
            || me
                .workspace
                .as_ref()
                .map_or(false, |ws| ws.resolver.is_some())
        {
            features.require(Feature::resolver())?;
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
            inherit: &InheritableFields,
        ) -> CargoResult<Option<BTreeMap<String, TomlDependency>>> {
            let dependencies = match new_deps {
                Some(dependencies) => dependencies,
                None => return Ok(None),
            };
            let mut proccessed_deps: BTreeMap<String, TomlDependency> = BTreeMap::new();
            for (n, v) in dependencies.iter() {
                let toml_dep = match v.clone() {
                    TomlDependency::Simple(ref version) => TomlDependency::Simple(version.clone()),
                    TomlDependency::Detailed(mut details) => {
                        details.infer_path_version(cx, n)?;
                        TomlDependency::Detailed(details)
                    }
                    TomlDependency::Workspace(ws_dep_details) => {
                        let mut details = ws_dep_details.to_detailed_dependency(cx, inherit, n)?;
                        details.infer_path_version(cx, n)?;
                        TomlDependency::Detailed(details)
                    }
                };
                proccessed_deps.insert(n.clone(), toml_dep.clone());
                let dep = toml_dep.to_dependency(n, cx, kind)?;
                validate_package_name(dep.name_in_toml().as_str(), "dependency name", "")?;
                cx.deps.push(dep);
            }

            Ok(Some(proccessed_deps))
        }

        // Collect the dependencies.
        let dependencies = process_dependencies(&mut cx, me.dependencies.as_ref(), None, inherit)?;
        let dev_deps = me
            .dev_dependencies
            .as_ref()
            .or_else(|| me.dev_dependencies2.as_ref());
        let dev_dependencies =
            process_dependencies(&mut cx, dev_deps, Some(DepKind::Development), inherit)?;
        let build_deps = me
            .build_dependencies
            .as_ref()
            .or_else(|| me.build_dependencies2.as_ref());
        let build_dependencies =
            process_dependencies(&mut cx, build_deps, Some(DepKind::Build), inherit)?;

        for (name, platform) in me.target.iter().flatten() {
            cx.platform = {
                let platform: Platform = name.parse()?;
                platform.check_cfg_attributes(&mut cx.warnings);
                Some(platform)
            };
            process_dependencies(&mut cx, platform.dependencies.as_ref(), None, inherit)?;
            let build_deps = platform
                .build_dependencies
                .as_ref()
                .or_else(|| platform.build_dependencies2.as_ref());
            process_dependencies(&mut cx, build_deps, Some(DepKind::Build), inherit)?;
            let dev_deps = platform
                .dev_dependencies
                .as_ref()
                .or_else(|| platform.dev_dependencies2.as_ref());
            process_dependencies(&mut cx, dev_deps, Some(DepKind::Development), inherit)?;
        }

        replace = me.replace(&mut cx)?;
        patch = me.patch(&mut cx)?;

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
        let unstable = config.cli_unstable();
        summary.unstable_gate(unstable.namespaced_features, unstable.weak_dep_features)?;

        let badges = ws_default(
            me.badges.clone(),
            inherit,
            |inherit| &inherit.badges,
            "badges",
        )?;

        let metadata = ManifestMetadata {
            description: MaybeWorkspace::from_option(&project.description),
            homepage: MaybeWorkspace::from_option(&project.homepage),
            documentation: MaybeWorkspace::from_option(&project.documentation),
            readme: readme_for_project(package_root, project),
            authors: MaybeWorkspace::from_option(&project.authors).unwrap_or_default(),
            license: MaybeWorkspace::from_option(&project.license),
            license_file: MaybeWorkspace::from_option(&project.license_file),
            repository: MaybeWorkspace::from_option(&project.repository),
            keywords: MaybeWorkspace::from_option(&project.keywords).unwrap_or_default(),
            categories: MaybeWorkspace::from_option(&project.categories).unwrap_or_default(),
            badges: MaybeWorkspace::from_option(&badges).unwrap_or_default(),
            links: project.links.clone(),
        };

        let workspace_config = match (me.workspace.as_ref(), project.workspace.as_ref()) {
            (Some(toml_workspace), None) => WorkspaceConfig::Root(
                WorkspaceRootConfig::from_toml_workspace(package_root, toml_workspace),
            ),
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

        let publish = if let Some(MaybeWorkspace::Defined(publish)) = project.publish.clone() {
            match publish {
                VecStringOrBool::VecString(ref vecstring) => Some(vecstring.clone()),
                VecStringOrBool::Bool(false) => Some(vec![]),
                VecStringOrBool::Bool(true) => None,
            }
        } else {
            None
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

        let toml_manifest = TomlManifest {
            cargo_features: me.cargo_features.clone(),
            package: None,
            project: Some(project.clone()),
            profile: profiles.clone(),
            lib: me.lib.clone(),
            bin: me.bin.clone(),
            example: me.example.clone(),
            test: me.test.clone(),
            bench: me.bench.clone(),
            dependencies,
            dev_dependencies,
            dev_dependencies2: me.dev_dependencies2.clone(),
            build_dependencies,
            build_dependencies2: me.build_dependencies2.clone(),
            features: me.features.clone(),
            target: me.target.clone(),
            replace: me.replace.clone(),
            patch: me.patch.clone(),
            workspace: me.workspace.clone(),
            badges,
        };
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
            Rc::new(toml_manifest),
            project.metabuild.clone().map(|sov| sov.0),
            resolve_behavior,
        );
        if project.license_file.is_some() && project.license.is_some() {
            manifest.warnings_mut().add_warning(
                "only one of `license` or \
                 `license-file` is necessary"
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

        if manifest.targets().iter().all(|t| t.is_custom_build()) {
            bail!(
                "no targets specified in the manifest\n\
                 either src/lib.rs, src/main.rs, a [lib] section, or \
                 [[bin]] section must be present"
            )
        }

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
        if me
            .workspace
            .as_ref()
            .map_or(false, |ws| ws.resolver.is_some())
        {
            features.require(Feature::resolver())?;
        }
        let resolve_behavior = me
            .workspace
            .as_ref()
            .and_then(|ws| ws.resolver.as_deref())
            .map(|r| ResolveBehavior::from_manifest(r))
            .transpose()?;
        let workspace_config = match me.workspace {
            Some(ref toml_workspace) => WorkspaceConfig::Root(
                WorkspaceRootConfig::from_toml_workspace(root, toml_workspace),
            ),
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

    fn parse_toml_project(
        project: &mut TomlProject,
        inherit: &InheritableFields,
    ) -> CargoResult<()> {
        project.version = ws_default(
            Some(project.version.clone()),
            inherit,
            |inherit| &inherit.version,
            "version",
        )?
        .ok_or_else(|| anyhow!("no version specified"))?;
        project.edition = ws_default(
            project.edition.clone(),
            inherit,
            |inherit| &inherit.edition,
            "edition",
        )?;
        project.description = ws_default(
            project.description.clone(),
            inherit,
            |inherit| &inherit.description,
            "description",
        )?;
        project.homepage = ws_default(
            project.homepage.clone(),
            inherit,
            |inherit| &inherit.homepage,
            "homepage",
        )?;
        project.documentation = ws_default(
            project.documentation.clone(),
            inherit,
            |inherit| &inherit.documentation,
            "documentation",
        )?;
        project.authors = ws_default(
            project.authors.clone(),
            inherit,
            |inherit| &inherit.authors,
            "authors",
        )?;
        project.license = ws_default(
            project.license.clone(),
            inherit,
            |inherit| &inherit.license,
            "license",
        )?;
        project.repository = ws_default(
            project.repository.clone(),
            inherit,
            |inherit| &inherit.repository,
            "repository",
        )?;
        project.keywords = ws_default(
            project.keywords.clone(),
            inherit,
            |inherit| &inherit.keywords,
            "keywords",
        )?;
        project.categories = ws_default(
            project.categories.clone(),
            inherit,
            |inherit| &inherit.categories,
            "categories",
        )?;
        project.publish = ws_default(
            project.publish.clone(),
            inherit,
            |inherit| &inherit.publish,
            "publish",
        )?;
        Ok(())
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
            {
                let version = spec.version().ok_or_else(|| {
                    anyhow!(
                        "replacements must specify a version \
                         to replace, but `{}` does not",
                        spec
                    )
                })?;
                dep.set_version_req(VersionReq::exact(version));
            }
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
// This is taken from https://github.com/Manishearth/pathdiff/blob/30bb4010a7f420d8f367b0a0699ca42b813ce73d/src/lib.rs
fn join_relative_path(
    root_path: &Path,
    current_path: &Path,
    relative_path: &str,
) -> CargoResult<String> {
    let path = root_path;
    let base = current_path;

    let rel = if path.is_absolute() != base.is_absolute() {
        if path.is_absolute() {
            Some(PathBuf::from(path))
        } else {
            None
        }
    } else {
        let mut ita = path.components();
        let mut itb = base.components();
        let mut comps: Vec<Component<'_>> = vec![];
        loop {
            match (ita.next(), itb.next()) {
                (None, None) => break,
                (Some(a), None) => {
                    comps.push(a);
                    comps.extend(ita.by_ref());
                    break;
                }
                (None, _) => comps.push(Component::ParentDir),
                (Some(a), Some(b)) if comps.is_empty() && a == b => (),
                (Some(a), Some(b)) if b == Component::CurDir => comps.push(a),
                (Some(_), Some(b)) if b == Component::ParentDir => (),
                (Some(a), Some(_)) => {
                    comps.push(Component::ParentDir);
                    for _ in itb {
                        comps.push(Component::ParentDir);
                    }
                    comps.push(a);
                    comps.extend(ita.by_ref());
                    break;
                }
            }
        }
        Some(comps.iter().map(|c| c.as_os_str()).collect())
    };
    rel.unwrap()
        .join(relative_path)
        .into_os_string()
        .into_string()
        .map_err(|_| anyhow!("could not convert path into `String`"))
}

/// Returns the name of the README file for a `TomlProject`.
fn readme_for_project(package_root: &Path, project: &TomlProject) -> Option<String> {
    match &project.readme {
        Some(MaybeWorkspace::Defined(value)) => match value {
            StringOrBool::Bool(false) => None,
            StringOrBool::Bool(true) => Some("README.md".to_string()),
            StringOrBool::String(v) => Some(v.clone()),
        },
        _ => default_readme_from_package_root(package_root),
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
            TomlDependency::Workspace(_) => unreachable!(),
        }
    }

    fn is_version_specified(&self) -> bool {
        match self {
            TomlDependency::Detailed(d) => d.version.is_some(),
            TomlDependency::Simple(..) => true,
            TomlDependency::Workspace(_) => unreachable!(),
        }
    }

    fn is_optional(&self) -> bool {
        match self {
            TomlDependency::Detailed(d) => d.optional.unwrap_or(false),
            TomlDependency::Simple(..) => false,
            TomlDependency::Workspace(_) => unreachable!(),
        }
    }
}

impl TomlWorkspaceDependency {
    fn to_detailed_dependency(
        &self,
        cx: &mut Context<'_, '_>,
        inherit: &InheritableFields,
        name: &str,
    ) -> CargoResult<DetailedTomlDependency> {
        if let Some(deps) = &inherit.dependencies {
            if let Some(dep) = deps.get(name) {
                match dep {
                    TomlDependency::Simple(version) => Ok(DetailedTomlDependency {
                        optional: self.optional.clone(),
                        features: self.features.clone(),
                        version: Some(version.to_owned()),
                        ..Default::default()
                    }),
                    TomlDependency::Detailed(inherit_details) => {
                        let features =
                            match (self.features.clone(), inherit_details.features.clone()) {
                                (Some(dep_feat), Some(inherit_feat)) => Some(
                                    dep_feat
                                        .into_iter()
                                        .chain(inherit_feat)
                                        .collect::<Vec<String>>(),
                                ),
                                (Some(dep_fet), None) => Some(dep_fet),
                                (None, Some(inherit_feat)) => Some(inherit_feat),
                                (None, None) => None,
                            };
                        let path = if let Some(p) = inherit_details.path.clone() {
                            join_relative_path(
                                inherit.ws_path.clone().unwrap().as_path(),
                                cx.root,
                                p.as_str(),
                            )
                            .map_or(None, |path| Some(path))
                        } else {
                            None
                        };
                        Ok(DetailedTomlDependency {
                            version: inherit_details.version.clone(),
                            registry: inherit_details.registry.clone(),
                            registry_index: inherit_details.registry_index.clone(),
                            path,
                            git: inherit_details.git.clone(),
                            branch: inherit_details.branch.clone(),
                            tag: inherit_details.tag.clone(),
                            rev: inherit_details.rev.clone(),
                            features,
                            optional: self.optional.clone(),
                            default_features: inherit_details.default_features.clone(),
                            default_features2: inherit_details.default_features2.clone(),
                            package: inherit_details.package.clone(),
                            public: inherit_details.public.clone(),
                        })
                    }
                    TomlDependency::Workspace(_) => unreachable!(),
                }
            } else {
                bail!(
                    "failed to get dependency `{}`, not found in [workspace.dependencies]",
                    name
                )
            }
        } else {
            bail!(
                "failed to get dependency `{}`, [workspace.dependencies] does not exist",
                name
            )
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
                    let msg = format!(
                        "key `{}` is ignored for dependency ({}). \
                         This will be considered an error in future versions",
                        key_name, name_in_toml
                    );
                    cx.warnings.push(msg)
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
                    let msg = format!(
                        "dependency ({}) specification is ambiguous. \
                         Only one of `git` or `path` is allowed. \
                         This will be considered an error in future versions",
                        name_in_toml
                    );
                    cx.warnings.push(msg)
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
            cx.features.require(Feature::rename_dependency())?;
            dep.set_explicit_name_in_toml(name_in_toml);
        }

        if let Some(p) = self.public {
            cx.features.require(Feature::public_dependency())?;

            if dep.kind() != DepKind::Normal {
                bail!("'public' specifier can only be used on regular dependencies, not {:?} dependencies", dep.kind());
            }

            dep.set_public(p);
        }
        Ok(dep)
    }

    fn infer_path_version(&mut self, cx: &mut Context<'_, '_>, name: &str) -> CargoResult<()> {
        if let (None, Some(p)) = (&self.version, &self.path) {
            let base_path = &cx.root.join(p.resolve(cx.config));
            let (manifest, _) =
                read_manifest(&base_path.join("Cargo.toml"), cx.source_id, cx.config)
                    .with_context(|| format!("failed to get dependency `{}`", name))?;
            self.version = if let EitherManifest::Real(ref intermediate) = manifest {
                let toml = intermediate.original();
                let package = toml
                    .package
                    .as_ref()
                    .or_else(|| toml.project.as_ref())
                    .ok_or_else(|| anyhow!("no `package` section found"))?;
                let v = match package.version {
                    MaybeWorkspace::Workspace(_) => {
                        let root_path =
                            find_workspace_root(&base_path.join("Cargo.toml"), cx.config)?
                                .expect("workspace was referenced, none found");
                        let (root_man, _) = read_manifest(&root_path, cx.source_id, cx.config)
                            .with_context(|| format!("failed to get workspace for `{}`", name))?;
                        if let WorkspaceConfig::Root(ws_config) = root_man.workspace_config() {
                            let inherit = ws_config.inheritable_fields().clone();
                            match (package.publish.as_ref(), inherit.publish) {
                                (
                                    Some(MaybeWorkspace::Defined(VecStringOrBool::Bool(false))),
                                    _,
                                ) => None,
                                (_, Some(VecStringOrBool::Bool(false))) => None,
                                _ => Some(inherit.version.expect(&format!(
                                    "workspace does not define version information required by {}",
                                    name
                                ))),
                            }
                        } else {
                            bail!(
                                "workspace does not define version information required by {}",
                                name
                            )
                        }
                    }
                    MaybeWorkspace::Defined(ref version) => match package.publish {
                        Some(MaybeWorkspace::Defined(VecStringOrBool::Bool(false))) => None,
                        Some(MaybeWorkspace::Workspace(_)) => {
                            let root_path =
                                find_workspace_root(&base_path.join("Cargo.toml"), cx.config)?
                                    .expect("workspace was referenced, none found");
                            let (root_man, _) = read_manifest(&root_path, cx.source_id, cx.config)
                                .with_context(|| format!("failed to get dependency `{}`", name))?;
                            if let WorkspaceConfig::Root(ws_config) = root_man.workspace_config() {
                                let inherit = ws_config.inheritable_fields().clone();
                                match inherit.publish {
                                    Some(VecStringOrBool::Bool(false)) => None,
                                    _ => Some(version.clone()),
                                }
                            } else {
                                bail!(
                                    "workspace does not define version information required by {}",
                                    name
                                )
                            }
                        }
                        _ => Some(version.clone()),
                    },
                };
                v.map(|ver| ver.to_string())
            } else {
                None
            };
        }

        Ok(())
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
#[derive(Serialize, Deserialize, Debug, Clone)]
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
